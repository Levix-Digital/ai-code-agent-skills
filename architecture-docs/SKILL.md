---
name: architecture-docs
description: Keeps a project's technical documentation clean and current - a short index (TECDOC.md), one living page per subsystem in docs/architecture describing how it works today, and immutable Architecture Decision Records in docs/adr - with scripts to create ADRs and pages, regenerate the index, validate links, structure and language, and migrate a legacy single-file TECDOC. Use whenever code changes affect architecture or behavior, a technical decision is made, or the technical docs need to be read, fixed or reorganized.
---

# 🏛️ Architecture Docs Skill (`architecture-docs`)

> **Paths**: `<skills_root>` is the folder that contains this skill (the parent of this `SKILL.md`'s folder). Use the real absolute path when running the commands.

Technical documentation rots in one predictable way: every change is **appended** as a new section ("v0.28.77: ..."), until the document is a changelog that no longer says how the system works. This skill prevents that by separating three kinds of content that must never be mixed:

| Content | Where | Rule |
|---|---|---|
| **How the system works today** | `docs/architecture/<subsystem>.md` | Living document: edited in place. Outdated text is replaced or removed. |
| **Why a decision was made** | `docs/adr/NNNN-<slug>.md` | One decision per file. Immutable once accepted, except its status and relation fields. |
| **When something changed** | GitHub release notes / Git history | Never in the architecture docs. |
| **Entry point** | `TECDOC.md` | Short index: overview, subsystem map and ADR list (the last two generated). |

Paths, limits and language are configurable in the project's `.dev-cycle.json`:
```json
"architecture_docs": {
  "index": "TECDOC.md",
  "architecture_dir": "docs/architecture",
  "adr_dir": "docs/adr",
  "max_lines": 800,
  "language": "en",
  "language_scope": ["index", "architecture", "adr"]
}
```
Add `"plans"` to `language_scope` to also check `docs/plans`.

---

## ✍️ Where Each Change Goes

| What happened | What to do |
|---|---|
| A subsystem's behavior, structure, data or configuration changed | **Edit** its page in `docs/architecture/` in place so it describes the new state. |
| A choice was made between alternatives with trade-offs | **New ADR** (`new_adr`), then link it from the affected subsystem pages. |
| A decision reverses an earlier one | **New ADR** with `--supersedes N`. The old ADR becomes `superseded`; its text is not edited. |
| A decision corrects or narrows part of an earlier one | **New ADR** with `--amends N`. The old ADR stays `accepted` with `amended_by`. |
| A feature or component was removed | **Remove** it from the subsystem page. Its ADRs stay as history. |
| A new subsystem appeared | **New page** (`new_subsystem`). |
| A release was published | **Nothing** here. |

**Never:**
- add a section titled with a version or date, or number headings by hand;
- append "update" paragraphs at the end of a page instead of editing the relevant section;
- rewrite the body of an accepted ADR (create a new one);
- describe planned work as if it existed (plans live in `docs/plans`, see `plan-memory`);
- write in any language other than the configured one (default English), including headings, tables and diagram labels.

---

## 📄 Formats

**Subsystem page** (`docs/architecture/<name>.md`):
```markdown
---
title: Retrieval Pipeline
summary: Finds the chunks that answer a question.
code: [crates/core/src/retrieval]      # source paths it documents; check_docs warns if they disappear
---

# Retrieval Pipeline
## Responsibilities
## Structure
## Behavior
## Interfaces
## Decisions          # links to the ADRs that shaped it
```
Keep each page under `max_lines`; split a large subsystem into several pages.

**ADR** (`docs/adr/0130-real-onnx-runtime.md`):
```markdown
---
id: 130
title: Real ONNX runtime with dynamic loading
status: accepted          # proposed | accepted | rejected | deprecated | superseded
date: '2026-10-15'
version: v0.45.0          # version that shipped it (never in the title)
subsystems: [onnx, rag]
supersedes: []
superseded_by: []
amends: [118, 129]
amended_by: []
---

# ADR-0130: Real ONNX runtime with dynamic loading
## Context
## Decision
## Alternatives Considered
## Consequences
```

---

## ⚙️ Commands

The `.py` scripts are in `<skills_root>/architecture-docs/`. Commands without `.py` run on the native engine: in sh shells (Linux, macOS, Git Bash) `"<skills_root>/dev-cycle-protocol/bin/dev-cycle" architecture-docs <command> [args]`, in PowerShell `& "<skills_root>/dev-cycle-protocol/bin/dev-cycle.ps1" architecture-docs <command> [args]`. All work on the Git repository of the current directory (or `--dir`).

| Task | Command |
|---|---|
| New ADR | `"<skills_root>/dev-cycle-protocol/bin/dev-cycle" architecture-docs new_adr --title "<decision>" [--version v1.2.0] [--subsystem rag,onnx] [--supersedes 129] [--amends 118,123] [--status proposed]` |
| New subsystem page | `"<skills_root>/dev-cycle-protocol/bin/dev-cycle" architecture-docs new_subsystem --name rag --title "Retrieval Pipeline" --summary "<one sentence>" [--code "path1,path2"]` |
| Regenerate the index | `"<skills_root>/dev-cycle-protocol/bin/dev-cycle" architecture-docs build_index` (`--check` to only verify) |
| Validate everything | `check_docs.py [--include-plans] [--max-warnings 0]` |
| Migrate a legacy document | `"<skills_root>/dev-cycle-protocol/bin/dev-cycle" architecture-docs extract_adrs [--source TECDOC.md] [--dry-run]` |

`check_docs.py` exits with code 1 on errors:
- headings with version numbers or manual numbering (index and subsystem pages);
- invalid ADR frontmatter, file name or H1; duplicate ADR ids;
- ADR relations not recorded in both directions; `superseded` without `superseded_by`;
- broken relative links and anchors; outdated index;
- headings not in the configured language.

It warns about long files, body text that looks like another language, ADRs missing a standard section and documented source paths that no longer exist. Put `<!-- lang-ok -->` on a line to exempt a proper name or quote from the language check.

---

## 🔒 Integration with `dev-cycle-protocol`

- **Phase 2 (Plan)**: list the subsystem pages and ADRs the change will touch.
- **Phase 6 (Documentation)**: apply "Where Each Change Goes", run `build_index` and `check_docs.py`. The README is owned by [`user-docs`](../user-docs/SKILL.md) and the structure itself by [`software-design`](../software-design/SKILL.md).
- **Gate 8**: `verify_gate.py --gate 8` runs `check_docs.py`; errors block the release. Projects that have not adopted the layout get a warning only.
- **Phase 0 (Product inception)**: the index, the first subsystem pages and an ADR per founding decision (`product-inception`).
- **Spec Kit**: accepted ADRs are listed in the generated constitution; after accepting, superseding or deprecating an ADR in a project that uses Spec Kit, run `dev-cycle-protocol/constitution.py generate` and commit the result with the ADR.

---

## 🔁 Migrating a Legacy Single-File TECDOC

For a project whose technical document is one long file with versioned sections and inline ADRs:

1. **Extract the ADRs**: `extract_adrs` copies each `ADR-NNN: Title` section to `docs/adr/` (text unchanged, version moved to frontmatter, rectification notes turned into `amends` / `amended_by`, links rewritten). The source is not modified.
2. **Fix each ADR**: translate into the configured language, restructure into Context / Decision / Alternatives Considered / Consequences without losing technical detail, set `subsystems`, and rename the file to an English slug (`git mv`, keeping the `NNNN-` prefix). Never change what was decided; record later corrections only as new ADRs.
3. **Define the subsystems from the code** (crates, packages, main modules), not from the old document's headings. Create one page per subsystem with `new_subsystem`.
4. **Write each subsystem page from the current code**, using the old document only as a source of facts to verify. Drop anything the code no longer does.
5. **Rewrite `TECDOC.md`** as the short index: hand-written overview and diagram, then `build_index` for the generated sections. The old content is removed; Git history keeps it.
6. **Run `check_docs.py`** until it reports zero errors, and review the remaining warnings.

Large migrations work best one subsystem per session, each reading the code it documents.
