import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SCRIPT = os.path.join(os.path.dirname(HERE), "verify_gate.py")
sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(HERE)), "product-inception", "tests"))

from test_check_product import TECH_SPEC, VISION  # noqa: E402

BACKLOG = """# Backlog

## Bugs

## Features

### FEAT-001: Upload photos
- **Priority**: high
- **Epic**: EPIC-001
- **Status**: open
- **Found**: 2026-10-23
- **Plan**: -
- **Description**: Upload.

## Improvements

## Adjustments

## Epics

### EPIC-001: Photos
- **Priority**: high
- **Status**: open
- **Found**: 2026-10-23
- **Description**: Photo management.

## Closed
"""

ROADMAP = """# Roadmap

## v0.1.0: MVP
- **Status**: planned
- **Goal**: Families keep photos in one place.
- **Items**: FEAT-001
"""


def make_project(tmp_path, config=None, backlog=BACKLOG, roadmap=ROADMAP, vision=VISION):
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    (tmp_path / "docs" / "product").mkdir(parents=True)
    (tmp_path / "docs" / "product" / "VISION.md").write_text(vision, encoding="utf-8")
    (tmp_path / "docs" / "product" / "TECH_SPEC.md").write_text(TECH_SPEC, encoding="utf-8")
    (tmp_path / "TECDOC.md").write_text("# Tech\n", encoding="utf-8")
    if backlog is not None:
        (tmp_path / "docs" / "BACKLOG.md").write_text(backlog, encoding="utf-8")
    if roadmap is not None:
        (tmp_path / "docs" / "ROADMAP.md").write_text(roadmap, encoding="utf-8")
    cfg = {"architecture": {"style": "layered"}} if config is None else config
    (tmp_path / ".dev-cycle.json").write_text(json.dumps(cfg), encoding="utf-8")
    return str(tmp_path)


def gate(project):
    return subprocess.run([sys.executable, SCRIPT, "--dir", project, "--gate", "0"],
                          capture_output=True, text=True, encoding="utf-8")


def test_gate_0_passes_on_a_complete_baseline(tmp_path):
    res = gate(make_project(tmp_path))
    assert res.returncode == 0, res.stdout
    assert "Product documents (product-inception) passed" in res.stdout
    assert "Backlog: 1 epic(s)" in res.stdout and "first: v0.1.0 MVP (1 item(s))" in res.stdout


def test_gate_0_lists_open_questions_without_blocking(tmp_path):
    vision = VISION.replace("## Open Questions\n\nNone.", "## Open Questions\n\n- Pricing in Brazil?")
    res = gate(make_project(tmp_path, vision=vision))
    assert res.returncode == 0, res.stdout
    assert "1 open question(s) remain" in res.stdout and "Pricing in Brazil?" in res.stdout


def test_gate_0_blocks_without_style_backlog_and_roadmap(tmp_path):
    res = gate(make_project(tmp_path, config={}, backlog=None, roadmap=None))
    assert res.returncode == 1
    assert "architecture.style is not set" in res.stdout
    assert "No backlog" in res.stdout and "No roadmap" in res.stdout


def test_gate_0_needs_epics_and_a_release_with_items(tmp_path):
    no_epic = BACKLOG.split("## Epics")[0].replace("- **Epic**: EPIC-001\n", "") + "## Closed\n"
    empty_release = ROADMAP.replace("- **Items**: FEAT-001", "- **Items**: -")
    res = gate(make_project(tmp_path, backlog=no_epic, roadmap=empty_release))
    assert res.returncode == 1
    assert "has no epic" in res.stdout and "no release with items" in res.stdout


def test_gate_0_blocks_on_placeholders(tmp_path):
    res = gate(make_project(tmp_path, vision=VISION.replace("Photos are spread", "[Describe the problem] Photos are spread")))
    assert res.returncode == 1 and "Product documents (product-inception) failed" in res.stdout
