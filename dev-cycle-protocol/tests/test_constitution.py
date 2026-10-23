import json
import os
import subprocess
import sys

import pytest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))

import constitution as con  # noqa: E402

TECH_SPEC = """# Demo: Technical Specification

Demo.

## Non-Functional Requirements

### Performance

p95 under 300 ms.

## Constraints

Hosting under 200 USD per month.
"""

ADR = """---
id: 1
title: Use PostgreSQL
status: {status}
date: '2026-10-23'
---

# ADR-0001: Use PostgreSQL
"""


@pytest.fixture
def project(tmp_path):
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    (tmp_path / ".dev-cycle.json").write_text(json.dumps({"project_name": "demo", "architecture": {"style": "layered"}}),
                                              encoding="utf-8")
    (tmp_path / ".specify" / "memory").mkdir(parents=True)
    (tmp_path / "docs" / "product").mkdir(parents=True)
    (tmp_path / "docs" / "product" / "TECH_SPEC.md").write_text(TECH_SPEC, encoding="utf-8")
    (tmp_path / "docs" / "adr").mkdir()
    (tmp_path / "docs" / "adr" / "0001-use-postgresql.md").write_text(ADR.format(status="accepted"), encoding="utf-8")
    return tmp_path


def text(project):
    return (project / con.CONSTITUTION).read_text(encoding="utf-8")


def test_generate_collects_every_source(project):
    assert con.main(["generate", "--dir", str(project)]) == 0
    t = text(project)
    assert t.startswith("<!-- dev-cycle-constitution\n")
    assert "# demo Constitution" in t
    assert "### I. Design" in t and "#### SOLID, in practice" in t
    assert "### II. Simplicity First" in t
    assert "### III. Architecture Style: layered" in t and "#### Dependency rule" in t
    assert "### IV. Development Cycle" in t and "**Bugs first**" in t
    assert "p95 under 300 ms." in t and "Hosting under 200 USD per month." in t
    assert "- ADR-0001: Use PostgreSQL (`docs/adr/0001-use-postgresql.md`)" in t
    assert t.rstrip().endswith("**Version**: 1.0.0 | **Ratified**: " + t.rstrip()[-10:] + " | **Last Amended**: " + t.rstrip()[-10:])
    assert con.check(str(project)) == []


def test_generation_is_deterministic_and_idempotent(project):
    first, changed = con.build(str(project), None, today="2026-10-23")
    assert changed
    again, changed = con.build(str(project), first, today="2026-11-01")
    assert again == first and not changed


def test_a_changed_source_makes_it_stale_and_bumps_the_version(project):
    con.main(["generate", "--dir", str(project)])
    (project / "docs" / "adr" / "0001-use-postgresql.md").write_text(ADR.format(status="superseded"), encoding="utf-8")
    assert any("stale" in p for p in con.check(str(project)))
    previous = text(project)
    new, changed = con.build(str(project), previous, today="2099-01-01")
    assert changed and "ADR-0001" not in new
    assert "**Version**: 1.1.0" in new and "**Last Amended**: 2099-01-01" in new
    assert "**Ratified**: 2099-01-01" not in new


def test_style_change_makes_it_stale(project):
    con.main(["generate", "--dir", str(project)])
    (project / ".dev-cycle.json").write_text(json.dumps({"project_name": "demo", "architecture": {"style": "hexagonal"}}),
                                             encoding="utf-8")
    assert any("stale" in p for p in con.check(str(project)))
    con.main(["generate", "--dir", str(project)])
    assert "Architecture Style: hexagonal" in text(project) and con.check(str(project)) == []


def test_hand_edits_are_detected_and_replaced(project):
    con.main(["generate", "--dir", str(project)])
    path = project / con.CONSTITUTION
    path.write_text(text(project).replace("Hosting under", "Hosting maybe under"), encoding="utf-8")
    assert any("edited by hand" in p for p in con.check(str(project)))
    assert con.main(["generate", "--dir", str(project)]) == 0
    assert "Hosting maybe" not in text(project) and con.check(str(project)) == []


def test_spec_kit_template_is_not_a_generated_constitution(project):
    (project / con.CONSTITUTION).write_text("# [PROJECT_NAME] Constitution\n", encoding="utf-8")
    assert any("still Spec Kit's template" in p for p in con.check(str(project)))


def test_requires_spec_kit(tmp_path):
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    assert con.main(["generate", "--dir", str(tmp_path)]) == 1
    assert con.check(str(tmp_path)) == ["Spec Kit is not initialized (.specify/ missing)"]


def test_without_product_documents_or_style(tmp_path):
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    (tmp_path / ".specify" / "memory").mkdir(parents=True)
    assert con.main(["generate", "--dir", str(tmp_path)]) == 0
    t = (tmp_path / con.CONSTITUTION).read_text(encoding="utf-8")
    assert "### III. Development Cycle" in t and "Product Requirements" not in t and "Architecture Decisions" not in t


HAND_WRITTEN = "# Photos Constitution\n\n## Core Principles\n\n### I. Privacy first\nNo photo leaves the family group.\n"


def test_hand_written_constitution_is_never_replaced_silently(project, capsys):
    (project / con.CONSTITUTION).write_text(HAND_WRITTEN, encoding="utf-8")
    assert any("is hand-written" in p for p in con.check(str(project)))
    assert con.main(["generate", "--dir", str(project)]) == 1
    assert "Move its principles into the sources first" in capsys.readouterr().out
    assert text(project) == HAND_WRITTEN
    assert con.main(["generate", "--dir", str(project), "--replace-hand-written"]) == 0
    assert (project / con.HAND_WRITTEN_COPY).read_text(encoding="utf-8") == HAND_WRITTEN
    assert text(project).startswith("<!-- dev-cycle-constitution") and con.check(str(project)) == []


def test_kinds_of_constitution():
    assert con.kind_of("# [PROJECT_NAME] Constitution\n") == "template"
    assert con.kind_of(HAND_WRITTEN) == "hand-written"
    assert con.kind_of("<!-- dev-cycle-constitution\nsources-sha256: x\n-->\n# X\n") == "generated"
