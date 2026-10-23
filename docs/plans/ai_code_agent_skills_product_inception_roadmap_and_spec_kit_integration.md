---
plan_id: ai_code_agent_skills_product_inception_roadmap_and_spec_kit_integration
project: ai-code-agent-skills
title: Product inception, roadmap and Spec Kit integration
version: v2.2.0
status: active
author: Claude
created_at: '2026-10-23T13:51:22'
updated_at: '2026-10-23T13:51:22'
session_id: null
tags:
- product-inception
- roadmap
- epics
- speckit
- dev-cycle-protocol
supersedes: null
superseded_by: null
parent_plan_id: null
milestones:
- name: M1 Spec Kit spike and compatibility check
  done: false
- name: M2 Epics and roadmap
  done: false
- name: M3 Product inception and gate 0
  done: false
- name: M4 Spec Kit preset and setup
  done: false
- name: M5 Generated constitution
  done: false
- name: M6 Protocol integration
  done: false
- name: M7 Documentation and release
  done: false
status_log:
- at: '2026-10-23T13:51:22'
  by: Claude
  status: active
  note: Plan created
---

# Product Inception, Roadmap and Spec Kit Integration

## 1. Goal and decisions

The cycle starts today when a change is already chosen (Phase 1, Discovery). Three things are missing:

- **The birth of a product.** A new product starts with an idea in a prompt, followed by rounds of questions and suggestions that end up in documents of different kinds (business, architecture, technical specification). This happens without a defined process, without fixed documents and without an approval gate.
- **A level between the backlog and the plan.** The backlog captures work and the plan details one change, but nothing says which items make up which release. There is no roadmap and no way to group the items of the complete ("ideal") product into epics.
- **A strong executor for the detail of a change.** Phases 1, 2 and 4 rely on one plan template. [GitHub Spec Kit](https://github.com/github/spec-kit) does exactly this work in depth (specification, clarification, technical plan, tasks, consistency analysis, implementation), is agent-agnostic like this repository, but has nothing for product inception, roadmap, documentation, release candidates, manual acceptance or releases.

The protocol stays the **guide** (phases, gates, roles, the user's decisions); Spec Kit becomes the **executor** of the detail inside the phases where it takes part.

Decisions:

- **D1 — Phase 0, Product Inception.** A new role skill, `product-inception`, owns a defined process from an idea to an approved product baseline, run once per product (or to adopt an existing project into the cycle):
  1. **Idea**: the user's prompt.
  2. **Interview**: rounds of at most five questions with suggestions (problem, users, value, constraints, out of scope, risks); answers and open questions are recorded as the conversation goes, never lost.
  3. **Business document** `docs/product/VISION.md`: problem, target users and personas, value proposition, business model, success metrics, scope and non-goals, risks.
  4. **Architecture**: the architecture style (`software-design`, set in `.dev-cycle.json`), the first `docs/architecture` pages and the ADRs of the founding decisions (`architecture-docs`).
  5. **Technical specification** `docs/product/TECH_SPEC.md`: stack, platforms, non-functional requirements (performance, security, privacy, availability, accessibility), integrations, data model overview, constraints.
  6. **Constitution** (only when Spec Kit is enabled): generated from the sources above (D7).
  7. **Product backlog**: the complete product, as epics and their features (D2).
  8. **Roadmap**: releases from the MVP onwards, each with its goal and its backlog items (D3).
  9. **Gate 0 (user)**: the user approves the product baseline; the cycle then starts with the first roadmap items.

  Each document is shown to the user and approved before the next one starts; a later answer that changes an approved document reopens it (edited in place, like every living document). The adoption mode writes the same documents from an existing code base and asks the user only what the code cannot answer.
- **D2 — Epics in the backlog.** A new item type `epic` (`EPIC-NNN`, section `## Epics`, with priority). Features, improvements and adjustments get an optional `Epic` field. An epic is never promoted itself: its items are. `validate` checks that the referenced epic exists and warns about an open epic whose items are all closed.
- **D3 — The roadmap.** One file, `docs/ROADMAP.md` (`roadmap.file` in `.dev-cycle.json`), owned by `plan-memory` like the backlog. Releases are ordered sections named by their target version, each with a goal, a status (`planned`, `in_progress`, `released`) and the list of backlog ids it delivers. **The roadmap is the single source of the item-to-release assignment**: backlog items do not repeat it. Releases are versions, so they meet the existing release flow (`devops-release`, gate 8). Discovery (Phase 1) shows the open items of the current release next to the bugs that come first; the user still decides what to promote.
- **D4 — Spec Kit is optional, per project.** A `speckit` section in `.dev-cycle.json` (`enabled`, `version`, `integration`). Without it, or with `enabled: false`, the cycle works exactly as today. Spec Kit is used only in the **full path**, for **feature plans**; the short path never uses it. Bug plans keep the bug template (reproduction, root cause, regression test); whether Spec Kit's own bug workflow fits is evaluated in M1 and decided by the user, not assumed.
- **D5 — Spec Kit is the detail; the plan file is the envelope.** Spec Kit's artifacts (`specs/NNN-slug/spec.md`, `plan.md`, `research.md`, `data-model.md`, `contracts/`, `tasks.md`) hold the detail. The `plan-memory` plan file keeps what the cycle needs and Spec Kit does not have: frontmatter (status, milestones, lineage, backlog items) plus a short body with the goal, a link to the spec folder (new frontmatter field `spec_dir`) and the protocol sections Spec Kit does not cover. Search and drift detection index the spec folder's files with the plan, so no detail is outside the index. Zero Compaction still holds: nothing is summarized away; the detail simply lives in the spec folder.
- **D6 — Branch names stay `devops-release`'s, through a Spec Kit preset.** Work branches keep the form `<type>/NNN-slug` (for example `feat/001-photo-upload`): the type first, the Spec Kit feature number after it. Spec Kit today finds the feature from a numeric (or timestamp) prefix of the branch name, so the protocol ships a **Spec Kit preset** (`dev-cycle-protocol/speckit-preset/`), installed as a local preset. It **wraps** (does not replace) Spec Kit's scripts: branch creation produces `<type>/NNN-slug` and feature resolution strips the `<type>/` prefix before calling the original script. When Spec Kit ships configurable branching ([github/spec-kit#2047](https://github.com/github/spec-kit/issues/2047)), the branch part of the preset becomes configuration. Rejected: `001-feat/slug` (spec folder nested under the slash, breaks the type-first convention) and per-command environment variables without a preset (depends on the agent remembering them every time).
- **D7 — The constitution is generated, never edited by hand.** `.specify/memory/constitution.md` is built from the existing sources of truth: `software-design`'s universal principles and the guide of the project's architecture style, the protocol's principles, and the product's decisions (`TECH_SPEC.md` constraints and non-functional requirements, accepted ADRs). It carries a "generated, do not edit" header with a hash of its sources. A principle changes in its source (for example an ADR that changes the architecture style) and the constitution is regenerated. Gate 3 fails when it is out of date. Spec Kit's "Constitution Check" in its plan then enforces the protocol's rules inside Spec Kit's own flow.
- **D8 — The Spec Kit version is pinned per project, and adaptation is explicit.**
  - The project's version is `speckit.version` in `.dev-cycle.json` (Spec Kit does not record it in the project). After `init`, Spec Kit's commands run from the project's own files; the CLI is needed only for `init` and `upgrade`, and the protocol always calls the exact version (`uvx --from git+https://github.com/github/spec-kit.git@vX.Y.Z specify ...`), never the machine's global install.
  - The skills repository records the last **verified** version (`dev-cycle-protocol/speckit-preset/compat.json`).
  - For a new project, the protocol checks the latest release with an automatic compatibility check (`init` in a temporary folder, the commands, paths, templates and branch resolution the protocol depends on, the preset applied). If it passes, the project pins it and the verified version moves forward. If it fails, the project pins the last verified version and the agent writes an adaptation document in the **skills repository** (`docs/protocol-updates/speckit-vX.Y.Z.md`: what diverged, what the protocol must change) and captures it in this repository's backlog. When the skills repository cannot be written (for example a cloud session of another project), the agent gives the document to the user instead.
  - Existing projects never upgrade on their own: `upgrade` is an explicit action the user asks for, guarded by the same check.
- **D9 — Spec Kit inside the phases.**

  | Phase | With Spec Kit |
  |---|---|
  | 0. Inception | the constitution is generated at the end (D7) |
  | 1. Discovery | items are promoted (envelope plan in `draft`); the work branch is created by `/speckit.specify` (through the preset), then `/speckit.clarify` until no open question is left |
  | 2. Plan | `/speckit.plan` (the preset's plan template adds the protocol sections: manual tests, documentation, CI and release, observability, security, migration and rollback), `/speckit.tasks`, `/speckit.analyze`; the envelope is saved with `spec_dir` and milestones from the phases of `tasks.md` |
  | 3. Approval | the same gate, plus the Spec Kit checks (D10) |
  | 4–5. Implementation, tests | `/speckit.implement` executes `tasks.md` under the protocol's rules (regression test per bug, never move on with a failing test); milestones are marked as phases of `tasks.md` complete |
  | 6–10 | unchanged; Phase 10 also marks the release `released` in the roadmap |

  With Spec Kit the work branch exists from Phase 1 (Spec Kit writes the spec on it), not from Phase 4; the spec and plan are committed on it after approval, as today.
- **D10 — Gates.**
  - New **gate 0** (`verify_gate.py --gate 0`), before asking for the baseline approval: `VISION.md` and `TECH_SPEC.md` pass `check_product.py` (required sections, no template placeholders), the architecture style is set, the architecture index exists, the backlog validates and has at least one epic, the roadmap validates and has at least one release; with Spec Kit, the constitution is current.
  - **Gate 3** with Spec Kit: the envelope's `spec_dir` exists with `spec.md`, `plan.md` and `tasks.md`; no `[NEEDS CLARIFICATION]` marker is left; the constitution is current; the branch name follows D6.
  - **Gate 8**: the roadmap validates; items of the release being shipped that are still open are listed, and the user decides whether they move to the next release.

Alternatives considered:

- **Spec Kit for the inception**: Spec Kit specifies one feature at a time; it has no product vision, epics or roadmap. Rejected; inception is the protocol's.
- **Spec Kit replacing `plan-memory`**: would lose the backlog, milestones, lineage, bugs first and drift detection. Rejected; the envelope keeps both (D5).
- **Release on each backlog item instead of a roadmap file**: the assignment would be scattered and the roadmap unreadable. Rejected (D3).
- **Spec Kit mandatory**: makes every project depend on an external, fast-moving tool and `uv`. Rejected (D4).
- **Replacing Spec Kit's scripts in the preset**: a copy that drifts on every Spec Kit release. Rejected in favor of wrapping (D6), unless M1 proves wrapping cannot work.

## 2. Design (`software-design`)

### Components

| Unit | Responsibility |
|---|---|
| `product-inception/SKILL.md` | The Phase 0 process (D1), the interview rules, adoption mode, gate 0 |
| `product-inception/templates/VISION_TEMPLATE.md`, `TECH_SPEC_TEMPLATE.md` | The two product documents |
| `product-inception/check_product.py` | Validates both documents: required sections, no placeholders, line limit (pure checks over text, like `check_readme.py`) |
| `plan-memory/_backlog.py`, `backlog.py` | `epic` type and the `Epic` field; `add --epic`, `set --epic`, `list --epic`; validation rules of D2; `promote` refuses an epic |
| `plan-memory/_roadmap.py` | Parse, render and validate `docs/ROADMAP.md` (pure functions over text, round-trip safe) |
| `plan-memory/roadmap.py` | CLI: `add-release`, `assign <ID...> --release V`, `unassign`, `move-release`, `set-status`, `show`, `current` (open items of the current release), `validate`; atomic writes, never commits |
| `plan-memory/templates/ROADMAP_TEMPLATE.md` | Initial file |
| `plan-memory/_store.py`, `save_plan.py`, `get_plan.py`, `search_plans.py`, `detect_drift.py`, `rebuild_index.py` | `spec_dir` field (`save_plan.py --spec-dir`); the index includes the spec folder's Markdown files; `get_plan.py` prints them after the envelope |
| `dev-cycle-protocol/speckit-preset/` | `preset.yml`; script wrappers for branch creation and feature resolution, in Bash and PowerShell; the plan template with the protocol sections (append strategy); the constitution template; `compat.json` with the verified version |
| `dev-cycle-protocol/speckit_setup.py` | `check --version V` (the compatibility check in a temporary folder), `latest`, `init` (pinned `specify init` with the configured integration, `specify preset add --dev`, writes `speckit.version`), `upgrade --to V` (check, then integration and preset upgrade), `status` |
| `dev-cycle-protocol/constitution.py` | `generate` and `check` (D7): collects the sources, renders the constitution with the source hash |
| `dev-cycle-protocol/project_config.py` | New sections `roadmap` (`file`) and `speckit` (`enabled`, `version`, `integration`), defaults and validation |
| `dev-cycle-protocol/verify_gate.py` | Gate 0; Spec Kit checks in gate 3; roadmap checks in gate 8 (D10) |
| `dev-cycle-protocol/inspect_project.py` | Shows the product baseline state, the roadmap's current release and the Spec Kit state (enabled, pinned version, `uv` present, constitution current) |

Data flow with Spec Kit: backlog item → `roadmap.py current` (Discovery) → `backlog.py promote` (envelope `draft`) → `/speckit.specify` and `/speckit.clarify` (branch `feat/NNN-slug`, `spec.md`) → `/speckit.plan`, `/speckit.tasks`, `/speckit.analyze` → `save_plan.py --spec-dir` (envelope `active`) → gate 3 → `/speckit.implement` → Phases 5 to 10 as today.

Errors: every script exits non-zero with a message naming the file, the id or the rule. `speckit_setup.py` reports a missing `uv` with the install command and never falls back to a global `specify`. A failed compatibility check prints the divergences and the path of the adaptation document draft.

Dependencies: `uv` (only when Spec Kit is enabled) and network access to GitHub for `init`, `check` and `upgrade`. No new Python packages.

Deliberately not abstracted: no generic "external spec tool" interface (one tool, Spec Kit); no issue-tracker sync for epics or the roadmap.

## 3. Affected surfaces and files

- New skill: `product-inception/` (`SKILL.md`, templates, `check_product.py`, tests).
- New in `plan-memory`: `_roadmap.py`, `roadmap.py`, `templates/ROADMAP_TEMPLATE.md`, tests; changed: `_backlog.py`, `backlog.py`, `_store.py`, `save_plan.py`, `get_plan.py`, `search_plans.py`, `detect_drift.py`, `rebuild_index.py`, `templates/BACKLOG_TEMPLATE.md`, `SKILL.md`.
- New in `dev-cycle-protocol`: `speckit-preset/`, `speckit_setup.py`, `constitution.py`, tests; changed: `SKILL.md` (Phase 0, D9 table, gates), `project_config.py`, `verify_gate.py`, `inspect_project.py`.
- Changed: `backlog/SKILL.md` (`--epic`), `architecture-docs/SKILL.md` (an ADR that changes a principle regenerates the constitution), `software-design/SKILL.md` (its principles feed the constitution).
- Changed: `README.md`, `docs/configuration.md`, `docs/cloud-sessions.md` (`uv` and network for Spec Kit), `tests/MANUAL_TESTS.md`, `.claude-plugin/plugin.json`, `.claude-plugin/marketplace.json`.
- New: `docs/protocol-updates/` (created by the first adaptation document).
- Surfaces: new commands and configuration keys, a new plan frontmatter field (`spec_dir`) and two new file formats (`ROADMAP.md`, epics in `BACKLOG.md`), all additive.

## 4. Tests (`quality-assurance`)

Automated (pytest):
- `check_product.py`: complete documents pass; a missing section, a leftover placeholder or an over-long document fails with the section named.
- Backlog: `epic` ids and section; `Epic` field round trip; unknown epic fails `validate`; an open epic with all items closed warns; `promote` of an epic fails.
- Roadmap: round trip byte-identical; `assign` of an unknown id fails; an item in two releases fails `validate`; `move-release` keeps order; `current` lists the open items of the first release not `released`; closed items are shown as done.
- `save_plan.py --spec-dir`: field stored; search and drift index the spec files; `get_plan.py` prints them.
- `constitution.py`: generation is deterministic; changing a source (style, ADR, principles) makes `check` fail; regeneration makes it pass.
- Preset wrappers (Bash and PowerShell where available in CI): `feat/001-x` resolves to feature `001-x`; branch creation with type `fix` gives `fix/NNN-slug`; a non-matching branch fails with the expected message.
- `speckit_setup.py`: version pinning in `.dev-cycle.json`; the exact `uvx` command line is built for each subcommand; a missing `uv` fails clearly. The real compatibility check runs only when `SPECKIT_ONLINE_TESTS=1` (network).
- `verify_gate.py`: gate 0 pass and each failure; gate 3 Spec Kit checks (missing `tasks.md`, a `[NEEDS CLARIFICATION]` marker, stale constitution, wrong branch form) and unchanged behavior with Spec Kit disabled; gate 8 roadmap checks.
- `project_config.py`: defaults and validation of `roadmap` and `speckit`.

Manual (`tests/MANUAL_TESTS.md`):
- Inception of a sample product from a one-line idea in Portuguese: question rounds, the documents in English, one approval per document, gate 0, first roadmap release.
- Adoption mode on an existing sample project.
- Full path with Spec Kit on a sample project: branch `feat/001-...`, spec, clarify, plan with the protocol sections, tasks, analyze, gate 3, implement, pull request.
- New project when the latest Spec Kit release fails the check: the previous verified version is pinned and the adaptation document is written.
- `upgrade` of a project to a newer verified version, with the preset kept.

Acceptance criteria: every manual test above passes; with Spec Kit disabled, the existing test suite and manual tests pass unchanged.

## 5. Documentation (`architecture-docs`, `user-docs`)

- New `product-inception/SKILL.md`.
- `dev-cycle-protocol/SKILL.md`: Phase 0 in the cycle diagram and text, the Spec Kit table (D9), branches with Spec Kit, gates 0, 3 and 8.
- `plan-memory/SKILL.md`: epics, roadmap commands, envelope plans and `spec_dir`.
- `backlog/SKILL.md`: `--epic`.
- `README.md`: the new skill in the table and its section, the workflow overview with Phase 0 and the roadmap, "Using Spec Kit" (enable, pin, upgrade, adaptation documents).
- `docs/configuration.md`: `roadmap` and `speckit` sections. `docs/cloud-sessions.md`: `uv` and network access for Spec Kit.

## 6. CI, release and observability (`devops-release`)

- The repository has no CI workflow; the new tests join the pytest command in the README. Online Spec Kit tests are opt-in (`SPECKIT_ONLINE_TESTS=1`).
- Version: plugin `2.1.0` → `2.2.0` (new skill and capabilities, backward compatible); `product-inception` added to `plugin.json`; `install.sh` and `install.ps1` link it (verify they discover skill folders, add it if they list them).
- Observability: `speckit_setup.py` prints the exact command it runs and the pinned version; `inspect_project.py` shows the Spec Kit state, so a misconfigured project is visible at Discovery.

## 7. Security, migration and rollback

- No secrets. Spec Kit is fetched from its official repository at an exact tag; never `latest` or a branch.
- Projects without the new sections behave exactly as today: no roadmap file means no roadmap checks; `speckit` absent or disabled means no Spec Kit; plans without `spec_dir` are complete plans as today; backlogs without epics stay valid.
- Rollback: revert the pull request. In a project, disabling Spec Kit (`speckit.enabled: false`) returns to the plan template; spec folders stay as readable Markdown.

## 8. Execution steps and milestones

- **M1 Spec Kit spike and compatibility check**: install the latest Spec Kit release in a scratch project; confirm how features are resolved and branches created, whether wrapping is enough (D6), the preset mechanics on Windows and Linux, and whether its bug workflow fits bug plans (D4, the user decides); write `speckit_setup.py check` and `compat.json` with the first verified version. The findings are added to this plan before M4.
- **M2 Epics and roadmap**: `epic` type and field, `_roadmap.py`, `roadmap.py`, template, `roadmap` configuration section, tests.
- **M3 Product inception and gate 0**: `product-inception` skill, templates, `check_product.py`, gate 0, adoption mode, tests.
- **M4 Spec Kit preset and setup**: the preset (branch wrappers, plan template, constitution template), `speckit_setup.py` `init`, `upgrade`, `status`, `latest`, the `speckit` configuration section, the adaptation document flow, tests.
- **M5 Generated constitution**: `constitution.py`, its sources, gate 3 check, tests.
- **M6 Protocol integration**: `spec_dir` envelopes, index of spec folders, the D9 flow in `dev-cycle-protocol/SKILL.md`, gate 3 and gate 8 checks, `inspect_project.py`, tests.
- **M7 Documentation and release**: SKILL files, README, configuration and cloud session docs, manual tests, plugin 2.2.0, pull request into `main`.
