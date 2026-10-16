---
name: plan-memory
description: Persistent vector storage and semantic retrieval of complete, uncompressed project architecture plans, roadmaps, and multi-step blueprints in ChromaDB. Ensures zero loss of engineering plans, with rich metadata (project, session_id, version, status, milestones) and lifecycle tracking. Project-agnostic.
---

# 📐 Project Plan Memory Skill (`plan-memory`)

> **Paths**: `<skills_root>` is the folder that contains this skill (the parent of this `SKILL.md`'s folder). Use the real absolute path when running the commands.

The **`plan-memory`** skill provides persistent, vector-indexed storage, lifecycle management, and semantic retrieval of **100% complete, uncompressed architectural plans, strategic roadmaps, and multi-step blueprints** across any software project.

- **ChromaDB Path**: `~/.ai-code-agent-memory/plan-memory/db` (override with the `PLAN_MEMORY_DB` environment variable). It lives outside the skill folder, so the skill can be versioned and copied without carrying data.
- **Project names**: always normalized to kebab-case (`AnyContext`, `any_context` and `any-context` all become `any-context`). Use the name of the project's Git repository root, the same one the `session-memory` skill uses.
- **Collection Name**: `project_plans`
- **Guiding Principle**: **Zero Compaction / Zero Loss**. Architectural decisions and blueprints must be preserved in their entirety (including Mermaid diagrams, tables, steps, formulas, and rationale) so they serve as persistent knowledge bases across active and future coding sessions.

---

## 🏛️ Architecture & ChromaDB Schema

Every plan entry in the `project_plans` collection preserves the full uncompressed Markdown document alongside rich structured metadata:

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        PROJECT PLAN MEMORY SCHEMA                                      │
├────────────────────┬───────────────────────────────────────────────────────────────────┤
│ Field              │ Description & Purpose                                             │
├────────────────────┼───────────────────────────────────────────────────────────────────┤
│ `id`               │ Unique Plan Identifier (e.g. `my_app_master_roadmap_v1_2_0`)       │
│ `document`         │ 100% Full Uncompressed Markdown content of the plan               │
│ `project`          │ Normalized project name (e.g. "any-context", "my-app")            │
│ `title`            │ Human-readable headline of the plan                               │
│ `session_id`       │ Conversation ID where the plan was conceived / approved           │
│ `version`          │ Target software version (e.g. "v0.30.37")                         │
│ `status`           │ Lifecycle status: `active`, `in_progress`, `completed`, `superseded`│
│ `created_at`       │ ISO 8601 creation timestamp                                       │
│ `updated_at`       │ ISO 8601 last update timestamp                                    │
│ `tags`             │ Comma-separated search keywords                                   │
│ `milestones`       │ Comma-separated target milestones                                 │
│ `completed_ms`     │ Comma-separated list of checked-off milestones                    │
│ `supersedes`       │ Older Plan ID that this plan replaces / deprecates                │
│ `superseded_by`    │ Newer Plan ID that superseded this plan                           │
│ `parent_plan_id`   │ Optional parent Plan ID for hierarchical planning trees           │
│ `status_notes`     │ Timestamped progress log and architectural notes                  │
│ `content_length`   │ Character count of the uncompressed plan                          │
└────────────────────┴───────────────────────────────────────────────────────────────────┘
```

---

## ⚙️ Operational Commands & Scripts

The skill provides a complete suite of Python utilities located in `<skills_root>/plan-memory`:

### 1. Save / Update a Plan with Lineage (`save_plan.py`)
Saves the full plan without cuts or summaries. Supports `--supersedes` to automatically archive a previous plan as `superseded`:
```powershell
python "<skills_root>/plan-memory/save_plan.py" `
  --project "<project>" `
  --title "Integrated Roadmap: Python -> Rust, Model Router & Model Kit" `
  --file "path/to/plan.md" `
  --session-id "<session_id>" `
  --version "v1.2.0" `
  --tags "roadmap,rust_refactor,model_router" `
  --milestones "Priority 1,Priority 2,Milestone 3" `
  --status "active" `
  --supersedes "<old_plan_id>"
```

### 2. Granular Milestone & Progress Tracker (`check_milestone.py`)
Tracks sub-step progress and renders visual progress bars:
```powershell
# Show the active plan's progress and milestone checklist:
python "<skills_root>/plan-memory/check_milestone.py" --project "<project>" --list

# Mark a milestone as completed:
python "<skills_root>/plan-memory/check_milestone.py" `
  --project "<project>" `
  --milestone "Priority 1" `
  --notes "Completed via PR #319 with a 100% green test suite"

# Uncheck a milestone (revert to pending if needed):
python "<skills_root>/plan-memory/check_milestone.py" `
  --project "<project>" `
  --milestone "Priority 1" `
  --uncheck
```

### 3. Architectural Drift Detector (`detect_drift.py`)
Compares any implementation proposal, issue or new prompt against the active plan and anti-pattern principles in ChromaDB:
```powershell
# Check a proposal's architectural alignment:
python "<skills_root>/plan-memory/detect_drift.py" `
  --project "<project>" `
  --proposal "Priority 1: build the native ReAct orchestrator in Rust, replacing the Python LangGraph with zero heavy frameworks"

# Check a proposal from a file in strict mode (exit code != 0 on conflict):
python "<skills_root>/plan-memory/detect_drift.py" `
  --project "<project>" `
  --file "path/to/proposal.md" `
  --strict
```

### 4. Automated Snapshot Exporter (`export_plan.py`)
Exports plans from ChromaDB to Markdown files with a standard YAML frontmatter header, or to JSON files, in the repository:
```powershell
# Export Markdown with frontmatter to docs/plans/:
python "<skills_root>/plan-memory/export_plan.py" `
  --project "<project>" `
  --output-dir "docs/plans"

# Export a JSON snapshot:
python "<skills_root>/plan-memory/export_plan.py" `
  --id "<plan_id>" `
  --output "docs/plans/roadmap.json" `
  --format json
```

### 5. List Plans with Progress Bar and Lineage (`list_plans.py`)
Shows every plan, or filters by project and status:
```powershell
# List all plans
python "<skills_root>/plan-memory/list_plans.py"

# Filter by project and status
python "<skills_root>/plan-memory/list_plans.py" --project "<project>" --status "active"
```

### 6. Retrieve a Complete Plan in Full (`get_plan.py`)
Retrieves the full text without truncation to guide the session or export to documentation:
```powershell
# Print to the terminal with a milestone and status header
python "<skills_root>/plan-memory/get_plan.py" --id "<plan_id>"

# Get the project's latest active plan
python "<skills_root>/plan-memory/get_plan.py" --project "<project>"
```

### 7. Semantic Search over Plans (`search_plans.py`)
Searches plans by vector proximity to a concept or keyword:
```powershell
python "<skills_root>/plan-memory/search_plans.py" --query "how to implement the model router in chat" --project "<project>"
```

### 8. Update Lifecycle Status (`update_plan_status.py`)
When steps are completed or new plans replace earlier ones:
```powershell
python "<skills_root>/plan-memory/update_plan_status.py" `
  --id "<plan_id>" `
  --status "completed" `
  --notes "Priority 1 and Priority 2 delivered successfully."
```

---

## 🔒 Mandatory Integration with `dev-cycle-protocol`

The whole development lifecycle follows the governance of:
👉 **[`dev-cycle-protocol`](../dev-cycle-protocol/SKILL.md)**

- **Save Trigger (Step 2: Complete Final Plan)**:
  Whenever a **Complete Final Plan (Blueprint)** or **Strategic Roadmap** is formulated in Step 2, the assistant **MUST** index the full plan via `save_plan.py` in ChromaDB before submitting it to the Step 3 Approval Gate. If it replaces an earlier plan, use `--supersedes`.
- **Drift Check (Step 2 / Pre-Gate)**:
  Use `detect_drift.py` to make sure the proposal does not violate inviolable guidelines or invert the milestones' execution order.
- **Prior Plan Lookup (Step 1: Discovery & Exploration)**:
  When starting a discussion about new features or priorities, the assistant **MUST** check `list_plans.py`, `check_milestone.py --list` or `search_plans.py` to verify the real milestone progress and the guidelines already agreed.
- **Milestone Completion (Step 8: Release)**:
  When closing the release, use `check_milestone.py` to mark the step as delivered or `update_plan_status.py` to finalize the plan.

---

## 🔁 Migrate a Legacy Database (`migrate_legacy.py`)

Copies plans from another database (document, metadata and embeddings) into the shared database, normalizing project names. IDs are kept, preserving `supersedes` and `parent_plan_id`. The source database is not changed.
```powershell
python "<skills_root>/plan-memory/migrate_legacy.py" --source "<legacy_chromadb_folder>" [--rename anycontext=any-context] [--dry-run]
```
