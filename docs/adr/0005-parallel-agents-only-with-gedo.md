---
id: 5
title: Parallel agents only with Gedo
status: accepted
date: '2026-10-25'
version: null
subsystems: [cycle, planning]
supersedes: []
superseded_by: []
amends: []
amended_by: []
---

# ADR-0005: Parallel agents only with Gedo

## Context

Running several agents at once in one repository causes collisions that plain Git cannot detect while they happen. Gedo detects collisions at the entity level and is Levix's own product, which must be dogfooded. Running roles in separate contexts also removes the confirmation bias of one agent reviewing its own work.

## Decision

The protocol runs agents in parallel only when Gedo is enabled (`gedo.enabled`). Without Gedo there is no parallelism of any kind: each role can still run in its own context, one after the other (FEAT-002), and parallel milestones and concurrent role runs exist only on Gedo (FEAT-003, FEAT-004). There is no fallback to Git worktrees.

## Alternatives Considered

- **Parallel agents on Git worktrees, Gedo when available**: works today, but collisions surface only at merge time, and it would compete with dogfooding Gedo.
- **Concurrent read-only agents without Gedo** (plan sections, review lenses): no Git writes, but still parallelism outside the rule; rejected to keep one simple rule.

## Consequences

- Parallel work waits for Gedo v0.5.0 (local streams and hidden checkouts for agents; v0.4.0 only brings the command line) and Gedo support for the project's language (Python in v0.6.0 for this repository).
- Role isolation ships earlier, in sequence, with the same benefit for review quality and a longer run time.
