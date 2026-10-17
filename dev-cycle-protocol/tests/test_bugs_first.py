import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SKILL = os.path.dirname(HERE)
SCRIPT = os.path.join(SKILL, "verify_gate.py")
sys.path.insert(0, SKILL)

from inspect_project import backlog_profile  # noqa: E402

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


def test_low_bugs_only_inform(tmp_path):
    root = project(tmp_path, severity="low")
    write_plan(root, "export", "feature")
    res = gate(root, "--gate", "3")
    assert res.returncode == 0 and "Other open bugs" in res.stdout


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


def test_inspect_project_reports_the_backlog(tmp_path):
    assert backlog_profile(str(tmp_path)) == {"file": "docs/BACKLOG.md", "exists": False}
    root = project(tmp_path)
    info = backlog_profile(str(root))
    assert info["open"]["bug"] == 1 and info["blocking_bugs"] == ["BUG-001 (high) Crash on start"]
