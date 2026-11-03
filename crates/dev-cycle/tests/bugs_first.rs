//! Bugs first at gates 3 and 8 (formerly `dev-cycle-protocol/tests/test_bugs_first.py`).

mod common;

use std::path::PathBuf;

use common::{Project, README, gate, has, make_project};

const BACKLOG: &str = "# Backlog

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
";

const ROADMAP: &str = "# Roadmap

## v0.1.0: MVP
- **Status**: released
- **Released**: 2026-10-01
- **Goal**: First usable version.
- **Items**: -

## v0.2.0: Albums
- **Status**: in_progress
- **Goal**: Albums.
- **Items**: {next_items}

## v1.0.0: Launch
- **Status**: planned
- **Goal**: Public launch.
- **Items**: {later_items}
";

fn backlog(severity: &str) -> String {
    BACKLOG.replace("{severity}", severity)
}

fn write_plan(p: &Project, plan_id: &str, kind: &str, backlog_items: &[&str]) -> PathBuf {
    let items: String = backlog_items.iter().map(|i| format!("\n- {i}")).collect();
    let items = if items.is_empty() {
        " []".to_string()
    } else {
        items
    };
    p.write(
        &format!("docs/plans/{plan_id}.md"),
        &format!(
            "---\nplan_id: {plan_id}\ntitle: {plan_id}\nkind: {kind}\nstatus: active\n\
             created_at: '2026-10-16T10:00:00'\nbacklog_items:{items}\n---\n\n# {plan_id}\n"
        ),
    )
}

fn project(severity: &str) -> Project {
    let p = Project::new();
    p.write("docs/BACKLOG.md", &backlog(severity));
    p
}

fn write_roadmap(p: &Project, next_items: &str, later_items: &str) {
    p.write(
        "docs/ROADMAP.md",
        &ROADMAP
            .replace("{next_items}", next_items)
            .replace("{later_items}", later_items),
    );
}

fn read(path: &PathBuf) -> String {
    std::fs::read_to_string(path).expect("plan")
}

#[test]
fn feature_plan_is_blocked_by_an_open_high_bug() {
    let p = project("high");
    write_plan(&p, "export", "feature", &[]);
    let res = gate(&p, &["--gate", "3"]);
    assert_eq!(res.code, 1, "{}", res.stdout);
    has(
        &res.stdout,
        &["Bugs first", "BUG-001 (high)", "--accept-open-bugs"],
    );
}

#[test]
fn user_can_accept_the_risk_and_it_is_recorded() {
    let p = project("high");
    let path = write_plan(&p, "export", "feature", &[]);
    let res = gate(
        &p,
        &[
            "--gate",
            "3",
            "--accept-open-bugs",
            "Demo on Friday needs export",
        ],
    );
    assert!(res.ok(), "{}", res.stdout);
    has(&read(&path), &["Demo on Friday needs export"]);
}

#[test]
fn bug_plans_and_plans_that_include_the_bug_are_not_blocked() {
    let p = project("high");
    write_plan(&p, "crash", "bug", &[]);
    assert!(gate(&p, &["--gate", "3"]).ok());
    p.remove("docs/plans/crash.md");
    write_plan(&p, "export", "feature", &["BUG-001"]);
    assert!(gate(&p, &["--gate", "3"]).ok());
}

#[test]
fn medium_bugs_block_by_default() {
    let p = project("medium");
    write_plan(&p, "export", "feature", &[]);
    let res = gate(&p, &["--gate", "3"]);
    assert_eq!(res.code, 1);
    has(&res.stdout, &["BUG-001 (medium)"]);
}

#[test]
fn low_bugs_without_a_roadmap_only_inform() {
    let p = project("low");
    write_plan(&p, "export", "feature", &[]);
    let res = gate(&p, &["--gate", "3"]);
    assert!(res.ok());
    has(&res.stdout, &["Other open bugs", "No roadmap"]);
}

#[test]
fn low_bug_in_the_next_release_is_triaged() {
    let p = project("low");
    write_roadmap(&p, "BUG-001", "-");
    write_plan(&p, "export", "feature", &[]);
    let res = gate(&p, &["--gate", "3"]);
    assert!(res.ok(), "{}", res.stdout);
    has(&res.stdout, &["every other bug is triaged"]);
}

#[test]
fn low_bug_not_scheduled_or_scheduled_later_blocks() {
    let p = project("low");
    write_roadmap(&p, "-", "-");
    write_plan(&p, "export", "feature", &[]);
    let res = gate(&p, &["--gate", "3"]);
    assert_eq!(res.code, 1, "{}", res.stdout);
    has(
        &res.stdout,
        &[
            "not triaged",
            "goes into v0.2.0 at the latest",
            "roadmap.py assign <ID> --release v0.2.0",
        ],
    );
    write_roadmap(&p, "-", "BUG-001");
    let res = gate(&p, &["--gate", "3"]);
    assert_eq!(res.code, 1);
    has(
        &res.stdout,
        &["scheduled for v1.0.0; the deadline is v0.2.0"],
    );
}

#[test]
fn bug_without_severity_is_not_triaged() {
    let p = project("");
    write_plan(&p, "export", "feature", &[]);
    let res = gate(&p, &["--gate", "3"]);
    assert_eq!(res.code, 1);
    has(&res.stdout, &["BUG-001 (?) Crash on start: no severity"]);
}

#[test]
fn untriaged_bugs_can_be_accepted_or_turned_off() {
    let p = project("low");
    write_roadmap(&p, "-", "-");
    let path = write_plan(&p, "export", "feature", &[]);
    assert!(
        gate(
            &p,
            &[
                "--gate",
                "3",
                "--accept-open-bugs",
                "Cosmetic, after the demo"
            ]
        )
        .ok()
    );
    has(&read(&path), &["Cosmetic, after the demo"]);
    p.write(".dev-cycle.json", r#"{"backlog": {"bug_deadline": "off"}}"#);
    assert!(gate(&p, &["--gate", "3"]).ok());
}

#[test]
fn bug_plan_is_not_blocked_by_untriaged_bugs() {
    let p = project("low");
    write_roadmap(&p, "-", "-");
    write_plan(&p, "other-bug", "bug", &[]);
    let res = gate(&p, &["--gate", "3"]);
    assert!(res.ok());
    has(&res.stdout, &["BUG-001 (not triaged)"]);
}

#[test]
fn configured_blocking_severities() {
    let p = project("high");
    p.write(
        ".dev-cycle.json",
        r#"{"backlog": {"blocking_severities": ["critical"]}}"#,
    );
    write_plan(&p, "export", "feature", &[]);
    assert!(gate(&p, &["--gate", "3"]).ok());
}

#[test]
fn gate_8_validates_the_backlog_and_lists_blocking_bugs() {
    let p = Project::new();
    make_project(&p, README, "x", None);
    p.write("docs/BACKLOG.md", &backlog("critical"));
    let res = gate(&p, &["--gate", "8", "--run", "v1.0.0-rc.1"]);
    has(
        &res.stdout,
        &[
            "Backlog (plan-memory) passed",
            "known limitations",
            "BUG-001 Crash on start",
        ],
    );
    p.write("docs/BACKLOG.md", &backlog("urgent"));
    let res = gate(&p, &["--gate", "8", "--run", "v1.0.0-rc.1"]);
    assert_eq!(res.code, 1);
    has(&res.stdout, &["Backlog (plan-memory) failed"]);
}

#[test]
fn gate_8_fails_while_a_bug_is_not_triaged() {
    let p = Project::new();
    make_project(&p, README, "x", None);
    p.write("docs/BACKLOG.md", &backlog("low"));
    write_roadmap(&p, "-", "BUG-001");
    let res = gate(&p, &["--gate", "8", "--run", "v0.2.0-rc.1"]);
    assert_eq!(res.code, 1);
    has(
        &res.stdout,
        &["Open bugs not triaged", "deadline is v0.2.0"],
    );
}
