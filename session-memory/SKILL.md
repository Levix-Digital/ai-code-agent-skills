---
name: session-memory
description: Persistent multi-project vector memory and context recovery for coding-agent sessions. Keeps, per project, a 2-layer rolling memory in a shared local ChromaDB (20-turn incremental checkpoints + one consolidated Master Summary), every record tagged with its project name. Use at the start of a session to recover a project's context, every 20 turns to checkpoint, and at the end of a session to consolidate.
---

# 🧠 Session Memory Skill (`session-memory`)

A **shared local ChromaDB** vector database that stores, **per project**, the context, architecture decisions, fixes, release history and next steps of development sessions. Works with any coding agent and any project.

- **ChromaDB**: `~/.ai-code-agent-memory/session-memory/db` (override with the `SESSION_MEMORY_DB` variable). It lives outside the skill folder, so the skill can be versioned and copied without carrying data.
- **Collection**: `session_history`
- **Commands**: run on the native engine, `<skills_root>/dev-cycle-protocol/bin/dev-cycle` (`<skills_root>` is the folder that contains this skill's folder). Use the real absolute path.

> [!CAUTION]
> **CRITICAL SYSTEM SAFETY RULE**:
> **NEVER restart or shut down the computer without an EXPLICIT and DIRECT request from the user in the active session.**
> Any autonomous or proactive reboot or shutdown, or one triggered as a consequence of tests, builds, deploys or updates, is strictly forbidden.

---

## 🏷️ Project Isolation (mandatory)

Every record carries the `project` metadata field and an ID namespaced by project:

| Record | Doc ID | Metadata |
|---|---|---|
| Master Summary | `<project>::master_summary` | `project`, `type=master_summary`, `last_updated`, `version` (optional) |
| Turn checkpoint | `<project>::turns::<session_id>::<start>_to_<end>` | `project`, `type=current_session_chunk`, `session_id`, `date`, `start_turn`, `end_turn` |

**How the project is resolved** (in this order): `--project <name>` ➔ `SESSION_MEMORY_PROJECT` variable ➔ name of the Git repository root of the current directory ➔ name of the current directory. The name is normalized to kebab-case (`AnyContext` ➔ `any-context`), the same rule the `plan-memory` skill uses.

Rules for the agent:
1. **At the start of the session**, determine the project (usually the name of the Git repository root you are working in) and **always pass `--project <name>` explicitly** in every command. Do not rely on the shell's current directory, which may change during the session.
2. Never read, write, consolidate or delete memory of **another** project unless the user asks. If the session switches projects, finalize the current project before moving to the next.
3. A project's Master Summary contains **only** facts about that project. Do not mix content across projects.
4. If `read_memory` finds nothing for the project, run `--list-projects`: the project may be stored under another name. Ask the user before assuming it is a new project.

---

## 🔄 2-Layer Memory Architecture (per project)

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│               ROLLING 2-LAYER SESSION MEMORY LIFECYCLE (per project)                   │
├─────────────────────────────────────────┬──────────────────────────────────────────────┤
│ 1. LAYER 1: Consolidated Master Summary │ 2. LAYER 2: 20-Turn Incremental Checkpoints  │
│    ('<project>::master_summary')        │    ('<project>::turns::<sid>::N_to_M')       │
├─────────────────────────────────────────┼──────────────────────────────────────────────┤
│ • ONE single, comprehensive text        │ • Every 20 turns during active session, full │
│ • Consolidates ALL prior sessions       │   raw text of turns 1-20, 21-40 is appended  │
│ • Architecture, decisions, releases     │ • Protects against context loss / crashes    │
├─────────────────────────────────────────┴──────────────────────────────────────────────┤
│ 3. SESSION FINALIZATION & MERGE (End of Session):                                      │
│ • Remaining turns (e.g. 41-53) are recorded to complete current session                │
│ • Current session is synthesized & merged with the project's previous Master Summary   │
│ • Result: NEW single '<project>::master_summary' in ChromaDB                           │
│ • Only THIS project's temporary turn chunks are cleaned up                             │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## ⚙️ Commands

### 1. Recover context (start of session) — `read_memory`
`read_memory` runs on the native engine. In sh shells (Linux, macOS, Git Bash):
```bash
"<skills_root>/dev-cycle-protocol/bin/dev-cycle" session-memory read_memory --project <project>
"<skills_root>/dev-cycle-protocol/bin/dev-cycle" session-memory read_memory --list-projects
```
In PowerShell, run `& "<skills_root>/dev-cycle-protocol/bin/dev-cycle.ps1" session-memory read_memory [arguments]` instead.
If there are turn checkpoints without an updated Master Summary (a previous session that was not finalized), consolidate them first with `finalize_session`.

### 2. Checkpoint every 20 turns — `save_incremental_turns`
`save_incremental_turns` runs on the native engine. In sh shells (Linux, macOS, Git Bash):
```bash
"<skills_root>/dev-cycle-protocol/bin/dev-cycle" session-memory save_incremental_turns --project <project> --start 1 --end 20 --file <turns.md>
```
In PowerShell, run `& "<skills_root>/dev-cycle-protocol/bin/dev-cycle.ps1" session-memory save_incremental_turns [arguments]` instead.
Prefer `--file` (a temporary file) over `--content` for long texts or texts with quotes. `--session-id` is optional (default: today's date).

### 3. End the session and consolidate the Master Summary — `finalize_session`
`finalize_session` runs on the native engine. In sh shells (Linux, macOS, Git Bash):
```bash
"<skills_root>/dev-cycle-protocol/bin/dev-cycle" session-memory finalize_session --project <project> --file <master_summary.md> [--version <x.y.z>]
```
In PowerShell, run `& "<skills_root>/dev-cycle-protocol/bin/dev-cycle.ps1" session-memory finalize_session [arguments]` instead.
The new summary **replaces** the previous one: it must contain everything still valid from the old summary plus what was done in the session. Read the current summary before writing the new one.

### 4. Migrate a legacy database (no project) — `migrate_legacy`
`migrate_legacy` runs on the native engine. In sh shells (Linux, macOS, Git Bash):
```bash
"<skills_root>/dev-cycle-protocol/bin/dev-cycle" session-memory migrate_legacy --source <legacy_chromadb_folder> --project <project> [--dry-run]
```
In PowerShell, run `& "<skills_root>/dev-cycle-protocol/bin/dev-cycle.ps1" session-memory migrate_legacy [arguments]` instead.
Copies records without `project` into the shared database, assigning them to the given project. The source database is not changed.

---

## 📝 Recommended Master Summary Structure

1. Project and architecture overview
2. Project-specific rules and governance
3. Consolidated history of releases and decisions (ADRs)
4. Current status (version, tests, open items)
5. Exact starting point for the next session

---

## 🔒 Governance

If the [`dev-cycle-protocol`](../dev-cycle-protocol/SKILL.md) skill is available, follow it for the development cycle. This skill only handles memory.
