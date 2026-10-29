# Implementation Plan: Rust engine with identical behavior

**Branch**: `levix/rust-m<N>-<script>` | **Date**: 2026-10-29 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/001-rust-migration/spec.md`. The envelope plan (`docs/plans/ai_code_agent_skills_translate_the_scripts_to_rust_with_identical_behavior.md`) holds the decisions (ADR-0007, D-dist, D-embed, D-store, D-pdf) and the milestones.

## Summary

Replace the Python scripts of the skills with one Rust binary, one script per pull request, without changing anything the protocol does. Each pull request proves identity with parity cases, a differential fuzz and a gate comparison against main, swaps the documentation and callers to the engine command, and deletes the script once nothing imports it.

## Technical Context

**Language/Version**: Rust (edition 2024, `rust-version` in `Cargo.toml`); reference Python 3.13, CI also on 3.10

**Primary Dependencies**: `regex`, `serde`/`serde_json` (harness), `chrono`, `unicode-normalization`, `terminal_size`; Rust crates only, no C toolchain unless D-store decides otherwise

**Storage**: project files (`docs/`, `.dev-cycle.json`, `specs/`); local data under `~/.ai-code-agent-memory/` from M27

**Testing**: `cargo test`, the parity harness (`cargo run -p parity -- run`), differential fuzz scripts, pytest until M43

**Target Platform**: Linux, macOS and Windows (x86_64 and aarch64), through the `sh` and `ps1` launchers

**Project Type**: CLI (one binary, `dev-cycle <skill> <script> [arguments]`)

**Performance Goals**: none beyond the Python scripts; output identity comes first

**Constraints**: byte-identical stdout, stderr, exit codes and files; no environment-specific behavior

**Scale/Scope**: 43 milestones; 16 done before this envelope (M1 to M16)

## Constitution Check

- **Design, Simplicity First**: one crate per skill plus the kernel; shared Python behavior (argparse, json, yaml, tomllib, fnmatch, Unicode tables) lives once in `crates/kernel/src/py`.
- **Architecture style (modular monolith)**: skill crates depend on the kernel and, where Python shares a module across skills, on the crate that holds its translation (for example `plan-memory` uses `architecture_docs::docs`).
- **Testability**: every command is a function from arguments to an exit code; the parity harness runs it in a temporary project.
- No violations to justify.

## Project Structure

### Documentation (this feature)

```text
specs/001-rust-migration/
├── spec.md
├── plan.md
└── tasks.md
```

### Source Code (repository root)

```text
Cargo.toml                 # workspace; version = engine version
crates/
├── kernel/                # project_config and the py/ layer
├── architecture-docs/
├── product-inception/
├── user-docs/
├── software-design/
├── quality-assurance/
├── devops-release/
├── plan-memory/
├── dev-cycle/             # the binary: COMMANDS table
└── parity/                # the harness
tests/parity/
├── cases/<skill>.json
├── fixtures/
└── golden/
dev-cycle-protocol/bin/    # launchers and engine-version
```

**Structure Decision**: the skill folders keep their `SKILL.md` and templates; their scripts move into the crate of the same name.

## Per-milestone procedure

1. Translate the script (and the shared modules it needs) into its skill crate; register it in `crates/dev-cycle/src/main.rs`.
2. Write parity cases and fixtures; record the goldens with Python 3.13 and run them on 3.10.
3. Run a differential fuzz of at least 3,000 runs on 3.13 and 1,500 on 3.10 with 0 differences.
4. Compare gates 0, 3, 8 and rc with main on every fixture (worktree of main, `DEV_CYCLE_BIN`).
5. Swap the `SKILL.md` lines, README, MANUAL_TESTS, architecture docs and callers; add the command to `ENGINE_CHECKERS` when `verify_gate.py` or a pytest case runs it.
6. Delete the script when nothing imports it; move its pytest cases to `run_checker` or to parity cases.
7. Raise the engine version, run fmt, clippy, tests, parity and pytest, mark the milestone (`check_milestone.py`) and the task below, and open the pull request.

## Complexity Tracking

None.

## Dev Cycle Protocol

### Backlog items and release

IMP-005 (docs/BACKLOG.md), release v2.10.0 (docs/ROADMAP.md). The engine has its own version, raised on every pull request that changes the crates (`engine-vX.Y.Z`).

### Tests (quality-assurance)

- Per milestone: parity cases with golden files (Python 3.13 and 3.10), a differential fuzz with 0 differences, the gate comparison with main, and the script's pytest cases moved to the engine (`verify_gate.run_checker`) or to parity cases when the script is deleted.
- Manual test per swap: run the skill's documented commands from its `SKILL.md` on this repository and compare with the Python output.
- Acceptance before M43: `/dev-cycle` and gates 0, 3, rc and 8 on a sample project and on this repository, with the last Python version and with the engine.

### Documentation (architecture-docs, user-docs)

- Each swap edits the `SKILL.md` lines, README rows, MANUAL_TESTS and the `docs/architecture` page that names the script.
- ADR-0007 (the engine); it amends ADR-0001 once the last script is swapped.
- M43: TECH_SPEC, VISION, README and `docs/installation.md` lose Python.

### CI, release and observability (devops-release)

- CI builds and tests the workspace on Linux, macOS and Windows and runs the parity harness; the engine workflow publishes `engine-vX.Y.Z` with SHA256SUMS on merges that change the crates.
- Version impact: minor for the plugin at v2.10.0; the engine version rises with every milestone.
- No new logs or telemetry: output must stay identical.

### Security, migration and rollback

- No credentials: platform calls keep going through `gh` and `glab`.
- No data migration: files and `~/.ai-code-agent-memory/` stay in the same places and formats (D-store).
- Rollback of a milestone = revert its pull request: the Python script and its documentation lines come back unchanged, and the launcher fetches the engine version of the reverted tree.
