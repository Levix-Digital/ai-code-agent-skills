---
plan_id: dev_cycle_protocol_release_gate_verifies_the_publishing_environments_have_a_required_reviewer
project: dev-cycle-protocol
title: Release gate verifies the publishing environments have a required reviewer
kind: feature
version: 2.10.1
status: draft
author: LeviGuilherme
created_at: '2026-11-13T21:18:21'
updated_at: '2026-11-13T21:18:21'
session_id: null
tags: []
supersedes: null
superseded_by: null
parent_plan_id: null
backlog_items:
- IMP-014
milestones: []
status_log:
- at: '2026-11-13T21:18:21'
  by: LeviGuilherme
  status: draft
  note: Plan created
---

# Release gate verifies the publishing environments have a required reviewer

Plan for the backlog items below (`dev-cycle-protocol` Phases 1 and 2).

## Backlog items

### IMP-014: Release gate verifies the publishing environments have a required reviewer
- **Priority**: high
- **Found**: 2026-11-13, Mirror releases run (PR #104)
- **Description**: devops-release asks for the release environment with the user as required reviewer and says the agent never runs promote.yml, but nothing verifies it: check_ci_policy only sees 'environment:' in the workflow file, and on 2026-11-13 the release environment of this repository had no reviewer, so Mirror releases (and a promotion) ran with no approval, started by the agent. (1) A check through the GitHub API (repos/{repo}/environments/{name}) that every environment used by a workflow that publishes a release or creates a release tag has at least one required reviewer; missing or empty is an error at the rc gate and in check_ci_policy when a token is available, a warning otherwise. (2) The rule that the agent never runs promote.yml covers every workflow that publishes a release, including mirror-releases.yml: the agent hands the user the command.

## 1. Goal and decisions

A release is published only after a person approves it. Today the protocol asks for it in prose (`devops-release`: the `release` environment with the user as required reviewer; the agent never runs `promote.yml`), but nothing checks it. On 2026-11-13 this repository's `release` environment had no reviewer, and the agent started Mirror releases, which published releases with no approval. This plan turns the rule into a check and widens it to every publishing workflow.

- **D1 — The check reads the environment, not the workflow file.** `check_ci_policy` already sees `environment:` in the file; the missing piece is whether that environment, on the platform, holds the run for a reviewer. It is read through the GitHub API with `gh` (`repos/{repo}/environments/{name}`, `protection_rules[].type == "required_reviewers"` with at least one reviewer), the same way `pr_providers` reaches the platform.
- **D2 — Error at the rc gate, warning elsewhere.** `verify_gate rc` fails when an environment used by a publishing workflow is missing or has no required reviewer, since that gate is the last step before the user's promotion. `check_ci_policy` stays a file-only check (its behavior and golden files do not change); it gains nothing here.
- **D3 — No platform access is not a pass.** When `gh` is missing, not signed in or the API refuses, the rc gate fails with the command the user runs to check it by hand; it never assumes the environment is protected.
- **D4 — Publishing workflow** means a workflow that publishes a release or creates a release tag (`publishes_release()` or `TAG_PUSH_RE` in `check_ci_policy`, reused, not copied), including `mirror-releases.yml`, `promote.yml` and `release.yml` when it runs on its own.
- **D5 — The agent never runs a publishing workflow.** The rule in `devops-release` and `dev-cycle-protocol` changes from "never runs the promote workflow" to "never starts a workflow that publishes a release or creates a release tag (promote, mirror, release by dispatch)": it hands the user the command.
- **Alternative rejected**: making `check_ci_policy` call the API. It would change a parity-tested command and make a file check depend on the network.

## 2. Design (`software-design`)

`crates/devops-release`: a new function `release_environments(root, repo)` that lists the publishing workflows and their `environment:` names (from the loaded workflows of `check_ci_policy`), and a `check_release_environments` command that queries each one through `gh api` and prints one line per environment (`✅ release: required reviewers LeviGuilherme`, `❌ release: no required reviewer`). The `gh` call goes through a small trait so tests feed canned API answers. `verify_gate rc` calls it after the manual test run check and fails on any ❌. The repository comes from the `origin` remote, as `pr_providers` does.

## 3. Affected surfaces and files

New command `devops-release check_release_environments`; `verify_gate rc` output gains the environment lines. Files: `crates/devops-release/src/release_environments.rs` (new), `crates/devops-release/src/lib.rs`, `crates/dev-cycle-protocol/src/verify_gate.rs`, `devops-release/SKILL.md`, `dev-cycle-protocol/SKILL.md`, `docs/maintaining.md`, `engine-version` and `Cargo.toml` (engine bump), `.claude-plugin/plugin.json` (2.10.1).

## 4. Tests (`quality-assurance`)

Unit tests with canned API answers: environment with a reviewer passes; no protection rules, an empty reviewer list, a missing environment (404) and a refused call each fail with their message; a publishing workflow with no `environment:` fails. A parity case for the new command with a stubbed `gh`. Existing parity cases of `check_ci_policy` and `verify_gate` stay unchanged except the rc gate cases, whose golden files gain the environment lines (recorded and reviewed in the pull request). Manual test: the release candidate test of `tests/MANUAL_TESTS.md` gains a step that removes the reviewer and expects the rc gate to fail.

## 5. Documentation (`architecture-docs`, `user-docs`)

`devops-release/SKILL.md` and `dev-cycle-protocol/SKILL.md`: the widened rule (D5) and the new check. `docs/maintaining.md`: the `release` environment with a required reviewer is a prerequisite that the rc gate verifies.

## 6. CI, release and observability (`devops-release`)

No workflow change. Version impact: patch, v2.10.1 (engine bump for the new command). The check runs locally at the rc gate, not in CI, since the workflow token cannot read environment protection rules.

## 7. Security, migration and rollback

Read-only API call with the user's own `gh` session; no new secret. Projects without a protected environment start failing the rc gate: that is the intended change, and the message names the setting to fix. Rollback: revert the pull request.

## 8. Execution steps and milestones

1. Check and command: `release_environments` and `check_release_environments`, with unit and parity tests.
2. Gate: `verify_gate rc` calls it; rc golden files recorded.
3. Rules and documentation: D5 in both skills, `docs/maintaining.md`, manual test step.
4. Version bump to 2.10.1 and engine bump; checks and pull request.
