//! Spec Kit at gates 3 and 8, the roadmap and AI attribution at gate 8, and the plan envelope
//! (formerly `dev-cycle-protocol/tests/test_speckit_gates.py`).

mod common;

use common::{Project, README, gate, has, make_project};
use plan_memory::index::index_sha;
use plan_memory::store::PlanStore;

const GIT_CONFIG: &str = r#"# Git Branching Workflow Extension Configuration
branch_numbering: sequential
branch_template: ""
branch_prefix: ""
init_commit_message: "[Spec Kit] Initial commit"
"#;

const PLAN: &str = "# Implementation Plan: Photo upload

## Summary

Upload photos.

## Dev Cycle Protocol

### Tests (quality-assurance)

Unit tests.
";

const ROADMAP: &str = "# Roadmap

## v1.0.0: Launch
- **Status**: in_progress
- **Goal**: Ship.
- **Items**: FEAT-001, FEAT-002
";

const BACKLOG: &str = "# Backlog

## Bugs

## Features

### FEAT-001: Upload
- **Priority**: high
- **Status**: open
- **Found**: 2026-10-23
- **Plan**: -
- **Description**: Upload.

## Improvements

## Adjustments

## Closed

### FEAT-002: Login
- **Priority**: high
- **Status**: done
- **Found**: 2026-10-01
- **Plan**: login
- **Closed**: 2026-10-10, PR #3
- **Description**: Login.
";

const CONFIG: &str = r#"{"project_name": "demo", "architecture": {"style": "layered"}, "speckit": {"enabled": true, "version": "v1.2.3", "integration": "claude", "script": "sh"}}"#;

struct Spec<'a> {
    branch: &'a str,
    files: &'a [&'a str],
    spec_text: &'a str,
    plan_text: &'a str,
    spec_dir: Option<&'a str>,
}

const DEFAULT: Spec = Spec {
    branch: "feat/001-photo-upload",
    files: &["spec.md", "plan.md", "tasks.md"],
    spec_text: "# Spec\n",
    plan_text: PLAN,
    spec_dir: Some("specs/001-photo-upload"),
};

fn speckit_project(s: &Spec) -> Project {
    let p = Project::new();
    p.git(&["init", "-q", "-b", "dev"]);
    p.git(&["commit", "-q", "--allow-empty", "-m", "init"]);
    p.git(&["checkout", "-q", "-b", s.branch]);
    p.write(".dev-cycle.json", CONFIG);
    p.write(
        ".specify/extensions/git/git-config.yml",
        &GIT_CONFIG.replace(r#"branch_prefix: """#, r#"branch_prefix: "feat""#),
    );
    p.write(
        ".specify/init-options.json",
        r#"{"speckit_version": "1.2.3"}"#,
    );
    p.write(".specify/presets/dev-cycle/preset.yml", "preset: {}\n");
    p.mkdir(".specify/memory");
    let root = p.root();
    let out = p.run(
        "dev-cycle-protocol",
        "constitution",
        &["generate", "--dir", &root],
    );
    assert!(out.ok(), "{}", out.all());
    p.mkdir("specs/001-photo-upload");
    for name in s.files {
        let text = match *name {
            "spec.md" => s.spec_text,
            "plan.md" => s.plan_text,
            _ => "# Tasks\n- [ ] T001 Upload\n",
        };
        p.write(&format!("specs/001-photo-upload/{name}"), text);
    }
    let spec_line = s
        .spec_dir
        .map_or(String::new(), |d| format!("spec_dir: {d}\n"));
    p.write(
        "docs/plans/photo_upload.md",
        &format!(
            "---\nplan_id: photo_upload\ntitle: Photo upload\nkind: feature\nstatus: active\n{spec_line}\
             created_at: '2026-10-23T10:00:00'\n---\n\n# Photo upload\n\nSee the spec folder.\n"
        ),
    );
    p
}

#[test]
fn gate_3_passes_with_complete_spec_kit_artifacts() {
    let p = speckit_project(&DEFAULT);
    let res = gate(&p, &["--gate", "3"]);
    assert!(res.ok(), "{}", res.stdout);
    has(
        &res.stdout,
        &[
            "Spec Kit artifacts complete in specs/001-photo-upload",
            "Constitution (constitution.py check) passed",
            "Spec Kit setup (speckit_setup.py status) passed",
        ],
    );
}

#[test]
fn gate_3_blocks_missing_tasks_and_open_clarifications() {
    let p = speckit_project(&Spec {
        files: &["spec.md", "plan.md"],
        spec_text: "# Spec\n- FR-001 [NEEDS CLARIFICATION: which formats?]\n",
        ..DEFAULT
    });
    let res = gate(&p, &["--gate", "3"]);
    assert_eq!(res.code, 1);
    has(
        &res.stdout,
        &["missing in specs/001-photo-upload: tasks.md"],
    );
    p.write("specs/001-photo-upload/tasks.md", "# Tasks\n");
    let res = gate(&p, &["--gate", "3"]);
    assert_eq!(res.code, 1);
    has(
        &res.stdout,
        &["spec.md has 1 open [NEEDS CLARIFICATION] marker(s)"],
    );
}

#[test]
fn gate_3_blocks_a_stale_constitution() {
    let p = speckit_project(&DEFAULT);
    p.write(
        ".dev-cycle.json",
        &CONFIG.replace(r#""layered""#, r#""hexagonal""#),
    );
    let res = gate(&p, &["--gate", "3"]);
    assert_eq!(res.code, 1);
    has(
        &res.stdout,
        &["Constitution (constitution.py check) failed", "stale"],
    );
}

#[test]
fn gate_3_warns_about_branch_preset_and_missing_spec_dir() {
    let p = speckit_project(&Spec {
        branch: "photo-upload",
        plan_text: "# Plan\n",
        spec_dir: None,
        ..DEFAULT
    });
    let res = gate(&p, &["--gate", "3"]);
    assert!(res.ok(), "{}", res.stdout);
    has(&res.stdout, &["has no spec_dir"]);
    let plan = p.read("docs/plans/photo_upload.md").replace(
        "status: active\n",
        "status: active\nspec_dir: specs/001-photo-upload\n",
    );
    p.write("docs/plans/photo_upload.md", &plan);
    let res = gate(&p, &["--gate", "3"]);
    assert!(res.ok(), "{}", res.stdout);
    has(
        &res.stdout,
        &[
            "is not a Spec Kit feature branch (feat/NNN-slug)",
            "has no 'Dev Cycle Protocol' sections",
        ],
    );
}

#[test]
fn gate_3_ignores_spec_kit_when_disabled() {
    let p = speckit_project(&Spec {
        files: &[],
        ..DEFAULT
    });
    p.write(
        ".dev-cycle.json",
        &CONFIG.replace(r#""enabled": true"#, r#""enabled": false"#),
    );
    let res = gate(&p, &["--gate", "3"]);
    assert!(res.ok());
    assert!(!res.stdout.contains("Spec Kit"), "{}", res.stdout);
}

#[test]
fn gate_8_lists_open_items_of_the_release() {
    let p = Project::new();
    make_project(&p, README, "x", None);
    p.write("docs/BACKLOG.md", BACKLOG);
    p.write("docs/ROADMAP.md", ROADMAP);
    let res = gate(&p, &["--gate", "8", "--run", "v1.0.0-rc.1"]);
    assert!(res.ok(), "{}", res.stdout);
    has(
        &res.stdout,
        &[
            "Roadmap (plan-memory) passed",
            "Items of v1.0.0 still open",
            "FEAT-001 [open] Upload",
        ],
    );
    let res = gate(&p, &["--gate", "8", "--run", "v2.0.0-rc.1"]);
    has(&res.stdout, &["v2.0.0 is not in the roadmap"]);
}

#[test]
fn plan_envelope_reads_its_spec_kit_artifacts() {
    let p = speckit_project(&DEFAULT);
    let root = p.root();
    let store = PlanStore::new(Some(&root), None).unwrap_or_else(|_| panic!("store"));
    let plan = store
        .get("photo_upload")
        .unwrap_or_else(|_| panic!("plan"))
        .expect("plan file");
    assert_eq!(plan.spec_dir().as_deref(), Some("specs/001-photo-upload"));
    let files = store
        .spec_files(&plan)
        .unwrap_or_else(|_| panic!("spec files"));
    let rels: Vec<&str> = files.iter().map(|(rel, _)| rel.as_str()).collect();
    assert_eq!(
        rels,
        [
            "specs/001-photo-upload/spec.md",
            "specs/001-photo-upload/plan.md",
            "specs/001-photo-upload/tasks.md"
        ]
    );
    let full = store.full_text(&plan).unwrap_or_else(|_| panic!("text"));
    assert!(full.starts_with(&plan.body) && full.contains("T001 Upload"));
    let before = index_sha(&store, &plan).unwrap_or_else(|_| panic!("sha"));
    p.write(
        "specs/001-photo-upload/tasks.md",
        "# Tasks\n- [x] T001 Upload\n",
    );
    assert_ne!(
        index_sha(&store, &plan).unwrap_or_else(|_| panic!("sha")),
        before
    );
}

#[test]
fn gate_8_blocks_ai_attribution_in_commit_messages() {
    let p = Project::new();
    make_project(&p, README, "x", None);
    p.git(&["init", "-q", "-b", "main"]);
    p.git(&["add", "-A"]);
    p.git(&[
        "commit",
        "-q",
        "-m",
        "feat: first\n\nCo-Authored-By: Claude <noreply@anthropic.com>",
    ]);
    let res = gate(&p, &["--gate", "8"]);
    assert_eq!(res.code, 1);
    has(
        &res.stdout,
        &["AI attribution in 1 place(s)", "co-author trailer"],
    );
    p.git(&["tag", "v1.0.0"]);
    p.git(&["commit", "-q", "--allow-empty", "-m", "fix: second"]);
    let res = gate(&p, &["--gate", "8"]);
    has(
        &res.stdout,
        &["No AI attribution in the commits since v1.0.0"],
    );
}

#[test]
fn gate_8_blocks_an_ai_git_identity() {
    let p = Project::new();
    make_project(&p, README, "x", None);
    p.git(&["init", "-q", "-b", "main"]);
    p.git(&["add", "-A"]);
    p.git_as(
        "Claude",
        "noreply@anthropic.com",
        &["commit", "-q", "-m", "feat: first"],
    );
    let res = gate(&p, &["--gate", "8"]);
    assert_eq!(res.code, 1);
    has(
        &res.stdout,
        &["AI attribution in 1 place(s)", "AI identity"],
    );
}
