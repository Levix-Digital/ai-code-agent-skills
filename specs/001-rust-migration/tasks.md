---

description: "Tasks of the Rust migration, one per milestone of the envelope plan"
---

# Tasks: Rust engine with identical behavior

**Input**: `specs/001-rust-migration/` (spec.md, plan.md)

**Prerequisites**: plan.md, spec.md

**Tests**: every task carries its own proof: parity cases, a differential fuzz with 0 differences and the gate comparison with main (plan.md, "Per-milestone procedure").

**Organization**: one task per milestone, in the plan's order. A task is one pull request; tasks run one after the other (no parallel work without Gedo). The pull request that finishes a task checks it here and marks its milestone with `check_milestone.py`.

## Format: `[ID] [Story] Description`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: The workspace, the parity harness, CI and the launchers.

- [x] T001 M1: Rust workspace parity harness and CI (#22)
- [x] T002 M2: Binary distribution and launcher (#25)

---

## Phase 2: Foundational (Kernel and technical docs)

**Purpose**: The kernel and the architecture-docs scripts.

- [x] T003 [US1] M3: translate `project_config` and swap its callers (#26)
- [x] T004 [US1] M4: translate `new_subsystem` and swap its callers (#27)
- [x] T005 [US1] M5: translate `new_adr` and swap its callers (#28)
- [x] T006 [US1] M6: translate `build_index` and swap its callers (#29)
- [x] T007 [US1] M7: translate `extract_adrs` and swap its callers (#30)
- [x] T008 [US1] M8: translate `check_docs` and swap its callers (#34)

---

## Phase 3: Checkers

**Purpose**: The checkers the gates run.

- [x] T009 [US1] M9: translate `check_product` and swap its callers (#36)
- [x] T010 [US1] M10: translate `check_readme` and swap its callers (#37)
- [x] T011 [US1] M11: translate `check_design` and swap its callers (#38)
- [x] T012 [US1] M12: translate `manual_tests` and swap its callers (#39)
- [x] T013 [US1] M13: translate `check_ci_policy` and swap its callers (#40)
- [x] T014 [US1] M14: translate `release_status` and swap its callers (#42)
- [x] T015 [US1] M15: translate `branching` and swap its callers (#43)

---

## Phase 4: Plan files without the index

**Purpose**: The plan-memory scripts that read files only.

- [x] T016 [US1] M16: translate `list_plans` and swap its callers (#44)
- [x] T017 [US1] M17: translate `get_plan` and swap its callers (#46)
- [x] T018 [US1] M18: translate `export_plan` and swap its callers
- [x] T019 [US1] M19: translate `roadmap` and swap its callers

---

## Phase 5: Orchestration

**Purpose**: The protocol's own scripts.

- [x] T020 [US1] M20: translate `constitution` and swap its callers
- [x] T021 [US1] M21: translate `speckit_setup` and swap its callers
- [x] T022 [US1] M22: translate `inspect_project` and swap its callers
- [x] T023 [US1] M23: translate `pr_sync` and swap its callers
- [ ] T024 [US1] M24: translate `gedo_feedback` and swap its callers
- [ ] T025 [US1] M25: translate `onboard` and swap its callers
- [ ] T026 [US1] M26: translate `verify_gate` and swap its callers

---

## Phase 6: Plan index (D-embed, D-store)

**Purpose**: **Blocked** until D-store is decided (before M27).

- [ ] T027 [US1] M27: translate `rebuild_index` and swap its callers
- [ ] T028 [US1] M28: translate `search_plans` and swap its callers
- [ ] T029 [US1] M29: translate `save_plan` and swap its callers
- [ ] T030 [US1] M30: translate `update_plan_status` and swap its callers
- [ ] T031 [US1] M31: translate `check_milestone` and swap its callers
- [ ] T032 [US1] M32: translate `backlog` and swap its callers
- [ ] T033 [US1] M33: translate `detect_drift` and swap its callers
- [ ] T034 [US1] M34: translate `plan-memory migrate_legacy` and swap its callers

---

## Phase 7: Session memory

**Purpose**: Session memory on the same store.

- [ ] T035 [US1] M35: translate `save_incremental_turns` and swap its callers
- [ ] T036 [US1] M36: translate `finalize_session` and swap its callers
- [ ] T037 [US1] M37: translate `read_memory` and swap its callers
- [ ] T038 [US1] M38: translate `session-memory migrate_legacy` and swap its callers

---

## Phase 8: Folder sync (D-pdf)

**Purpose**: PDF text is the one accepted exception.

- [ ] T039 [US1] M39: translate `clean` and swap its callers
- [ ] T040 [US1] M40: translate `stats` and swap its callers
- [ ] T041 [US1] M41: translate `sync` and swap its callers
- [ ] T042 [US1] M42: translate `query` and swap its callers

---

## Phase 9: Polish

**Purpose**: Remove Python and update the docs.

- [ ] T043 [US2] M43: Remove the Python remnants

---

## Dependencies & Execution Order

- Phase 1 blocks everything; the shared modules are translated with the first task that needs them.
- T027 to T038 wait for the D-store decision.
- T043 runs after every other task and after the acceptance run on a sample project (spec.md, User Story 2).
