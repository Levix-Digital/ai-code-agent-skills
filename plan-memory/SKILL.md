---
name: plan-memory
description: Shared, versioned planning of a project in its repository - the backlog (docs/BACKLOG.md) where bugs, features, improvements and adjustments are captured for later and grouped under epics, the roadmap (docs/ROADMAP.md) of releases and the items each one delivers, and complete, uncompressed plans (docs/plans, feature or bug plans) with milestones, lineage, status history, promotion of backlog items, semantic search and drift detection through a local ChromaDB index. Use to capture or triage backlog items, plan epics and releases, promote items into a plan, save an approved plan, check progress, find earlier decisions, or validate a proposal against the active plan. Project-agnostic.
---

# 📐 Project Plan Memory Skill (`plan-memory`)

> **Paths**: `<skills_root>` is the folder that contains this skill (the parent of this `SKILL.md`'s folder). Use the real absolute path when running the commands.

The **`plan-memory`** skill keeps **100% complete, uncompressed architectural plans, strategic roadmaps and multi-step blueprints** of a project, shared by everyone who works on it.

- **Source of truth**: one Markdown file per plan in the project repository, `docs/plans/<plan_id>.md` (YAML frontmatter + plan body). Plans are versioned, reviewed and merged with the code. Change the folder with `"plans_dir"` in the project's `.dev-cycle.json`.
- **Search index**: a local ChromaDB at `~/.ai-code-agent-memory/plan-memory/db` (override with `PLAN_MEMORY_DB`), collection `plan_index`. It is **disposable**: every command syncs it from the files (by SHA-256), and `rebuild_index` (engine) recreates it.
- **Project**: resolved from the Git repository of the current directory (or `--dir`): `project_name` in `.dev-cycle.json`, else the repository folder name, normalized to kebab-case. `--project` overrides it.
- **Levels**: the **backlog** (`docs/BACKLOG.md`) captures work in seconds, without discovery, grouped under epics; the **roadmap** (`docs/ROADMAP.md`) orders the releases and assigns items to them; a **plan** is created when the user promotes backlog items and holds the discovery and the planning (see "Backlog" and "Roadmap").
- **Spec Kit envelopes**: when the project uses Spec Kit, a feature plan is an envelope (`spec_dir`) of a Spec Kit folder (`specs/NNN-slug/`) that holds the detail; search, drift detection and `get_plan` read the folder's artifacts with the plan (see "Plans with Spec Kit").
- **Guiding Principle**: **Zero Compaction / Zero Loss**. Plans are preserved in their entirety (Mermaid diagrams, tables, steps, formulas, rationale) as persistent knowledge for every developer and session.

---

## 🏛️ Plan File Format

```markdown
---
plan_id: my_app_blueprint_v1_2_0_two_stage_search
project: my-app
title: Two-stage search with local re-ranking
kind: feature              # feature (default) | bug
version: v1.2.0
status: in_progress        # draft | planned | active | in_progress | completed | superseded
author: Jane Doe           # git user.name of whoever created it
created_at: '2026-10-15T16:36:00'
updated_at: '2026-10-16T09:10:00'
session_id: 8abbc3bf-...   # conversation where the plan was conceived (optional)
tags: [search, reranker]
supersedes: my_app_blueprint_v1_1_0_search
superseded_by: null
parent_plan_id: null
backlog_items: [FEAT-004]  # backlog items the plan delivers (optional)
spec_dir: specs/004-reranker  # Spec Kit folder with the plan's detail (optional)
milestones:
- name: M1 Runtime base
  done: true
  done_at: '2026-10-15'
  by: Jane Doe
- name: M2 Reranker
  done: false
status_log:
- at: '2026-10-15T16:36:00'
  by: Jane Doe
  status: active
  note: Plan created
---

# Plan title

...full plan body in Markdown...
```

- The scripts **only rewrite the frontmatter**; the body is written once by `save_plan.py` (or edited by hand). Milestone and status changes produce small, readable diffs.
- The file name is the plan ID. Lineage fields (`supersedes`, `superseded_by`, `parent_plan_id`) reference other plan IDs.
- **The scripts never commit.** The agent commits plan files with the work they describe (see "Team workflow").

---

## ⚙️ Commands

All scripts are in `<skills_root>/plan-memory/` and accept `--dir <project dir>` (default: current directory) and `--project <name>`.

### 1. Save / Update a Plan with Lineage (`save_plan.py`)
Writes `docs/plans/<plan_id>.md` with the full plan, without cuts or summaries. Saving an existing ID updates title, body and fields, keeping author, creation date and milestone progress. `--supersedes` marks the older plan as `superseded`:
```powershell
python "<skills_root>/plan-memory/save_plan.py" `
  --title "Integrated Roadmap: Python -> Rust, Model Router & Model Kit" `
  --file "path/to/plan.md" `
  --version "v1.2.0" `
  --tags "roadmap,rust_refactor,model_router" `
  --milestones "Priority 1,Priority 2,Milestone 3" `
  --status "active" `
  --session-id "<session_id>" `
  --supersedes "<old_plan_id>"
```
Options: `--id` (default: `<project>_<title>` slug), `--parent-id`, `--content` instead of `--file`, `--kind feature|bug`, `--backlog-items "BUG-001,FEAT-002"` (links the items, which become `planned`), `--spec-dir specs/NNN-slug` (the plan is the envelope of that Spec Kit folder).

### 2. Milestone & Progress Tracker (`check_milestone.py`)
```powershell
# Progress and checklist of the latest active plan (or --id <plan_id>):
python "<skills_root>/plan-memory/check_milestone.py" --list

# Mark a milestone as completed (records date and author):
python "<skills_root>/plan-memory/check_milestone.py" --milestone "Priority 1" --notes "Completed via PR #319"

# Revert a milestone to pending:
python "<skills_root>/plan-memory/check_milestone.py" --milestone "Priority 1" --uncheck
```
The first completed milestone moves an `active` plan to `in_progress`; the last one moves it to `completed`.

### 3. Architectural Drift Detector (`detect_drift.py`)
Compares any implementation proposal, issue or new prompt against the active plan: semantic similarity, milestone coverage and order, and the project's own anti-pattern rules.
```powershell
python "<skills_root>/plan-memory/detect_drift.py" --proposal "Priority 1: build the native ReAct orchestrator in Rust, replacing the Python LangGraph"
python "<skills_root>/plan-memory/detect_drift.py" --file "path/to/proposal.md" --strict   # exit code != 0 on conflict
```

Anti-pattern rules live in the `drift_rules` key of the project's `.dev-cycle.json` (looked up from `--dir` upwards). Without rules, only similarity and milestones are checked.
```json
"drift_rules": [
  { "tech": "langchain", "conflict_msg": "The baseline plan removes LangChain." },
  {
    "tech": "parallel database",
    "negative_markers": ["single source", "settings.db"],
    "proposal_allow": [],
    "conflict_msg": "Proposal hints at a parallel database, violating the single source of truth."
  }
]
```
A rule raises a conflict when a plan in force (`active` / `in_progress`) mentions `tech` together with a rejection marker, and the proposal mentions `tech` without an allowed phrase.
- `negative_markers` (optional): words that mark the rejection. Default: `remove`, `replace`, `without`, `forbidden`, ... plus their Portuguese equivalents, kept while existing plans are not yet translated.
- `proposal_allow` (optional): phrases that make a mention acceptable. Default: `without <tech>`, `remove <tech>`, `replace <tech>`, ... plus their Portuguese equivalents.

### 4. List Plans (`list_plans`)
`list_plans` runs on the native engine. In sh shells (Linux, macOS, Git Bash):
```sh
"<skills_root>/dev-cycle-protocol/bin/dev-cycle" plan-memory list_plans [--status active] [--kind bug]
```
In PowerShell, run `& "<skills_root>/dev-cycle-protocol/bin/dev-cycle.ps1" plan-memory list_plans [arguments]` instead.

### 5. Read a Complete Plan (`get_plan`)
`get_plan` runs on the native engine. In sh shells (Linux, macOS, Git Bash):
```sh
"<skills_root>/dev-cycle-protocol/bin/dev-cycle" plan-memory get_plan --id "<plan_id>"
"<skills_root>/dev-cycle-protocol/bin/dev-cycle" plan-memory get_plan                  # latest active plan
```
In PowerShell, run `& "<skills_root>/dev-cycle-protocol/bin/dev-cycle.ps1" plan-memory get_plan [arguments]` instead.

### 6. Semantic Search (`search_plans.py`)
```powershell
python "<skills_root>/plan-memory/search_plans.py" --query "how to implement the model router in chat" [--limit 5] [--all-projects]
```

### 7. Update Lifecycle Status (`update_plan_status.py`)
```powershell
python "<skills_root>/plan-memory/update_plan_status.py" --id "<plan_id>" --status "completed" --notes "Priority 1 and 2 delivered."
```

### 8. Export (`export_plan`)
Copies a plan elsewhere, as JSON or Markdown with or without frontmatter. `export_plan` runs on the native engine. In sh shells (Linux, macOS, Git Bash):
```sh
"<skills_root>/dev-cycle-protocol/bin/dev-cycle" plan-memory export_plan --id "<plan_id>" --format json --output-dir "out/"
```
In PowerShell, run `& "<skills_root>/dev-cycle-protocol/bin/dev-cycle.ps1" plan-memory export_plan [arguments]` instead.

### 9. Rebuild the Index (`rebuild_index`)
`rebuild_index` runs on the native engine. In sh shells (Linux, macOS, Git Bash):
```bash
"<skills_root>/dev-cycle-protocol/bin/dev-cycle" plan-memory rebuild_index [--project <name>] [--dir <path>]
```
In PowerShell, run `& "<skills_root>/dev-cycle-protocol/bin/dev-cycle.ps1" plan-memory rebuild_index [arguments]` instead.

---

## 📥 Backlog (`backlog.py`)

The first level of planning: one Markdown file (`backlog.file` in `.dev-cycle.json`, default `docs/BACKLOG.md`), created by the first `add` from `templates/BACKLOG_TEMPLATE.md`.

```markdown
## Bugs | Features | Improvements | Adjustments | Epics | Closed

### BUG-001: Frontier preset selects an outdated model id
- **Severity**: high              # bugs: critical | high | medium | low
- **Status**: open                # open | planned | in_progress (done | dropped | duplicate | superseded under Closed)
- **Found**: 2026-10-16, v0.45.0, PR #333
- **Plan**: -                     # plan id once promoted
- **Description**: `/preset frontier` sets `claude-3-7-sonnet`, which the provider may reject.
```

Features (`FEAT-NNN`), improvements (`IMP-NNN`) and adjustments (`ADJ-NNN`) have `Priority` (high, medium, low) instead of `Severity`. Any item but an epic may have `- **Epic**: EPIC-NNN`. **Epics** (`EPIC-NNN`, section `## Epics`) group the items of the complete product; they have `Priority`, `Status` (`open` until closed), `Found` and `Description`, no `Plan`, and are never promoted: their items are. `validate` warns about an open epic whose items are all closed. Ids are never reused: closed items stay under `## Closed` with a `- **Closed**:` note (date and pull request, reason, `duplicate of <ID>`, or `superseded by <ID>[, <ID>...]`). An item is **superseded** when it is replaced by different, better defined items (an idea split or reshaped after inception); a **duplicate** is the same request captured twice.

| Task | Command |
|---|---|
| Capture | `backlog.py add --type bug\|feature\|improvement\|adjustment\|epic --title "<t>" --description "<d>" [--severity S] [--priority P] [--epic EPIC-NNN] [--found-in "v1.2.0, PR #12"]` |
| Capture failed manual tests | `backlog.py add --from-failed --run vX.Y.Z-rc.N [--severity S]` |
| Summary (bugs before new work first) | `backlog.py summary [--json]` |
| List, show | `backlog.py list [--type T] [--status S] [--epic EPIC-NNN] [--all] [--json]`, `backlog.py show <ID>` |
| Change | `backlog.py set <ID> [--title] [--description] [--severity] [--priority] [--status] [--plan] [--epic ID\|-]` |
| Promote into a plan | `backlog.py promote <ID> [<ID> ...] [--title T] [--id PLAN_ID] [--version V]` or `--into <PLAN_ID>` |
| Close | `backlog.py close <ID> [--ref "PR #12"]`, `drop <ID> --reason R`, `duplicate <ID> --of <ID>`, `supersede <ID> --by <ID>[,<ID>...]`, `reopen <ID>` |
| Validate | `backlog.py validate` |

- **Capture** costs nothing: no discovery, no plan. The user does it with the [`/backlog`](../backlog/SKILL.md) command; the agent does it for bugs it finds outside the current scope.
- **Promotion** creates a `draft` plan from `templates/BUG_PLAN_TEMPLATE.md` (every item a bug: reproduction, evidence and root cause, fix design, regression test) or `templates/FEATURE_PLAN_TEMPLATE.md`, with the items copied into its `Backlog items` section, and links both ways. The discovery is written in that plan; it is saved again as `active`, with milestones, for approval.
- **Linked items follow their plan**: `planned` while the plan is draft or active, `in_progress` when it is in progress, closed as `done` when it is completed (`check_milestone.py`, `update_plan_status.py`).
- **Triage is the user's decision**: the agent never promotes, drops or re-prioritizes on its own.
- The script never commits and refuses to rewrite a file whose structure it cannot read, so hand edits are never lost.

---

## 🗺️ Roadmap (`roadmap`)

The releases of the product in delivery order (`roadmap.file` in `.dev-cycle.json`, default `docs/ROADMAP.md`), created by the first `add-release` from `templates/ROADMAP_TEMPLATE.md`. It is the **only** place that assigns backlog items to releases.

`roadmap` runs on the native engine. In sh shells (Linux, macOS, Git Bash):
```bash
"<skills_root>/dev-cycle-protocol/bin/dev-cycle" plan-memory roadmap current
```
In PowerShell, run `& "<skills_root>/dev-cycle-protocol/bin/dev-cycle.ps1" plan-memory roadmap [arguments]` instead. The commands below are its arguments.

```markdown
## v0.1.0: MVP
- **Status**: in_progress          # planned | in_progress | released
- **Target**: 2026-12               # optional, free text
- **Released**: 2026-12-10          # released only
- **Goal**: Families keep their photos in one private place.
- **Items**: FEAT-001, FEAT-002, BUG-003    # '-' when empty
```

| Task | Command |
|---|---|
| Add a release | `roadmap add-release vX.Y.Z --name N --goal G [--target T] [--before V \| --after V]` |
| Change a release | `roadmap set-release vX.Y.Z [--name] [--goal] [--target] [--status planned\|in_progress\|released]` |
| Reorder | `roadmap move-release vX.Y.Z --before V \| --after V` |
| Assign items | `roadmap assign <ID> [<ID> ...] --release vX.Y.Z [--move]` (an epic adds its open items not in another release) |
| Take items out | `roadmap unassign <ID> [...]` |
| Show | `roadmap show [--json]`, `roadmap current [--json]` (open items of the first release not released, bugs first, and the unscheduled items) |
| Validate | `roadmap validate` (format, version order, an item in one release only, ids in the backlog, no epics) |

- Released releases are frozen: their items cannot be moved or removed. `set-release --status released` records the date and lists items still open.
- Moving items between releases is the user's decision, like triage.

---

## 🧩 Plans with Spec Kit

When the project uses Spec Kit (`speckit.enabled`, see `dev-cycle-protocol`), the detail of a feature lives in Spec Kit's folder (`specs/NNN-slug/`: `spec.md`, `plan.md`, `research.md`, `data-model.md`, `quickstart.md`, `contracts/`, `tasks.md`). The plan file is its **envelope**: frontmatter (status, milestones, lineage, backlog items, `spec_dir`) and a short body (goal and decisions, link to the folder). Nothing is copied or summarized: `get_plan` prints the artifacts after the body (and `--output` writes both), and the search index and `detect_drift.py` read the plan and its artifacts as one document, re-indexed when any of them changes.

---

## 👥 Team Workflow

- **Phase 2 (plan)**: save the plan with `save_plan.py`. The file is part of the proposal the user approves.
- **Phase 3 (approval)**: once approved, commit the plan file (`docs(plan): <title>`) on the work branch before implementing. In a team, plan files go through pull requests like code.
- **Phases 4 to 10**: commit milestone and status changes with the work that completes them.
- Someone else's plan changes arrive with `git pull`; the next command re-indexes them automatically.
- Two people editing the same plan get an ordinary Git merge on readable YAML lines.

---

## 🔒 Integration with `dev-cycle-protocol`

The whole development lifecycle follows 👉 **[`dev-cycle-protocol`](../dev-cycle-protocol/SKILL.md)**:
- **Phase 0 (Product inception)**: the product backlog as epics and their items, and the roadmap from the MVP (`product-inception`).
- **Phase 1 (Discovery)**: check `list_plans` (engine), `check_milestone.py --list` or `search_plans.py` for real progress and agreed guidelines, `backlog.py summary` for open bugs and captured ideas and `roadmap current` (engine) for the items of the current release; promote the items the user chooses.
- **Phase 2 (Plan)**: save the full plan with `save_plan.py` (with `--supersedes` when it replaces one, `--spec-dir` with Spec Kit) and run `detect_drift.py`. The plan follows the orchestrator's template, with a section from each role.
- **Phase 3 (Plan approval)**: `verify_gate --gate 3` (engine) checks there is an active plan, warns if its file is not committed, and blocks a non-bug plan while open blocking bugs (default `critical`, `high`, `medium`) or bugs not triaged are outside it (bugs first). A bug is triaged when it has a valid severity and, when its severity does not block, it is in the next release of the roadmap (`backlog.bug_deadline`). `backlog.py summary` lists the bugs not triaged; `backlog.py add` of a bug says where it goes.
- **Phase 4 (Implementation)**: mark milestones with `check_milestone.py` as they are completed.
- **Phase 10 (Release)**: close the remaining milestones or finish the plan with `update_plan_status.py`, and mark the release released (`roadmap set-release vX.Y.Z --status released` on the engine).

---

## 🔁 Migrating from the Old Database (`migrate_legacy.py`)

One-time migration from the old ChromaDB-only storage (collection `project_plans`) into `docs/plans`, reconciling plan files that already exist there:
```powershell
python "<skills_root>/plan-memory/migrate_legacy.py" [--source "<old_chromadb_folder>"] [--orphan-status completed] [--dry-run]
```
- Database plans become files; a file with the same ID keeps the newer body (last commit vs. database `updated_at`).
- A file matching a plan's content under another ID is renamed (`git mv`) to the database ID, which lineage references use.
- Files with no database entry keep their ID with minimal metadata.
- The source database is not modified; review the result and commit it.
