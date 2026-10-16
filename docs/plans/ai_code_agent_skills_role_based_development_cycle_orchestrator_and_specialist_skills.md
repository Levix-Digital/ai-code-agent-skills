---
plan_id: ai_code_agent_skills_role_based_development_cycle_orchestrator_and_specialist_skills
project: ai-code-agent-skills
title: 'Role-Based Development Cycle: Orchestrator and Specialist Skills'
version: v2.0.0
status: in_progress
author: Levi Silveira
created_at: '2026-10-16T13:41:24'
updated_at: '2026-10-16T13:45:46'
session_id: null
tags:
- dev-cycle
- roles
- qa
- devops
- user-docs
- software-design
supersedes: null
superseded_by: null
parent_plan_id: null
milestones:
- name: M1 Project configuration schema
  done: true
  done_at: '2026-10-16'
  by: Levi Silveira
- name: M2 software-design skill
  done: false
- name: M3 user-docs skill
  done: false
- name: M4 quality-assurance skill
  done: false
- name: M5 devops-release skill
  done: false
- name: M6 dev-cycle-protocol as orchestrator
  done: false
- name: M7 Repository plumbing
  done: false
- name: M8 Apply to any-context
  done: false
status_log:
- at: '2026-10-16T13:41:24'
  by: Levi Silveira
  status: active
  note: Plan created
- at: '2026-10-16T13:45:46'
  by: Levi Silveira
  note: 'Milestone ''M1 Project configuration schema'' completed: project_config.py loader with defaults, legacy-key mapping, validation and migration; inspect_project, verify_gate, plan-memory and architecture-docs read it; 9 tests'
---

# Role-Based Development Cycle: Orchestrator and Specialist Skills

## 1. Goal and Architecture Decisions

Restructure the skills so the development cycle works like a team of specialists coordinated by one orchestrator. Each role is a small skill that owns one kind of artifact and ships a script that verifies it, because rules without a checker degrade over time (the README and `MANUAL_TESTS.md` of any-context grew into changelogs under the current rules).

Decisions agreed with the user:

| # | Decision |
|---|---|
| D1 | `dev-cycle-protocol` becomes the **orchestrator**: phases, gates and which role skill runs in each phase. It keeps no role-specific rules of its own. |
| D2 | Role skills: **software-design** (architect), **architecture-docs** (existing, technical docs), **quality-assurance** (QA), **devops-release** (DevOps), **user-docs** (technical writer), **plan-memory** (existing, planning), plus the existing `code-review` capability as reviewer. |
| D3 | Universal engineering principles in every project: modularity, clear separation of responsibilities, SOLID, Clean Code, explicit error handling. Design patterns only when they solve a real problem. Counterweight: the simplest solution that keeps responsibilities separate; an abstraction needs a second implementation, a test seam or an external dependency to isolate. |
| D4 | The architecture style (hexagonal, layered, MVC, pipeline, ...) is a **per-project setting**; hexagonal is no longer imposed. |
| D5 | `.dev-cycle.json` stays at the repository root and becomes **the project configuration for all skills**, with one section per skill. Existing keys keep working. |
| D6 | README: a dedicated `user-docs` skill; end-user content only; edited in place; no version sections; **always English**. |
| D7 | Manual tests: one **curated** suite organized by feature, edited in place, obsolete tests removed, each test with `Result: [ ] Pass [ ] Fail [ ] Blocked` and a notes line; a script reports failures. Replaces the cumulative "never compact" rule. |
| D8 | Branching: `main` (released), `dev` (integration), short-lived `feat/`, `fix/`, `docs/`, `chore/` branches from `dev`, hotfix branches from `main`. CI runs on pull requests into **both** `dev` and `main`. |
| D9 | Releases use **release candidates**: tag `vX.Y.Z-rc.N` from `dev` publishes a pre-release; the user installs it and runs the curated manual tests; failures are fixed in `dev` and produce `rc.N+1`. |
| D10 | **The user's merge into `main` is the release gate.** The agent never merges into `main`, never tags a final release by hand and never publishes without that merge. The merge triggers the final tag `vX.Y.Z` and the release; the agent follows it until the assets are verified. |
| D11 | Two paths: **full** (features, refactors, schema or release-affecting changes) and **short** (small fixes, docs-only, typos), chosen by the orchestrator from the change type. |
| D12 | Telemetry and observability are a planning item and a test item, owned by `devops-release`, scaled to the project type (CLI: structured logs and diagnostics; services: metrics, traces, health checks). |
| D13 | Everything the skills write into a project is English, except answers to the user, which follow the user's language. |

## 2. Surface Parity Mapping

The "surfaces" are the agents that load the skills: Claude Code (`~/.claude/skills`), cross-agent (`~/.agents/skills`), Gemini (`~/.gemini/config/skills`) and Claude Code cloud sessions (plugin). `install.ps1` / `install.sh` link every skill folder automatically; the plugin manifest lists skills explicitly and must be updated.

## 3. Affected Files

Skills repository (`~/.ai-code-agent-skills`):

- `dev-cycle-protocol/SKILL.md` (rewrite as orchestrator), `verify_gate.py` (new gates), `inspect_project.py` (read new config sections).
- New `software-design/SKILL.md` and `software-design/styles/*.md` (one short guide per architecture style).
- New `user-docs/SKILL.md`, `user-docs/check_readme.py`, `user-docs/README_TEMPLATE.md`.
- New `quality-assurance/SKILL.md`, `quality-assurance/manual_tests.py`, `quality-assurance/MANUAL_TESTS_TEMPLATE.md`.
- New `devops-release/SKILL.md`, `devops-release/check_ci_policy.py`, `devops-release/release_status.py`, `devops-release/templates/` (workflow snippets: PR CI, RC pre-release, tag-on-merge).
- `architecture-docs/SKILL.md` (link to software-design; read config section), `plan-memory/SKILL.md` (plan template references roles).
- `.gitignore` (allowlist new folders and `docs/`), `.claude-plugin/plugin.json` (skills list, version 2.0.0), `README.md` (skill table, configuration reference).

any-context (separate PR, milestone 8): `README.md`, `tests/MANUAL_TESTS.md`, `.github/workflows/e2e-tests.yml`, `.github/workflows/release.yml`, a new tag-on-merge job, `.dev-cycle.json`.

## 4. Documentation Plan

- Each new skill has its `SKILL.md` with: purpose, when the orchestrator calls it, the artifact it owns, rules, commands, and the checker it ships.
- Skills repository `README.md`: new skill table, a "Roles and phases" section with the phase diagram, and the full `.dev-cycle.json` reference.
- The `.dev-cycle.json` schema (D5):

```json
{
  "project_name": "any-context",
  "architecture": { "style": "hexagonal", "core_path": "crates/any-context-core-rs", "surfaces": ["cli", "tui"] },
  "test_commands": ["cargo test --workspace"],
  "docs": { "user_doc": "README.md", "tec_doc": "TECDOC.md" },
  "architecture_docs": { "max_lines": 800, "language": "en" },
  "tests": { "manual_tests": "tests/MANUAL_TESTS.md" },
  "devops": {
    "integration_branch": "dev",
    "production_branch": "main",
    "releases_repo": "Levix-Digital/any-context-releases",
    "release_candidates": true,
    "version_files": ["Cargo.toml", "version.txt"],
    "observability": "cli"
  },
  "drift_rules": []
}
```

Old flat keys (`core_path`, `surfaces`, `doc_files`, `releases_repo`, `canonical_datastore`) keep being read as fallbacks.

## 5. Automated Testing Strategy

Every new script gets unit tests with fixtures, run by a small `tests/` folder per skill (`python -m pytest <skill>/tests`):

- `check_readme.py`: version headings, changelog-style sections, internal paths and implementation detail markers, non-English text, missing standard sections, length limit.
- `manual_tests.py`: parsing of tests and results, `--failed`, `--summary`, `--run-sheet --feature <x>`, rejection of malformed result lines (two boxes checked).
- `check_ci_policy.py`: workflow triggers (PR into integration and production branches), pre-release handling of `-rc` tags, tag-on-merge job, required permissions.
- `verify_gate.py`: new gates against fixture repositories.

The scripts are run against the any-context repository as an integration check before milestone 8.

## 6. Manual Test Plan

Recorded with the new QA format in the skills repository (`tests/MANUAL_TESTS.md`): run a full-path cycle and a short-path cycle on a sample project, cut an RC, mark results, and confirm the orchestrator routes failures back to fixes and blocks the release gate.

## 7. Secrets, Migration and Rollback

- No secrets involved; `verify_gate.py` keeps the secret hygiene check.
- Migration: config reading is backward compatible; projects without new sections keep working with defaults.
- Rollback: each milestone is its own commit/PR in the skills repository; reverting one restores the previous skill text. The plugin version moves to 2.0.0 only when milestone 7 lands.

## 8. Step-by-Step Execution

Work in the skills repository on `feat/role-based-cycle`, one commit per milestone, pull request into `main` (the skills repository has no `dev`).

### Milestone 1: Project configuration schema
Define the `.dev-cycle.json` sections and fallbacks; update `inspect_project.py` and `verify_gate.py` to read them; document the schema.

### Milestone 2: software-design skill
Principles (D3), architecture style guides (hexagonal, layered, MVC, pipeline, modular monolith), how to choose and record the style in `.dev-cycle.json` and an ADR, and a review checklist used before the pull request.

### Milestone 3: user-docs skill
README structure (what it is and for whom, installation, quick start, usage, configuration, troubleshooting, links to technical docs, license), rules (edit in place, no version sections, no implementation detail, English only, size limit), `README_TEMPLATE.md` and `check_readme.py`.

### Milestone 4: quality-assurance skill
Automated test strategy per stack (unit, integration, regression test for every bug fix), what stays manual, the curated manual suite format (D7), curation rules (review affected features on every change, remove obsolete tests, edit in place), RC run sheets, `manual_tests.py` (summary, failures, run sheet, reset results for a new RC).

### Milestone 5: devops-release skill
Branching model (D8), conventional commits and branch names, branch protection, CI policy (PR checks into `dev` and `main`, concurrency per pull request), the RC flow (D9), version bump in `dev` before the RC, tag-on-merge and release follow-through (D10), the GITHUB_TOKEN limitation (tags pushed with it do not trigger other workflows: the release runs in the same workflow or uses a dedicated token), hotfix flow, rollback, observability checklist (D12), `check_ci_policy.py`, `release_status.py` (RC and final release asset verification with `gh`), workflow templates.

### Milestone 6: dev-cycle-protocol as orchestrator
New phase model with the roles called in each phase:

| Phase | Roles |
|---|---|
| 1. Discovery | orchestrator, plan-memory (prior plans), software-design (current architecture) |
| 2. Plan | plan-memory, software-design, quality-assurance (acceptance criteria and test plan), devops-release (CI, release, observability impact), user-docs (README impact) |
| 3. Approval gate (user) | orchestrator |
| 4. Implementation on a feature branch | software-design principles |
| 5. Automated tests | quality-assurance |
| 6. Documentation | architecture-docs, user-docs |
| 7. Review and pull request into `dev` | code review, devops-release (CI green on the pull request) |
| 8. Release candidate | devops-release (bump, `rc` tag, pre-release verified), quality-assurance (run sheet) |
| 9. User acceptance gate (manual tests) | user marks results; failures go back to phase 4 |
| 10. Release gate: user merges `dev` into `main` | devops-release follows the final release until assets are verified; plan-memory closes milestones |

Short path: phases 1, 4, 5, 6 (only if affected), 7; no plan file, no RC. Remove the rules that conflict with this plan (cumulative tests, autonomous merge and release, mandatory hexagonal architecture). `verify_gate.py` gains `--gate rc` (manual test results all Pass for the run sheet) and keeps gate 3 and gate 8.

### Milestone 7: Repository plumbing
`.gitignore` allowlist, plugin manifest (new skills, version 2.0.0), README, pytest run for all skills, manual tests of the skills repository.

### Milestone 8: Apply to any-context (separate pull request in that repository)
- Rewrite `README.md` with `user-docs` until `check_readme.py` passes.
- Curate `tests/MANUAL_TESTS.md` with `quality-assurance` against the current code (remove obsolete scenarios, merge duplicates, organize by feature, add result lines).
- CI: run the test suite on pull requests into `dev`.
- Release: `-rc` tags publish GitHub pre-releases (the updater's "latest" lookup ignores pre-releases, so normal users never get an RC); merging into `main` creates the final tag and release.
- Move `.dev-cycle.json` to the new sections.
- Validate with `check_ci_policy.py`, `check_readme.py`, `manual_tests.py --summary` and `check_docs.py`.
