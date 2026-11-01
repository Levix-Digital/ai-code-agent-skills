---
name: folder-sync
description: Smart sync and incremental vector RAG for any folder/workspace using ChromaDB. Recursively indexes documents (.docx, .pdf, .md, .txt, code) via the /sync command and lets you talk to the folder's knowledge base via /folder.
---

# Folder Sync & Brain (Incremental RAG for Folders)

> **Paths**: `<skills_root>` is the folder that contains this skill (the parent of this `SKILL.md`'s folder). Use the real absolute path when running the commands.

This skill lets the agent recursively index any local folder into a local **ChromaDB** vector database **incrementally** (via SHA-256 hash) and run high-precision semantic searches (macro and micro) without blowing the conversation's token limit.

## Commands and Triggers

### 1. `/sync` (Incremental Sync of the Current Directory)
When the user types `/sync` or asks to sync/index the current folder:
1. Run the sync (native engine; in PowerShell, `& "<skills_root>/dev-cycle-protocol/bin/dev-cycle.ps1" folder-sync sync --dir "."`):
   ```bash
   "<skills_root>/dev-cycle-protocol/bin/dev-cycle" folder-sync sync --dir "."
   ```
2. The sync will:
   - Recursively scan every supported file (`.docx`, `.pdf`, `.md`, `.txt`, `.json`, `.csv`, `.py`, `.ts`, etc.).
   - Automatically ignore `.git`, `node_modules`, `.venv`, `.folder_brain` and non-text binaries.
   - Check each file's SHA-256 hash against the local cache. If the file has not changed, **it is skipped in milliseconds**.
   - If the file was modified/created, extract the text, build smart overlapping chunks and update the local ChromaDB (`.folder_brain/chroma_db`).
   - Detect `.gdoc` files and warn about cloud documents that still need to be exported.
3. Present a clear summary: total indexed files, new/modified, unchanged and generated chunks.

---

### 2. `/folder` (Activate and Query the Folder Context)
When the user types `/folder` (or asks questions about the folder's content after indexing it):
1. Run the semantic search on the folder's ChromaDB (native engine; in PowerShell, `& "<skills_root>/dev-cycle-protocol/bin/dev-cycle.ps1" folder-sync query --dir "." --query "<user question or topic>" --top-k 6`):
   ```bash
   "<skills_root>/dev-cycle-protocol/bin/dev-cycle" folder-sync query --dir "." --query "<user question or topic>" --top-k 6
   ```
2. The search returns:
   - **Macro View**: Summary of the relevant documents found.
   - **Micro Passages (Chunks)**: Exact paragraphs with file paths, page/section number and similarity score.
3. Use the returned passages to answer with maximum precision, citing files as clickable markdown links (`[file_name](file:///absolute_path)`).

---

### 3. Statistics or Cleanup (`/folder-stats`, `/folder-clean`)
- **Show index statistics** (native engine; in PowerShell, `& "<skills_root>/dev-cycle-protocol/bin/dev-cycle.ps1" folder-sync stats --dir "."`):
  ```bash
  "<skills_root>/dev-cycle-protocol/bin/dev-cycle" folder-sync stats --dir "."
  ```
- **Reset / reindex from scratch** (native engine; in PowerShell, `& "<skills_root>/dev-cycle-protocol/bin/dev-cycle.ps1" folder-sync sync --dir "." --force`):
  ```bash
  "<skills_root>/dev-cycle-protocol/bin/dev-cycle" folder-sync sync --dir "." --force
  ```
- **Remove the folder's index** (native engine; in PowerShell, `& "<skills_root>/dev-cycle-protocol/bin/dev-cycle.ps1" folder-sync clean --dir "."`):
  ```bash
  "<skills_root>/dev-cycle-protocol/bin/dev-cycle" folder-sync clean --dir "."
  ```

---

## Skill Layout
- `sync` (engine command `folder-sync sync`, code in `crates/folder-sync`): Incremental indexing and hierarchical chunking, with an extractor for each file format (.docx, .pdf, .txt, .md, code).
- `query` (engine command `folder-sync query`, code in `crates/folder-sync`): Semantic search with ranking and formatting.
- `stats` (engine command `folder-sync stats`, code in `crates/folder-sync`): Statistics inspector for the folder's index.
- `clean` (engine command `folder-sync clean`, code in `crates/folder-sync`): Removes the folder's `.folder_brain` index.
