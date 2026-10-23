import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SKILL = os.path.dirname(HERE)
sys.path.insert(0, SKILL)

import check_product as cp  # noqa: E402

VISION = """# PhotoShare

PhotoShare helps families keep their photos in one private place.

## Problem

Photos are spread over phones and chats; families lose them.

## Target Users

Parents who take most of the family photos (primary) and relatives who view them.

## Value Proposition

One private album the whole family can add to, without social media.

## Business Model

Free up to 10 GB; a family plan for larger storage.

## Success Metrics

- 60% of invited relatives open the album in the first week.

## Scope

- Upload and organize photos.
- Share albums with the family.

## Non-Goals

- Public profiles.

## Risks and Assumptions

- Storage costs stay below the plan price.

## Open Questions

None.
"""

TECH_SPEC = """# PhotoShare: Technical Specification

A mobile app with a REST backend, layered architecture.

## Platforms and Stack

Android and iOS (Flutter), Python backend, PostgreSQL.

## Architecture Overview

See TECDOC.md.

## Non-Functional Requirements

### Performance

Uploads of 10 MB finish in under 5 s on 4G.

## Integrations

Object storage for the photos.

## Data

Users, families, albums, photos.

## Constraints

Hosting budget under 200 USD per month.

## Open Questions

None.
"""


def write(project, vision=VISION, tech=TECH_SPEC):
    (project / "docs" / "product").mkdir(parents=True, exist_ok=True)
    if vision is not None:
        (project / "docs" / "product" / "VISION.md").write_text(vision, encoding="utf-8")
    if tech is not None:
        (project / "docs" / "product" / "TECH_SPEC.md").write_text(tech, encoding="utf-8")


def project(tmp_path):
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    return tmp_path


def errors(results):
    return [e for r in results for e in r.errors]


def test_complete_documents_pass(tmp_path):
    p = project(tmp_path)
    write(p)
    results = cp.run(str(p))
    assert errors(results) == [] and all(r.open_questions == [] for r in results)


def test_templates_fail_with_placeholders(tmp_path):
    p = project(tmp_path)
    assert cp.init_docs(str(p)) == ["docs/product/VISION.md", "docs/product/TECH_SPEC.md"]
    assert cp.init_docs(str(p)) == []
    found = " | ".join(errors(cp.run(str(p))))
    assert "template placeholder left in place: [Product name]" in found
    assert "TECH_SPEC.md" in found and "VISION.md" in found


def test_missing_and_empty_sections(tmp_path):
    p = project(tmp_path)
    write(p, vision=VISION.replace("## Business Model\n\nFree up to 10 GB; a family plan for larger storage.\n", "")
                         .replace("Public profiles.", "").replace("- \n", ""))
    found = " | ".join(errors(cp.run(str(p))))
    assert "missing section: ## Business Model" in found
    assert "section 'Non-Goals' is empty" in found


def test_open_questions_are_warnings(tmp_path):
    p = project(tmp_path)
    write(p, tech=TECH_SPEC.replace("## Open Questions\n\nNone.", "## Open Questions\n\n- Which push service?\n- iPad support?"))
    results = cp.run(str(p))
    assert errors(results) == []
    tech = [r for r in results if r.kind == "tech_spec"][0]
    assert tech.open_questions == ["Which push service?", "iPad support?"]


def test_links_and_checkboxes_are_not_placeholders(tmp_path):
    p = project(tmp_path)
    write(p, vision=VISION.replace("Photos are spread", "See [the survey](survey.md) and - [x] done. Photos are spread"))
    assert errors(cp.run(str(p))) == []


def test_missing_documents_and_configured_paths(tmp_path):
    p = project(tmp_path)
    (p / ".dev-cycle.json").write_text(json.dumps({"product": {"vision": "product/vision.md"}}), encoding="utf-8")
    found = " | ".join(errors(cp.run(str(p))))
    assert "product/vision.md:1: business document not found" in found
    assert cp.main(["--dir", str(p)]) == 1


def test_portuguese_text_is_reported(tmp_path):
    p = project(tmp_path)
    write(p, vision=VISION.replace("## Problem", "## Problema da família"))
    assert any("may not be in English: família" in e for e in errors(cp.run(str(p))))


def test_json_output(tmp_path, capsys):
    p = project(tmp_path)
    write(p)
    assert cp.main(["--dir", str(p), "--json"]) == 0
    data = json.loads(capsys.readouterr().out)
    assert [d["kind"] for d in data] == ["vision", "tech_spec"]
