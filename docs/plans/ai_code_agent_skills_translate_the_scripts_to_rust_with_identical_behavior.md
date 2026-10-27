---
plan_id: ai_code_agent_skills_translate_the_scripts_to_rust_with_identical_behavior
project: ai-code-agent-skills
title: Translate the scripts to Rust with identical behavior
kind: feature
version: v2.10.0
status: in_progress
author: Levi Silveira
created_at: '2026-10-27T18:17:46'
updated_at: '2026-10-27T18:31:33'
session_id: null
tags:
- rust
- translation
- parity
supersedes: null
superseded_by: null
parent_plan_id: null
backlog_items:
- IMP-005
milestones:
- name: M1 Rust workspace parity harness and CI
  done: true
  done_at: '2026-10-27'
  by: Levi Silveira
- name: M2 Binary distribution and launcher
  done: false
- name: M3 project_config
  done: false
- name: M4 new_subsystem
  done: false
- name: M5 new_adr
  done: false
- name: M6 build_index
  done: false
- name: M7 extract_adrs
  done: false
- name: M8 check_docs
  done: false
- name: M9 check_product
  done: false
- name: M10 check_readme
  done: false
- name: M11 check_design
  done: false
- name: M12 manual_tests
  done: false
- name: M13 check_ci_policy
  done: false
- name: M14 release_status
  done: false
- name: M15 branching
  done: false
- name: M16 list_plans
  done: false
- name: M17 get_plan
  done: false
- name: M18 export_plan
  done: false
- name: M19 roadmap
  done: false
- name: M20 constitution
  done: false
- name: M21 speckit_setup
  done: false
- name: M22 inspect_project
  done: false
- name: M23 pr_sync
  done: false
- name: M24 gedo_feedback
  done: false
- name: M25 onboard
  done: false
- name: M26 verify_gate
  done: false
- name: M27 rebuild_index
  done: false
- name: M28 search_plans
  done: false
- name: M29 save_plan
  done: false
- name: M30 update_plan_status
  done: false
- name: M31 check_milestone
  done: false
- name: M32 backlog
  done: false
- name: M33 detect_drift
  done: false
- name: M34 plan-memory migrate_legacy
  done: false
- name: M35 save_incremental_turns
  done: false
- name: M36 finalize_session
  done: false
- name: M37 read_memory
  done: false
- name: M38 session-memory migrate_legacy
  done: false
- name: M39 clean
  done: false
- name: M40 stats
  done: false
- name: M41 sync
  done: false
- name: M42 query
  done: false
- name: M43 Remove the Python remnants
  done: false
status_log:
- at: '2026-10-27T18:17:46'
  by: Levi Silveira
  status: draft
  note: Plan created
- at: '2026-10-27T18:25:17'
  by: Levi Silveira
  status: active
  note: Approved by the user, without Spec Kit for this plan only (a translation needs no specification beyond identical output); Spec Kit stays enabled for the work after it
- at: '2026-10-27T18:31:33'
  by: Levi Silveira
  note: Milestone 'M1 Rust workspace parity harness and CI' completed
---

# Translate the scripts to Rust with identical behavior

Plan for the backlog items below (`dev-cycle-protocol` Phases 1 and 2).

## Backlog items

- IMP-005: Translate the scripts to Rust with identical behavior

## 1. Goal and decisions

Replace the 40 Python scripts of the skills (and the 14 `_*.py` modules they share) with one Rust binary, without changing anything the protocol does. The translation is literal: same commands, same flags, same stdout and stderr text, same exit codes, same files written, same files read. No feature, fix or clean-up rides along; a bug found during the translation is captured in the backlog and fixed in both languages, or kept identical in Rust, never fixed in Rust only.

Decisions (ADR-0007, proposed):

- **One Rust binary** built from a Cargo workspace in this repository; the skill folders keep their `SKILL.md` and templates.
- **Proof of identity**: a parity harness runs the Python script and the Rust command on the same fixtures and compares stdout, stderr, exit code and the files written, byte for byte. Before a Python script is deleted its outputs are recorded as golden files, so the proof keeps running without Python.
- **One script per pull request**: when the outputs are identical, the script's `SKILL.md` lines (and every caller) switch to the Rust command and the Python script is deleted in the same pull request.
- **Shared Python modules** (`_docs.py`, `_store.py`, `project_config.py` and the others) are translated with the first script that needs them and deleted only when their last Python importer is gone.
- **Python callers in the window**: `verify_gate.py` runs the checkers as subprocesses; it calls the Rust command for every checker already swapped.

Decisions taken with the plan approval (2026-10-27) and the one still open:

- **D-dist (decided)**: a binary per target on GitHub Releases, fetched and checksum-verified by a small launcher (`sh` and `ps1`) on first use over plain HTTPS (the repository is public).
- **D-embed (decided)**: chromadb's similarity scores (`detect_drift`, `search_plans`, session memory and folder search) come from an embedding model; the same model run from Rust may differ in the last decimals. Results must come in the same order with scores equal within a stated tolerance.
- **D-store (open, before M27)**: session memory lives only in chromadb's local store; reading it from Rust needs a SQLite library compiled from C by Cargo. Whether that fits "pure Rust".
- **D-pdf (decided 2026-10-27)**: PDF text extraction in `folder-sync` uses a pure-Rust library (`pdf-extract`, on `lopdf`) and is the one accepted exception to identity: the text of a PDF page may differ from pypdf's in spacing, line breaks and order. Everything around it (other formats, page markers, sections, chunking, index metadata, output) stays identical; PDF parity cases check the page and section structure instead of the extracted text.

## 2. Design (`software-design`)

- `Cargo.toml` workspace at the root, crates under `crates/`: one crate per skill (modular monolith, ADR-0002) plus a small kernel crate (project configuration, principle 9 guard, output helpers) and the binary crate.
- Command shape: `dev-cycle <skill> <script> [the same arguments]`, for example `dev-cycle plan-memory backlog validate --dir X`. Argument parsing reproduces argparse's usage and error messages exactly.
- Rust dependencies: Rust crates only, no C toolchain, unless D-store says otherwise.
- The parity harness lives in `tests/parity/`: fixtures (project trees, recorded platform and Gedo responses), one case file per script, and a runner usable from `cargo test`.

## 3. Affected surfaces and files

- The `SKILL.md` files: each swap changes only the command line of the swapped script.
- `dev-cycle-protocol/verify_gate.py` and other Python callers, until they are translated themselves.
- New: `Cargo.toml`, `crates/`, `tests/parity/`, CI workflow for Linux, Windows and macOS.
- Removed at the end: every `*.py` of the skills, `requirements.txt`, the pytest suites (after their cases exist as Rust tests or golden files).

## 4. Tests (`quality-assurance`)

- Every swap pull request: the existing pytest cases of the script pass against the Rust command, the parity cases are identical, and the golden files are committed.
- Scripts without tests today (architecture-docs, session-memory, folder-sync) get parity cases written against the Python script first.
- Manual test per swap: run the skill's documented commands from its `SKILL.md` on this repository and compare with the Python output.
- Acceptance: at the end, `/dev-cycle` and gates 0, 3, rc and 8 give the same results on this repository as before the migration.

## 5. Documentation (`architecture-docs`, `user-docs`)

- ADR-0007 now (proposed), accepted with this plan; it amends ADR-0001 (Python scripts) once the last script is swapped.
- At the end (M43): TECH_SPEC (stack, platforms), VISION (no Python, no `pip install`), README and `docs/installation.md`, TECDOC pages that name Python scripts.

## 6. CI, release and observability (`devops-release`)

- CI builds and tests the workspace on Linux, Windows and macOS, and runs the parity harness.
- Release builds for the targets chosen in D-dist; version stays in `.claude-plugin/plugin.json`.
- No new logs or telemetry: output must stay identical.

## 7. Security, migration and rollback

- No credentials: platform calls keep going through `gh` and `glab`.
- Rollback of a swap = revert its pull request: the Python script and its `SKILL.md` lines come back unchanged.
- Local data under `~/.ai-code-agent-memory/` is read and written in the same places and formats (D-store).

## 8. Execution steps and milestones

Each milestone after M2 is one pull request that translates one script, proves identity, swaps the `SKILL.md` lines and deletes the Python script. The order goes from scripts nothing else depends on to the orchestrators, and leaves the chromadb scripts for last.

1. M1 Rust workspace, parity harness and CI (no SKILL.md changes)
2. M2 Binary distribution and launcher (D-dist)
3. M3-M8 Kernel and technical docs: project_config, new_subsystem, new_adr, build_index, extract_adrs, check_docs
4. M9-M15 Checkers: check_product, check_readme, check_design, manual_tests, check_ci_policy, release_status, branching
5. M16-M19 Plan files without the index: list_plans, get_plan, export_plan, roadmap
6. M20-M26 Orchestration: constitution, speckit_setup, inspect_project, pr_sync, gedo_feedback, onboard, verify_gate
7. M27-M34 Plan index (D-embed, D-store): rebuild_index, search_plans, save_plan, update_plan_status, check_milestone, backlog, detect_drift, plan-memory migrate_legacy
8. M35-M38 Session memory: save_incremental_turns, finalize_session, read_memory, session-memory migrate_legacy
9. M39-M42 Folder sync (PDF text exception): clean, stats, sync, query
10. M43 Remove the Python remnants and update the docs
