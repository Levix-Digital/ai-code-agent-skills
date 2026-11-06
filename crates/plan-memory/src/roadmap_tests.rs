//! The roadmap model: round trip, releases, assignment, release status and validation
//! (formerly the model tests of `plan-memory/tests/test_roadmap.py`).

use super::*;
use crate::backlog::Backlog;

const SAMPLE: &str = "# Roadmap

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
";

const BACKLOG: &str = "# Backlog

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
";

fn versions(r: &Roadmap) -> Vec<&str> {
    r.releases.iter().map(|r| r.version.as_str()).collect()
}

fn release<'a>(r: &'a Roadmap, version: &str) -> &'a Release {
    &r.releases[r.find(version).unwrap()]
}

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn has_field(r: &Release, name: &str) -> bool {
    r.fields.iter().any(|(n, _)| n == name)
}

#[track_caller]
fn err<T: std::fmt::Debug>(r: Result<T, RoadmapError>, piece: &str) {
    match r {
        Err(RoadmapError::Error(m)) => assert!(m.contains(piece), "{piece:?} not in {m:?}"),
        other => panic!("expected an error with {piece:?}, got {other:?}"),
    }
}

#[test]
fn round_trip_keeps_the_file_identical() {
    assert_eq!(Roadmap::parse(SAMPLE).render(), SAMPLE);
    let crlf = SAMPLE.replace('\n', "\r\n");
    assert_eq!(Roadmap::parse(&crlf).render(), crlf);
}

#[test]
fn template_is_a_valid_empty_roadmap() {
    let roadmap = Roadmap::parse(TEMPLATE);
    assert!(roadmap.releases.is_empty());
    assert!(roadmap.validate(None).is_empty());
}

#[test]
fn parse_releases_and_items() {
    let r = Roadmap::parse(SAMPLE);
    assert_eq!(versions(&r), ["v0.1.0", "v0.2.0", "v1.0.0"]);
    assert_eq!(release(&r, "v0.2.0").items(), ["FEAT-002", "FEAT-003"]);
    assert_eq!(
        release(&r, "v0.2.0").get("Goal"),
        "Organize photos into albums."
    );
    assert!(release(&r, "v1.0.0").items().is_empty());
    assert_eq!(r.release_of("feat-003"), Some(1));
    assert_eq!(r.current().unwrap().version, "v0.2.0");
    assert!(r.validate(Some(&Backlog::parse(BACKLOG))).is_empty());
}

#[test]
fn add_and_move_releases() {
    let mut r = Roadmap::parse(SAMPLE);
    r.add_release(
        "v0.3.0",
        "Search",
        "Find photos",
        Some("2027-02"),
        None,
        Some("v0.2.0"),
    )
    .unwrap();
    assert_eq!(versions(&r), ["v0.1.0", "v0.2.0", "v0.3.0", "v1.0.0"]);
    assert!(
        r.add_release("v0.3.0", "Again", "x", None, None, None)
            .is_err()
    );
    assert!(r.add_release("0.4", "Bad", "x", None, None, None).is_err());
    assert!(r.add_release("v0.4.0", "", "x", None, None, None).is_err());
    r.move_release("v1.0.0", Some("v0.3.0"), None).unwrap();
    assert_eq!(versions(&r), ["v0.1.0", "v0.2.0", "v1.0.0", "v0.3.0"]);
    assert!(r.validate(None).iter().any(|p| p.contains("version order")));
    assert!(r.move_release("v0.3.0", Some("v9.9.9"), None).is_err());
    assert_eq!(versions(&r), ["v0.1.0", "v0.2.0", "v1.0.0", "v0.3.0"]);
}

#[test]
fn assign_moves_only_when_asked() {
    let mut r = Roadmap::parse(SAMPLE);
    assert_eq!(
        r.assign(&ids(&["FEAT-004"]), "v1.0.0", false).unwrap(),
        ["FEAT-004"]
    );
    err(
        r.assign(&ids(&["FEAT-004"]), "v0.2.0", false),
        "already in v1.0.0",
    );
    r.assign(&ids(&["FEAT-004"]), "v0.2.0", true).unwrap();
    assert!(release(&r, "v1.0.0").items().is_empty());
    assert_eq!(release(&r, "v1.0.0").get("Items"), "-");
    assert_eq!(
        release(&r, "v0.2.0").items(),
        ["FEAT-002", "FEAT-003", "FEAT-004"]
    );
    assert!(
        r.assign(&ids(&["FEAT-004"]), "v0.2.0", false)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn released_releases_are_frozen() {
    let mut r = Roadmap::parse(SAMPLE);
    err(r.assign(&ids(&["FEAT-004"]), "v0.1.0", false), "released");
    err(r.unassign(&ids(&["FEAT-001"])), "released");
    err(r.assign(&ids(&["FEAT-001"]), "v1.0.0", true), "released");
    assert!(r.assign(&ids(&["NOPE-1"]), "v1.0.0", false).is_err());
}

#[test]
fn release_status_and_date() {
    let mut r = Roadmap::parse(SAMPLE);
    err(
        r.update_release("v0.2.0", None, None, None, Some("released"), None),
        "needs its release date",
    );
    r.update_release(
        "v0.2.0",
        None,
        None,
        None,
        Some("released"),
        Some("2027-01-10".into()),
    )
    .unwrap();
    assert_eq!(release(&r, "v0.2.0").get("Released"), "2027-01-10");
    r.update_release("v0.2.0", None, None, None, Some("in_progress"), None)
        .unwrap();
    assert!(!has_field(release(&r, "v0.2.0"), "Released"));
    r.update_release("v0.2.0", None, None, Some(""), None, None)
        .unwrap();
    assert!(!has_field(release(&r, "v0.2.0"), "Target"));
}

#[test]
fn validation_problems() {
    let bad = SAMPLE
        .replace(
            "- **Items**: -",
            "- **Items**: FEAT-002, EPIC-001, FEAT-099",
        )
        .replace("- **Released**: 2026-10-01\n", "");
    let problems = Roadmap::parse(&bad)
        .validate(Some(&Backlog::parse(BACKLOG)))
        .join(" | ");
    for piece in [
        "FEAT-002 is also in v0.2.0",
        "EPIC-001 is an epic",
        "FEAT-099 is not in the backlog",
        "needs '- **Released**: <date>'",
    ] {
        assert!(problems.contains(piece), "{piece:?} not in {problems}");
    }
    let order = SAMPLE
        .replace(
            "- **Status**: planned",
            "- **Status**: released\n- **Released**: 2027-03-01",
        )
        .replace("- **Status**: in_progress", "- **Status**: planned");
    assert!(
        Roadmap::parse(&order)
            .validate(None)
            .iter()
            .any(|p| p.contains("comes after one that is not released"))
    );
    let broken = Roadmap::parse(&format!("{SAMPLE}\n## Next things\n"));
    assert!(
        broken
            .validate(None)
            .iter()
            .any(|p| p.contains("release heading"))
    );
}
