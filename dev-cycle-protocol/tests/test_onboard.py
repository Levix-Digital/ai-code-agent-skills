import json
import os
import subprocess
import sys

import pytest

HERE = os.path.dirname(os.path.abspath(__file__))
SKILL = os.path.dirname(HERE)
sys.path.insert(0, SKILL)

import onboard  # noqa: E402
from _onboard import is_new, options, recommendation  # noqa: E402

BASELINE = {"vision": True, "tech_spec": True, "style": "layered", "tec_doc": True, "epics": 2, "roadmap": True}
SET_UP = {"repo": {"git": True, "commits": 5, "has_code": True}, "config": {"exists": True, "problems": []},
          "branches": {"integration": "dev", "integration_exists": True, "production": "main"},
          "baseline": BASELINE, "speckit": {"state": "declined", "uv": True},
          "roadmap": {"current": "v0.2.0 Albums", "open_items": ["FEAT-004 (high) Albums"]}}


def state(**changes):
    s = json.loads(json.dumps(SET_UP))
    s.update(changes)
    return s


def kinds(opts):
    return [o["kind"] for o in opts]


# ---------------------------------------------------------------- rules

def test_nothing_pending_goes_straight_on_with_one_line():
    rec = recommendation(state(), "add an export command")
    assert rec["mode"] == "existing" and rec["ask"] is False
    assert rec["summary"].startswith("Project up to date (current release v0.2.0 Albums, no blocking bugs, no plans in progress)")
    assert rec["summary"].endswith("Going on with: add an export command")
    assert kinds(rec["options"]) == ["release", "request"]


def test_without_a_request_the_options_are_shown():
    rec = recommendation(state())
    assert rec["ask"] is True and rec["recommended"] == "release"


def test_competing_items_come_first_in_order_and_ask():
    s = state(plans=[{"id": "p", "title": "Export", "status": "in_progress", "done": 1, "total": 3}],
              rc={"run": "v1.0.0-rc.1", "version": "v1.0.0", "not_run": 2, "failed": 1, "released": False},
              bugs=["BUG-007 (high) Crash on start"])
    rec = recommendation(s, "add search")
    assert rec["ask"] is True and rec["recommended"] == "plan:p"
    assert kinds(rec["options"]) == ["plan", "rc", "bugs", "release", "request"]
    assert "Resume the plan 'Export' (1/3 milestones)" in rec["options"][0]["title"]
    assert "Finish the acceptance of v1.0.0-rc.1 (2 not run, 1 failed)" in rec["options"][1]["title"]
    assert "BUG-007" in rec["options"][2]["title"]


def test_untriaged_bugs_compete_with_the_request():
    rec = recommendation(state(untriaged=["BUG-004 (low) Typo: not scheduled; it goes into v0.2.0 at the latest"]), "add search")
    assert rec["ask"] is True and rec["recommended"] == "triage"
    assert "Triage the open bugs: BUG-004" in rec["options"][0]["title"]


def test_finished_plans_and_released_candidates_are_not_pending():
    s = state(plans=[{"id": "p", "title": "Done", "status": "in_progress", "done": 3, "total": 3}],
              rc={"run": "v1.0.0-rc.2", "version": "v1.0.0", "not_run": 0, "failed": 0, "released": True})
    assert kinds(options(s)) == ["release"]


def test_an_accepted_candidate_waits_for_the_release_gate():
    s = state(rc={"run": "v1.0.0-rc.2", "version": "v1.0.0", "not_run": 0, "failed": 0, "released": False})
    assert "is accepted: open the release pull request" in options(s)[0]["title"]


def test_never_used_repository_lists_the_initial_setup_in_order():
    s = state(config={"exists": False}, branches={"integration": "dev", "integration_exists": False, "production": "main"},
              baseline={}, speckit={"state": "undecided", "uv": True}, roadmap=None)
    rec = recommendation(s, "fix the login")
    assert rec["ask"] is True
    assert rec["summary"].startswith("This repository is not set up for the cycle yet")
    assert [o["key"] for o in rec["options"]] == ["config", "branch", "speckit-ask", "adoption", "request"]


def test_spec_kit_states():
    assert "speckit-ask" not in [o["key"] for o in options(state())]                          # declined: not asked again
    unmanaged = options(state(speckit={"state": "unmanaged", "uv": True}))
    assert unmanaged[0]["key"] == "speckit-adopt" and "hand-written constitution is kept" in unmanaged[0]["title"]
    broken = options(state(speckit={"state": "enabled", "uv": True, "problems": ["preset 'dev-cycle' is not installed"]}))
    assert broken[0]["key"] == "speckit-fix" and "preset" in broken[0]["reason"]
    no_uv = options(state(speckit={"state": "undecided", "uv": False}))
    assert no_uv[0]["key"] == "speckit-ask" and "needs uv" in no_uv[0]["title"]


def test_new_repository_starts_phase_0():
    s = {"repo": {"git": True, "commits": 0, "has_code": False}, "config": {"exists": False}, "baseline": {},
         "speckit": {"state": "undecided", "uv": True}}
    assert is_new(s)
    rec = recommendation(s, "an app where families keep their photos")
    assert rec["mode"] == "new" and rec["ask"] is False and rec["recommended"] == "inception"
    assert rec["options"][-1]["title"] == "Start the product from the idea: an app where families keep their photos"
    assert recommendation(s)["ask"] is True


def test_code_without_baseline_is_an_existing_project():
    s = {"repo": {"git": True, "commits": 3, "has_code": True}, "config": {"exists": True, "problems": []},
         "branches": {"integration": "dev", "integration_exists": True}, "baseline": {}, "speckit": {"state": "declined"}}
    assert not is_new(s)
    assert [o["key"] for o in options(s, "x")] == ["adoption", "request"]


def test_a_repository_on_another_model_is_asked_once():
    trunk = {"model": "trunk", "integration_branch": None, "reason": "only 'main' and short-lived branches"}
    s = state(branches={"integration": "dev", "integration_exists": False, "production": "main",
                        "model": "integration", "decided": False, "detected": trunk})
    keys = [o["key"] for o in options(s)]
    assert keys[0] == "branching" and "branch" not in keys
    assert "looks like 'trunk'" in options(s)[0]["title"]
    s["branches"]["decided"] = True                     # integration chosen: its branch is proposed
    assert [o["key"] for o in options(s)][0] == "branch"
    s["branches"].update(model="trunk", decided=False)  # the default already matches: nothing to ask
    assert "branching" not in [o["key"] for o in options(s)]


def test_an_accepted_candidate_under_promote_names_the_promotion():
    s = state(rc={"run": "v1.0.0-rc.2", "version": "v1.0.0", "not_run": 0, "failed": 0, "released": False})
    s["branches"]["gate"] = "promote"
    assert "then you promote it" in options(s)[0]["title"]


def test_config_problems_are_a_setup_item():
    opts = options(state(config={"exists": True, "problems": ["speckit.version 'latest' must be an exact release tag"]}))
    assert opts[0]["key"] == "config" and "exact release tag" in opts[0]["reason"]


# ---------------------------------------------------------------- collection

SUITE = """# Manual Tests

## Basics

### MT-BASIC-001: Demo starts

- **Covers**: cli
- **Steps**:
  1. Run `demo`
- **Expected**:
  - A prompt appears
- **Result** (v1.0.0-rc.2): [{mark}] Pass [ ] Fail [ ] Blocked
- **Notes**:

### MT-BASIC-002: Demo stops

- **Covers**: cli
- **Steps**:
  1. Quit
- **Expected**:
  - It stops
- **Result** (v1.0.0-rc.1): [x] Pass [ ] Fail [ ] Blocked
- **Notes**:
"""


def run_git(root, *args):
    subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=root, check=True, capture_output=True)


@pytest.fixture
def repo(tmp_path):
    run_git(tmp_path, "init", "-q", "-b", "main")
    (tmp_path / "app.py").write_text("print('hi')\n", encoding="utf-8")
    run_git(tmp_path, "add", "-A")
    run_git(tmp_path, "commit", "-q", "-m", "init")
    return tmp_path


def test_collect_a_repository_that_never_used_the_protocol(repo):
    s = onboard.collect(str(repo))
    assert s["repo"]["git"] and s["repo"]["commits"] == 1 and s["repo"]["has_code"]
    assert s["config"]["exists"] is False
    assert s["branches"]["integration_exists"] is False and s["branches"]["production_exists"] is True
    assert s["speckit"]["state"] == "undecided" and s["rc"] is None and s["findings"] == []
    assert s["branches"]["detected"]["model"] == "trunk" and s["branches"]["decided"] is False
    rec = recommendation(s)
    assert [o["key"] for o in rec["options"]][:2] == ["config", "speckit-ask"]     # the model comes with the config


def test_collect_release_candidate_and_tags(repo):
    (repo / "tests").mkdir()
    (repo / "tests" / "MANUAL_TESTS.md").write_text(SUITE.format(mark=" "), encoding="utf-8")
    rc = onboard.collect(str(repo))["rc"]
    assert rc == {"run": "v1.0.0-rc.2", "version": "v1.0.0", "not_run": 1, "failed": 0, "released": False}
    run_git(repo, "tag", "v1.0.0")
    assert onboard.collect(str(repo))["rc"]["released"] is True


def test_collect_spec_kit_unmanaged_and_declined(repo):
    (repo / ".specify").mkdir()
    (repo / ".specify" / "init-options.json").write_text(json.dumps({"speckit_version": "1.2.3"}), encoding="utf-8")
    s = onboard.collect(str(repo))
    assert s["speckit"]["state"] == "unmanaged" and s["speckit"]["installed"] == "v1.2.3"
    (repo / ".dev-cycle.json").write_text(json.dumps({"speckit": {"enabled": False}}), encoding="utf-8")
    assert onboard.collect(str(repo))["speckit"]["state"] == "declined"


def test_unreadable_backlog_is_a_finding(repo):
    (repo / "docs").mkdir()
    (repo / "docs" / "BACKLOG.md").write_bytes(b"\xff\xfe broken")
    s = onboard.collect(str(repo))
    assert any(f.startswith("backlog: could not be read") for f in s["findings"])


def test_cli_json_and_text(repo, capsys):
    assert onboard.main(["diagnose", "--dir", str(repo), "--json", "--request", "fix login"]) == 0
    data = json.loads(capsys.readouterr().out)
    assert data["mode"] == "existing" and data["ask"] is True and data["state"]["repo"]["commits"] == 1
    assert onboard.main(["diagnose", "--dir", str(repo)]) == 0
    out = capsys.readouterr().out
    assert "not set up for the cycle yet" in out and "1. Create .dev-cycle.json" in out and "(recommended)" in out


# ---------------------------------------------------------------- assisted setup

from project_config import load_project_config  # noqa: E402


def test_proposal_from_detection(repo):
    (repo / "README.md").write_text("# Demo\n", encoding="utf-8")
    (repo / "pyproject.toml").write_text("[project]\nname='demo'\n", encoding="utf-8")
    (repo / "tests").mkdir()
    data = onboard.propose_config(str(repo))
    assert data["exists"] is False
    add = data["additions"]
    assert add["project_name"] == repo.name and add["test_commands"] == ["python -m pytest"]
    assert add["docs"] == {"user_doc": "README.md"}
    assert add["devops"] == {"branching": "trunk", "production_branch": "main", "version_files": ["pyproject.toml"]}
    assert data["branching"]["model"] == "trunk"
    assert "architecture" not in add
    run_git(repo, "branch", "develop")
    assert onboard.propose_config(str(repo))["additions"]["devops"]["integration_branch"] == "develop"


def test_existing_keys_are_never_proposed_or_overwritten(repo):
    (repo / ".dev-cycle.json").write_text(json.dumps({"project_name": "mine", "devops": {"integration_branch": "develop"}}),
                                          encoding="utf-8")
    add = onboard.propose_config(str(repo))["additions"]
    assert "project_name" not in add and "integration_branch" not in add["devops"]
    path, added = onboard.write_config(str(repo), add)
    data = json.loads(open(path, encoding="utf-8").read())
    assert data["project_name"] == "mine" and data["devops"]["integration_branch"] == "develop"
    assert data["devops"]["production_branch"] == "main"
    with pytest.raises(ValueError, match="already sets devops.integration_branch"):
        onboard.write_config(str(repo), {"devops": {"integration_branch": "dev"}})
    assert onboard.propose_config(str(repo))["additions"] == {}


def test_write_config_cli(repo, tmp_path, capsys):
    proposal = tmp_path / "add.json"
    proposal.write_text(json.dumps({"project_name": "demo"}), encoding="utf-8")
    assert onboard.main(["write-config", "--dir", str(repo), "--file", str(proposal)]) == 0
    assert "added project_name" in capsys.readouterr().out
    assert onboard.main(["write-config", "--dir", str(repo), "--file", str(tmp_path / "missing.json")]) == 1


def test_create_integration_branch(repo):
    assert "dev (from main)" in onboard.create_integration_branch(str(repo))
    assert onboard.branch_exists(str(repo), "dev")
    with pytest.raises(ValueError, match="already exists"):
        onboard.create_integration_branch(str(repo))


def test_create_branch_only_for_the_integration_model(repo):
    (repo / ".dev-cycle.json").write_text(json.dumps({"devops": {"branching": "trunk"}}), encoding="utf-8")
    with pytest.raises(ValueError, match="'trunk' branching model has no integration branch"):
        onboard.create_integration_branch(str(repo))


def test_collect_reads_the_branching_model(repo):
    (repo / ".dev-cycle.json").write_text(json.dumps({"devops": {"branching": "release-branches"}}), encoding="utf-8")
    b = onboard.collect(str(repo))["branches"]
    assert (b["model"], b["decided"], b["gate"]) == ("release-branches", True, "promote")
    run_git(repo, "branch", "release/1.0")
    assert onboard.detect_branching(str(repo))["model"] == "release-branches"


def test_explicit_false_is_a_decision(repo):
    (repo / ".dev-cycle.json").write_text(json.dumps({"speckit": {"version": None}}), encoding="utf-8")
    cfg = load_project_config(str(repo))
    assert cfg.section("speckit")["enabled"] is False and not cfg.is_set("speckit", "enabled")
    (repo / ".dev-cycle.json").write_text(json.dumps({"speckit": {"enabled": False}}), encoding="utf-8")
    assert load_project_config(str(repo)).is_set("speckit", "enabled")


# ---------------------------------------------------------------- agent attribution

def test_attribution_item_only_for_claude_code_without_settings():
    keys = lambda s: [o["key"] for o in options(s)]
    assert "attribution" not in keys(state())
    assert "attribution" not in keys(state(agent={"name": "codex", "attribution_hidden": False}))
    assert keys(state(agent={"name": "claude-code", "attribution_hidden": False}))[0] == "attribution"
    assert "attribution" not in keys(state(agent={"name": "claude-code", "attribution_hidden": True}))


def test_set_attribution_keeps_other_settings(repo):
    (repo / ".claude").mkdir()
    (repo / ".claude" / "settings.json").write_text(json.dumps({"enabledPlugins": {"x@y": True}}), encoding="utf-8")
    assert not onboard.attribution_hidden(str(repo))
    assert onboard.collect(str(repo), "claude-code")["agent"] == {"name": "claude-code", "attribution_hidden": False}
    assert onboard.main(["set-attribution", "--dir", str(repo)]) == 0
    data = json.loads((repo / ".claude" / "settings.json").read_text(encoding="utf-8"))
    assert data == {"enabledPlugins": {"x@y": True}, "attribution": {"commit": "", "pr": "", "sessionUrl": False}}
    assert onboard.attribution_hidden(str(repo))


def test_attribution_false_or_local_settings_count(repo):
    (repo / ".claude").mkdir()
    (repo / ".claude" / "settings.local.json").write_text(json.dumps({"attribution": False}), encoding="utf-8")
    assert onboard.attribution_hidden(str(repo))


# ---------------------------------------------------------------- pull requests

def test_open_pull_requests_with_pending_items_come_right_after_plans():
    s = state(plans=[{"id": "p", "title": "Export", "status": "in_progress", "done": 1, "total": 3}],
              prs=[{"number": 12, "title": "Sharing", "summary": "PR #12 Sharing: 2 threads waiting (1 for you), CI red",
                    "needs_action": True, "for_user": 1},
                   {"number": 13, "title": "Docs", "summary": "PR #13 Docs: waiting for review", "needs_action": False, "for_user": 0}],
              bugs=["BUG-1 (high) Crash"])
    rec = recommendation(s, "add search")
    assert [o["key"] for o in rec["options"]][:3] == ["plan:p", "pr:12", "bugs"]
    assert rec["options"][1]["title"] == "Work on PR #12 Sharing: 2 threads waiting (1 for you), CI red"
    assert rec["options"][1]["reason"] == "1 item(s) need your decision"
    assert "1 pull request waiting" in rec["summary"]


def test_a_pending_pull_request_stops_the_straight_path():
    s = state(prs=[{"number": 12, "title": "x", "summary": "PR #12 x: CI red", "needs_action": True, "for_user": 0}])
    assert recommendation(s, "add search")["ask"] is True


def test_review_provider_none_skips_the_platform(repo):
    (repo / ".dev-cycle.json").write_text(json.dumps({"review": {"provider": "none"}}), encoding="utf-8")
    assert onboard.collect(str(repo))["prs"] == []


# ---------------------------------------------------------------- session started outside the repository
def make_repo(path):
    path.mkdir()
    run_git(path, "init", "-q", "-b", "main")
    (path / "app.py").write_text("print('hi')\n", encoding="utf-8")
    run_git(path, "add", "-A")
    run_git(path, "commit", "-q", "-m", "init")
    return path


def test_outside_a_repository_the_only_repository_below_is_used(tmp_path, capsys):
    project = make_repo(tmp_path / "app")
    (tmp_path / "notes").mkdir()
    assert onboard.resolve_root(str(tmp_path)) == (str(project), [str(project)])
    assert onboard.main(["diagnose", "--dir", str(tmp_path), "--json"]) == 0
    data = json.loads(capsys.readouterr().out)
    assert data["state"]["repo"]["git"] is True and data["summary"].startswith(f"Using {project}")
    assert all(o["key"] != "git" for o in data["options"])


def test_outside_several_repositories_the_user_chooses(tmp_path, capsys):
    for name in ("web", "api"):
        make_repo(tmp_path / name)
    assert onboard.resolve_root(str(tmp_path))[0] is None
    assert onboard.main(["diagnose", "--dir", str(tmp_path), "--json"]) == 0
    data = json.loads(capsys.readouterr().out)
    assert data["mode"] == "choose" and data["ask"] is True
    assert [o["title"] for o in data["options"]] == ["api", "web"]
    assert onboard.main(["propose-config", "--dir", str(tmp_path)]) == 1


def test_a_folder_without_repositories_is_still_a_new_project(tmp_path):
    assert onboard.resolve_root(str(tmp_path)) == (str(tmp_path), [])


# ---------------------------------------------------------------- files the .gitignore keeps out

def test_an_allowlist_gitignore_hides_the_protocol_files(repo, tmp_path, capsys):
    (repo / ".gitignore").write_text("/*\n!/.gitignore\n!/app.py\n!/docs/\n", encoding="utf-8")
    s = onboard.collect(str(repo), agent="claude-code")
    assert {i["path"]: i["entry"] for i in s["ignored"]} == {
        ".dev-cycle.json": "!/.dev-cycle.json", "TECDOC.md": "!/TECDOC.md", "tests/MANUAL_TESTS.md": "!/tests/",
        ".specify/memory/constitution.md": "!/.specify/",
        "specs/001-feature/spec.md": "!/specs/", ".claude/settings.json": "!/.claude/"}
    item = [o for o in recommendation(s)["options"] if o["key"] == "gitignore"][0]
    assert "!/.claude/, !/.dev-cycle.json, !/.specify/, !/TECDOC.md, !/specs/, !/tests/" in item["title"]
    proposal = repo / "add.json"
    proposal.write_text(json.dumps({"project_name": "demo"}), encoding="utf-8")
    assert onboard.main(["write-config", "--dir", str(repo), "--file", str(proposal)]) == 0
    assert "keeps .dev-cycle.json out of Git: add '!/.dev-cycle.json'" in capsys.readouterr().out


def test_tracked_or_allowed_files_and_declined_spec_kit_are_not_reported(repo):
    (repo / ".gitignore").write_text("docs/\n", encoding="utf-8")
    (repo / ".dev-cycle.json").write_text(json.dumps({"speckit": {"enabled": False}}), encoding="utf-8")
    s = onboard.collect(str(repo))
    assert [i["path"] for i in s["ignored"]] == ["docs/BACKLOG.md", "docs/ROADMAP.md", "docs/plans/plan.md",
                                                 "docs/product/VISION.md", "docs/product/TECH_SPEC.md",
                                                 "docs/architecture/subsystem.md", "docs/adr/0001-decision.md"]
    assert s["ignored"][0]["entry"] == "!/docs/"
    (repo / ".gitignore").write_text("", encoding="utf-8")
    assert onboard.collect(str(repo))["ignored"] == []
