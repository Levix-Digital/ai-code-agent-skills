---
plan_id: ai_code_agent_skills_branching_models
project: ai-code-agent-skills
title: Branching models
version: v2.5.0
status: active
author: Levix Digital
created_at: '2026-10-23T21:02:03'
updated_at: '2026-10-23T21:02:03'
session_id: null
tags:
- devops-release
- branching
- dev-cycle-protocol
supersedes: null
superseded_by: null
parent_plan_id: null
milestones:
- name: BR-M1 Presets
  done: false
- name: BR-M2 CI policy and templates
  done: false
- name: BR-M3 Onboarding and gates
  done: false
- name: BR-M4 Documentation and release
  done: false
status_log:
- at: '2026-10-23T21:02:03'
  by: Levix Digital
  status: active
  note: Plan created
- at: '2026-10-23T21:02:30'
  by: Levix Digital
  status: active
  note: 'Approved by the user: all three presets, starting with trunk'
---

# Branching Models

## 1. Goal and decisions

Today `devops-release` imposes one branching model: `main` with released code, `dev` for integration, release candidates tagged on `dev` and the user's merge of `dev` into `main` as the release gate. Only the branch names can change. Projects that work on `main` alone (trunk-based) are told to create `dev`, and products that maintain several versions have no place for `release/x.y` branches.

This plan lets each project choose its branching model in `.dev-cycle.json` (`devops.branching`), from a closed set of presets. Each preset answers the same questions: where work branches start, where pull requests go, where a candidate is cut, how a hotfix travels and which action of the user releases.

Decisions:

- **D1 — Presets, not free-form.** `devops.branching` is one of `integration` (default, today's model), `trunk` and `release-branches`. A free-form model could not tell the gates where the release gate is.

  | | `integration` | `trunk` | `release-branches` |
  |---|---|---|---|
  | Long-lived branches | `main` + `dev` | `main` | `main` + `release/X.Y` |
  | Work branches from / pull requests into | `dev` | `main` | `main` (fixes of a release in acceptance or maintenance: `release/X.Y`, then forward to `main`) |
  | Candidate `vX.Y.Z-rc.N` tagged on | `dev` | `main` | `release/X.Y` |
  | Release gate (the user) | merges `dev` into `main` (tag-on-merge) | runs the promote workflow for the accepted candidate | runs the promote workflow for the accepted candidate |
  | Hotfix | `hotfix/<slug>` from `main`, back into `dev` | `fix/<slug>` into `main`; from the release tag when `main` holds unreleased work | `fix/<slug>` into `release/X.Y`, then forward to `main` |

- **D2 — The promote gate.** Without a merge into a production branch, the user's release action is the promote workflow (`templates/promote.yml`): started by the user with the accepted candidate tag, behind a GitHub environment with required reviewers. It tags the exact commit of the candidate as `vX.Y.Z` and calls the release workflow, so what was accepted is what is released. The agent never runs it.
- **D3 — Environment branches are not a preset.** `staging`/`qa`/`prod` branches drift and rebuild the same code several times; environments are deployment targets of one artifact.
- **D4 — Backward compatible, decided once.** A missing key means `integration`, so every existing project keeps working. `/dev-cycle` detects the model from the repository (`release/*` branches, a `dev`/`develop` branch, `main` alone), proposes it and records the user's choice once, like the Spec Kit question.
- **D5 — One owner.** `devops-release/branching.py` holds the rules of the presets (pure functions plus a `show` and `rc-branch` CLI); the orchestrator, the CI policy checker and the onboarding read them from there.

## 2. Design (`software-design`)

| Unit | Change |
|---|---|
| `devops-release/branching.py` (new) | `BranchingModel` from the `devops` section: work base, pull request targets, candidate branch for a version, release gate (`merge` or `promote`), long-lived branches; `detect()` from branch names; CLI `show`, `rc-branch` |
| `dev-cycle-protocol/project_config.py` | `devops.branching` (default `integration`) and `devops.release_branch_prefix` (default `release`), validated |
| `devops-release/check_ci_policy.py` | Pull request CI for the model's targets; final tag on merge (`integration`) or a promote workflow (`trunk`, `release-branches`); a tag-on-merge on `main` under `trunk` is an error (every merge would release) |
| `devops-release/templates/promote.yml` (new), `release.yml`, `ci.yml` | Promote workflow; release comments; `make_latest: legacy` so a maintenance release never becomes "latest"; CI branch lists per model |
| `dev-cycle-protocol/onboard.py`, `_onboard.py` | Detect and propose the model; `branching` setup item asked once; the integration branch item only for `integration`; the accepted candidate option names the model's gate |
| `dev-cycle-protocol/verify_gate.py` | Gate 8 names the model's release gate; with `promote` the candidate tag must exist |

## 3. Affected surfaces and files

Scripts above, `devops-release/SKILL.md`, `dev-cycle-protocol/SKILL.md`, `dev-cycle/SKILL.md`, `product-inception/SKILL.md`, `quality-assurance/SKILL.md`, `README.md`, `docs/configuration.md`, `docs/pull-requests.md`, `tests/MANUAL_TESTS.md`, plugin version. All additive.

## 4. Tests (`quality-assurance`)

Automated: each preset's rules; detection; configuration defaults and validation; the CI policy per preset with the templates; the onboarding item and gate wording; gate 8 under `promote`. Manual: `/dev-cycle` in a repository with `main` only proposes `trunk`; a candidate promoted under `trunk`.

## 5. Documentation

Branching section of `devops-release` per preset; Phases 4, 7, 8 and 10 of the protocol in terms of the model; configuration reference.

## 6. CI, release and observability

No workflow change in this repository. Plugin `2.4.0` → `2.5.0`. `branching.py show` prints the effective model so a wrong gate can be traced.

## 7. Security, migration and rollback

No migration: the default is today's model. Rollback: revert the pull request.

## 8. Milestones

- **BR-M1 Presets**: `branching.py`, configuration, tests.
- **BR-M2 CI policy and templates**: checker, `promote.yml`, tests.
- **BR-M3 Onboarding and gates**: detection, setup item, gate 8, tests.
- **BR-M4 Documentation and release**: skills, README, guides, manual tests, plugin 2.5.0.
