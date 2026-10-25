import json
import os
import subprocess
import sys

import pytest

HERE = os.path.dirname(os.path.abspath(__file__))
SKILL = os.path.dirname(HERE)
sys.path.insert(0, SKILL)

import gedo_feedback as gf  # noqa: E402

SCRIPT = os.path.join(SKILL, "gedo_feedback.py")


def project(tmp_path, gedo=None):
    data = {} if gedo is None else {"gedo": gedo}
    (tmp_path / ".dev-cycle.json").write_text(json.dumps(data), encoding="utf-8")
    return str(tmp_path)


def run(tmp_path, *args):
    return gf.main(["--dir", str(tmp_path), *args])


ADD = ["add", "--kind", "missed-collision", "--title", "Radar missed an edit of the same function",
       "--scenario", "Two threads edited parse_rust in separate worktrees",
       "--expected", "gedo radar reports the collision", "--actual", "Nothing reported until the merge"]


def entries(tmp_path, folder="docs/gedo-feedback"):
    path = tmp_path / folder
    return sorted(p.name for p in path.iterdir()) if path.is_dir() else []


def test_recording_is_off_by_default(tmp_path, capsys):
    project(tmp_path)
    assert run(tmp_path, *ADD) == 2
    assert "gedo.enabled is false" in capsys.readouterr().out
    assert entries(tmp_path) == []
    assert run(tmp_path, "status") == 0
    assert "Gedo feedback: off" in capsys.readouterr().out


def test_feedback_can_be_turned_off_while_gedo_is_on(tmp_path, capsys):
    project(tmp_path, {"enabled": True, "feedback": False})
    assert run(tmp_path, *ADD) == 2
    assert "gedo.feedback is false" in capsys.readouterr().out


def test_add_writes_one_file_per_entry_and_round_trips(tmp_path, capsys):
    project(tmp_path, {"enabled": True})
    assert run(tmp_path, *ADD, "--severity", "high", "--related", "G3, FEAT-074", "--gedo-version", "0.4.0") == 0
    names = entries(tmp_path)
    assert len(names) == 1 and names[0].endswith("-radar-missed-an-edit-of-the.md")
    entry_id = names[0][:-3]
    entry = gf.FeedbackStore(str(tmp_path / "docs/gedo-feedback")).get(entry_id)
    assert entry["title"] == "Radar missed an edit of the same function"
    assert entry["fields"]["Kind"] == "missed-collision" and entry["fields"]["Severity"] == "high"
    assert entry["fields"]["Status"] == "open" and entry["fields"]["Related"] == "G3, FEAT-074"
    assert entry["sections"]["Actual"] == "Nothing reported until the merge"
    assert "Workaround" not in entry["sections"]
    assert gf.validate_entry(entry) == []


def test_duplicates_are_refused_unless_allowed(tmp_path, capsys):
    project(tmp_path, {"enabled": True})
    assert run(tmp_path, *ADD) == 0
    assert run(tmp_path, *ADD) == 1
    assert "same kind and title" in capsys.readouterr().out
    assert run(tmp_path, *ADD, "--allow-duplicate") == 0
    names = entries(tmp_path)
    assert len(names) == 2 and any(n.endswith("-the-2.md") for n in names)


def test_list_issue_reported_and_close(tmp_path, capsys):
    project(tmp_path, {"enabled": True, "feedback_dir": "notes/gedo"})
    run(tmp_path, *ADD, "--found-in", "feat/007-private-thing")
    entry_id = entries(tmp_path, "notes/gedo")[0][:-3]
    capsys.readouterr()

    assert run(tmp_path, "list", "--json") == 0
    rows = json.loads(capsys.readouterr().out)
    assert [r["id"] for r in rows] == [entry_id] and rows[0]["status"] == "open"

    body = tmp_path / "body.md"
    assert run(tmp_path, "issue", entry_id, "--body-file", str(body), "--json") == 0
    issue = json.loads(capsys.readouterr().out)
    assert issue["repo"] == "Levix-Digital/gedo"
    assert issue["title"] == "[missed-collision] Radar missed an edit of the same function"
    assert "## Scenario" in issue["body"] and body.read_text(encoding="utf-8") == issue["body"]
    assert "private-thing" not in issue["body"]

    assert run(tmp_path, "reported", entry_id, "--issue", "not a url") == 1
    assert run(tmp_path, "reported", entry_id, "--issue", "https://github.com/Levix-Digital/gedo/issues/9") == 0
    assert run(tmp_path, "issue", entry_id) == 1
    assert run(tmp_path, "close", entry_id, "--reason", "fixed in v0.4.1") == 0
    capsys.readouterr()
    assert run(tmp_path, "list") == 0
    assert "No Gedo feedback entries." in capsys.readouterr().out
    assert run(tmp_path, "list", "--all", "--json") == 0
    assert json.loads(capsys.readouterr().out)[0]["status"] == "closed"
    assert run(tmp_path, "validate") == 0


def test_issue_refuses_ai_attribution(tmp_path, capsys):
    project(tmp_path, {"enabled": True})
    run(tmp_path, "add", "--kind", "crash", "--title", "Daemon exits on save",
        "--scenario", "Saving a file", "--expected", "Ingest", "--actual", "Crash, as Claude found")
    entry_id = entries(tmp_path)[0][:-3]
    assert run(tmp_path, "issue", entry_id) == 1
    assert "principle 9" in capsys.readouterr().out


def test_validate_reports_broken_entries(tmp_path, capsys):
    project(tmp_path, {"enabled": True})
    folder = tmp_path / "docs/gedo-feedback"
    folder.mkdir(parents=True)
    (folder / "2026-01-01-bad.md").write_text(
        "# Bad\n\n- **Id**: other\n- **Kind**: oops\n- **Severity**: medium\n- **Status**: reported\n", encoding="utf-8")
    assert run(tmp_path, "validate") == 1
    out = capsys.readouterr().out
    assert "does not match the file name" in out and "Kind 'oops'" in out
    assert "needs its Issue link" in out and "missing section '## Scenario'" in out


def test_diagnostics_file_keeps_the_last_lines(tmp_path):
    project(tmp_path, {"enabled": True})
    log = tmp_path / "gedo.log"
    log.write_text("\n".join(f"line {i}" for i in range(250)), encoding="utf-8")
    assert run(tmp_path, *ADD, "--diagnostics-file", str(log)) == 0
    entry_id = entries(tmp_path)[0][:-3]
    text = (tmp_path / "docs/gedo-feedback" / f"{entry_id}.md").read_text(encoding="utf-8")
    assert "line 249" in text and "line 49\n" not in text and "last 200 of 250 lines" in text


def test_doctor_without_gedo_is_noted_not_fatal(tmp_path, monkeypatch):
    monkeypatch.setenv("PATH", str(tmp_path / "empty"))
    assert "not found on PATH" in gf.run_doctor()


def test_cli_entry_point(tmp_path):
    project(tmp_path)
    done = subprocess.run([sys.executable, SCRIPT, "--dir", str(tmp_path), "status", "--json"],
                          capture_output=True, text=True)
    assert done.returncode == 0
    assert json.loads(done.stdout)["active"] is False
