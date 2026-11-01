---
title: Memory and Support Skills
summary: Each developer's session memory, folder RAG and the Gedo feedback log.
code: [session-memory, crates/session-memory, folder-sync, gedo-feedback]
---

# Memory and Support Skills

Each developer's session memory, folder RAG and the Gedo feedback log.

## Responsibilities

Owns the support skills: each developer's session memory per project, retrieval over any folder, and the Gedo feedback log.

## Structure

- `session-memory/`: saves and recovers session context in a local ChromaDB, namespaced by project.
- `crates/session-memory` (engine): `read_memory`, `save_incremental_turns`, `finalize_session` and the helpers of `session-memory/_common.py` it needs (project name, store path, collection), over `crates/vector-store`.
- `folder-sync/scripts/`: incremental indexing (`sync.py`), questions (`query.py`), stats and cleanup over a folder, with text, `.docx` and `.pdf` extractors.
- `crates/gedo-feedback/src/gedo_feedback.rs` (engine command `gedo-feedback gedo_feedback`): one Markdown file per Gedo problem in `docs/gedo-feedback/`, off until `gedo.enabled`.

## Behavior

Session memory and indexes live under `~/.ai-code-agent-memory/` and are never versioned. Gedo feedback files are versioned, one per entry, so parallel threads never conflict.

## Interfaces

Called by the agent on demand (`/sync`, `/folder`) and by the cycle when Gedo is enabled.

## Decisions

- [ADR-0001](../adr/0001-skills-in-the-open-skill-md-format-with-python-scripts.md): skills in the open format with Python scripts
