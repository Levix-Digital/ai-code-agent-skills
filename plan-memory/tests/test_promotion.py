import os
import sys

import pytest

HERE = os.path.dirname(os.path.abspath(__file__))
SKILL = os.path.dirname(HERE)
sys.path.insert(0, SKILL)
sys.path.append(os.path.join(SKILL, "..", "dev-cycle-protocol"))

import _backlog_cli as cli  # noqa: E402
from _backlog import Backlog, BacklogError  # noqa: E402
from _backlog_store import BacklogFile  # noqa: E402
from _store import PlanStore, slugify_id  # noqa: E402
from _check_milestone import check_milestone  # noqa: E402
from _save_plan import save_plan  # noqa: E402
from _update_plan_status import update_plan_status  # noqa: E402
sys.path.append(os.path.join(SKILL, "..", "dev-cycle-protocol", "tests"))
from _engine import run_checker  # noqa: E402


@pytest.fixture(autouse=True)
def no_search_index(monkeypatch):
    """The ChromaDB index is not under test here."""
    monkeypatch.setattr(PlanStore, "sync_index", lambda self, *a, **k: None)


@pytest.fixture
def project(tmp_path):
    for args in (("bug", "Crash on start", "It crashes.", "high"),
                 ("bug", "Wrong label", "Typo in a label.", "low"),
                 ("feature", "Export answers", "Export to Markdown.", None)):
        extra = ["--severity", args[3]] if args[3] else []
        assert cli.main(["add", "--type", args[0], "--title", args[1], "--description", args[2], *extra,
                         "--dir", str(tmp_path)]) == 0
    return tmp_path


def items(project) -> Backlog:
    return BacklogFile(str(project)).load()


def test_promote_one_bug_creates_a_draft_bug_plan(project):
    assert cli.main(["promote", "BUG-001", "--dir", str(project)]) == 0
    store = PlanStore(str(project))
    plan = store.get(f"{slugify_id(store.project)}_crash_on_start")
    assert plan.kind == "bug" and plan.meta["status"] == "draft" and plan.backlog_items == ["BUG-001"]
    assert "## 1. Triage" in plan.body and "## 2. Reproduction" in plan.body
    assert "### BUG-001: Crash on start" in plan.body and "- **Severity**: high" in plan.body
    item = items(project).find("BUG-001")
    assert (item.status, item.plan) == ("planned", plan.meta["plan_id"])
    assert items(project).validate() == []


def test_promote_mixed_items_needs_a_title_and_makes_a_feature_plan(project, capsys):
    assert cli.main(["promote", "BUG-002", "FEAT-001", "--dir", str(project)]) == 1
    assert "needs --title" in capsys.readouterr().out
    assert cli.main(["promote", "BUG-002", "FEAT-001", "--title", "Export polish", "--id", "export_polish",
                     "--dir", str(project)]) == 0
    plan = PlanStore(str(project)).get("export_polish")
    assert plan.kind == "feature" and plan.backlog_items == ["BUG-002", "FEAT-001"]
    assert "## 1. Goal and decisions" in plan.body


def test_promote_refuses_planned_closed_and_unknown_items(project):
    bf = BacklogFile(str(project))
    cli.promote(bf, ["BUG-001"], plan_id="crash")
    with pytest.raises(BacklogError, match="already promoted into plan 'crash'"):
        cli.promote(bf, ["BUG-001"])
    backlog = bf.load()
    backlog.close("BUG-002", "dropped", "2026-10-16, not reproducible")
    bf.save(backlog)
    with pytest.raises(BacklogError, match="is closed"):
        cli.promote(bf, ["BUG-002"])
    with pytest.raises(BacklogError, match="no such backlog item"):
        cli.promote(bf, ["FEAT-009"])


def test_promote_into_an_existing_plan(project):
    bf = BacklogFile(str(project))
    cli.promote(bf, ["BUG-001"], plan_id="crash")
    cli.promote(bf, ["FEAT-001"], into="crash")
    plan = PlanStore(str(project)).get("crash")
    assert plan.backlog_items == ["BUG-001", "FEAT-001"] and plan.kind == "feature"
    section = plan.body.split("## Backlog items", 1)[1].split("\n## ", 1)[0]
    assert "### BUG-001" in section and "### FEAT-001" in section
    assert items(project).find("FEAT-001").plan == "crash"
    with pytest.raises(BacklogError, match="not found"):
        cli.promote(bf, ["BUG-002"], into="nope")


def test_items_follow_the_plan_status_and_close_when_it_completes(project):
    store = PlanStore(str(project))
    cli.promote(BacklogFile(str(project)), ["BUG-001", "BUG-002"], title="Crash fixes", plan_id="crash")
    save_plan(store, "Crash fixes", store.get("crash").body, plan_id="crash", status="active",
              milestones="M1 Fix,M2 Release")
    check_milestone(store, plan_id="crash", milestone_name="M1 Fix")
    assert items(project).find("BUG-001").status == "in_progress"
    check_milestone(store, plan_id="crash", milestone_name="M2 Release")
    backlog = items(project)
    assert [i.id for i in backlog.sections["Closed"]] == ["BUG-001", "BUG-002"]
    assert backlog.find("BUG-001").status == "done" and "plan crash" in backlog.find("BUG-001").get("Closed")
    assert backlog.validate() == []


def test_update_plan_status_completed_closes_items(project):
    store = PlanStore(str(project))
    cli.promote(BacklogFile(str(project)), ["FEAT-001"], plan_id="export")
    update_plan_status(store, "export", "completed")
    assert items(project).find("FEAT-001").status == "done"


def test_save_plan_with_backlog_items_links_them(project, capsys):
    store = PlanStore(str(project))
    save_plan(store, "Manual link", "# Manual link\n", plan_id="manual", kind="bug", backlog_items="bug-002,BUG-404")
    out = capsys.readouterr().out
    assert "BUG-404 (plan 'manual') is not in docs/BACKLOG.md" in out
    item = items(project).find("BUG-002")
    assert (item.status, item.plan) == ("planned", "manual")
    with pytest.raises(ValueError, match="kind"):
        save_plan(store, "Bad", "x", plan_id="bad", kind="epic")


def test_list_plans_filters_by_kind(project):
    store = PlanStore(str(project))
    cli.promote(BacklogFile(str(project)), ["BUG-001"], plan_id="crash")
    save_plan(store, "Roadmap", "# Roadmap\n", plan_id="roadmap")
    code, out = run_checker("plan-memory", "list_plans.py", ["--kind", "bug", "--dir", str(project)])
    assert code == 0
    assert "crash" in out and "roadmap" not in out and "Kind: bug" in out and "Backlog    : BUG-001" in out
