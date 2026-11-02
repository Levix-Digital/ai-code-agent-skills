---
plan_id: ai_code_agent_skills_release_status_reads_no_version_from_json_version_files
project: ai-code-agent-skills
title: release_status reads no version from JSON version files
kind: bug
version: 2.10.0
status: active
author: Levi Silveira
created_at: '2026-11-02T01:23:19'
updated_at: '2026-11-02T01:24:42'
session_id: null
tags: []
supersedes: null
superseded_by: null
parent_plan_id: null
backlog_items:
- BUG-006
milestones:
- name: Regression tests
  done: false
- name: Fix
  done: false
- name: Checks and pull request
  done: false
status_log:
- at: '2026-11-02T01:23:19'
  by: Levi Silveira
  status: draft
  note: Plan created
- at: '2026-11-02T01:24:41'
  by: Levi Silveira
  status: planned
  note: Plan updated
- at: '2026-11-02T01:24:42'
  by: Levi Silveira
  status: active
  note: Plan ready for approval
---

# release_status reads no version from JSON version files

Bug plan: discovery and planning of the backlog items below (`dev-cycle-protocol` Phases 1 and 2). Replace every instruction paragraph with the findings.

## Backlog items

### BUG-006: release_status reads no version from JSON files other than package.json
- **Severity**: medium
- **Found**: 2026-11-01, Rust migration acceptance test (gate 8 on this repository)
- **Description**: read_version_file parses only package.json as JSON. Any other JSON version file (.claude-plugin/plugin.json, composer.json, manifest.json) falls to the generic pattern version\s*[=:], which never matches "version": because of the closing quote. release_status version and next-rc fail with 'no version found', and gate 8 is blocked. This repository's own v2.10.0 gate 8 fails on .claude-plugin/plugin.json. Same behavior in the Python script and the engine.

## 1. Triage

- **Expected vs. actual**: `release_status version` should print the version held in every file of `devops.version_files` and fail only when they disagree (`devops-release/SKILL.md`, the `release_status version` row: "Reads the version from `devops.version_files` (`Cargo.toml`, `package.json`, `pyproject.toml`, `version.txt`, ...)"; `docs/configuration.md`: "Files that hold the version number"). A JSON manifest with a top-level `"version"` holds the version, but every JSON file except `package.json` gives `no version found`.
- **Verdict**: bug. The documentation promises any version file, the generic pattern was meant to cover the other formats, and `.claude-plugin/plugin.json` is this repository's own version file.
- **Suspected code paths**: `read_version_file` in `devops-release/release_status.py` and `crates/devops-release/src/release_status.rs`: only `package.json` is read as JSON; every other name falls to the generic pattern.
- **Severity**: medium, confirmed. Gate 8 is blocked for every project whose version lives in a JSON file other than `package.json` (plugin manifests, `composer.json`, `manifest.json`, `app.json`), including this repository's v2.10.0. The only workaround is to drop the file from `devops.version_files`, which turns off the version check. No data is touched.

## 2. Reproduction

On main (`f48feb2`), Linux, in this repository (`devops.version_files: [".claude-plugin/plugin.json"]`, which holds `"version": "2.8.1"`):

```
$ bin/dev-cycle devops-release release_status version
❌ .claude-plugin/plugin.json: no version found        (exit code 1)
$ python3 devops-release/release_status.py version
❌ .claude-plugin/plugin.json: no version found        (exit code 1)
```

`verify_gate --gate 8` reports `❌ Version files (devops-release) failed: .claude-plugin/plugin.json: no version found` and blocks. Found by the migration acceptance test, where the Python script and the engine gave the same result.

## 3. Evidence and root cause

- Confirmed: `read_version_file` dispatches on the file name. `cargo.toml` and `pyproject.toml` read TOML tables, `package.json` reads JSON, `version.txt` and `version` read the whole file, and anything else uses `version\s*[=:]\s*["']([^"']+)["']`.
- Confirmed: in JSON the key is quoted, so the text is `"version": "2.8.1"`. After `version` comes `"`, not a space, `=` or `:`, so the pattern never matches. It works for `version = "1.0"` (Gradle, Python sources) and `version: '1.0'`, which is what it was written for.
- Confirmed: the pattern exists since `69f2fe2` (2026-10-16); the tests cover only `package.json` among JSON files.

## 4. Fix design (`software-design`)

Read every file whose name ends in `.json` the way `package.json` is read: its top-level `"version"` string, or no version. The `package.json` branch becomes the `.json` branch; nothing else changes. Same change in `release_status.py` and in the engine (the migration rule: a bug is fixed in both languages), so the parity cases keep comparing them.

Alternative considered: make the generic pattern accept a closing quote after `version`. It is a smaller edit, but it would take the first `"version"` anywhere in the file, nested objects included, while JSON can be read exactly.

Deliberately left out: unquoted YAML or INI values (`version: 1.2.3` in `pubspec.yaml`, `version = 1.2.3` in `setup.cfg`). They are a separate request, not this defect.

## 5. Affected surfaces and files

- `devops-release/release_status.py`, `crates/devops-release/src/release_status.rs` (`read_version_file`). Commands affected: `release_status version`, `release_status next-rc`, and the version check of `verify_gate --gate 8`. No option, message or exit code changes; a JSON file that holds a version now reports it.
- Same defect elsewhere: no other code reads versions; `check_ci_policy` only checks that the files exist.

## 6. Tests (`quality-assurance`)

- Python regression test: `test_read_version_from_supported_files` gains `.claude-plugin/plugin.json` and `composer.json` (expected `1.2.3`) and a JSON file without a top-level version (expected none). It fails on main and passes after the fix.
- Parity case `release-status-version-json` on a new fixture with `.claude-plugin/plugin.json`, `composer.json` and `package.json`: recorded from the fixed Python script, compared with the engine on Linux, macOS and Windows.
- Gate 8 on this repository no longer reports the version file (checked by hand: `release_status version` prints `2.8.1`).
- Manual tests: none; the automated tests cover the case.

## 7. Documentation (`architecture-docs`, `user-docs`)

`devops-release/SKILL.md` (the `release_status version` row) and `docs/configuration.md` (`devops.version_files`) name JSON files with a top-level `version`. BACKLOG and ROADMAP record BUG-006 (v2.10.0).

## 8. Release, migration and rollback (`devops-release`)

- Version impact: a fix released with v2.10.0. It touches an engine crate, so the engine version rises by one minor version.
- No data is affected: the command only reads.
- Rollback: revert the pull request.

## 9. Execution steps and milestones

1. Regression tests (Python test and parity case) that fail on main.
2. Fix in `release_status.py` and the engine; the tests and the parity case pass.
3. Full checks (pytest, cargo test, clippy, fmt, parity, `check_docs`, `check_readme`), close BUG-006 in the backlog, pull request with review.
