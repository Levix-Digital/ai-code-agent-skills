//! Shared helpers of the engine's command tests: a temporary project folder, and the engine
//! run on it the way the skills run it, with the skills read from this checkout.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The checkout's root, which holds the skills.
pub fn skills_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A folder removed when the test ends.
pub struct Project {
    pub dir: PathBuf,
}

static COUNT: AtomicUsize = AtomicUsize::new(0);

impl Project {
    pub fn new() -> Project {
        let n = COUNT.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "dev-cycle-test-{}-{n}-{}",
            std::process::id(),
            std::thread::current()
                .name()
                .unwrap_or("t")
                .replace(':', "_")
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temporary folder");
        // The canonical path, as the commands print it (macOS links /var to /private/var).
        // (Not on Windows, where it would add the \\?\ prefix.)
        let dir = if cfg!(windows) {
            dir
        } else {
            dir.canonicalize().expect("canonical path")
        };
        Project { dir }
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.dir.join(rel)
    }

    pub fn root(&self) -> String {
        self.dir.to_string_lossy().into_owned()
    }

    /// Writes a file, creating its folders.
    pub fn write(&self, rel: &str, text: &str) -> PathBuf {
        let path = self.path(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("folders");
        }
        std::fs::write(&path, text).expect("write");
        path
    }

    pub fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.path(rel)).expect("read")
    }

    pub fn exists(&self, rel: &str) -> bool {
        self.path(rel).exists()
    }

    pub fn mkdir(&self, rel: &str) {
        std::fs::create_dir_all(self.path(rel)).expect("folders");
    }

    pub fn remove(&self, rel: &str) {
        std::fs::remove_file(self.path(rel)).expect("remove");
    }

    /// Runs git in the folder with a fixed test identity; panics when it fails.
    pub fn git(&self, args: &[&str]) -> String {
        self.git_as("t", "t@t", args)
    }

    pub fn git_as(&self, name: &str, email: &str, args: &[&str]) -> String {
        let out = Command::new("git")
            .args([
                "-c",
                &format!("user.name={name}"),
                "-c",
                &format!("user.email={email}"),
            ])
            .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"])
            .args(args)
            .current_dir(&self.dir)
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// The plan search index of the project's commands, beside the folder (so no command
    /// writes the user's own index).
    pub fn db(&self) -> PathBuf {
        let mut name = self.dir.clone().into_os_string();
        name.push(".db");
        PathBuf::from(name)
    }

    /// Runs an engine command in the folder.
    pub fn run(&self, skill: &str, command: &str, args: &[&str]) -> Out {
        run_with(&self.dir, skill, command, args, Some(&self.db()))
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
        let _ = std::fs::remove_dir_all(self.db());
    }
}

/// What a command printed and its exit code.
#[derive(Debug)]
pub struct Out {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Out {
    /// stdout and stderr together, trimmed, as `run_checker` gave them.
    pub fn all(&self) -> String {
        format!("{}{}", self.stdout, self.stderr).trim().to_string()
    }

    pub fn ok(&self) -> bool {
        self.code == 0
    }
}

/// Runs `dev-cycle <skill> <command> <args>` from `cwd`.
pub fn run_in(cwd: &Path, skill: &str, command: &str, args: &[&str]) -> Out {
    run_with(cwd, skill, command, args, None)
}

/// [`run_in`], with the plan search index in `db` when given.
pub fn run_with(cwd: &Path, skill: &str, command: &str, args: &[&str], db: Option<&Path>) -> Out {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_dev-cycle"));
    cmd.arg(skill)
        .arg(command)
        .args(args)
        .current_dir(cwd)
        .env("DEV_CYCLE_SKILLS_ROOT", skills_root())
        .env("GIT_CONFIG_NOSYSTEM", "1");
    if let Some(db) = db {
        cmd.env("PLAN_MEMORY_DB", db);
    }
    let out = cmd.output().expect("dev-cycle");
    Out {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// Asserts that every piece is in the text, showing the text when one is missing.
#[track_caller]
pub fn has(text: &str, pieces: &[&str]) {
    for p in pieces {
        assert!(text.contains(p), "{p:?} not found in:\n{text}");
    }
}

// The small project of the gate rc and 8 tests.

pub const README: &str = "# Demo

Demo answers questions about your notes.

## Installation

Run the installer.

## Usage

Run `demo ask`.

## Features

- Answers

## Requirements

- Any OS

## Configuration

None.

## Troubleshooting

None.

## Documentation

See the docs.

## License

MIT.
";

const SUITE: &str = "# Manual Tests

## Basics

### MT-BASIC-001: Demo starts

- **Covers**: cli
- **Steps**:
  1. Run `demo`
- **Expected**:
  - A prompt appears
- **Result** (v1.0.0-rc.1): [{mark}] Pass [ ] Fail [ ] Blocked
- **Notes**:
";

/// README, TECDOC, a one-test suite whose run v1.0.0-rc.1 has `mark` on Pass, a version file
/// and `.dev-cycle.json` with `devops` (default: only the version file).
pub fn make_project(p: &Project, readme: &str, mark: &str, devops: Option<&str>) {
    p.write("README.md", readme);
    p.write("TECDOC.md", "# Tech\n");
    p.write("tests/MANUAL_TESTS.md", &SUITE.replace("{mark}", mark));
    p.write("version.txt", "1.0.0\n");
    let devops = devops.unwrap_or(r#"{"version_files": ["version.txt"]}"#);
    p.write(
        ".dev-cycle.json",
        &format!(r#"{{"architecture": {{"style": "layered"}}, "devops": {devops}}}"#),
    );
}

/// `verify_gate --dir <project> <args>`.
pub fn gate(p: &Project, args: &[&str]) -> Out {
    let root = p.root();
    let mut all = vec!["--dir", root.as_str()];
    all.extend_from_slice(args);
    p.run("dev-cycle-protocol", "verify_gate", &all)
}

// The backlogs of the backlog and roadmap command tests.

/// A backlog with bugs, a feature and a closed bug.
pub const BACKLOG: &str = "# Backlog

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

/// A backlog with an epic of two features.
pub const BACKLOG_EPICS: &str = "# Backlog

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

/// A JSON value printed by a command, read back.
pub fn json(text: &str) -> kernel::py::Value {
    kernel::py::json::loads(text).unwrap_or_else(|_| panic!("JSON: {text}"))
}

/// The `key` of every object in a JSON list, as strings.
pub fn keys_of(list: &kernel::py::Value, key: &str) -> Vec<String> {
    list.as_list()
        .expect("a JSON list")
        .iter()
        .map(|v| {
            v.as_dict()
                .expect("an object")
                .get_or_none(key)
                .as_str()
                .expect("a string")
                .to_string()
        })
        .collect()
}
