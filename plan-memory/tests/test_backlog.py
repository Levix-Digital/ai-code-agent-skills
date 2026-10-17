import json
import os
import subprocess
import sys

import pytest

HERE = os.path.dirname(os.path.abspath(__file__))
SKILL = os.path.dirname(HERE)
sys.path.insert(0, SKILL)

from _backlog import Backlog, BacklogError  # noqa: E402
import backlog as cli  # noqa: E402

TEMPLATE = os.path.join(SKILL, "templates", "BACKLOG_TEMPLATE.md")

SAMPLE = """# Backlog

Intro.

## Bugs

### BUG-001: Preset selects an outdated model id
- **Severity**: high
- **Status**: open
- **Found**: 2026-10-16, v0.45.0, PR #333
- **Plan**: -
- **Description**: `/preset frontier` sets a model id
  that the provider may reject.

### BUG-002: Search threshold too low
- **Severity**: low
- **Status**: planned
- **Found**: 2026-10-16
- **Plan**: fix_search_threshold
- **Description**: min_score 0.02.

## Features

### FEAT-001: Export answers
- **Priority**: medium
- **Status**: open
- **Found**: 2026-10-16
- **Plan**: -
- **Description**: Export to Markdown.

## Improvements

## Adjustments

## Closed

### BUG-003: Old crash
- **Severity**: critical
- **Status**: done
- **Found**: 2026-10-01
- **Plan**: -
- **Closed**: 2026-10-02, PR #300
- **Description**: Fixed.
"""


def template():
    with open(TEMPLATE, encoding="utf-8") as f:
        return f.read()


# --- model ---------------------------------------------------------------------------

def test_round_trip_keeps_the_file_identical():
    assert Backlog(SAMPLE).render() == SAMPLE
    assert Backlog(template()).render() == template()
    crlf = SAMPLE.replace("\n", "\r\n")
    assert Backlog(crlf).render() == crlf


def test_sample_is_valid_and_fields_join_continuation_lines():
    backlog = Backlog(SAMPLE)
    assert backlog.validate() == []
    bug = backlog.find("bug-001")
    assert bug.type == "bug" and bug.rank == "high" and bug.plan is None
    assert bug.get("Description") == "`/preset frontier` sets a model id that the provider may reject."


def test_next_id_counts_closed_items():
    backlog = Backlog(SAMPLE)
    assert backlog.next_id("bug") == "BUG-004"
    assert backlog.next_id("feature") == "FEAT-002"
    assert backlog.next_id("adjustment") == "ADJ-001"


def test_add_per_type_and_required_values():
    backlog = Backlog(template())
    bug = backlog.add("bug", "Crash on start", "It crashes.", "2026-10-16", severity="critical")
    idea = backlog.add("improvement", "Faster sync", "Sync faster.", "2026-10-16")
    assert (bug.id, bug.section, bug.rank) == ("BUG-001", "Bugs", "critical")
    assert (idea.id, idea.section, idea.rank) == ("IMP-001", "Improvements", "medium")
    assert backlog.validate() == []
    with pytest.raises(BacklogError, match="needs a severity"):
        backlog.add("bug", "No severity", "x", "2026-10-16")
    with pytest.raises(BacklogError, match="title"):
        backlog.add("feature", " ", "x", "2026-10-16")
    rendered = Backlog(backlog.render())
    assert [i.id for i in rendered.items] == ["BUG-001", "IMP-001"]


def test_validation_problems():
    bad = SAMPLE.replace("### FEAT-001: Export answers", "### BUG-001: Export answers") \
                .replace("- **Severity**: low", "- **Severity**: urgent") \
                .replace("- **Plan**: fix_search_threshold", "- **Plan**: -") \
                .replace("- **Status**: done", "- **Status**: open") + "\n## Ideas\n"
    text = "\n".join(Backlog(bad).validate())
    assert "duplicate id" in text
    assert "a bug belongs under '## Bugs'" in text
    assert "severity must be one of" in text
    assert "a planned item needs its plan id" in text
    assert "status under '## Closed' must be one of" in text
    assert "unknown section '## Ideas'" in text


def test_missing_fields_and_bad_headings():
    text = "\n".join(Backlog("# B\n\n## Features\n\n### FEAT-001 no colon\n\n### FEAT-002: T\n- **Status**: open\n").validate())
    assert "item heading must be" in text
    assert "missing '- **Priority**:'" in text and "missing '- **Description**:'" in text


def test_update_rules():
    backlog = Backlog(SAMPLE)
    backlog.update("BUG-001", severity="medium", title="New title")
    assert backlog.find("BUG-001").rank == "medium"
    with pytest.raises(BacklogError, match="is a bug"):
        backlog.update("BUG-001", priority="high")
    with pytest.raises(BacklogError, match="not a bug"):
        backlog.update("FEAT-001", severity="high")
    with pytest.raises(BacklogError, match="has no plan"):
        backlog.update("FEAT-001", status="planned")
    backlog.update("FEAT-001", plan="export_plan", status="planned")
    assert backlog.find("FEAT-001").plan == "export_plan"
    with pytest.raises(BacklogError, match="use close, drop or duplicate"):
        backlog.update("FEAT-001", status="done")
    with pytest.raises(BacklogError, match="no such backlog item"):
        backlog.update("FEAT-099", title="x")


def test_close_drop_duplicate_and_reopen():
    backlog = Backlog(SAMPLE)
    backlog.close("FEAT-001", "done", "2026-10-20, PR #340")
    backlog.close("BUG-002", "duplicate", "2026-10-20, duplicate of BUG-001")
    assert [i.id for i in backlog.sections["Closed"]] == ["BUG-003", "FEAT-001", "BUG-002"]
    assert backlog.validate() == []
    with pytest.raises(BacklogError, match="already closed"):
        backlog.close("FEAT-001", "done", "x")
    with pytest.raises(BacklogError, match="is closed"):
        backlog.update("FEAT-001", status="open")
    item = backlog.reopen("FEAT-001")
    assert item.section == "Features" and item.status == "open" and "Closed" not in item.fields
    assert Backlog(backlog.render()).validate() == []


def test_open_items_and_blocking_bugs_are_ordered():
    backlog = Backlog(SAMPLE)
    backlog.add("bug", "Worse", "x", "2026-10-16", severity="critical")
    assert [i.id for i in backlog.open_items()] == ["BUG-004", "BUG-001", "BUG-002", "FEAT-001"]
    assert [i.id for i in backlog.blocking_bugs(["critical", "high"])] == ["BUG-004", "BUG-001"]
    assert backlog.blocking_bugs([]) == []


# --- CLI -----------------------------------------------------------------------------

def run(tmp_path, *args):
    return cli.main([*args, "--dir", str(tmp_path)])


def test_cli_add_creates_the_file_and_validates(tmp_path, capsys):
    assert run(tmp_path, "validate") == 0
    assert run(tmp_path, "add", "--type", "feature", "--title", "Export", "--description", "Export answers.",
               "--found-in", "v1.0.0") == 0
    assert "FEAT-001: Export (medium)" in capsys.readouterr().out
    assert run(tmp_path, "add", "--type", "bug", "--title", "Crash", "--description", "Crashes.", "--severity", "high") == 0
    path = tmp_path / "docs" / "BACKLOG.md"
    text = path.read_text(encoding="utf-8")
    assert "### FEAT-001: Export" in text and "v1.0.0" in text and "### BUG-001: Crash" in text
    assert run(tmp_path, "validate") == 0
    assert run(tmp_path, "add", "--type", "bug", "--title", "x", "--description", "y") == 1
    assert "needs a severity" in capsys.readouterr().out


def test_cli_summary_list_and_closing(tmp_path, capsys):
    (tmp_path / "docs").mkdir()
    (tmp_path / "docs" / "BACKLOG.md").write_text(SAMPLE, encoding="utf-8")
    assert run(tmp_path, "summary", "--json") == 0
    data = json.loads(capsys.readouterr().out)
    assert data["blocking_bugs"] == ["BUG-001"] and data["closed"] == 1
    assert data["types"]["bug"] == {"open": 1, "planned": 1, "in_progress": 0}
    assert run(tmp_path, "summary") == 0
    assert "come before new work" in capsys.readouterr().out

    assert run(tmp_path, "close", "BUG-001", "--ref", "PR #9") == 0
    assert run(tmp_path, "drop", "FEAT-001", "--reason", "Out of scope") == 0
    assert run(tmp_path, "duplicate", "BUG-002", "--of", "BUG-002") == 1
    capsys.readouterr()
    assert run(tmp_path, "list", "--all", "--json") == 0
    items = {i["id"]: i for i in json.loads(capsys.readouterr().out)}
    assert items["BUG-001"]["status"] == "done" and "PR #9" in items["BUG-001"]["closed"]
    assert items["FEAT-001"]["status"] == "dropped"
    assert run(tmp_path, "validate") == 0


def test_cli_custom_file_from_config(tmp_path):
    (tmp_path / ".dev-cycle.json").write_text(json.dumps({"backlog": {"file": "BACKLOG.md"}}), encoding="utf-8")
    assert run(tmp_path, "add", "--type", "adjustment", "--title", "Rename", "--description", "Rename it.") == 0
    assert (tmp_path / "BACKLOG.md").is_file()


def test_cli_refuses_to_rewrite_an_unreadable_file(tmp_path, capsys):
    (tmp_path / "docs").mkdir()
    original = SAMPLE + "\n## Ideas\n\nSomething important.\n"
    (tmp_path / "docs" / "BACKLOG.md").write_text(original, encoding="utf-8")
    assert run(tmp_path, "close", "BUG-001") == 1
    assert "structural problems" in capsys.readouterr().out
    assert (tmp_path / "docs" / "BACKLOG.md").read_text(encoding="utf-8") == original


def test_script_runs_standalone(tmp_path):
    script = os.path.join(SKILL, "backlog.py")
    res = subprocess.run([sys.executable, script, "summary", "--dir", str(tmp_path)],
                         capture_output=True, text=True, encoding="utf-8")
    assert res.returncode == 0 and "No backlog" in res.stdout
