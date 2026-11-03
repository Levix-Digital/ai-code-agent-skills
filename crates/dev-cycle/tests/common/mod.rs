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

    /// Runs an engine command in the folder.
    pub fn run(&self, skill: &str, command: &str, args: &[&str]) -> Out {
        run_in(&self.dir, skill, command, args)
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
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
    let out = Command::new(env!("CARGO_BIN_EXE_dev-cycle"))
        .arg(skill)
        .arg(command)
        .args(args)
        .current_dir(cwd)
        .env("DEV_CYCLE_SKILLS_ROOT", skills_root())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("dev-cycle");
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
