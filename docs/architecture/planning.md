---
title: Product and Planning
summary: Product inception, the backlog with epics, the roadmap of releases and the versioned plans with milestones.
code: [product-inception, plan-memory, backlog, crates/plan-memory]
---

# Product and Planning

Product inception, the backlog with epics, the roadmap of releases and the versioned plans with milestones.

## Responsibilities

Owns what gets built and in which order: product inception (Phase 0), the backlog with epics, the roadmap of releases and the complete plans with milestones.

## Structure

- `product-inception/`: the Phase 0 process and `check_product` (engine) for `docs/product/VISION.md` and `TECH_SPEC.md`.
- `plan-memory/backlog.py` and `backlog/`: capture, triage, close and promote items in `docs/BACKLOG.md`; bugs carry a severity, others a priority.
- `crates/plan-memory` (engine): `list_plans`, `get_plan`, `export_plan` and the plan store they read (`PlanStore` and `read_plan` of `plan-memory/_store.py`, which stays for the Python scripts that import it), `rebuild_index` with the plan search index it rebuilds (`crates/vector-store`: chromadb's local store, shared with the Python scripts), and `roadmap`: releases and item assignment in `docs/ROADMAP.md`, with the backlog reader it needs (also the blocking bugs, the bug triage and the backlog validation that `inspect_project` and `verify_gate` use, and the plan's status log that gate 3 writes when the user accepts open bugs). `plan-memory/_roadmap.py` and `_roadmap_store.py` stay for the Python scripts that import them (`backlog.py`), and `roadmap.py` for the backlog tests that drive it (`tests/test_roadmap.py`).
- `plan-memory/save_plan.py`, `check_milestone.py`, `update_plan_status.py`, `search_plans.py`, `detect_drift.py`: plans as Markdown with YAML frontmatter in `docs/plans/`, lineage, milestones, search and drift checks.

## Behavior

Files are the source of truth; the ChromaDB index under `~/.ai-code-agent-memory/` is disposable and rebuilt from them. Bugs first by severity: critical, high and medium bugs come before new work, low bugs go into the next release at the latest, and no bug stays untriaged.

## Interfaces

Called by the cycle (Phases 0 to 3, gates 0 and 3) and the `/backlog` command. Writes files; never commits.

## Decisions

- [ADR-0001](../adr/0001-skills-in-the-open-skill-md-format-with-python-scripts.md): plans and backlog as files read by any agent
- [ADR-0005](../adr/0005-parallel-agents-only-with-gedo.md): milestone dependencies prepare parallel work on Gedo
