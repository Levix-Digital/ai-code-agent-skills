//! The plan search index: a full rebuild, then the incremental sync of the commands that search
//! it (formerly `plan-memory/tests/test_plan_index.py`, which checked the same index between
//! the Python scripts and the engine).

mod common;

use std::path::Path;

use common::{Out, Project, has, skills_root};

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("folder");
    for entry in std::fs::read_dir(from).expect("fixture") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("copy");
        }
    }
}

fn project() -> Project {
    let p = Project::new();
    copy_dir(
        &skills_root().join("tests/parity/fixtures/plans-list"),
        &p.dir,
    );
    p
}

fn cmd(p: &Project, command: &str, args: &[&str]) -> Out {
    let root = p.root();
    let mut all = args.to_vec();
    all.extend(["--dir", root.as_str()]);
    let res = p.run("plan-memory", command, &all);
    assert!(res.ok(), "{}", res.all());
    res
}

fn search(p: &Project, query: &str) -> Out {
    cmd(p, "search_plans", &["--query", query, "--limit", "1"])
}

#[test]
fn rebuild_then_sync_only_what_changed() {
    let p = project();
    has(
        &cmd(&p, "rebuild_index", &[]).all(),
        &["7 updated, 0 removed, 7 plan(s)", "Index rebuilt"],
    );
    // Nothing changed: the search finds the index up to date.
    let res = search(&p, "Alpha feature");
    assert!(!res.all().contains("synced"), "{}", res.all());
    has(&res.stdout, &["Plan ID    : alpha"]);
    let gamma = p.path("docs/plans/gamma.md");
    let text = std::fs::read_to_string(&gamma).expect("gamma");
    std::fs::write(&gamma, format!("{text}\nOne more line.\n")).expect("gamma");
    p.remove("docs/plans/delta.md");
    let res = search(&p, "Alpha feature");
    has(&res.all(), &["1 updated, 1 removed, 6 plan(s)"]);
    assert!(!search(&p, "Alpha feature").all().contains("synced"));
    has(
        &cmd(&p, "rebuild_index", &[]).all(),
        &["6 updated, 0 removed, 6 plan(s)"],
    );
}
