//! The curated manual test suite: parsing, validation and the commands (formerly
//! `quality-assurance/tests/test_manual_tests.py`).

mod common;

use common::{Out, Project, has};
use quality_assurance::manual_tests::Suite;

const SUITE: &str = "# Manual Tests

Intro text.

## Installation

### MT-INST-001: Fresh install puts actx on PATH

- **Covers**: installer
- **Prerequisites**: clean machine
- **Steps**:
  1. Run the installer:
     ```bash
     ### not a heading inside code
     ./install.sh
     ```
- **Expected**:
  - `actx --version` prints the version
- **Result** (v1.0.0-rc.1): [x] Pass [ ] Fail [ ] Blocked
- **Notes**:

## Web Sources

### MT-WEB-001: Re-sync does not duplicate chunks

- **Covers**: ingestion
- **Steps**:
  1. Sync twice
- **Expected**:
  - Same chunk count
- **Result** (v1.0.0-rc.1): [ ] Pass [x] Fail [ ] Blocked
- **Notes**: count grew from 120 to 240
  after the second sync

### MT-WEB-002: Unreachable site is reported

- **Covers**: crawler
- **Steps**:
  1. Add an unreachable URL
- **Expected**:
  - A warning in the footer
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:
";

const FILE: &str = "tests/MANUAL_TESTS.md";

fn cli(p: &Project, args: &[&str]) -> Out {
    let (command, rest) = args.split_first().expect("command");
    let root = p.root();
    let mut all = vec![*command];
    all.extend_from_slice(rest);
    all.extend(["--dir", root.as_str()]);
    p.run("quality-assurance", "manual_tests", &all)
}

fn ids(s: &Suite) -> Vec<&str> {
    s.tests.iter().map(|t| t.id.as_str()).collect()
}

#[test]
fn parse_features_ids_results_and_notes() {
    let s = Suite::new("x.md", SUITE);
    assert_eq!(ids(&s), ["MT-INST-001", "MT-WEB-001", "MT-WEB-002"]);
    let (inst, web1, web2) = (&s.tests[0], &s.tests[1], &s.tests[2]);
    assert_eq!(inst.feature.as_deref(), Some("Installation"));
    assert_eq!(inst.run.as_deref(), Some("v1.0.0-rc.1"));
    assert_eq!(inst.status, Some("Pass"));
    assert_eq!(web1.status, Some("Fail"));
    assert_eq!(
        s.notes_text(web1),
        "count grew from 120 to 240 after the second sync"
    );
    assert!(web2.run.is_none() && web2.status.is_none());
    assert!(s.validate().is_empty());
}

#[test]
fn validation_problems() {
    let bad = format!(
        "{}\n### TEST-9: bad id\n\n- **Result**: Pass\n",
        SUITE
            .replace("MT-WEB-002", "MT-WEB-001")
            .replace(
                "- **Result**: [ ] Pass [ ] Fail [ ] Blocked",
                "- **Result**: [x] Pass [x] Fail [ ] Blocked"
            )
            .replace("- **Covers**: crawler\n", "")
    );
    let problems = Suite::new("x.md", &bad).validate().join("\n");
    has(
        &problems,
        &[
            "duplicate id",
            "more than one result box is marked",
            "missing field 'Covers'",
            "id must look like MT-<AREA>-NNN",
            "Result line must be",
        ],
    );
}

#[test]
fn test_outside_feature_is_an_error() {
    let problems = Suite::new("x.md", "# T\n\n### MT-A-001: x\n- **Covers**: a\n").validate();
    assert!(
        problems
            .iter()
            .any(|p| p.contains("not under a '## <Feature>' section")),
        "{problems:?}"
    );
}

#[test]
fn summary_failed_and_gate() {
    let p = Project::new();
    p.write(FILE, SUITE);
    let res = cli(&p, &["summary", "--run", "v1.0.0-rc.1"]);
    has(&res.stdout, &["2 test(s)", "Pass 1", "Fail 1"]);
    let res = cli(&p, &["failed", "--run", "v1.0.0-rc.1"]);
    assert_eq!(res.code, 1);
    has(&res.stdout, &["MT-WEB-001 [Fail]", "count grew"]);
    let res = cli(&p, &["gate", "--run", "v1.0.0-rc.1"]);
    assert_eq!(res.code, 1);
    has(&res.stdout, &["1/2 Pass"]);
    assert_eq!(cli(&p, &["gate", "--run", "v9.9.9-rc.1"]).code, 1);
}

#[test]
fn start_run_labels_clears_and_keeps_other_tests() {
    let p = Project::new();
    p.write(FILE, SUITE);
    let res = cli(
        &p,
        &[
            "start-run",
            "--run",
            "v1.0.0-rc.2",
            "--feature",
            "Web Sources",
        ],
    );
    assert!(res.ok(), "{}", res.stdout);
    let text = p.read(FILE);
    let s = Suite::new(FILE, &text);
    let (inst, web1, web2) = (&s.tests[0], &s.tests[1], &s.tests[2]);
    // Untouched.
    assert_eq!(inst.run.as_deref(), Some("v1.0.0-rc.1"));
    assert_eq!(inst.status, Some("Pass"));
    assert_eq!(web1.run.as_deref(), Some("v1.0.0-rc.2"));
    assert!(web1.status.is_none());
    assert_eq!(web2.run.as_deref(), Some("v1.0.0-rc.2"));
    assert!(web2.status.is_none());
    // Old notes cleared.
    assert_eq!(s.notes_text(web1), "");
    assert!(!text.contains("after the second sync"));
    assert!(text.contains("### not a heading inside code"));
    assert!(s.validate().is_empty());
}

#[test]
fn gate_passes_when_all_marked_pass() {
    let p = Project::new();
    p.write(FILE, SUITE);
    cli(
        &p,
        &["start-run", "--run", "v1.0.0-rc.2", "--id", "MT-WEB-001"],
    );
    let text = p.read(FILE).replace(
        "- **Result** (v1.0.0-rc.2): [ ] Pass [ ] Fail [ ] Blocked",
        "- **Result** (v1.0.0-rc.2): [x] Pass [ ] Fail [ ] Blocked",
    );
    p.write(FILE, &text);
    let res = cli(&p, &["gate", "--run", "v1.0.0-rc.2"]);
    assert!(res.ok());
    has(&res.stdout, &["all 1 test(s) Pass"]);
}

#[test]
fn start_run_rejects_unknown_selection_and_preserves_crlf() {
    let p = Project::new();
    let path = p.write(FILE, &SUITE.replace('\n', "\r\n"));
    let before = std::fs::read(&path).expect("suite");
    let res = cli(&p, &["start-run", "--run", "r1", "--feature", "Nope"]);
    assert_eq!(res.code, 1);
    has(&res.stdout, &["Not found: Nope"]);
    assert_eq!(std::fs::read(&path).expect("suite"), before);
    assert!(cli(&p, &["start-run", "--run", "r1", "--all"]).ok());
    let after = String::from_utf8(std::fs::read(&path).expect("suite")).expect("utf-8");
    assert!(after.contains("\r\n"));
    assert!(!after.replace("\r\n", "").contains("\n\n"));
}

#[test]
fn validate_cli_and_missing_file() {
    let p = Project::new();
    // No suite yet.
    assert_eq!(cli(&p, &["validate"]).code, 1);
    p.write(FILE, SUITE);
    let res = cli(&p, &["validate"]);
    assert!(res.ok());
    has(&res.stdout, &["3 test(s), 0 problem(s)"]);
}
