//! The backlog command: adding, summary, list, closing, bug triage, the configured file,
//! failed manual tests and the epic and superseded options (formerly the command tests of
//! `plan-memory/tests/test_backlog.py` and `test_roadmap.py`).

mod common;

use common::{BACKLOG, BACKLOG_EPICS, Out, Project, has, json, keys_of};

const ROADMAP: &str = "# Roadmap

## v0.1.0: MVP
- **Status**: in_progress
- **Goal**: First usable version.
- **Items**: -
";

const MANUAL_SUITE: &str = "# Manual Tests

## Sync

### MT-SYNC-001: Sync finishes

- **Covers**: sync
- **Steps**:
  1. Run sync
- **Expected**:
  - It finishes
- **Result** (v1.0.0-rc.1): [ ] Pass [x] Fail [ ] Blocked
- **Notes**: Hangs at 90%.

### MT-SYNC-002: Cancel works

- **Covers**: sync
- **Steps**:
  1. Cancel
- **Expected**:
  - It stops
- **Result** (v1.0.0-rc.1): [x] Pass [ ] Fail [ ] Blocked
- **Notes**:
";

const FILE: &str = "docs/BACKLOG.md";

fn backlog(p: &Project, args: &[&str]) -> Out {
    let root = p.root();
    let mut all = args.to_vec();
    all.extend(["--dir", root.as_str()]);
    p.run("plan-memory", "backlog", &all)
}

fn roadmap(p: &Project, args: &[&str]) -> Out {
    let root = p.root();
    let mut all = args.to_vec();
    all.extend(["--dir", root.as_str()]);
    p.run("plan-memory", "roadmap", &all)
}

fn get<'a>(v: &'a kernel::py::Value, key: &str) -> &'a kernel::py::Value {
    v.as_dict().expect("an object").get_or_none(key)
}

#[test]
fn add_creates_the_file_and_validates() {
    let p = Project::new();
    assert!(backlog(&p, &["validate"]).ok());
    let res = backlog(
        &p,
        &[
            "add",
            "--type",
            "feature",
            "--title",
            "Export",
            "--description",
            "Export answers.",
            "--found-in",
            "v1.0.0",
        ],
    );
    assert!(res.ok(), "{}", res.all());
    has(&res.stdout, &["FEAT-001: Export (medium)"]);
    let res = backlog(
        &p,
        &[
            "add",
            "--type",
            "bug",
            "--title",
            "Crash",
            "--description",
            "Crashes.",
            "--severity",
            "high",
        ],
    );
    assert!(res.ok(), "{}", res.all());
    has(
        &p.read(FILE),
        &["### FEAT-001: Export", "v1.0.0", "### BUG-001: Crash"],
    );
    assert!(backlog(&p, &["validate"]).ok());
    let res = backlog(
        &p,
        &["add", "--type", "bug", "--title", "x", "--description", "y"],
    );
    assert_eq!(res.code, 1);
    has(&res.stdout, &["needs a severity"]);
}

#[test]
fn summary_list_and_closing() {
    let p = Project::new();
    p.write(FILE, BACKLOG);
    let res = backlog(&p, &["summary", "--json"]);
    assert!(res.ok(), "{}", res.all());
    let data = json(&res.stdout);
    assert!(get(&data, "blocking_bugs").py_eq(&json(r#"["BUG-001"]"#)));
    assert_eq!(get(&data, "closed").as_i64(), Some(1));
    assert!(
        get(get(&data, "types"), "bug")
            .py_eq(&json(r#"{"open": 1, "planned": 1, "in_progress": 0}"#))
    );
    let res = backlog(&p, &["summary"]);
    assert!(res.ok());
    has(&res.stdout, &["come before new work"]);
    assert!(backlog(&p, &["close", "BUG-001", "--ref", "PR #9"]).ok());
    assert!(backlog(&p, &["drop", "FEAT-001", "--reason", "Out of scope"]).ok());
    assert_eq!(
        backlog(&p, &["duplicate", "BUG-002", "--of", "BUG-002"]).code,
        1
    );
    let res = backlog(&p, &["list", "--all", "--json"]);
    assert!(res.ok());
    let items = json(&res.stdout);
    let item = |id: &str| {
        items
            .as_list()
            .unwrap()
            .iter()
            .find(|i| get(i, "id").as_str() == Some(id))
            .cloned()
            .expect("item")
    };
    assert_eq!(get(&item("BUG-001"), "status").as_str(), Some("done"));
    assert!(
        get(&item("BUG-001"), "closed")
            .as_str()
            .unwrap()
            .contains("PR #9")
    );
    assert_eq!(get(&item("FEAT-001"), "status").as_str(), Some("dropped"));
    assert!(backlog(&p, &["validate"]).ok());
}

#[test]
fn add_says_where_a_bug_goes_and_summary_lists_the_untriaged() {
    let p = Project::new();
    p.write("docs/ROADMAP.md", ROADMAP);
    let res = backlog(
        &p,
        &[
            "add",
            "--type",
            "bug",
            "--title",
            "Crash",
            "--description",
            "Crashes.",
            "--severity",
            "medium",
        ],
    );
    assert!(res.ok(), "{}", res.all());
    has(&res.stdout, &["comes before new features"]);
    let res = backlog(
        &p,
        &[
            "add",
            "--type",
            "bug",
            "--title",
            "Typo",
            "--description",
            "Typo.",
            "--severity",
            "low",
        ],
    );
    assert!(res.ok(), "{}", res.all());
    has(&res.stdout, &["roadmap.py assign BUG-002 --release v0.1.0"]);
    let res = backlog(&p, &["summary", "--json"]);
    assert!(res.ok());
    let data = json(&res.stdout);
    assert!(get(&data, "blocking_bugs").py_eq(&json(r#"["BUG-001"]"#)));
    assert_eq!(keys_of(get(&data, "untriaged_bugs"), "id"), ["BUG-002"]);
    let res = backlog(&p, &["summary"]);
    assert!(res.ok());
    has(&res.stdout, &["not triaged yet"]);
}

#[test]
fn custom_file_from_config() {
    let p = Project::new();
    p.write(".dev-cycle.json", r#"{"backlog": {"file": "BACKLOG.md"}}"#);
    let res = backlog(
        &p,
        &[
            "add",
            "--type",
            "adjustment",
            "--title",
            "Rename",
            "--description",
            "Rename it.",
        ],
    );
    assert!(res.ok(), "{}", res.all());
    assert!(p.exists("BACKLOG.md"));
}

#[test]
fn refuses_to_rewrite_an_unreadable_file() {
    let p = Project::new();
    let original = format!("{BACKLOG}\n## Ideas\n\nSomething important.\n");
    p.write(FILE, &original);
    let res = backlog(&p, &["close", "BUG-001"]);
    assert_eq!(res.code, 1);
    has(&res.stdout, &["structural problems"]);
    assert_eq!(p.read(FILE), original);
}

#[test]
fn summary_without_a_backlog() {
    let p = Project::new();
    let res = backlog(&p, &["summary"]);
    assert!(res.ok());
    has(&res.stdout, &["No backlog"]);
}

#[test]
fn from_failed_captures_failed_manual_tests_once() {
    let p = Project::new();
    p.write("tests/MANUAL_TESTS.md", MANUAL_SUITE);
    assert_eq!(backlog(&p, &["add", "--from-failed"]).code, 1);
    assert!(backlog(&p, &["add", "--from-failed", "--run", "v1.0.0-rc.1"]).ok());
    let res = backlog(&p, &["add", "--from-failed", "--run", "v1.0.0-rc.1"]);
    assert!(res.ok());
    has(&res.stdout, &["already in the backlog"]);
    let res = backlog(&p, &["list", "--type", "bug", "--json"]);
    let bugs = json(&res.stdout);
    let bugs = bugs.as_list().unwrap();
    assert_eq!(bugs.len(), 1);
    let bug = &bugs[0];
    assert_eq!(get(bug, "severity").as_str(), Some("high"));
    assert_eq!(
        get(bug, "title").as_str(),
        Some("MT-SYNC-001 fails: Sync finishes")
    );
    let text = p.read(FILE);
    has(&text, &["Hangs at 90%.", "- **Found**: ", "v1.0.0-rc.1"]);
    let res = backlog(&p, &["add", "--from-failed", "--run", "v9.9.9-rc.1"]);
    assert!(res.ok());
    has(&res.stdout, &["No manual test is marked Fail"]);
}

#[test]
fn add_needs_type_title_and_description() {
    let p = Project::new();
    let res = backlog(&p, &["add", "--type", "feature", "--title", "x"]);
    assert_eq!(res.code, 1);
    has(
        &res.stdout,
        &["add needs --type, --title and --description"],
    );
}

// --- epics and superseded items ----------------------------------------------------------

fn epics_project() -> Project {
    let p = Project::new();
    p.git(&["init", "-q"]);
    p.write(FILE, BACKLOG_EPICS);
    p
}

#[test]
fn promote_refuses_an_epic() {
    let p = epics_project();
    let res = backlog(&p, &["promote", "EPIC-001"]);
    assert_eq!(res.code, 1);
    has(
        &res.stdout,
        &["promote its items instead (FEAT-002, FEAT-003)"],
    );
}

#[test]
fn epic_options() {
    let p = epics_project();
    let res = backlog(
        &p,
        &[
            "add",
            "--type",
            "feature",
            "--title",
            "Tags",
            "--description",
            "Tag photos",
            "--epic",
            "EPIC-001",
        ],
    );
    assert!(res.ok(), "{}", res.all());
    let res = backlog(&p, &["list", "--epic", "EPIC-001", "--json"]);
    assert!(res.ok());
    // By priority.
    assert_eq!(
        keys_of(&json(&res.stdout), "id"),
        ["FEAT-002", "FEAT-005", "FEAT-003"]
    );
    let res = backlog(&p, &["summary"]);
    assert!(res.ok());
    has(&res.stdout, &["(0/3 items closed)"]);
}

#[test]
fn supersede_and_roadmap_hint() {
    let p = epics_project();
    roadmap(
        &p,
        &[
            "add-release",
            "v0.1.0",
            "--name",
            "MVP",
            "--goal",
            "First version",
        ],
    );
    roadmap(&p, &["assign", "FEAT-002", "--release", "v0.1.0"]);
    let res = backlog(&p, &["supersede", "FEAT-002", "--by", "FEAT-004"]);
    assert!(res.ok(), "{}", res.all());
    has(&res.stdout, &["closed as superseded by FEAT-004"]);
    let res = roadmap(&p, &["validate"]);
    assert!(res.ok(), "{}", res.all());
    has(
        &res.stdout,
        &["FEAT-002 in v0.1.0 was superseded; unassign it; assign FEAT-004"],
    );
    let res = backlog(&p, &["supersede", "FEAT-003", "--by", "FEAT-004"]);
    assert!(res.ok(), "{}", res.all());
    has(&res.stdout, &["was in plan 'share_albums'"]);
}
