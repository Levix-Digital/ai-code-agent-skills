---
plan_id: ai_code_agent_skills_the_dev_cycle_command_and_project_onboarding
project: ai-code-agent-skills
title: The /dev-cycle command and project onboarding
version: v2.3.0
status: in_progress
author: Levi Silveira
created_at: '2026-10-23T14:43:16'
updated_at: '2026-10-23T14:47:31'
session_id: null
tags:
- dev-cycle
- onboarding
- speckit
- dev-cycle-protocol
supersedes: null
superseded_by: null
parent_plan_id: null
milestones:
- name: M1 Diagnosis
  done: true
  done_at: '2026-10-23'
  by: Levi Silveira
- name: M2 Assisted setup
  done: false
- name: M3 Spec Kit in onboarding
  done: false
- name: M4 The dev-cycle command
  done: false
- name: M5 Documentation and release
  done: false
status_log:
- at: '2026-10-23T14:43:16'
  by: Levi Silveira
  status: active
  note: Plan created
- at: '2026-10-23T14:45:11'
  by: Levi Silveira
  status: active
  note: Approved by the user
- at: '2026-10-23T14:47:31'
  by: Levi Silveira
  note: 'Milestone ''M1 Diagnosis'' completed: _onboard.py rules (D2-D4, new vs existing, setup items, Spec Kit states) and onboard.py diagnose (repository, config, branches, baseline, plans, release candidate, bugs, roadmap, Spec Kit); 15 tests'
---

# The /dev-cycle Command and Project Onboarding

## 1. Goal and decisions

The protocol should guide the user, not the other way around. Today the user has to know which phase to start from, which scripts to run and which pieces are missing in a repository (configuration, integration branch, product baseline, Spec Kit). A repository that never used the protocol gets no help at all.

This plan adds **one command**, `/dev-cycle`, that starts every session: it diagnoses the repository, proposes where to start, sets up what is missing after the user confirms, and hands over to the right phase.

Decisions:

- **D1 — One command.** `/dev-cycle ["<text>"]` is the only entry point besides `/backlog`. The protocol asks; the user does not need to remember commands or phases. No separate `check` command.
- **D2 — The diagnosis always comes first, sized to what it finds.**
  - Nothing pending competes with the request: one line ("current release v0.2.0, no blocking bugs, no plan in progress") and the agent goes straight on.
  - Something competes (a plan in progress, a release candidate waiting for acceptance, an open blocking bug): the agent shows it and asks, with numbered options and a recommendation.
  - Setup items missing (a repository that never used the protocol): one "initial setup" list in the recommended order, confirmed at once ("all") or item by item, instead of loose questions.
- **D3 — What the text means depends on the repository.**
  - New or empty repository (no product baseline and no meaningful code): the text is the product idea and the agent starts Phase 0 (`product-inception`). Without text it asks for the idea.
  - Existing repository: the text is a request and, after the diagnosis, the agent starts Phase 1 (full or short path as usual). Without text it shows the diagnosis and the options.
- **D4 — Recommended order of the options** (the first one that applies is the recommendation):
  1. resume the plan in progress (milestones left);
  2. finish the acceptance of a pending release candidate;
  3. plan the open blocking bugs (bugs first);
  4. initial setup items that are missing (D5, D6);
  5. adopt a product baseline (Phase 0, adoption mode) when there is none;
  6. the next open items of the current release (`roadmap.py current`);
  7. the user's request, when there is one, takes its place in this list: a request never silently overrides items 1 to 3.
- **D5 — Assisted configuration, after confirmation.** When `.dev-cycle.json` is missing, the agent proposes one filled from detection (project name, languages, test commands, documentation paths, branches, version files it can find) and shows it; it is written only after the user confirms or adjusts it. An existing file is never overwritten: only missing keys are proposed. A missing integration branch (`devops.integration_branch`, default `dev`) is proposed the same way and created from the production branch after confirmation.
- **D6 — Spec Kit is at most one yes/no question, asked once.**

  | Found | Action |
  |---|---|
  | No `.specify/` and no decision in `.dev-cycle.json` | Ask once whether to use Spec Kit for the detail of features, with a short explanation and a recommendation. The answer is recorded (`speckit.enabled: true` or `false`), so a "no" is not asked again. "Yes" runs `speckit_setup.py init` and `constitution.py generate`. |
  | `.specify/` exists but the protocol does not manage it (Spec Kit used without the protocol) | Offer the adoption: `speckit_setup.py adopt` pins the installed version when it passes the compatibility check (otherwise proposes the upgrade to the verified version), installs the `dev-cycle` preset and sets `feat/NNN-slug` branches. Existing `specs/` stay as they are. |
  | Enabled with problems (`speckit_setup.py status`: version drift, preset missing, branch prefix, stale constitution) | Show them in the diagnosis and offer the fix. |
  | `uv` missing | Explain how to install it, or continue without Spec Kit; the question comes back on a later run. |

- **D7 — A hand-written constitution is never lost.** An adopted Spec Kit repository has a constitution written by hand, while the protocol's is generated. `constitution.py generate` refuses to replace a hand-written constitution (anything that is neither generated by it nor Spec Kit's untouched template) unless `--replace-hand-written` is given; it then keeps a copy (`.specify/memory/constitution.hand-written.md`). Before that, the agent shows each principle of the old constitution and proposes where it goes: constraints and non-functional requirements to `TECH_SPEC.md`, decisions to ADRs, general design rules as a suggestion for `software-design`; the user confirms.
- **D8 — The diagnosis is a script.** `dev-cycle-protocol/onboard.py` reads the repository and prints the state, the findings and the ordered recommendations (text or JSON), so the behavior is deterministic and tested; the `dev-cycle` skill is a thin router like `backlog`. Nothing is changed by the diagnosis; every change happens after the user's choice, through the existing scripts (`speckit_setup.py`, `constitution.py`) and two new subcommands of `onboard.py` (`propose-config`, `write-config`).

Alternatives considered:

- **Two commands (`/dev-cycle` and `/dev-cycle-check`)**: more to remember, against D1. Rejected.
- **Go straight to Phase 1 when there is a request**: hides a plan in progress or an open blocking bug. Rejected (D2).
- **Write the configuration without asking**: a wrong guess would silently change how every gate behaves. Rejected (D5).
- **Regenerate an adopted constitution directly**: loses the project's own principles. Rejected (D7).

## 2. Design (`software-design`)

### Components

| Unit | Responsibility |
|---|---|
| `dev-cycle/SKILL.md` (new skill, the `/dev-cycle` command) | Reads `$ARGUMENTS`, runs `onboard.py --json`, applies D2 to D4, asks the user, then hands over to `product-inception` (Phase 0) or `dev-cycle-protocol` (Phase 1 or the chosen item); runs the setup steps the user confirmed |
| `dev-cycle-protocol/onboard.py` | `diagnose [--request "<text>"] [--json]`: state, findings and ordered recommendations; `propose-config [--json]`: the `.dev-cycle.json` proposal (new file or missing keys); `write-config --file <json>`: writes it, refusing to overwrite existing keys |
| `dev-cycle-protocol/_onboard.py` | Pure functions: the state model, the recommendation rules (D4), the proportional summary (D2), the config proposal and merge (D5); no I/O, so the rules are unit-tested |
| `dev-cycle-protocol/speckit_setup.py` | New `adopt` subcommand (D6): pin the installed version after the check, preset, branch prefix, `speckit` section |
| `dev-cycle-protocol/constitution.py` | Hand-written guard and `--replace-hand-written` with a backup (D7); `check` reports a hand-written constitution as such |
| `dev-cycle-protocol/project_config.py` | Tells "speckit.enabled not set" (undecided) from an explicit `false` (raw file lookup) |

### State read by the diagnosis

| Area | Source |
|---|---|
| Repository | Git present, commits, current branch, uncommitted changes, meaningful code (source files outside docs and tests) |
| Configuration | `.dev-cycle.json` present and valid (`project_config.py`), detection (`inspect_project.py`) |
| Branches | Integration and production branches, local or remote |
| Product baseline | Product documents, architecture style, technical docs index, backlog epics, roadmap |
| Work in progress | Active and in-progress plans with milestone progress (`plan-memory`) |
| Release | The latest release candidate run in the manual test suite with tests not run or failed, and no final tag for its version yet |
| Bugs | Open blocking bugs (`backlog.blocking_severities`) |
| Roadmap | Current release and its open items |
| Spec Kit | `speckit_setup.py status`, `.specify/` without protocol management, `uv` available, decision recorded or not |

Errors: an unreadable file (backlog, roadmap, suite, plan) becomes a finding with the file and the reason, never a crash; the diagnosis always completes.

Deliberately not abstracted: no plugin system for new checks; the areas above are plain functions.

## 3. Affected surfaces and files

- New skill `dev-cycle/` (`SKILL.md`), added to `.gitignore` (allowlist) and to the plugin.
- New: `dev-cycle-protocol/onboard.py`, `dev-cycle-protocol/_onboard.py`, tests.
- Changed: `dev-cycle-protocol/speckit_setup.py` (`adopt`), `constitution.py` (guard), `project_config.py` (undecided vs. false), `dev-cycle-protocol/SKILL.md` ("Starting a session"), `product-inception/SKILL.md` (entry from `/dev-cycle`).
- Changed: `README.md` (Usage starts with `/dev-cycle`), `docs/spec-kit.md` (adoption), `docs/configuration.md`, `tests/MANUAL_TESTS.md`, `.claude-plugin/plugin.json` and `marketplace.json`.
- Surfaces: a new command, two new script subcommands, a new constitution flag; all additive.

## 4. Tests (`quality-assurance`)

Automated (pytest):
- Recommendation rules: each item of D4 in isolation and the order when several apply; a request never overrides items 1 to 3; a request with nothing pending gives a one-line summary.
- New or empty repository with text: Phase 0 with the idea; existing repository with text: Phase 1 after the diagnosis.
- State detection on fixture repositories: never-used repository (all setup items listed in order), plan in progress, pending release candidate (and not pending once the final tag exists), blocking bugs, current release items, unreadable backlog reported as a finding.
- Config proposal from detection; merge proposes only missing keys and `write-config` refuses to overwrite.
- Spec Kit states of D6: undecided, explicit false (not asked again), `.specify/` not managed (adoption offered), enabled with problems, `uv` missing.
- `speckit_setup.py adopt` (offline parts): pins the installed version, writes the section, sets the prefix; refuses when Spec Kit is not initialized; the compatibility check runs online only with `SPECKIT_ONLINE_TESTS=1`.
- Constitution guard: Spec Kit's template is replaced; a hand-written one is refused without the flag and backed up with it; `check` names it as hand-written.

Manual (`tests/MANUAL_TESTS.md`):
- `/dev-cycle "<idea>"` in an empty folder starts Phase 0.
- `/dev-cycle` in a repository that never used the protocol: initial setup list, confirmed config written, `dev` created, Spec Kit question asked once.
- `/dev-cycle "<request>"` with a plan in progress: the agent asks before starting the request.
- `/dev-cycle` in a repository that used Spec Kit alone: adoption with the hand-written constitution moved into the sources.

Acceptance criteria: every manual test above passes; existing tests and gates behave as before.

## 5. Documentation (`architecture-docs`, `user-docs`)

- New `dev-cycle/SKILL.md`; `dev-cycle-protocol/SKILL.md` gets "Starting a session (`/dev-cycle`)".
- README: the Usage section starts with `/dev-cycle`; the skill table lists the command.
- `docs/spec-kit.md`: adopting a repository that already uses Spec Kit; `docs/configuration.md`: explicit `speckit.enabled: false` as a recorded decision.

## 6. CI, release and observability (`devops-release`)

No workflow changes; the new tests join the pytest command. Version: plugin `2.2.0` → `2.3.0` (new command, backward compatible). The diagnosis prints what it read and from where, so a wrong recommendation can be traced.

## 7. Security, migration and rollback

- The diagnosis only reads. Writes (configuration, branch, Spec Kit, constitution) happen after the user's confirmation and never overwrite: configuration keys are only added, a hand-written constitution is backed up.
- No migration: existing projects keep working; `/dev-cycle` is optional.
- Rollback: revert the pull request.

## 8. Execution steps and milestones

- **M1 Diagnosis**: `_onboard.py` state and rules, `onboard.py diagnose`, tests.
- **M2 Assisted setup**: `propose-config`, `write-config`, integration branch proposal, undecided vs. false in `project_config.py`, tests.
- **M3 Spec Kit in onboarding**: `speckit_setup.py adopt`, the constitution guard, the D6 states in the diagnosis, tests.
- **M4 The /dev-cycle command**: `dev-cycle/SKILL.md`, protocol and product-inception entries, manual tests.
- **M5 Documentation and release**: README, guides, plugin 2.3.0, pull request into `main`.
