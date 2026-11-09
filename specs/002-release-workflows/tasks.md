---

description: "Tasks of the candidate and promote workflows for the skills releases"
---

# Tasks: Candidate and promote workflows for the skills releases

**Input**: `specs/002-release-workflows/` (spec.md, plan.md, research.md, data-model.md, contracts/workflows.md, quickstart.md)

**Prerequisites**: plan.md, spec.md

**Tests**: `check_ci_policy` is the automated check; the quickstart runs for real at v2.10.0 (no test framework for workflows in this repository).

**Organization**: by user story; one pull request for all tasks.

## Format: `[ID] [P?] [Story] Description`

---

## Phase 1: Setup

- [x] T001 Run `check_ci_policy` on `main` and record its warning (no promote workflow) as the failing baseline

---

## Phase 2: User Story 1 - The maintainer gets a release candidate to accept (Priority: P1) 🎯 MVP

**Goal**: a `vX.Y.Z-rc.N` tag publishes a checked pre-release.

**Independent Test**: quickstart step 2.

- [x] T002 [US1] Create `.github/workflows/release.yml` from `devops-release/templates/release.yml`: `v*` tag push and `workflow_call` with `tag` (contracts/workflows.md)
- [x] T003 [US1] In `release.yml`, fail before publishing on a bad tag format, a final tag reached by a push instead of the promotion, a `plugin.json` version different from the tag's, and a missing `engine-v<engine-version>` release (FR-002)
- [x] T004 [US1] In `release.yml`, build the notes: install line, engine line, generated notes since the last final `vX.Y.Z` (FR-003, research R3); publish as pre-release for `-rc`, latest only for final tags (FR-001, R4)

**Checkpoint**: candidates can be published.

---

## Phase 3: User Story 2 - The maintainer promotes the accepted candidate (Priority: P1)

**Goal**: the maintainer's approved promotion tags the candidate's commit and releases it.

**Independent Test**: quickstart step 4.

- [x] T005 [US2] Create `.github/workflows/promote.yml` from `devops-release/templates/promote.yml`, with the `release` environment and the version read from `.claude-plugin/plugin.json` when no candidate is given (FR-004)
- [x] T006 [US2] Keep the template's refusals before tagging: invalid or missing candidate, existing final tag (FR-005); call `release.yml` with the final tag
- [x] T007 [US2] `check_ci_policy` passes with 0 warnings (FR-006)

---

## Phase 4: User Story 3 - A maintainer knows how to cut a release (Priority: P2)

- [x] T008 [P] [US3] Add "Release the skills" to `docs/maintaining.md` (FR-007)

---

## Phase 5: Polish

- [x] T009 Run the repository checks (`cargo test --workspace`, parity, gate 3) and open the pull request; mark the plan milestones
- [ ] T010 After merge, run quickstart step 2 with `v2.10.0-rc.1` (part of the v2.10.0 release, Phase 8)

---

## Dependencies & Execution Order

- T001 → T002 → T003 → T004 → T005 → T006 → T007; T008 any time; T009 after all; T010 after merge, the version bump and the engine release.
- US2 calls the workflow of US1, so US1 comes first.

## Plan milestones

- Workflows: T001 to T007
- Documentation: T008
- Checks and pull request: T009
