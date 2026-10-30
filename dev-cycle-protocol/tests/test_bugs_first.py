import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SKILL = os.path.dirname(HERE)
SCRIPT = os.path.join(SKILL, "verify_gate.py")
sys.path.insert(0, SKILL)

from test_verify_gate import make_project  # noqa: E402

BACKLOG = """# Backlog

## Bugs

### BUG-001: Crash on start
- **Severity**: {severity}
- **Status**: open
- **Found**: 2026-10-16
- **Plan**: -
- **Description**: It crashes.

## Features

## Improvements

## Adjustments

## Closed
"""


def write_plan(root, plan_id, kind, backlog_items=()):
    plans = root / "docs" / "plans"
    plans.mkdir(parents=True, exist_ok=True)
    items = "".join(f"\n- {i}" for i in backlog_items)
    (plans / f"{plan_id}.md").write_text(
        f"---\nplan_id: {plan_id}\ntitle: {plan_id}\nkind: {kind}\nstatus: active\n"
        f"created_at: '2026-10-16T10:00:00'\nbacklog_items:{items or ' []'}\n---\n\n# {plan_id}\n",
        encoding="utf-8")
    return plans / f"{plan_id}.md"


def project(tmp_path, severity="high"):
    (tmp_path / "docs").mkdir(exist_ok=True)
    (tmp_path / "docs" / "BACKLOG.md").write_text(BACKLOG.format(severity=severity), encoding="utf-8")
    return tmp_path


def gate(root, *args):
    return subprocess.run([sys.executable, SCRIPT, "--dir", str(root), *args],
                          capture_output=True, text=True, encoding="utf-8")


def test_feature_plan_is_blocked_by_an_open_high_bug(tmp_path):
    root = project(tmp_path)
    write_plan(root, "export", "feature")
    res = gate(root, "--gate", "3")
    assert res.returncode == 1, res.stdout
    assert "Bugs first" in res.stdout and "BUG-001 (high)" in res.stdout and "--accept-open-bugs" in res.stdout


def test_user_can_accept_the_risk_and_it_is_recorded(tmp_path):
    root = project(tmp_path)
    path = write_plan(root, "export", "feature")
    res = gate(root, "--gate", "3", "--accept-open-bugs", "Demo on Friday needs export")
    assert res.returncode == 0, res.stdout
    assert "Demo on Friday needs export" in path.read_text(encoding="utf-8")


def test_bug_plans_and_plans_that_include_the_bug_are_not_blocked(tmp_path):
    root = project(tmp_path)
    write_plan(root, "crash", "bug")
    assert gate(root, "--gate", "3").returncode == 0
    (root / "docs" / "plans" / "crash.md").unlink()
    write_plan(root, "export", "feature", ["BUG-001"])
    assert gate(root, "--gate", "3").returncode == 0


ROADMAP = """# Roadmap

## v0.1.0: MVP
- **Status**: released
- **Released**: 2026-10-01
- **Goal**: First usable version.
- **Items**: -

## v0.2.0: Albums
- **Status**: in_progress
- **Goal**: Albums.
- **Items**: {next_items}

## v1.0.0: Launch
- **Status**: planned
- **Goal**: Public launch.
- **Items**: {later_items}
"""


def write_roadmap(root, next_items="-", later_items="-"):
    (root / "docs" / "ROADMAP.md").write_text(ROADMAP.format(next_items=next_items, later_items=later_items),
                                              encoding="utf-8")


def test_medium_bugs_block_by_default(tmp_path):
    root = project(tmp_path, severity="medium")
    write_plan(root, "export", "feature")
    res = gate(root, "--gate", "3")
    assert res.returncode == 1 and "BUG-001 (medium)" in res.stdout


def test_low_bugs_without_a_roadmap_only_inform(tmp_path):
    root = project(tmp_path, severity="low")
    write_plan(root, "export", "feature")
    res = gate(root, "--gate", "3")
    assert res.returncode == 0 and "Other open bugs" in res.stdout and "No roadmap" in res.stdout


def test_low_bug_in_the_next_release_is_triaged(tmp_path):
    root = project(tmp_path, severity="low")
    write_roadmap(root, next_items="BUG-001")
    write_plan(root, "export", "feature")
    res = gate(root, "--gate", "3")
    assert res.returncode == 0, res.stdout
    assert "every other bug is triaged" in res.stdout


def test_low_bug_not_scheduled_or_scheduled_later_blocks(tmp_path):
    root = project(tmp_path, severity="low")
    write_roadmap(root)
    write_plan(root, "export", "feature")
    res = gate(root, "--gate", "3")
    assert res.returncode == 1, res.stdout
    assert "not triaged" in res.stdout and "goes into v0.2.0 at the latest" in res.stdout
    assert "roadmap.py assign <ID> --release v0.2.0" in res.stdout
    write_roadmap(root, later_items="BUG-001")
    res = gate(root, "--gate", "3")
    assert res.returncode == 1 and "scheduled for v1.0.0; the deadline is v0.2.0" in res.stdout


def test_bug_without_severity_is_not_triaged(tmp_path):
    root = project(tmp_path, severity="")
    write_plan(root, "export", "feature")
    res = gate(root, "--gate", "3")
    assert res.returncode == 1 and "BUG-001 (?) Crash on start: no severity" in res.stdout


def test_untriaged_bugs_can_be_accepted_or_turned_off(tmp_path):
    root = project(tmp_path, severity="low")
    write_roadmap(root)
    path = write_plan(root, "export", "feature")
    assert gate(root, "--gate", "3", "--accept-open-bugs", "Cosmetic, after the demo").returncode == 0
    assert "Cosmetic, after the demo" in path.read_text(encoding="utf-8")
    (root / ".dev-cycle.json").write_text(json.dumps({"backlog": {"bug_deadline": "off"}}), encoding="utf-8")
    assert gate(root, "--gate", "3").returncode == 0


def test_bug_plan_is_not_blocked_by_untriaged_bugs(tmp_path):
    root = project(tmp_path, severity="low")
    write_roadmap(root)
    write_plan(root, "other-bug", "bug")
    res = gate(root, "--gate", "3")
    assert res.returncode == 0 and "BUG-001 (not triaged)" in res.stdout


def test_configured_blocking_severities(tmp_path):
    root = project(tmp_path, severity="high")
    (root / ".dev-cycle.json").write_text(json.dumps({"backlog": {"blocking_severities": ["critical"]}}), encoding="utf-8")
    write_plan(root, "export", "feature")
    assert gate(root, "--gate", "3").returncode == 0


def test_gate_8_validates_the_backlog_and_lists_blocking_bugs(tmp_path):
    root = make_project(tmp_path)
    (tmp_path / "docs").mkdir(exist_ok=True)
    (tmp_path / "docs" / "BACKLOG.md").write_text(BACKLOG.format(severity="critical"), encoding="utf-8")
    res = gate(root, "--gate", "8", "--run", "v1.0.0-rc.1")
    assert "Backlog (plan-memory) passed" in res.stdout
    assert "known limitations" in res.stdout and "BUG-001 Crash on start" in res.stdout
    (tmp_path / "docs" / "BACKLOG.md").write_text(BACKLOG.format(severity="urgent"), encoding="utf-8")
    res = gate(root, "--gate", "8", "--run", "v1.0.0-rc.1")
    assert res.returncode == 1 and "Backlog (plan-memory) failed" in res.stdout


def test_gate_8_fails_while_a_bug_is_not_triaged(tmp_path):
    root = make_project(tmp_path)
    (tmp_path / "docs").mkdir(exist_ok=True)
    (tmp_path / "docs" / "BACKLOG.md").write_text(BACKLOG.format(severity="low"), encoding="utf-8")
    write_roadmap(tmp_path, later_items="BUG-001")
    res = gate(root, "--gate", "8", "--run", "v0.2.0-rc.1")
    assert res.returncode == 1 and "Open bugs not triaged" in res.stdout and "deadline is v0.2.0" in res.stdout
