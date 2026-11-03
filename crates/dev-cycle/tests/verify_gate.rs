//! `verify_gate`: gates rc and 8 on a small project (formerly
//! `dev-cycle-protocol/tests/test_verify_gate.py`).

mod common;

use common::{Project, README, gate, has, make_project};

#[test]
fn gate_rc_requires_run_and_all_pass() {
    let p = Project::new();
    make_project(&p, README, "x", None);
    assert_eq!(gate(&p, &["--gate", "rc"]).code, 1);
    let ok = gate(&p, &["--gate", "rc", "--run", "v1.0.0-rc.1"]);
    assert!(ok.ok());
    has(&ok.stdout, &["CANDIDATE ACCEPTED"]);
    assert_eq!(gate(&p, &["--gate", "rc", "--run", "v1.0.0-rc.9"]).code, 1);
}

#[test]
fn gate_rc_fails_when_a_test_is_not_pass() {
    let p = Project::new();
    make_project(&p, README, " ", None);
    let res = gate(&p, &["--gate", "rc", "--run", "v1.0.0-rc.1"]);
    assert_eq!(res.code, 1);
    has(&res.stdout, &["NOT ACCEPTED"]);
}

#[test]
fn gate_8_passes_on_a_compliant_project() {
    let p = Project::new();
    make_project(&p, README, "x", None);
    let res = gate(&p, &["--gate", "8", "--run", "v1.0.0-rc.1"]);
    assert!(res.ok(), "{}", res.stdout);
    has(
        &res.stdout,
        &[
            "README check (user-docs) passed",
            "Manual test suite (quality-assurance) passed",
            "Version files (devops-release) passed",
            "Release candidate run v1.0.0-rc.1",
        ],
    );
}

#[test]
fn gate_8_blocks_on_a_changelog_readme() {
    let p = Project::new();
    make_project(
        &p,
        &format!("{README}\n## What's New\n\n- Faster\n"),
        "x",
        None,
    );
    let res = gate(&p, &["--gate", "8"]);
    assert_eq!(res.code, 1);
    has(&res.stdout, &["README check (user-docs) failed"]);
}

#[test]
fn gate_8_blocks_on_failed_candidate_and_bad_versions() {
    let p = Project::new();
    make_project(
        &p,
        README,
        " ",
        Some(r#"{"version_files": ["version.txt", "missing.toml"]}"#),
    );
    let res = gate(&p, &["--gate", "8", "--run", "v1.0.0-rc.1"]);
    assert_eq!(res.code, 1);
    has(
        &res.stdout,
        &[
            "Version files (devops-release) failed",
            "Release candidate run v1.0.0-rc.1 (quality-assurance) failed",
        ],
    );
}

#[test]
fn gate_8_without_run_only_warns_about_candidate() {
    let p = Project::new();
    make_project(&p, README, "x", None);
    let res = gate(&p, &["--gate", "8"]);
    assert!(res.ok());
    has(&res.stdout, &["no --run was given"]);
}

#[test]
fn gate_8_names_the_release_gate_of_the_branching_model() {
    let p = Project::new();
    make_project(&p, README, "x", None);
    has(
        &gate(&p, &["--gate", "8", "--run", "v1.0.0-rc.1"]).stdout,
        &["READY FOR THE RELEASE PULL REQUEST"],
    );
    make_project(
        &p,
        README,
        "x",
        Some(r#"{"branching": "trunk", "version_files": ["version.txt"]}"#),
    );
    let res = gate(&p, &["--gate", "8", "--run", "v1.0.0-rc.1"]);
    assert!(res.ok(), "{}", res.stdout);
    has(
        &res.stdout,
        &[
            "READY FOR THE PROMOTION (the user runs the promote workflow for v1.0.0-rc.1)",
            "Tag v1.0.0-rc.1 not found locally",
        ],
    );
}
