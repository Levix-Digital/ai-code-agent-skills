---
plan_id: ai_code_agent_skills_folder_sync_stats_fails_on_any_synced_index
project: ai-code-agent-skills
title: folder-sync stats fails on any synced index
kind: bug
version: 2.10.0
status: in_progress
author: Levi Silveira
created_at: '2026-11-01T15:59:11'
updated_at: '2026-11-01T16:18:14'
session_id: null
tags: []
supersedes: null
superseded_by: null
parent_plan_id: null
backlog_items:
- BUG-005
milestones:
- name: Regression test
  done: true
  done_at: '2026-11-01'
  by: Levi Silveira
- name: Fix
  done: true
  done_at: '2026-11-01'
  by: Levi Silveira
- name: Checks and pull request
  done: false
status_log:
- at: '2026-11-01T15:59:11'
  by: Levi Silveira
  status: draft
  note: Plan created
- at: '2026-11-01T16:00:14'
  by: Levi Silveira
  status: planned
  note: Plan updated
- at: '2026-11-01T16:00:39'
  by: Levi Silveira
  status: active
  note: Plan ready for approval
- at: '2026-11-01T16:17:23'
  by: Levi Silveira
  status: in_progress
  note: Plan approved by the user (gate 3)
- at: '2026-11-01T16:18:10'
  by: Levi Silveira
  note: Milestone 'Regression test' completed
- at: '2026-11-01T16:18:14'
  by: Levi Silveira
  note: Milestone 'Fix' completed
---

# folder-sync stats fails on any synced index

Bug plan: discovery and planning of the backlog items below (`dev-cycle-protocol` Phases 1 and 2).

## Backlog items

### BUG-005: folder-sync stats fails on any synced index
- **Severity**: medium
- **Found**: 2026-11-01, Rust migration M40 review of stats.py
- **Description**: stats.py formats the last sync time with datetime.fromtimestamp, but never imports datetime. Every index that has synced at least one file has a last sync time, so /folder-stats stops with NameError instead of showing the statistics. Only an empty or missing index works.

## 1. Triage

- **Expected vs. actual**: `/folder-stats` (`folder-sync/scripts/stats.py --dir .`) should print the index statistics: files, chunks, volume, last sync and the largest documents (README "folder-sync", `folder-sync/SKILL.md` section 3, and the code itself, which formats the last sync as `%d/%m/%Y %H:%M:%S`). Instead it stops with `NameError` as soon as the index has one synced file.
- **Verdict**: bug. The statistics are documented and the code already tries to show them.
- **Suspected code paths**: `folder-sync/scripts/stats.py`, `show_stats`, the line that formats `last_sync` with `datetime.fromtimestamp`; the module imports `os`, `sys`, `sqlite3`, `argparse` and `Path`, but not `datetime`.
- **Severity**: medium, confirmed. The command is unusable on every real index, but it only reads and loses no data.

## 2. Reproduction

On main (`ab9e78d`), Python 3.13, Linux:

1. In an empty folder, write `a.md` and run `python folder-sync/scripts/sync.py --dir .` (indexes 1 file, 1 chunk).
2. Run `python folder-sync/scripts/stats.py --dir .`.

Output (exit code 1):

```
  File ".../folder-sync/scripts/stats.py", line 33, in show_stats
    last_sync_str = datetime.fromtimestamp(last_sync).strftime("%d/%m/%Y %H:%M:%S") if last_sync else "Nunca"
                    ^^^^^^^^
NameError: name 'datetime' is not defined. Did you forget to import 'datetime'?
```

A folder with no index prints `ℹ️ Folder '<name>' has no synced .folder_brain index yet.` and works.

## 3. Evidence and root cause

- Confirmed: `stats.py` never imported `datetime`; `git log -S datetime` shows the call exists since the first commit (`18284da`) and no version ever had the import.
- Confirmed: `sync.py` stores `time.time()` in `last_synced_at` for every synced file, so `MAX(last_synced_at)` is never empty once a file is indexed, and the failing branch always runs. Only an index with zero files skips it.
- No test covers `stats.py`, so the error was never seen.

## 4. Fix design (`software-design`)

Add `from datetime import datetime` to `stats.py`. Nothing else changes: same messages, same date format, local time as `fromtimestamp` gives it.

Deliberately left out: the Portuguese leftovers in the output (`Volume total de dados`, `Nunca`, `Maiores documentos indexados`) and the unused `os` import. They are behavior, not this defect; changing them would also change what the Rust translation (IMP-005, M40) must reproduce. They are captured as a separate backlog item.

## 5. Affected surfaces and files

- `folder-sync/scripts/stats.py` (the `/folder-stats` command output). No interface, option or data format changes.
- Same defect elsewhere: searched `folder-sync/scripts` and the other skills for `datetime.` without the import; none.

## 6. Tests (`quality-assurance`)

- Regression test `folder-sync/tests/test_stats.py`: builds `.folder_brain/sync_state.sqlite` with two files (the schema `sync.py` creates), runs `stats.py`, and checks exit code 0, the counts, the volume, the last sync line (`datetime.fromtimestamp(...).strftime("%d/%m/%Y %H:%M:%S")`, so it holds in any time zone) and the largest documents in order. A second test checks the folder without an index. It fails before the fix (NameError) and passes after.
- No chromadb or embedding model is needed: `stats.py` only reads the SQLite state.
- Manual tests: none to add; the suite has no folder-sync test and the automated one covers the case.
- After the fix, M40 translates the fixed script to Rust; its parity cases cover the same index, and the test then runs against the engine.

## 7. Documentation (`architecture-docs`, `user-docs`)

None to correct: README and `folder-sync/SKILL.md` already describe what the command shows. BACKLOG and ROADMAP record BUG-005 (v2.10.0, with IMP-005).

## 8. Release, migration and rollback (`devops-release`)

- Version impact: patch-level fix to a skill script, released with v2.10.0. It touches no engine crate, so the engine version stays.
- No data is affected: the command only reads.
- Rollback: revert the commit.

## 9. Execution steps and milestones

1. Regression test that fails on main.
2. Add the import; the test passes.
3. Full test suite, `check_docs`, `check_readme`, `check_design`; close BUG-005 in the backlog; pull request with review.
