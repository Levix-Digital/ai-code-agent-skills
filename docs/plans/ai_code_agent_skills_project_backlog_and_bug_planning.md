---
plan_id: ai_code_agent_skills_project_backlog_and_bug_planning
project: ai-code-agent-skills
title: Project backlog and bug planning
version: v2.1.0
status: in_progress
author: Levi Silveira
created_at: '2026-10-16T17:55:20'
updated_at: '2026-10-16T18:04:01'
session_id: null
tags:
- backlog
- bugs
- plan-memory
- dev-cycle-protocol
supersedes: null
superseded_by: null
parent_plan_id: null
milestones:
- name: M1 Backlog store and CLI
  done: true
  done_at: '2026-10-16'
  by: Levi Silveira
- name: M2 Plan kinds and promotion
  done: false
- name: M3 Cycle integration
  done: false
- name: M4 Documentation and release
  done: false
- name: M5 Apply to any-context
  done: false
status_log:
- at: '2026-10-16T17:55:20'
  by: Levi Silveira
  status: active
  note: Plan created
- at: '2026-10-16T18:04:01'
  by: Levi Silveira
  note: 'Milestone ''M1 Backlog store and CLI'' completed: backlog.py, _backlog.py, template, backlog config section, 14 tests'
---

# Project Backlog and Bug Planning

## 1. Goal and decisions

Today the cycle knows only two outcomes for a bug found during work: fix it now (short path) or mention it in a pull request description, where it is lost after the merge. Ideas for features, improvements and adjustments live outside the project (for example in a notes app). There is no place to **capture** work cheaply and **plan it later**, and no rule that bugs come before new features.

Decisions:

- **D1 — Two levels.**
  - **Level 1, the backlog**: one Markdown file in the repository, `docs/BACKLOG.md`, where an item is captured in seconds: id, title, one or two lines, where it was found, severity (bugs) or priority (others). No discovery, no plan.
  - **Level 2, the plan**: when the user decides, one or more backlog items are **promoted** into a plan-memory plan. The plan holds the discovery and the planning; a bug plan uses a bug template (reproduction, evidence, root cause, fix design, regression test).
- **D2 — One backlog for every kind of work.** Item types: `bug` (`BUG-NNN`), `feature` (`FEAT-NNN`), `improvement` (`IMP-NNN`), `adjustment` (`ADJ-NNN`). Ids are stable and never reused.
- **D3 — The planner owns the backlog.** `plan-memory` gets `backlog.py` (capture, list, triage, promote, close, validate). `quality-assurance` owns what a good bug report contains and turns deferred manual test failures into backlog bugs. `dev-cycle-protocol` owns when the backlog is read and the priority rule.
- **D4 — Bugs first.** An open bug with a blocking severity (default `critical` and `high`, configurable) blocks the plan approval gate of a feature, improvement or adjustment plan, unless the user explicitly accepts the risk (`verify_gate.py --gate 3 --accept-open-bugs "<reason>"`, recorded in the plan's status log). Lower-severity bugs are listed, never blocking.
- **D5 — The agent never triages on its own.** It captures items (bugs it finds and cannot fix within the current scope, ideas the user dictates); promoting, dropping or re-prioritizing is the user's decision.
- **D6 — Bugs found during work.** In scope and small: fix now with a regression test. Otherwise: capture with `backlog.py add` and report the id to the user. A pull request never carries a loose "points to decide" list again; it links backlog ids.
- **D7 — Closing.** An item becomes `done` when its plan is completed (automatic, through `check_milestone.py` and `update_plan_status.py`) or by hand with the pull request link (`backlog.py close`). Closed items move to a `Closed` section and stay in the file, so ids are not reused; Git keeps the history.
- **D8 — The `/backlog` command.** The plugin ships a slash command for capture without leaving the conversation:
  - `/backlog --feature|--improvement|--adjustment|--bug "<text>" [--priority high|medium|low] [--severity critical|high|medium|low]` captures one item. The agent writes a short title and a faithful English description from the user's text (no added scope), runs `backlog.py add`, and answers with one line: id, title and priority or severity.
  - Priority defaults to `medium`. For `--bug`, severity is required: when the user gives none, the agent estimates it and says so in the answer, so the user can correct it with `backlog.py set`.
  - `/backlog` with no type shows `backlog.py summary`.
  - Agents without plugin commands reach the same behavior by asking in natural language ("add to the backlog as a feature: ..."), handled by the plan-memory skill.

Alternatives considered:

- **A separate skill for bugs**: would duplicate plan-memory's storage, index, status and milestones. Rejected.
- **One file per backlog item**: fewer merge conflicts in a team, but heavier to capture and to read at a glance. Rejected for now; the single file is the format the user reads and edits by hand.
- **GitHub Issues as the source**: needs the network and the API, is not versioned with the code and is harder for the agent to edit. Can be added later as a one-way export.
- **A bug-only backlog first, ideas later**: would mean designing and migrating twice.

## 2. Design (`software-design`)

### Backlog file format (`docs/BACKLOG.md`)

```markdown
# Backlog

Work captured for later planning. Promote items into plans with `backlog.py promote`.

## Bugs

### BUG-001: Frontier preset selects an outdated model id
- **Severity**: high            # critical | high | medium | low
- **Status**: open              # open | planned | in_progress
- **Found**: 2026-10-16, v0.45.0, PR #333
- **Plan**: -                   # plan id once promoted
- **Description**: `/preset frontier` sets `claude-3-7-sonnet`, which the provider may no longer accept.

## Features
### FEAT-001: ...
- **Priority**: medium          # high | medium | low
- **Status**: open
- **Found**: 2026-10-16
- **Plan**: -
- **Description**: ...

## Improvements
## Adjustments

## Closed

### BUG-000: ...
- **Status**: done              # done | dropped | duplicate
- **Closed**: 2026-10-20, PR #340   # or the reason for dropped / the duplicated id
...
```

Validation rules: unique ids with the prefix of their section, required fields per type, valid enum values, `Plan` set when `planned` or `in_progress`, closed items only under `Closed`.

### Components

| Unit | Responsibility |
|---|---|
| `plan-memory/_backlog.py` | Parse and render the backlog file (pure functions over text); item model; validation; next id per type (max seen + 1, closed items included). |
| `plan-memory/backlog.py` | CLI: `add`, `list`, `summary`, `show`, `set` (severity, priority, status), `promote`, `close`, `drop`, `validate`, `add --from-failed --run <rc>`. Writes the file; never commits. |
| `plan-memory/templates/BACKLOG_TEMPLATE.md` | Initial file, created by the first `add`. |
| `plan-memory/templates/BUG_PLAN_TEMPLATE.md`, `FEATURE_PLAN_TEMPLATE.md` | Bodies used by `promote`, pre-filled with the items. |
| `plan-memory/_store.py` | New frontmatter fields `kind` (`feature` default, `bug`) and `backlog_items` (list of ids). |
| `plan-memory/check_milestone.py`, `update_plan_status.py` | On `completed`, close the plan's backlog items as `done` with the plan id. |
| `plan-memory/list_plans.py`, `get_plan.py` | Show `kind` and linked items. |
| `dev-cycle-protocol/project_config.py` | New `backlog` section: `file` (default `docs/BACKLOG.md`), `blocking_severities` (default `["critical", "high"]`). |
| `dev-cycle-protocol/verify_gate.py` | Gate 3: bugs-first rule (D4) and `--accept-open-bugs`; gate 8: `backlog.py validate` and a warning listing open critical/high bugs (for the release notes' known limitations). |
| `commands/backlog.md` (plugin root) | The `/backlog` slash command (D8): parses `$ARGUMENTS`, calls `backlog.py add` or `summary`, never promotes or triages. |
| `quality-assurance/manual_tests.py` | Exposes its parser so `backlog.py add --from-failed` reuses it (no second parser). |

`promote <ID...> [--title T] [--into <plan_id>]` creates a plan (`status: draft`, `kind: bug` when every item is a bug, else `feature`) through the same store code as `save_plan.py`, with the items' text in the body, and marks the items `planned` with the plan id. `--into` adds items to an existing plan. Promoting a `planned` item again is an error naming its plan.

Deliberately not abstracted: no storage interface for other backends (one file format), no issue-tracker sync.

Errors: every command exits non-zero with a message naming the id and the rule; writes are atomic (temp file and replace) so a failed command never leaves a half-written backlog.

## 3. Affected surfaces and files

- New: `plan-memory/_backlog.py`, `plan-memory/backlog.py`, `plan-memory/templates/*`, `plan-memory/tests/`.
- New: `commands/backlog.md`.
- Changed: `plan-memory/_store.py`, `check_milestone.py`, `update_plan_status.py`, `list_plans.py`, `get_plan.py`, `save_plan.py` (`--kind`, `--backlog-items`), `plan-memory/SKILL.md`; `dev-cycle-protocol/SKILL.md`, `project_config.py`, `verify_gate.py`, `inspect_project.py` (backlog summary); `quality-assurance/SKILL.md`, `manual_tests.py`; `README.md`, `docs/configuration.md`, `tests/MANUAL_TESTS.md`; `.claude-plugin/plugin.json`.

## 4. Tests (`quality-assurance`)

Automated (pytest, `plan-memory/tests/` plus the existing suites):
- Parse/render round trip keeps the file byte-identical when nothing changes.
- `add` per type allocates the next id, also after items were closed; creates the file from the template.
- `validate` reports duplicate ids, wrong prefixes, missing fields, invalid enums, `planned` without a plan.
- `promote` creates a draft plan of the right kind with the items in the body, links both ways; promoting twice fails; `--into` appends.
- Completing the plan (last milestone, or `update_plan_status --status completed`) closes its items as `done`.
- `close`, `drop` (requires a reason), `duplicate` move items to `Closed`.
- `add --from-failed --run R` creates one bug per failed manual test, skipping ones already captured.
- Gate 3: a feature plan with an open high bug fails; passes with `--accept-open-bugs` (reason logged); a bug plan is never blocked; low bugs only warn.
- Gate 8: an invalid backlog fails; open critical/high bugs produce a warning.
- `project_config`: defaults and validation of the `backlog` section.

Manual (`tests/MANUAL_TESTS.md` of the skills repository):
- `/backlog --feature "..."` in Portuguese: one English item is captured with the next `FEAT` id and `medium` priority, and the answer is one line; `/backlog --bug "..."` without severity: the answer states the estimated severity; `/backlog` alone shows the summary.
- Capture during a session: the agent finds an out-of-scope bug, captures it and reports the id.
- Triage conversation: the agent shows the backlog summary at discovery and waits for the user's decisions.

## 5. Documentation (`architecture-docs`, `user-docs`)

- `plan-memory/SKILL.md`: backlog format and commands, plan kinds, promotion and closing.
- `dev-cycle-protocol/SKILL.md`: Phase 1 reads the backlog summary; D4–D6 rules; gate table.
- `quality-assurance/SKILL.md`: bug report content, regression test per bug plan, deferring manual test failures.
- `README.md` and `docs/configuration.md`: the `/backlog` command, the backlog in the workflow overview and the `backlog` configuration section.

## 6. CI, release and observability (`devops-release`)

No workflow changes; the new tests join the existing pytest command in the README and CI. Version: plugin `2.0.0` → `2.1.0` (new capability, backward compatible).

## 7. Security, migration and rollback

- No secrets involved. Projects without a backlog keep working: the file is created by the first `add`; gate 3's rule is skipped when there is no file.
- Plans without `kind` are read as `feature`; no migration needed.
- Rollback: revert the pull request; backlog files in projects stay readable Markdown.

## 8. Execution steps and milestones

- **M1 Backlog store and CLI**: `_backlog.py`, `backlog.py` (all commands except `promote` and `--from-failed`), template, `backlog` config section, tests.
- **M2 Plan kinds and promotion**: `kind` and `backlog_items` in plan-memory, plan templates, `promote`, automatic closing, listing, tests.
- **M3 Cycle integration**: the `/backlog` command, protocol rules, gate 3 bugs-first and gate 8 checks, `--from-failed`, quality-assurance rules, tests.
- **M4 Documentation and release**: SKILL files, README, configuration reference, manual tests, plugin 2.1.0, pull request.
- **M5 Apply to any-context**: `backlog` section in `.dev-cycle.json`, `docs/BACKLOG.md` with the four items left open in PR #333 (outdated `frontier` model id, Deep Search `min_score` 0.02, outdated `.clinerules`, data indexed before field encryption still in plaintext), only captured, without discovery; pull request into `dev`. Update the agent memory rule "fix detected bugs" to D6.
