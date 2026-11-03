//! The roadmap command: creating the file, expanding epics, the current release, releasing
//! and the configured path (formerly the command tests of `plan-memory/tests/test_roadmap.py`).

mod common;

use common::{BACKLOG_EPICS, Out, Project, has, json, keys_of};

fn project() -> Project {
    let p = Project::new();
    p.git(&["init", "-q"]);
    p.write("docs/BACKLOG.md", BACKLOG_EPICS);
    p
}

fn roadmap(p: &Project, args: &[&str]) -> Out {
    let root = p.root();
    let mut all = args.to_vec();
    all.extend(["--dir", root.as_str()]);
    p.run("plan-memory", "roadmap", &all)
}

fn release(p: &Project, version: &str, name: &str, goal: &str) {
    let res = roadmap(p, &["add-release", version, "--name", name, "--goal", goal]);
    assert!(res.ok(), "{}", res.all());
}

#[test]
fn creates_the_roadmap_and_expands_epics() {
    let p = project();
    release(&p, "v0.1.0", "MVP", "First version");
    release(&p, "v0.2.0", "Next", "More");
    assert!(
        roadmap(
            &p,
            &["assign", "EPIC-001", "BUG-002", "--release", "v0.1.0"]
        )
        .ok()
    );
    has(
        &p.read("docs/ROADMAP.md"),
        &["- **Items**: FEAT-002, FEAT-003, BUG-002"],
    );
    let res = roadmap(&p, &["assign", "EPIC-001", "--release", "v0.2.0"]);
    assert!(res.ok());
    has(&res.stdout, &["has no open item to assign"]);
    let res = roadmap(&p, &["assign", "FEAT-001", "--release", "v0.2.0"]);
    assert_eq!(res.code, 1);
    has(&res.stdout, &["closed"]);
    assert!(roadmap(&p, &["validate"]).ok());
}

#[test]
fn current_lists_bugs_first_and_unscheduled_items() {
    let p = project();
    release(&p, "v0.1.0", "MVP", "First version");
    assert!(
        roadmap(
            &p,
            &["assign", "FEAT-002", "BUG-002", "--release", "v0.1.0"]
        )
        .ok()
    );
    let res = roadmap(&p, &["current", "--json"]);
    assert!(res.ok(), "{}", res.all());
    let data = json(&res.stdout);
    let data = data.as_dict().unwrap();
    let version = data
        .get_or_none("release")
        .as_dict()
        .unwrap()
        .get_or_none("version");
    assert_eq!(version.as_str(), Some("v0.1.0"));
    assert_eq!(
        keys_of(data.get_or_none("open_items"), "id"),
        ["BUG-002", "FEAT-002"]
    );
    // By priority.
    assert!(
        data.get_or_none("unscheduled")
            .py_eq(&json(r#"["FEAT-004", "FEAT-003"]"#))
    );
}

#[test]
fn release_warns_about_open_items() {
    let p = project();
    release(&p, "v0.1.0", "MVP", "First version");
    assert!(roadmap(&p, &["assign", "FEAT-002", "--release", "v0.1.0"]).ok());
    let res = roadmap(&p, &["set-release", "v0.1.0", "--status", "released"]);
    assert!(res.ok(), "{}", res.all());
    has(&res.stdout, &["Still open in this release: FEAT-002"]);
    let res = roadmap(&p, &["validate"]);
    assert!(res.ok());
    has(&res.stdout, &["is released but FEAT-002 is still open"]);
}

#[test]
fn without_a_roadmap() {
    let p = project();
    let show = roadmap(&p, &["show"]);
    let validate = roadmap(&p, &["validate"]);
    assert!(show.ok() && validate.ok());
    has(&validate.stdout, &["No roadmap"]);
    assert_eq!(
        roadmap(&p, &["assign", "FEAT-002", "--release", "v0.1.0"]).code,
        1
    );
}

#[test]
fn configured_roadmap_path() {
    let p = project();
    p.write(
        ".dev-cycle.json",
        r#"{"roadmap": {"file": "planning/ROADMAP.md"}}"#,
    );
    release(&p, "v1.0.0", "One", "Ship");
    assert!(p.exists("planning/ROADMAP.md"));
}
