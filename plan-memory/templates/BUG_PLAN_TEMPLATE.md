# {title}

Bug plan: discovery and planning of the backlog items below (`dev-cycle-protocol` Phases 1 and 2). Replace every instruction paragraph with the findings.

## Backlog items

{items}

## 1. Reproduction

Exact steps, environment and version that reproduce the problem. Read the real logs first and quote the relevant lines.

## 2. Evidence and root cause

What the code does and why it fails: files, functions and data involved. Separate confirmed facts from hypotheses; a hypothesis is confirmed before the fix is designed.

## 3. Fix design (`software-design`)

The smallest correct change, where it lives, and what is deliberately left out. Decisions with alternatives become ADRs (`architecture-docs`).

## 4. Affected surfaces and files

Interfaces in `architecture.surfaces` that change, files and modules, and other places with the same defect.

## 5. Tests (`quality-assurance`)

The regression test that fails before the fix and passes after it; manual tests to add, change or remove.

## 6. Documentation (`architecture-docs`, `user-docs`)

Subsystem pages, ADRs and README sections to correct.

## 7. Release, migration and rollback (`devops-release`)

Version impact, data already affected by the bug and how it is repaired, how to roll back.

## 8. Execution steps and milestones
