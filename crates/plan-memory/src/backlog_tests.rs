//! The backlog model: round trip, ids, adding, updating, closing, epics, superseded items and
//! bug triage (formerly the model tests of `plan-memory/tests/test_backlog.py` and
//! `test_roadmap.py`).

use super::*;
use crate::roadmap::Roadmap;

const SAMPLE: &str = "# Backlog

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
";

/// A backlog with epics (the roadmap tests' sample).
const EPICS: &str = "# Backlog

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

const ROADMAP: &str = "# Roadmap

## v0.1.0: MVP
- **Status**: in_progress
- **Goal**: First usable version.
- **Items**: BUG-002
";

fn ids(items: &[&Item]) -> Vec<String> {
    items.iter().map(|i| i.id.clone()).collect()
}

fn closed_ids(b: &Backlog) -> Vec<String> {
    let closed = SECTIONS.iter().position(|s| *s == "Closed").unwrap();
    b.sections[closed].iter().map(|i| i.id.clone()).collect()
}

#[track_caller]
fn err<T: std::fmt::Debug>(r: Result<T, BacklogError>, piece: &str) {
    match r {
        Err(BacklogError(m)) => assert!(m.contains(piece), "{piece:?} not in {m:?}"),
        Ok(v) => panic!("expected an error with {piece:?}, got {v:?}"),
    }
}

fn add(b: &mut Backlog, kind: &str, title: &str, desc: &str) -> Result<String, BacklogError> {
    b.add(kind, title, desc, "2026-10-16", None, None, None)
}

fn severities(list: &[&str]) -> Value {
    Value::list_of_str(list)
}

// --- model -------------------------------------------------------------------------------

#[test]
fn round_trip_keeps_the_file_identical() {
    assert_eq!(Backlog::parse(SAMPLE).render(), SAMPLE);
    assert_eq!(Backlog::parse(BACKLOG_TEMPLATE).render(), BACKLOG_TEMPLATE);
    let crlf = SAMPLE.replace('\n', "\r\n");
    assert_eq!(Backlog::parse(&crlf).render(), crlf);
}

#[test]
fn sample_is_valid_and_fields_join_continuation_lines() {
    let b = Backlog::parse(SAMPLE);
    assert!(b.validate().is_empty(), "{:?}", b.validate());
    let bug = b.find("bug-001").unwrap();
    assert_eq!(bug.kind(), Some("bug"));
    assert_eq!(bug.rank(), "high");
    assert!(bug.plan().is_none());
    assert_eq!(
        bug.get("Description"),
        "`/preset frontier` sets a model id that the provider may reject."
    );
}

#[test]
fn next_id_counts_closed_items() {
    let b = Backlog::parse(SAMPLE);
    assert_eq!(b.next_id("BUG"), "BUG-004");
    assert_eq!(b.next_id("FEAT"), "FEAT-002");
    assert_eq!(b.next_id("ADJ"), "ADJ-001");
}

#[test]
fn add_per_type_and_required_values() {
    let mut b = Backlog::parse(BACKLOG_TEMPLATE);
    let bug = b
        .add(
            "bug",
            "Crash on start",
            "It crashes.",
            "2026-10-16",
            Some("critical"),
            None,
            None,
        )
        .unwrap();
    let idea = add(&mut b, "improvement", "Faster sync", "Sync faster.").unwrap();
    let bug = b.find(&bug).unwrap();
    assert_eq!((bug.id.as_str(), bug.section.as_str()), ("BUG-001", "Bugs"));
    assert_eq!(bug.rank(), "critical");
    let idea = b.find(&idea).unwrap();
    assert_eq!(
        (idea.id.as_str(), idea.section.as_str()),
        ("IMP-001", "Improvements")
    );
    assert_eq!(idea.rank(), "medium");
    assert!(b.validate().is_empty());
    err(add(&mut b, "bug", "No severity", "x"), "needs a severity");
    err(add(&mut b, "feature", " ", "x"), "title");
    let rendered = Backlog::parse(&b.render());
    let all: Vec<&Item> = rendered.items().collect();
    assert_eq!(ids(&all), ["BUG-001", "IMP-001"]);
}

#[test]
fn validation_problems() {
    let bad = format!(
        "{}\n## Ideas\n",
        SAMPLE
            .replace(
                "### FEAT-001: Export answers",
                "### BUG-001: Export answers"
            )
            .replace("- **Severity**: low", "- **Severity**: urgent")
            .replace("- **Plan**: fix_search_threshold", "- **Plan**: -")
            .replace("- **Status**: done", "- **Status**: open")
    );
    let text = Backlog::parse(&bad).validate().join("\n");
    for piece in [
        "duplicate id",
        "a bug belongs under '## Bugs'",
        "severity must be one of",
        "a planned item needs its plan id",
        "status under '## Closed' must be one of",
        "unknown section '## Ideas'",
    ] {
        assert!(text.contains(piece), "{piece:?} not in:\n{text}");
    }
}

#[test]
fn missing_fields_and_bad_headings() {
    let text = Backlog::parse(
        "# B\n\n## Features\n\n### FEAT-001 no colon\n\n### FEAT-002: T\n- **Status**: open\n",
    )
    .validate()
    .join("\n");
    assert!(text.contains("item heading must be"), "{text}");
    assert!(text.contains("missing '- **Priority**:'"), "{text}");
    assert!(text.contains("missing '- **Description**:'"), "{text}");
}

#[test]
fn update_rules() {
    let mut b = Backlog::parse(SAMPLE);
    b.update(
        "BUG-001",
        Some("New title"),
        None,
        Some("medium"),
        None,
        None,
        None,
        None,
    )
    .unwrap();
    assert_eq!(b.find("BUG-001").unwrap().rank(), "medium");
    err(
        b.update("BUG-001", None, None, None, Some("high"), None, None, None),
        "is a bug",
    );
    err(
        b.update("FEAT-001", None, None, Some("high"), None, None, None, None),
        "not a bug",
    );
    err(
        b.update(
            "FEAT-001",
            None,
            None,
            None,
            None,
            Some("planned"),
            None,
            None,
        ),
        "has no plan",
    );
    b.update(
        "FEAT-001",
        None,
        None,
        None,
        None,
        Some("planned"),
        Some("export_plan"),
        None,
    )
    .unwrap();
    assert_eq!(
        b.find("FEAT-001").unwrap().plan().as_deref(),
        Some("export_plan")
    );
    err(
        b.update("FEAT-001", None, None, None, None, Some("done"), None, None),
        "use close, drop or duplicate",
    );
    err(
        b.update("FEAT-099", Some("x"), None, None, None, None, None, None),
        "no such backlog item",
    );
}

#[test]
fn close_drop_duplicate_and_reopen() {
    let mut b = Backlog::parse(SAMPLE);
    b.close("FEAT-001", "done", "2026-10-20, PR #340").unwrap();
    b.close("BUG-002", "duplicate", "2026-10-20, duplicate of BUG-001")
        .unwrap();
    assert_eq!(closed_ids(&b), ["BUG-003", "FEAT-001", "BUG-002"]);
    assert!(b.validate().is_empty());
    err(b.close("FEAT-001", "done", "x"), "already closed");
    err(
        b.update("FEAT-001", None, None, None, None, Some("open"), None, None),
        "is closed",
    );
    b.reopen("FEAT-001").unwrap();
    let item = b.find("FEAT-001").unwrap();
    assert_eq!(item.section, "Features");
    assert_eq!(item.status(), "open");
    assert!(!item.fields.iter().any(|(n, _)| n == "Closed"));
    assert!(Backlog::parse(&b.render()).validate().is_empty());
}

#[test]
fn open_items_and_blocking_bugs_are_ordered() {
    let mut b = Backlog::parse(SAMPLE);
    b.add(
        "bug",
        "Worse",
        "x",
        "2026-10-16",
        Some("critical"),
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        ids(&b.open_items()),
        ["BUG-004", "BUG-001", "BUG-002", "FEAT-001"]
    );
    let blocking = b.blocking_bugs(&severities(&["critical", "high"])).unwrap();
    assert_eq!(ids(&blocking), ["BUG-004", "BUG-001"]);
    assert!(b.blocking_bugs(&severities(&[])).unwrap().is_empty());
}

// --- bug triage --------------------------------------------------------------------------

#[test]
fn bug_triage_needs_a_severity_and_the_next_release() {
    let b = Backlog::parse(SAMPLE);
    let blocking = severities(&["critical", "high", "medium"]);
    let next = Value::str("next_release");
    let untriaged = |b: &Backlog, r: Option<&Roadmap>, blocking: &Value, deadline: &Value| {
        bug_triage(b, r, blocking, deadline)
            .unwrap()
            .into_iter()
            .map(|(i, why)| (i.id.clone(), why))
            .collect::<Vec<_>>()
    };
    let roadmap = Roadmap::parse(ROADMAP);
    let t = triage(&b, Some(&roadmap), &blocking, &next, &[]).unwrap();
    assert!(t.untriaged.is_empty());
    assert_eq!(t.next_release.as_deref(), Some("v0.1.0"));
    let unscheduled = Roadmap::parse(&ROADMAP.replace("BUG-002", "-"));
    assert_eq!(
        untriaged(&b, Some(&unscheduled), &blocking, &next),
        [(
            "BUG-002".to_string(),
            "not scheduled; it goes into v0.1.0 at the latest".to_string()
        )]
    );
    let t = triage(
        &b,
        Some(&unscheduled),
        &blocking,
        &next,
        &["bug-002".to_string()],
    )
    .unwrap();
    assert!(t.untriaged.is_empty());
    let released = Roadmap::parse(&ROADMAP.replace("in_progress", "released"));
    assert_eq!(
        untriaged(&b, Some(&released), &severities(&[]), &next)[0].1,
        "no release left in the roadmap to schedule it"
    );
    let t = triage(&b, None, &severities(&[]), &next, &[]).unwrap();
    assert!(t.untriaged.is_empty() && !t.deadline_checked);
    assert!(untriaged(&b, Some(&unscheduled), &severities(&[]), &Value::str("off")).is_empty());
    let no_severity = Backlog::parse(&SAMPLE.replace("- **Severity**: high\n", ""));
    assert_eq!(
        untriaged(&no_severity, None, &severities(&["high"]), &next),
        [("BUG-001".to_string(), "no severity".to_string())]
    );
}

// --- epics -------------------------------------------------------------------------------

#[test]
fn epic_items_and_progress() {
    let b = Backlog::parse(EPICS);
    assert_eq!(
        ids(&b.epic_items("EPIC-001", false)),
        ["FEAT-002", "FEAT-003"]
    );
    assert_eq!(
        b.find("FEAT-002").unwrap().epic().as_deref(),
        Some("EPIC-001")
    );
    assert!(b.validate().is_empty());
    assert_eq!(Backlog::parse(EPICS).render(), EPICS);
}

#[test]
fn add_and_set_epic() {
    let mut b = Backlog::parse(EPICS);
    let item = b
        .add(
            "feature",
            "Tags",
            "Tag photos",
            "2026-10-23",
            None,
            None,
            Some("epic-001"),
        )
        .unwrap();
    assert_eq!(b.find(&item).unwrap().epic().as_deref(), Some("EPIC-001"));
    let epic = b
        .add(
            "epic",
            "Social",
            "Social features",
            "2026-10-23",
            None,
            Some("low"),
            None,
        )
        .unwrap();
    assert_eq!(epic, "EPIC-002");
    assert!(
        !b.find(&epic)
            .unwrap()
            .fields
            .iter()
            .any(|(n, _)| n == "Plan")
    );
    err(
        b.add(
            "feature",
            "X",
            "Y",
            "2026-10-23",
            None,
            None,
            Some("FEAT-002"),
        ),
        "not an epic",
    );
    err(
        b.add("epic", "X", "Y", "2026-10-23", None, None, Some("EPIC-001")),
        "another epic",
    );
    b.update(
        "FEAT-004",
        None,
        None,
        None,
        None,
        None,
        None,
        Some("EPIC-002"),
    )
    .unwrap();
    assert_eq!(
        b.find("FEAT-004").unwrap().epic().as_deref(),
        Some("EPIC-002")
    );
    b.update("FEAT-004", None, None, None, None, None, None, Some("-"))
        .unwrap();
    assert!(b.find("FEAT-004").unwrap().epic().is_none());
    err(
        b.update(
            "EPIC-001",
            None,
            None,
            None,
            None,
            Some("planned"),
            None,
            None,
        ),
        "is an epic",
    );
    assert!(b.validate().is_empty());
}

#[test]
fn epic_validation_and_warnings() {
    let bad = EPICS.replace(
        "- **Epic**: EPIC-001\n- **Status**: open",
        "- **Epic**: EPIC-009\n- **Status**: open",
    );
    assert!(
        Backlog::parse(&bad)
            .validate()
            .iter()
            .any(|p| p.contains("epic EPIC-009 is not in the backlog"))
    );
    let planned_epic = EPICS.replace(
        "- **Priority**: high\n- **Status**: open\n- **Found**: 2026-10-01\n- **Description**: Everything",
        "- **Priority**: high\n- **Status**: planned\n- **Found**: 2026-10-01\n- **Description**: Everything",
    );
    assert!(
        Backlog::parse(&planned_epic)
            .validate()
            .iter()
            .any(|p| p.contains("never promoted"))
    );
    let mut b = Backlog::parse(EPICS);
    b.close("FEAT-002", "done", "2026-10-23").unwrap();
    b.close("FEAT-003", "dropped", "2026-10-23, out of scope")
        .unwrap();
    assert!(
        b.warnings()
            .iter()
            .any(|w| w.contains("every item is closed"))
    );
}

#[test]
fn old_backlog_without_epics_section_stays_identical() {
    let old = format!("{}## Closed\n", EPICS.split("## Epics").next().unwrap())
        .replace("- **Epic**: EPIC-001\n", "");
    assert_eq!(Backlog::parse(&old).render(), old);
}

// --- superseded items --------------------------------------------------------------------

#[test]
fn supersede_closes_with_the_replacements() {
    let mut b = Backlog::parse(EPICS);
    let a = b
        .add(
            "feature",
            "Share album by link",
            "Link",
            "2026-10-23",
            None,
            None,
            Some("EPIC-001"),
        )
        .unwrap();
    let c = b
        .add(
            "feature",
            "Invite relatives",
            "Invite",
            "2026-10-23",
            None,
            None,
            Some("EPIC-001"),
        )
        .unwrap();
    b.supersede(
        "FEAT-003",
        &[a.clone(), c.to_lowercase(), a.clone()],
        "2026-10-23",
    )
    .unwrap();
    let item = b.find("FEAT-003").unwrap();
    assert!(item.closed() && item.status() == "superseded");
    assert_eq!(
        item.get("Closed"),
        format!("2026-10-23, superseded by {a}, {c}")
    );
    assert_eq!(item.superseded_by(), [a.clone(), c.clone()]);
    assert!(b.validate().is_empty());
    let rendered = Backlog::parse(&b.render());
    assert_eq!(rendered.find("FEAT-003").unwrap().superseded_by(), [a, c]);
}

#[test]
fn supersede_rules() {
    let mut b = Backlog::parse(EPICS);
    let by = |ids: &[&str]| ids.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    err(
        b.supersede("FEAT-002", &by(&["FEAT-002"]), "2026-10-23"),
        "itself",
    );
    err(
        b.supersede("FEAT-002", &by(&["FEAT-001"]), "2026-10-23"),
        "closed",
    );
    err(
        b.supersede("FEAT-002", &by(&["EPIC-001"]), "2026-10-23"),
        "an epic is superseded by epics",
    );
    err(
        b.supersede("FEAT-002", &by(&["FEAT-099"]), "2026-10-23"),
        "no such",
    );
    err(b.supersede("FEAT-002", &[], "2026-10-23"), "--by");
}

#[test]
fn superseded_validation() {
    let closed = "
### FEAT-009: Old idea
- **Priority**: low
- **Status**: superseded
- **Found**: 2026-10-01
- **Plan**: -
- **Closed**: 2026-10-20, superseded by {by}
- **Description**: Old.
";
    let with = |text: &str| Backlog::parse(&format!("{EPICS}{text}")).validate();
    assert!(with(&closed.replace("{by}", "FEAT-004")).is_empty());
    let problems = with(&closed.replace("{by}", "FEAT-077")).join(" | ");
    assert!(
        problems.contains("superseded by unknown item FEAT-077"),
        "{problems}"
    );
    let problems = with(&closed.replace("superseded by {by}", "replaced")).join(" | ");
    assert!(problems.contains("needs 'superseded by <ID>"), "{problems}");
}
