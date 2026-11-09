---
plan_id: ai_code_agent_skills_candidate_and_promote_workflows_for_the_skills_releases
project: ai-code-agent-skills
title: Candidate and promote workflows for the skills releases
kind: feature
version: 2.10.0
status: active
author: LeviGuilherme
created_at: '2026-11-09T12:27:37'
updated_at: '2026-11-09T12:32:11'
session_id: null
tags: []
supersedes: null
superseded_by: null
parent_plan_id: null
backlog_items:
- IMP-013
spec_dir: specs/002-release-workflows
milestones:
- name: Workflows
  done: false
- name: Documentation
  done: false
- name: Checks and pull request
  done: false
status_log:
- at: '2026-11-09T12:27:37'
  by: LeviGuilherme
  status: draft
  note: Plan created
- at: '2026-11-09T12:27:59'
  by: LeviGuilherme
  status: active
  note: Plan updated
- at: '2026-11-09T12:32:11'
  by: LeviGuilherme
  status: active
  note: Plan updated
---

Plan for the backlog items below (`dev-cycle-protocol` Phases 1 and 2). It is the envelope of the Spec Kit feature [`specs/002-release-workflows`](../../specs/002-release-workflows/): the specification, the technical plan with the protocol's sections, research, contract, quickstart and tasks are there.

## Backlog items

### IMP-013: Candidate and promote workflows for the skills releases
- **Priority**: high
- **Found**: 2026-11-09, v2.10.0 release
- **Description**: The repository follows the trunk model but no workflow publishes a product release: a vX.Y.Z-rc.N tag gives no pre-release and there is no promote workflow for the user's release gate (check_ci_policy warns). Add release.yml (pre-release for candidates, release for final tags, checks the version file and the engine release) and promote.yml (the user's promotion of an accepted candidate, held by the release environment).

## Goal and decisions

Every release of the skills, starting with v2.10.0, goes through a candidate (Phase 8), the maintainer's acceptance (Phase 9) and the maintainer's promotion (Phase 10). Details in `specs/002-release-workflows/plan.md` and `research.md`.

- **D1** The release is the repository at its tag (install from a clone); the engine keeps its own `engine-vX.Y.Z` releases.
- **D2** The release refuses a bad tag, a final tag reached by a push, a version file that disagrees with the tag and a missing engine release, before publishing.
- **D3** Notes are generated since the last final `vX.Y.Z`, with the install command and the engine version on top.
- **D4** `promote.yml` tags the accepted candidate's commit and calls the release in the same run, held by the `release` environment (created by the maintainer, required reviewer). No new secret.

## Consistency analysis (`/speckit-analyze`)

- Coverage: FR-001 to FR-007 each have at least one task (T002 to T008); SC-001 to SC-003 are checked by the v2.10.0 release itself (T010 and the promotion).
- Constitution: no conflict (Simplicity First, Safe delivery, Security, Follow-through, No AI attribution, the trunk and `plugin.json` constraint).
- Fixed during analysis: the edge case "a final release is never published from a tag push" had no requirement or task (the template's release workflow publishes any `v*` tag); FR-002, T003 and the contract now refuse it.
- Open: none.

## Milestones

1. Workflows (T001 to T007)
2. Documentation (T008)
3. Checks and pull request (T009)
