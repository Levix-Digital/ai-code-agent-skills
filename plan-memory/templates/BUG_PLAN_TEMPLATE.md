# {title}

Bug plan: discovery and planning of the backlog items below (`dev-cycle-protocol` Phases 1 and 2). Replace every instruction paragraph with the findings.

## Backlog items

{items}

## 1. Triage

Is it really a bug? Before planning a fix:
- **Expected vs. actual**: what happens, what should happen, and where the expected behavior is defined (documentation, specification, tests, an earlier version). No source for the expectation means it may be a feature request.
- **Verdict**: `bug`, or not a bug: works as designed, misuse or configuration, environment or third party, feature request, duplicate of another item. When it is not a bug, stop here and propose to the user how to close or convert the item (`backlog.py drop`, `duplicate`, or capture a feature and `supersede`).
- **Suspected code paths**: files and functions most likely involved, found from the logs, the stack trace and a search of the code, each with why it is suspected.
- **Severity**: confirm the backlog severity or propose a new one with the reason.

## 2. Reproduction

Exact steps, environment and version that reproduce the problem. Read the real logs first and quote the relevant lines.

## 3. Evidence and root cause

What the code does and why it fails: files, functions and data involved. Separate confirmed facts from hypotheses; a hypothesis is confirmed before the fix is designed.

## 4. Fix design (`software-design`)

The smallest correct change, where it lives, and what is deliberately left out. Decisions with alternatives become ADRs (`architecture-docs`).

## 5. Affected surfaces and files

Interfaces in `architecture.surfaces` that change, files and modules, and other places with the same defect.

## 6. Tests (`quality-assurance`)

The regression test that fails before the fix and passes after it; manual tests to add, change or remove.

## 7. Documentation (`architecture-docs`, `user-docs`)

Subsystem pages, ADRs and README sections to correct.

## 8. Release, migration and rollback (`devops-release`)

Version impact, data already affected by the bug and how it is repaired, how to roll back.

## 9. Execution steps and milestones
