---
title: Documentation
summary: Living technical docs with ADRs and end-user READMEs, each with a checker.
code: [architecture-docs, user-docs]
---

# Documentation

Living technical docs with ADRs and end-user READMEs, each with a checker.

## Responsibilities

Owns the technical documentation (how the system works today and why) and the end-user READMEs.

## Structure

- `architecture-docs/`: `TECDOC.md` index, `docs/architecture/` living pages, `docs/adr/` immutable ADRs; `new_adr`, `new_subsystem`, `build_index` and `extract_adrs` (engine), `check_docs.py`.
- `user-docs/`: README template and `check_readme.py` (standard sections, current state only, English, links, length).

## Behavior

Pages are edited in place; decisions go to new ADRs (superseding or amending older ones); release history stays in Git and release notes. The generated parts of `TECDOC.md` are rebuilt by `build_index`.

## Interfaces

Called in Phase 0 (founding ADRs) and Phase 6, and by gate 8.

## Decisions

- [ADR-0001](../adr/0001-skills-in-the-open-skill-md-format-with-python-scripts.md): checkers as Python scripts
