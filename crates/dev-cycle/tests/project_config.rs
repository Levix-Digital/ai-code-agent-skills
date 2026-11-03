//! `project_config --migrate --write` and `--validate` (formerly the command test of
//! `dev-cycle-protocol/tests/test_project_config.py`; the configuration itself is tested in
//! `crates/kernel/tests/project_config.rs`).

mod common;

use common::Project;

#[test]
fn cli_migrate_write_and_validate() {
    let p = Project::new();
    p.write(
        ".dev-cycle.json",
        r#"{"core_path": "src", "architecture": {"style": "hexagonal"}}"#,
    );
    let root = p.root();
    let res = p.run(
        "dev-cycle-protocol",
        "project_config",
        &["--dir", &root, "--migrate", "--write"],
    );
    assert!(res.ok(), "{}", res.all());
    let written: String = p.read(".dev-cycle.json").split_whitespace().collect();
    assert_eq!(
        written,
        r#"{"architecture":{"style":"hexagonal","core_path":"src"}}"#
    );
    let res = p.run(
        "dev-cycle-protocol",
        "project_config",
        &["--dir", &root, "--validate"],
    );
    assert!(res.ok(), "{}", res.all());
}
