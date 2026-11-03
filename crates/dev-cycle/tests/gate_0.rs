//! `verify_gate --gate 0`: the product baseline (formerly
//! `dev-cycle-protocol/tests/test_gate_0.py`).

mod common;

use common::{Project, gate, has};

const VISION: &str = "# PhotoShare

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
";

const TECH_SPEC: &str = "# PhotoShare: Technical Specification

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
";

const BACKLOG: &str = "# Backlog

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
";

const ROADMAP: &str = "# Roadmap

## v0.1.0: MVP
- **Status**: planned
- **Goal**: Families keep photos in one place.
- **Items**: FEAT-001
";

const LAYERED: &str = r#"{"architecture": {"style": "layered"}}"#;

fn make_project(
    p: &Project,
    config: &str,
    backlog: Option<&str>,
    roadmap: Option<&str>,
    vision: &str,
) {
    p.git(&["init", "-q"]);
    p.write("docs/product/VISION.md", vision);
    p.write("docs/product/TECH_SPEC.md", TECH_SPEC);
    p.write("TECDOC.md", "# Tech\n");
    if let Some(b) = backlog {
        p.write("docs/BACKLOG.md", b);
    }
    if let Some(r) = roadmap {
        p.write("docs/ROADMAP.md", r);
    }
    p.write(".dev-cycle.json", config);
}

#[test]
fn gate_0_passes_on_a_complete_baseline() {
    let p = Project::new();
    make_project(&p, LAYERED, Some(BACKLOG), Some(ROADMAP), VISION);
    let res = gate(&p, &["--gate", "0"]);
    assert!(res.ok(), "{}", res.stdout);
    has(
        &res.stdout,
        &[
            "Product documents (product-inception) passed",
            "Backlog: 1 epic(s)",
            "first: v0.1.0 MVP (1 item(s))",
        ],
    );
}

#[test]
fn gate_0_lists_open_questions_without_blocking() {
    let p = Project::new();
    let vision = VISION.replace(
        "## Open Questions\n\nNone.",
        "## Open Questions\n\n- Pricing in Brazil?",
    );
    make_project(&p, LAYERED, Some(BACKLOG), Some(ROADMAP), &vision);
    let res = gate(&p, &["--gate", "0"]);
    assert!(res.ok(), "{}", res.stdout);
    has(
        &res.stdout,
        &["1 open question(s) remain", "Pricing in Brazil?"],
    );
}

#[test]
fn gate_0_blocks_without_style_backlog_and_roadmap() {
    let p = Project::new();
    make_project(&p, "{}", None, None, VISION);
    let res = gate(&p, &["--gate", "0"]);
    assert_eq!(res.code, 1);
    has(
        &res.stdout,
        &["architecture.style is not set", "No backlog", "No roadmap"],
    );
}

#[test]
fn gate_0_needs_epics_and_a_release_with_items() {
    let p = Project::new();
    let no_epic = format!(
        "{}## Closed\n",
        BACKLOG
            .split("## Epics")
            .next()
            .unwrap()
            .replace("- **Epic**: EPIC-001\n", "")
    );
    let empty_release = ROADMAP.replace("- **Items**: FEAT-001", "- **Items**: -");
    make_project(&p, LAYERED, Some(&no_epic), Some(&empty_release), VISION);
    let res = gate(&p, &["--gate", "0"]);
    assert_eq!(res.code, 1);
    has(&res.stdout, &["has no epic", "no release with items"]);
}

#[test]
fn gate_0_blocks_on_placeholders() {
    let p = Project::new();
    let vision = VISION.replace(
        "Photos are spread",
        "[Describe the problem] Photos are spread",
    );
    make_project(&p, LAYERED, Some(BACKLOG), Some(ROADMAP), &vision);
    let res = gate(&p, &["--gate", "0"]);
    assert_eq!(res.code, 1);
    has(
        &res.stdout,
        &["Product documents (product-inception) failed"],
    );
}
