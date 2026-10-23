import json
import os
import subprocess
import sys

import pytest

HERE = os.path.dirname(os.path.abspath(__file__))
SKILL = os.path.dirname(HERE)
sys.path.insert(0, SKILL)

from _backlog import Backlog, BacklogError  # noqa: E402
from _roadmap import Roadmap, RoadmapError  # noqa: E402
import backlog as backlog_cli  # noqa: E402
import roadmap as cli  # noqa: E402

TEMPLATE = os.path.join(SKILL, "templates", "ROADMAP_TEMPLATE.md")

SAMPLE = """# Roadmap

Intro.

## v0.1.0: MVP
- **Status**: released
- **Released**: 2026-10-01
- **Goal**: First usable version.
- **Items**: FEAT-001, BUG-001

## v0.2.0: Albums
- **Status**: in_progress
- **Target**: 2027-01
- **Goal**: Organize photos
  into albums.
- **Items**: FEAT-002,
  FEAT-003

## v1.0.0: Launch
- **Status**: planned
- **Goal**: Public launch.
- **Items**: -
"""

BACKLOG = """# Backlog

## Bugs

### BUG-002: Upload hangs
- **Severity**: high
- **Status**: open
- **Found**: 2026-10-20
- **Plan**: -
- **Description**: Hangs.

## Features

### FEAT-002: Albums
- **Priority**: high
- **Epic**: EPIC-001
- **Status**: open
- **Found**: 2026-10-01
- **Plan**: -
- **Description**: Albums.

### FEAT-003: Share albums
- **Priority**: low
- **Epic**: EPIC-001
- **Status**: planned
- **Found**: 2026-10-01
- **Plan**: share_albums
- **Description**: Share.

### FEAT-004: Search
- **Priority**: medium
- **Status**: open
- **Found**: 2026-10-01
- **Plan**: -
- **Description**: Search.

## Improvements

## Adjustments

## Epics

### EPIC-001: Photo organization
- **Priority**: high
- **Status**: open
- **Found**: 2026-10-01
- **Description**: Everything about organizing photos.

## Closed

### FEAT-001: Upload
- **Priority**: high
- **Status**: done
- **Found**: 2026-09-01
- **Plan**: upload
- **Closed**: 2026-10-01, PR #1
- **Description**: Upload.

### BUG-001: Crash
- **Severity**: critical
- **Status**: done
- **Found**: 2026-09-01
- **Plan**: crash
- **Closed**: 2026-09-20, PR #2
- **Description**: Crash.
"""


def test_round_trip_keeps_the_file_identical():
    assert Roadmap(SAMPLE).render() == SAMPLE
    crlf = SAMPLE.replace("\n", "\r\n")
    assert Roadmap(crlf).render() == crlf


def test_template_is_a_valid_empty_roadmap():
    text = open(TEMPLATE, encoding="utf-8").read()
    roadmap = Roadmap(text)
    assert roadmap.releases == [] and roadmap.validate() == []


def test_parse_releases_and_items():
    roadmap = Roadmap(SAMPLE)
    assert [r.version for r in roadmap.releases] == ["v0.1.0", "v0.2.0", "v1.0.0"]
    assert roadmap.find("v0.2.0").items == ["FEAT-002", "FEAT-003"]
    assert roadmap.find("v0.2.0").get("Goal") == "Organize photos into albums."
    assert roadmap.find("v1.0.0").items == []
    assert roadmap.release_of("feat-003").version == "v0.2.0"
    assert roadmap.current().version == "v0.2.0"
    assert Roadmap(SAMPLE).validate(Backlog(BACKLOG)) == []


def test_add_and_move_releases():
    roadmap = Roadmap(SAMPLE)
    roadmap.add_release("v0.3.0", "Search", "Find photos", target="2027-02", after="v0.2.0")
    assert [r.version for r in roadmap.releases] == ["v0.1.0", "v0.2.0", "v0.3.0", "v1.0.0"]
    with pytest.raises(RoadmapError):
        roadmap.add_release("v0.3.0", "Again", "x")
    with pytest.raises(RoadmapError):
        roadmap.add_release("0.4", "Bad", "x")
    with pytest.raises(RoadmapError):
        roadmap.add_release("v0.4.0", "", "x")
    roadmap.move_release("v1.0.0", before="v0.3.0")
    assert [r.version for r in roadmap.releases] == ["v0.1.0", "v0.2.0", "v1.0.0", "v0.3.0"]
    assert any("version order" in p for p in roadmap.validate())
    with pytest.raises(RoadmapError):
        roadmap.move_release("v0.3.0", before="v9.9.9")
    assert [r.version for r in roadmap.releases] == ["v0.1.0", "v0.2.0", "v1.0.0", "v0.3.0"]


def test_assign_moves_only_when_asked():
    roadmap = Roadmap(SAMPLE)
    assert roadmap.assign(["FEAT-004"], "v1.0.0") == ["FEAT-004"]
    with pytest.raises(RoadmapError, match="already in v1.0.0"):
        roadmap.assign(["FEAT-004"], "v0.2.0")
    roadmap.assign(["FEAT-004"], "v0.2.0", move=True)
    assert roadmap.find("v1.0.0").items == [] and roadmap.find("v1.0.0").get("Items") == "-"
    assert roadmap.find("v0.2.0").items == ["FEAT-002", "FEAT-003", "FEAT-004"]
    assert roadmap.assign(["FEAT-004"], "v0.2.0") == []


def test_released_releases_are_frozen():
    roadmap = Roadmap(SAMPLE)
    with pytest.raises(RoadmapError, match="released"):
        roadmap.assign(["FEAT-004"], "v0.1.0")
    with pytest.raises(RoadmapError, match="released"):
        roadmap.unassign(["FEAT-001"])
    with pytest.raises(RoadmapError, match="released"):
        roadmap.assign(["FEAT-001"], "v1.0.0", move=True)
    with pytest.raises(RoadmapError):
        roadmap.assign(["NOPE-1"], "v1.0.0")


def test_release_status_and_date():
    roadmap = Roadmap(SAMPLE)
    with pytest.raises(RoadmapError):
        roadmap.update_release("v0.2.0", status="released")
    roadmap.update_release("v0.2.0", status="released", released_on="2027-01-10")
    assert roadmap.find("v0.2.0").get("Released") == "2027-01-10"
    roadmap.update_release("v0.2.0", status="in_progress")
    assert "Released" not in roadmap.find("v0.2.0").fields
    roadmap.update_release("v0.2.0", target="")
    assert "Target" not in roadmap.find("v0.2.0").fields


def test_validation_problems():
    bad = SAMPLE.replace("- **Items**: -", "- **Items**: FEAT-002, EPIC-001, FEAT-099") \
                .replace("- **Released**: 2026-10-01\n", "")
    problems = " | ".join(Roadmap(bad).validate(Backlog(BACKLOG)))
    assert "FEAT-002 is also in v0.2.0" in problems
    assert "EPIC-001 is an epic" in problems
    assert "FEAT-099 is not in the backlog" in problems
    assert "needs '- **Released**: <date>'" in problems
    order = SAMPLE.replace("- **Status**: planned", "- **Status**: released\n- **Released**: 2027-03-01") \
                  .replace("- **Status**: in_progress", "- **Status**: planned")
    assert any("comes after one that is not released" in p for p in Roadmap(order).validate())
    broken = Roadmap(SAMPLE + "\n## Next things\n")
    assert any("release heading" in p for p in broken.validate())


# ---------------------------------------------------------------- CLI

@pytest.fixture
def project(tmp_path):
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    (tmp_path / "docs").mkdir()
    (tmp_path / "docs" / "BACKLOG.md").write_text(BACKLOG, encoding="utf-8")
    return tmp_path


def run(project, *argv):
    return cli.main(list(argv) + ["--dir", str(project)])


def test_cli_creates_the_roadmap_and_expands_epics(project, capsys):
    assert run(project, "add-release", "v0.1.0", "--name", "MVP", "--goal", "First version") == 0
    assert run(project, "add-release", "v0.2.0", "--name", "Next", "--goal", "More") == 0
    assert run(project, "assign", "EPIC-001", "BUG-002", "--release", "v0.1.0") == 0
    text = (project / "docs" / "ROADMAP.md").read_text(encoding="utf-8")
    assert "- **Items**: FEAT-002, FEAT-003, BUG-002" in text
    assert run(project, "assign", "EPIC-001", "--release", "v0.2.0") == 0
    assert "has no open item to assign" in capsys.readouterr().out
    assert run(project, "assign", "FEAT-001", "--release", "v0.2.0") == 1
    assert "closed" in capsys.readouterr().out
    assert run(project, "validate") == 0


def test_cli_current_lists_bugs_first_and_unscheduled_items(project, capsys):
    run(project, "add-release", "v0.1.0", "--name", "MVP", "--goal", "First version")
    run(project, "assign", "FEAT-002", "BUG-002", "--release", "v0.1.0")
    capsys.readouterr()
    assert run(project, "current", "--json") == 0
    data = json.loads(capsys.readouterr().out)
    assert data["release"]["version"] == "v0.1.0"
    assert [d["id"] for d in data["open_items"]] == ["BUG-002", "FEAT-002"]
    assert data["unscheduled"] == ["FEAT-004", "FEAT-003"]   # by priority


def test_cli_release_warns_about_open_items(project, capsys):
    run(project, "add-release", "v0.1.0", "--name", "MVP", "--goal", "First version")
    run(project, "assign", "FEAT-002", "--release", "v0.1.0")
    capsys.readouterr()
    assert run(project, "set-release", "v0.1.0", "--status", "released") == 0
    out = capsys.readouterr().out
    assert "Still open in this release: FEAT-002" in out
    assert run(project, "validate") == 0
    assert "is released but FEAT-002 is still open" in capsys.readouterr().out


def test_cli_without_a_roadmap(project, capsys):
    assert run(project, "show") == 0 and run(project, "validate") == 0
    assert "No roadmap" in capsys.readouterr().out
    assert run(project, "assign", "FEAT-002", "--release", "v0.1.0") == 1


def test_configured_roadmap_path(project):
    (project / ".dev-cycle.json").write_text(json.dumps({"roadmap": {"file": "planning/ROADMAP.md"}}), encoding="utf-8")
    assert run(project, "add-release", "v1.0.0", "--name", "One", "--goal", "Ship") == 0
    assert (project / "planning" / "ROADMAP.md").is_file()


# ---------------------------------------------------------------- epics in the backlog

def test_epic_items_and_progress():
    backlog = Backlog(BACKLOG)
    assert [i.id for i in backlog.epic_items("EPIC-001")] == ["FEAT-002", "FEAT-003"]
    assert backlog.find("FEAT-002").epic == "EPIC-001"
    assert backlog.validate() == []
    assert Backlog(BACKLOG).render() == BACKLOG


def test_add_and_set_epic():
    backlog = Backlog(BACKLOG)
    item = backlog.add("feature", "Tags", "Tag photos", "2026-10-23", epic="epic-001")
    assert item.epic == "EPIC-001"
    epic = backlog.add("epic", "Social", "Social features", "2026-10-23", priority="low")
    assert epic.id == "EPIC-002" and "Plan" not in epic.fields
    with pytest.raises(BacklogError, match="not an epic"):
        backlog.add("feature", "X", "Y", "2026-10-23", epic="FEAT-002")
    with pytest.raises(BacklogError, match="another epic"):
        backlog.add("epic", "X", "Y", "2026-10-23", epic="EPIC-001")
    backlog.update("FEAT-004", epic="EPIC-002")
    assert backlog.find("FEAT-004").epic == "EPIC-002"
    backlog.update("FEAT-004", epic="-")
    assert backlog.find("FEAT-004").epic is None
    with pytest.raises(BacklogError, match="is an epic"):
        backlog.update("EPIC-001", status="planned")
    assert backlog.validate() == []


def test_epic_validation_and_warnings():
    bad = BACKLOG.replace("- **Epic**: EPIC-001\n- **Status**: open", "- **Epic**: EPIC-009\n- **Status**: open")
    assert any("epic EPIC-009 is not in the backlog" in p for p in Backlog(bad).validate())
    planned_epic = BACKLOG.replace("- **Priority**: high\n- **Status**: open\n- **Found**: 2026-10-01\n- **Description**: Everything",
                                   "- **Priority**: high\n- **Status**: planned\n- **Found**: 2026-10-01\n- **Description**: Everything")
    assert any("never promoted" in p for p in Backlog(planned_epic).validate())
    backlog = Backlog(BACKLOG)
    backlog.close("FEAT-002", "done", "2026-10-23")
    backlog.close("FEAT-003", "dropped", "2026-10-23, out of scope")
    assert any("every item is closed" in w for w in backlog.warnings())


def test_old_backlog_without_epics_section_stays_identical():
    old = BACKLOG.split("## Epics")[0] + "## Closed\n"
    old = old.replace("- **Epic**: EPIC-001\n", "")
    assert Backlog(old).render() == old


def test_promote_refuses_an_epic(project, capsys):
    assert backlog_cli.main(["promote", "EPIC-001", "--dir", str(project)]) == 1
    assert "promote its items instead (FEAT-002, FEAT-003)" in capsys.readouterr().out


def test_backlog_cli_epic_options(project, capsys):
    assert backlog_cli.main(["add", "--type", "feature", "--title", "Tags", "--description", "Tag photos",
                             "--epic", "EPIC-001", "--dir", str(project)]) == 0
    capsys.readouterr()
    assert backlog_cli.main(["list", "--epic", "EPIC-001", "--json", "--dir", str(project)]) == 0
    ids = [i["id"] for i in json.loads(capsys.readouterr().out)]
    assert ids == ["FEAT-002", "FEAT-005", "FEAT-003"]   # by priority
    assert backlog_cli.main(["summary", "--dir", str(project)]) == 0
    assert "(0/3 items closed)" in capsys.readouterr().out


# ---------------------------------------------------------------- superseded items

def test_supersede_closes_with_the_replacements():
    backlog = Backlog(BACKLOG)
    a = backlog.add("feature", "Share album by link", "Link", "2026-10-23", epic="EPIC-001")
    b = backlog.add("feature", "Invite relatives", "Invite", "2026-10-23", epic="EPIC-001")
    item = backlog.supersede("FEAT-003", [a.id, b.id.lower(), a.id], "2026-10-23")
    assert item.closed and item.status == "superseded"
    assert item.get("Closed") == f"2026-10-23, superseded by {a.id}, {b.id}"
    assert item.superseded_by == [a.id, b.id]
    assert backlog.validate() == []
    rendered = Backlog(backlog.render())
    assert rendered.find("FEAT-003").superseded_by == [a.id, b.id]


def test_supersede_rules():
    backlog = Backlog(BACKLOG)
    with pytest.raises(BacklogError, match="itself"):
        backlog.supersede("FEAT-002", ["FEAT-002"], "2026-10-23")
    with pytest.raises(BacklogError, match="closed"):
        backlog.supersede("FEAT-002", ["FEAT-001"], "2026-10-23")
    with pytest.raises(BacklogError, match="an epic is superseded by epics"):
        backlog.supersede("FEAT-002", ["EPIC-001"], "2026-10-23")
    with pytest.raises(BacklogError, match="no such"):
        backlog.supersede("FEAT-002", ["FEAT-099"], "2026-10-23")
    with pytest.raises(BacklogError, match="--by"):
        backlog.supersede("FEAT-002", [], "2026-10-23")


def test_superseded_validation():
    closed = """
### FEAT-009: Old idea
- **Priority**: low
- **Status**: superseded
- **Found**: 2026-10-01
- **Plan**: -
- **Closed**: 2026-10-20, superseded by {by}
- **Description**: Old.
"""
    assert Backlog(BACKLOG + closed.format(by="FEAT-004")).validate() == []
    problems = " | ".join(Backlog(BACKLOG + closed.format(by="FEAT-077")).validate())
    assert "superseded by unknown item FEAT-077" in problems
    problems = " | ".join(Backlog(BACKLOG + closed.replace("superseded by {by}", "replaced")).validate())
    assert "needs 'superseded by <ID>" in problems


def test_cli_supersede_and_roadmap_hint(project, capsys):
    run(project, "add-release", "v0.1.0", "--name", "MVP", "--goal", "First version")
    run(project, "assign", "FEAT-002", "--release", "v0.1.0")
    assert backlog_cli.main(["supersede", "FEAT-002", "--by", "FEAT-004", "--dir", str(project)]) == 0
    assert "closed as superseded by FEAT-004" in capsys.readouterr().out
    assert run(project, "validate") == 0
    out = capsys.readouterr().out
    assert "FEAT-002 in v0.1.0 was superseded; unassign it; assign FEAT-004" in out
    assert backlog_cli.main(["supersede", "FEAT-003", "--by", "FEAT-004", "--dir", str(project)]) == 0
    assert "was in plan 'share_albums'" in capsys.readouterr().out
