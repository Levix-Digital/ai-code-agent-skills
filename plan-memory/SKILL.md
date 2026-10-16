---
name: plan-memory
description: Shared, versioned storage of complete, uncompressed project architecture plans, roadmaps and multi-step blueprints as Markdown files in the project repository (docs/plans), with milestones, lineage, status history, semantic search and drift detection through a local ChromaDB index. Use to save an approved plan, check progress, find earlier decisions, or validate a proposal against the active plan. Project-agnostic.
---

# 📐 Project Plan Memory Skill (`plan-memory`)

> **Paths**: `<skills_root>` is the folder that contains this skill (the parent of this `SKILL.md`'s folder). Use the real absolute path when running the commands.

The **`plan-memory`** skill keeps **100% complete, uncompressed architectural plans, strategic roadmaps and multi-step blueprints** of a project, shared by everyone who works on it.

- **Source of truth**: one Markdown file per plan in the project repository, `docs/plans/<plan_id>.md` (YAML frontmatter + plan body). Plans are versioned, reviewed and merged with the code. Change the folder with `"plans_dir"` in the project's `.dev-cycle.json`.
- **Search index**: a local ChromaDB at `~/.ai-code-agent-memory/plan-memory/db` (override with `PLAN_MEMORY_DB`), collection `plan_index`. It is **disposable**: every command syncs it from the files (by SHA-256), and `rebuild_index.py` recreates it.
- **Project**: resolved from the Git repository of the current directory (or `--dir`): `project_name` in `.dev-cycle.json`, else the repository folder name, normalized to kebab-case. `--project` overrides it.
- **Guiding Principle**: **Zero Compaction / Zero Loss**. Plans are preserved in their entirety (Mermaid diagrams, tables, steps, formulas, rationale) as persistent knowledge for every developer and session.

---

## 🏛️ Plan File Format

```markdown
---
plan_id: my_app_blueprint_v1_2_0_two_stage_search
project: my-app
title: Two-stage search with local re-ranking
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
Options: `--id` (default: `<project>_<title>` slug), `--parent-id`, `--content` instead of `--file`.

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

### 4. List Plans (`list_plans.py`)
```powershell
python "<skills_root>/plan-memory/list_plans.py" [--status active]
```

### 5. Read a Complete Plan (`get_plan.py`)
```powershell
python "<skills_root>/plan-memory/get_plan.py" --id "<plan_id>"
python "<skills_root>/plan-memory/get_plan.py"                      # latest active plan
```

### 6. Semantic Search (`search_plans.py`)
```powershell
python "<skills_root>/plan-memory/search_plans.py" --query "how to implement the model router in chat" [--limit 5] [--all-projects]
```

### 7. Update Lifecycle Status (`update_plan_status.py`)
```powershell
python "<skills_root>/plan-memory/update_plan_status.py" --id "<plan_id>" --status "completed" --notes "Priority 1 and 2 delivered."
```

### 8. Export (`export_plan.py`)
Copies a plan elsewhere, as JSON or Markdown with or without frontmatter:
```powershell
python "<skills_root>/plan-memory/export_plan.py" --id "<plan_id>" --format json --output-dir "out/"
```

### 9. Rebuild the Index (`rebuild_index.py`)
```powershell
python "<skills_root>/plan-memory/rebuild_index.py"
```

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
- **Phase 1 (Discovery)**: check `list_plans.py`, `check_milestone.py --list` or `search_plans.py` for real progress and agreed guidelines.
- **Phase 2 (Plan)**: save the full plan with `save_plan.py` (with `--supersedes` when it replaces one) and run `detect_drift.py`. The plan follows the orchestrator's template, with a section from each role.
- **Phase 3 (Plan approval)**: `verify_gate.py --gate 3` checks there is an active plan and warns if its file is not committed.
- **Phase 4 (Implementation)**: mark milestones with `check_milestone.py` as they are completed.
- **Phase 10 (Release)**: close the remaining milestones or finish the plan with `update_plan_status.py`.

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
