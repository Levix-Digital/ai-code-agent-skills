//! `.dev-cycle.json`: defaults, sections, legacy keys, validation and migration (formerly
//! `dev-cycle-protocol/tests/test_project_config.py`).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use kernel::project_config::{
    ARCHITECTURE_STYLES, ProjectConfig, find_manifest, load_project_config,
};
use kernel::py::json::loads;
use kernel::py::{Dict, Value};

struct Dir(PathBuf);

static COUNT: AtomicUsize = AtomicUsize::new(0);

impl Dir {
    fn new() -> Dir {
        let n = COUNT.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("dev-cycle-config-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("folder");
        Dir(dir)
    }

    fn path(&self) -> &str {
        self.0.to_str().expect("utf-8 path")
    }

    fn manifest(&self, json: &str) -> PathBuf {
        let path = self.0.join(".dev-cycle.json");
        std::fs::write(&path, json).expect("manifest");
        path
    }

    fn config(&self) -> ProjectConfig {
        load_config(self.path())
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn load_config(dir: &str) -> ProjectConfig {
    load_project_config(Some(dir)).unwrap_or_else(|_| panic!("config of {dir}"))
}

fn json(text: &str) -> Value {
    loads(text).unwrap_or_else(|_| panic!("JSON: {text}"))
}

#[track_caller]
fn assert_value(actual: &Value, expected: &str) {
    assert!(
        actual.py_eq(&json(expected)),
        "{} != {expected}",
        actual.py_repr()
    );
}

#[track_caller]
fn assert_section(cfg: &ProjectConfig, name: &str, expected: &str) {
    assert_value(&Value::Dict(cfg.section(name).clone()), expected);
}

fn key<'a>(d: &'a Dict, k: &str) -> &'a Value {
    d.get_or_none(k)
}

fn problems(cfg: &ProjectConfig) -> String {
    cfg.validate().join("\n")
}

#[test]
fn defaults_without_manifest() {
    let d = Dir::new();
    let cfg = d.config();
    assert!(cfg.manifest_path.is_none());
    let name = Path::new(d.path()).file_name().unwrap().to_str().unwrap();
    assert_eq!(cfg.project_name(), name);
    let devops = cfg.section("devops");
    assert_value(key(devops, "integration_branch"), r#""dev""#);
    assert_value(key(devops, "production_branch"), r#""main""#);
    assert!(matches!(
        key(devops, "release_candidates"),
        Value::Bool(true)
    ));
    assert_value(key(cfg.section("docs"), "user_doc"), r#""README.md""#);
    assert_value(
        key(cfg.section("tests"), "manual_tests"),
        r#""tests/MANUAL_TESTS.md""#,
    );
    assert_value(key(cfg.section("plans"), "dir"), r#""docs/plans""#);
    assert_section(
        &cfg,
        "backlog",
        r#"{"file": "docs/BACKLOG.md", "blocking_severities": ["critical", "high", "medium"], "bug_deadline": "next_release"}"#,
    );
    assert!(key(cfg.section("architecture"), "style").is_none());
    assert!(cfg.validate().is_empty());
}

#[test]
fn sections_override_defaults() {
    let d = Dir::new();
    d.manifest(
        r#"{"project_name": "demo", "architecture": {"style": "layered"},
            "devops": {"integration_branch": "develop", "release_candidates": false},
            "test_commands": ["pytest"]}"#,
    );
    let cfg = d.config();
    assert_eq!(cfg.project_name(), "demo");
    assert_value(key(cfg.section("architecture"), "style"), r#""layered""#);
    assert_value(key(cfg.section("architecture"), "surfaces"), "[]");
    let devops = cfg.section("devops");
    assert_value(key(devops, "integration_branch"), r#""develop""#);
    assert_value(key(devops, "production_branch"), r#""main""#);
    assert!(matches!(
        key(devops, "release_candidates"),
        Value::Bool(false)
    ));
    assert_value(cfg.get("test_commands"), r#"["pytest"]"#);
    assert!(cfg.validate().is_empty());
}

#[test]
fn legacy_flat_keys_are_mapped() {
    let d = Dir::new();
    d.manifest(
        r#"{"core_path": "src/core", "surfaces": ["cli"], "releases_repo": "org/releases",
            "plans_dir": "plans",
            "doc_files": {"user_doc": "USER.md", "tec_doc": "ARCH.md", "manual_tests": "MT.md"}}"#,
    );
    let cfg = d.config();
    assert_value(
        key(cfg.section("architecture"), "core_path"),
        r#""src/core""#,
    );
    assert_value(key(cfg.section("architecture"), "surfaces"), r#"["cli"]"#);
    assert_value(
        key(cfg.section("devops"), "releases_repo"),
        r#""org/releases""#,
    );
    assert_value(key(cfg.section("plans"), "dir"), r#""plans""#);
    assert_section(
        &cfg,
        "docs",
        r#"{"user_doc": "USER.md", "tec_doc": "ARCH.md", "readme_max_lines": 400}"#,
    );
    assert_value(key(cfg.section("tests"), "manual_tests"), r#""MT.md""#);
    assert!(cfg.legacy_keys_used.iter().any(|k| k == "core_path"));
    assert!(
        cfg.legacy_keys_used
            .iter()
            .any(|k| k == "doc_files.user_doc")
    );
}

#[test]
fn new_location_wins_over_legacy_key() {
    let d = Dir::new();
    d.manifest(r#"{"releases_repo": "org/old", "devops": {"releases_repo": "org/new"}}"#);
    assert_value(
        key(d.config().section("devops"), "releases_repo"),
        r#""org/new""#,
    );
}

#[test]
fn validation_reports_problems() {
    let d = Dir::new();
    d.manifest(
        r#"{"architecture": {"style": "spaghetti"},
            "devops": {"observability": "telepathy", "release_candidates": "yes"},
            "test_commands": "cargo test", "backlog": {"blocking_severities": ["urgent"]},
            "mystery": 1}"#,
    );
    let text = problems(&d.config());
    for piece in [
        "architecture.style 'spaghetti'",
        "devops.observability 'telepathy'",
        "release_candidates",
        "test_commands must be a list",
        "unknown top-level key 'mystery'",
        "backlog.blocking_severities",
    ] {
        assert!(text.contains(piece), "{piece:?} not in:\n{text}");
    }
}

#[test]
fn manifest_found_from_subdirectory() {
    let d = Dir::new();
    let manifest = d.manifest(r#"{"project_name": "root-project"}"#);
    let nested = d.0.join("a").join("b");
    std::fs::create_dir_all(&nested).expect("folders");
    let nested = nested.to_str().unwrap();
    assert_eq!(find_manifest(Some(nested)), Some(manifest));
    assert_eq!(load_config(nested).project_name(), "root-project");
}

#[test]
fn migrate_moves_legacy_keys_and_keeps_the_rest() {
    let d = Dir::new();
    d.manifest(
        r#"{"project_name": "demo", "core_path": "src", "doc_files": {"manual_tests": "tests/MT.md"},
            "releases_repo": "org/rel", "drift_rules": [{"tech": "x", "conflict_msg": "no x"}],
            "architecture_docs": {"max_lines": 500}}"#,
    );
    let migrated = d.config().migrated().unwrap_or_else(|_| panic!("migrated"));
    assert_value(
        &Value::Dict(migrated),
        r#"{"project_name": "demo", "architecture": {"core_path": "src"},
            "tests": {"manual_tests": "tests/MT.md"}, "devops": {"releases_repo": "org/rel"},
            "architecture_docs": {"max_lines": 500},
            "drift_rules": [{"tech": "x", "conflict_msg": "no x"}]}"#,
    );
    // The file itself is untouched.
    assert!(!d.config().legacy_keys_used.is_empty());
}

#[test]
fn known_architecture_styles() {
    assert_eq!(
        ARCHITECTURE_STYLES,
        [
            "hexagonal",
            "layered",
            "mvc",
            "pipeline",
            "modular-monolith",
            "other"
        ]
    );
}

#[test]
fn product_roadmap_and_speckit_defaults() {
    let d = Dir::new();
    let cfg = d.config();
    assert_section(&cfg, "roadmap", r#"{"file": "docs/ROADMAP.md"}"#);
    assert_value(
        key(cfg.section("product"), "vision"),
        r#""docs/product/VISION.md""#,
    );
    assert_value(
        key(cfg.section("product"), "tech_spec"),
        r#""docs/product/TECH_SPEC.md""#,
    );
    assert_section(
        &cfg,
        "speckit",
        r#"{"enabled": false, "version": null, "integration": "claude", "script": null, "branch_prefix": "feat"}"#,
    );
    assert!(cfg.validate().is_empty());
}

#[test]
fn speckit_validation() {
    let d = Dir::new();
    d.manifest(r#"{"speckit": {"enabled": true, "version": "latest", "script": "bat"}}"#);
    let text = problems(&d.config());
    assert!(
        text.contains("speckit.version 'latest' must be an exact release tag"),
        "{text}"
    );
    assert!(text.contains("speckit.script 'bat'"), "{text}");
    d.manifest(r#"{"speckit": {"enabled": true}}"#);
    assert!(problems(&d.config()).contains("must be set when speckit.enabled"));
    d.manifest(r#"{"speckit": {"enabled": true, "version": "v1.2.3", "script": "ps"}}"#);
    assert!(d.config().validate().is_empty());
}

#[test]
fn branching_defaults_and_validation() {
    let d = Dir::new();
    let cfg = d.config();
    assert_value(key(cfg.section("devops"), "branching"), r#""integration""#);
    assert_value(
        key(cfg.section("devops"), "release_branch_prefix"),
        r#""release""#,
    );
    d.manifest(r#"{"devops": {"branching": "trunk"}}"#);
    assert!(d.config().validate().is_empty());
    d.manifest(r#"{"devops": {"branching": "gitflow", "release_branch_prefix": "rel ease"}}"#);
    let text = problems(&d.config());
    assert!(text.contains("devops.branching 'gitflow'") && text.contains("release_branch_prefix"));
}

#[test]
fn gedo_is_off_by_default_and_validated() {
    let d = Dir::new();
    assert_section(
        &d.config(),
        "gedo",
        r#"{"enabled": false, "feedback": true, "feedback_dir": "docs/gedo-feedback", "repo": "Levix-Digital/gedo"}"#,
    );
    d.manifest(r#"{"gedo": {"enabled": true}}"#);
    assert!(d.config().validate().is_empty());
    d.manifest(r#"{"gedo": {"enabled": "yes", "feedback_dir": "", "repo": "gedo"}}"#);
    let text = problems(&d.config());
    assert!(
        text.contains("gedo.enabled must be true or false"),
        "{text}"
    );
    assert!(
        text.contains("gedo.feedback_dir") && text.contains("gedo.repo 'gedo'"),
        "{text}"
    );
}
