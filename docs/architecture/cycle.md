---
title: Cycle Orchestration
summary: The /dev-cycle entry command, the phases and roles, the onboarding diagnosis and the gates.
code: [dev-cycle, dev-cycle-protocol/SKILL.md, dev-cycle-protocol/onboard.py, dev-cycle-protocol/_onboard.py, dev-cycle-protocol/verify_gate.py, dev-cycle-protocol/inspect_project.py]
---

# Cycle Orchestration

The /dev-cycle entry command, the phases and roles, the onboarding diagnosis and the gates.

## Responsibilities

Owns the cycle itself: the single entry command, the phases 0 to 10 with the role that acts in each one, the full and short paths, the user's gates and the checks behind them. It does not own the content each role produces (plans, reviews, tests, docs); it calls the role skills for that.

## Structure

- `dev-cycle/SKILL.md`: the `/dev-cycle` command: diagnose, propose the options in order, do what the user chose, diagnose again.
- `dev-cycle-protocol/SKILL.md`: phases, roles, principles, gates and the rules of each path.
- `dev-cycle-protocol/onboard.py`: reads the repository (configuration, branching model, branches, product baseline, plans, release candidate, bugs, roadmap, Spec Kit, pull requests, protocol files the ignore rules hide) and writes only after the user confirms (`write-config`, `create-branch`, `set-attribution`).
- `dev-cycle-protocol/_onboard.py`: pure rules that turn that state into the ordered options and the recommendation.
- `dev-cycle-protocol/verify_gate.py`: gates `0`, `3`, `rc` and `8`, each running the role skills' checkers.
- `dev-cycle-protocol/inspect_project.py`: detects stack, build tools, test runners and surfaces.

## Behavior

The diagnosis never writes. Competing work comes first, in this order: a plan in progress, a pull request with pending items, a pending release candidate, open blocking bugs and bugs not triaged, then setup items, then the request. When a session starts outside a repository, the only repository below is used; with several, the user chooses. Spec Kit and the branching model are asked about once and recorded in `.dev-cycle.json`.

## Interfaces

- Called by the user through the agent (`/dev-cycle`) and by the protocol's phases.
- Calls `plan-memory` (backlog, roadmap, plans), `pr_sync.py`, `quality-assurance`, `devops-release`, `architecture-docs`, `user-docs`, `product-inception` and Spec Kit setup.

## Decisions

- [ADR-0001](../adr/0001-skills-in-the-open-skill-md-format-with-python-scripts.md): skills in the open format with Python scripts
- [ADR-0002](../adr/0002-modular-monolith-of-skills-by-role.md): modular monolith of skills by role
- [ADR-0004](../adr/0004-gedo-integration-only-through-its-command-line.md): Gedo only through its command line
- [ADR-0005](../adr/0005-parallel-agents-only-with-gedo.md): parallel agents only with Gedo
