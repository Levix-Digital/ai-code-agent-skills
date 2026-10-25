---
id: 2
title: Modular monolith of skills by role
status: accepted
date: '2026-10-25'
version: null
subsystems: [cycle, configuration]
supersedes: []
superseded_by: []
amends: []
amended_by: []
---

# ADR-0002: Modular monolith of skills by role

## Context

The protocol has many roles (product, planner, architect, reviewer, QA, DevOps, writers) that evolve at different speeds but ship together in one plugin and share the project configuration.

## Decision

The repository is a modular monolith: one module per skill, each with a small public interface (its `SKILL.md` and its script commands), and a small shared kernel (`dev-cycle-protocol/project_config.py` for `.dev-cycle.json`, `dev-cycle-protocol/_no_ai.py` for principle 9). Skills talk through files in the project (backlog, roadmap, plans, product docs, ADRs) and documented commands. The gates (`verify_gate.py`) call the role skills' checkers; role skills never call the orchestrator.

## Alternatives Considered

- **One skill per repository**: independent releases, but every change to the shared configuration becomes a cross-repository change.
- **A single large skill**: no boundaries; every agent loads all of it for any task.

## Consequences

- Skills load only when their task needs them; a change to one role rarely touches another.
- The shared kernel must stay small and backward compatible: configuration keys are only added, and old files keep working.
