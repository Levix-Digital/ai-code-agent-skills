---
plan_id: ai_code_agent_skills_translate_the_scripts_to_rust_with_identical_behavior
project: ai-code-agent-skills
title: Translate the scripts to Rust with identical behavior
kind: feature
version: v2.10.0
status: in_progress
author: Levi Silveira
created_at: '2026-10-27T18:17:46'
updated_at: '2026-10-29T13:57:44'
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
  done: true
  done_at: '2026-10-27'
  by: Levi Silveira
- name: M3 project_config
  done: true
  done_at: '2026-10-27'
  by: Levi Silveira
- name: M4 new_subsystem
  done: true
  done_at: '2026-10-28'
  by: Levi Silveira
- name: M5 new_adr
  done: true
  done_at: '2026-10-28'
  by: Levi Silveira
- name: M6 build_index
  done: true
  done_at: '2026-10-28'
  by: Levi Silveira
- name: M7 extract_adrs
  done: true
  done_at: '2026-10-28'
  by: Levi Silveira
- name: M8 check_docs
  done: true
  done_at: '2026-10-28'
  by: Levi Silveira
- name: M9 check_product
  done: true
  done_at: '2026-10-28'
  by: Levi Silveira
- name: M10 check_readme
  done: true
  done_at: '2026-10-28'
  by: Levi Silveira
- name: M11 check_design
  done: true
  done_at: '2026-10-29'
  by: Levi Silveira
- name: M12 manual_tests
  done: true
  done_at: '2026-10-29'
  by: Levi Silveira
- name: M13 check_ci_policy
  done: true
  done_at: '2026-10-29'
  by: Levi Silveira
- name: M14 release_status
  done: true
  done_at: '2026-10-29'
  by: Levi Silveira
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
- at: '2026-10-27T22:51:27'
  by: Levi Silveira
  note: Milestone 'M2 Binary distribution and launcher' completed
- at: '2026-10-27T23:25:38'
  by: Levi Silveira
  note: 'Milestone ''M3 project_config'' completed: Engine 0.2.0: project_config in Rust (crates/kernel), with the Python-compatible json, argparse, textwrap and os.path layer; 17 parity cases; project_config.py stays as a module for the Python scripts that import it.'
- at: '2026-10-28T10:48:22'
  by: Levi Silveira
  note: 'Milestone ''M4 new_subsystem'' completed: Engine 0.3.0: new_subsystem in Rust (crates/architecture-docs), with a PyYAML-compatible safe_load and safe_dump in the kernel; 7 parity cases; _docs.py and build_index.py stay for the Python scripts that import them.'
- at: '2026-10-28T11:23:38'
  by: Levi Silveira
  note: 'Milestone ''M5 new_adr'' completed: Engine 0.4.0: new_adr in Rust, with argparse choices and the local date in the kernel; 8 parity cases and a 3,300-run differential fuzz with 0 differences.'
- at: '2026-10-28T11:38:46'
  by: Levi Silveira
  note: 'Milestone ''M6 build_index'' completed: Engine 0.5.0: the build_index command in Rust; 11 parity cases and a 3,000-run differential fuzz with 0 differences; build_index.py stays as a module for check_docs.py and extract_adrs.py.'
- at: '2026-10-28T12:16:31'
  by: Levi Silveira
  note: 'Milestone ''M7 extract_adrs'' completed: Engine 0.6.0: extract_adrs in Rust; kernel gained Python 3.13 tables for \w, lower(), \s and \d (every code point checked); 8 parity cases and a 3,000-run differential fuzz with 0 differences.'
- at: '2026-10-28T14:01:08'
  by: Levi Silveira
  note: 'Milestone ''M8 check_docs'' completed: Engine 0.7.0: check_docs in Rust; verify_gate runs it on the engine; build_index.py removed; _docs.py stays for check_product.py, check_readme.py and constitution.py; 10 parity cases and a 3,600-run differential fuzz with 0 differences.'
- at: '2026-10-28T16:21:43'
  by: Levi Silveira
  note: 'Milestone ''M9 check_product'' completed: Engine 0.8.0: check_product in Rust (crates/product-inception, templates built into the engine); verify_gate runs it on the engine; its pytest cases became 12 parity cases; a 3,000-run differential fuzz with 0 differences.'
- at: '2026-10-28T16:49:58'
  by: Levi Silveira
  note: 'Milestone ''M10 check_readme'' completed: Engine 0.9.0: check_readme in Rust (crates/user-docs); verify_gate runs it on the engine; its pytest cases became 10 parity cases; a 3,000-run differential fuzz with 0 differences; _docs.py stays for constitution.py.'
- at: '2026-10-29T08:31:38'
  by: Levi Silveira
  note: 'Milestone ''M11 check_design'' completed: Engine 0.10.0: check_design in Rust (crates/software-design), with Python''s \w in the kernel''s patterns and os.walk order; verify_gate runs it on the engine; its pytest cases became 10 parity cases; 4,500 differential fuzz runs with 0 differences.'
- at: '2026-10-29T08:32:55'
  by: Levi Silveira
  note: 'Milestone ''M11 check_design'' completed: Engine 0.10.0: check_design in Rust (crates/software-design), with Python''s \w in the shared patterns and the os.walk order; verify_gate runs it on the engine; its pytest cases became 10 parity cases; 4,500 differential fuzz runs with 0 differences.'
- at: '2026-10-29T09:33:22'
  by: Levi Silveira
  note: 'Milestone ''M12 manual_tests'' completed: Engine 0.11.0: quality-assurance manual_tests on the engine (kernel argparse gains a positional choice and append options); 22 parity cases, 4,500-run differential fuzz on 3.13 and 3.10 with 0 differences; gates rc and 8 match main. manual_tests.py stays as a module while onboard.py and backlog.py import load_suite.'
- at: '2026-10-29T13:20:07'
  by: Levi Silveira
  note: 'Milestone ''M13 check_ci_policy'' completed: Engine 0.12.0: check_ci_policy in Rust (crates/devops-release, with the BranchingModel it reads; the kernel gains fnmatch and one-directory glob); verify_gate runs it on the engine and its pytest cases now call the engine; 11 parity cases; 4,500 differential fuzz runs on 3.13 and 3.10 with 0 differences; gate 8 matches main.'
- at: '2026-10-29T13:57:44'
  by: Levi Silveira
  note: 'Milestone ''M14 release_status'' completed: Kernel tomllib port; 10 parity cases; fuzz 4500 runs 0 differences; gates 8/rc match main on 42 fixtures; script kept as module (branching imports project_version)'
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
- Projects that use the protocol keep working without any change, during and after the migration (user's rule, 2026-10-27): same commands for the agents, same project files and formats. Before M43 removes Python, `/dev-cycle` and the gates run on a sample project (and on this repository) with the last Python version and with the Rust engine, and the results are compared.

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
