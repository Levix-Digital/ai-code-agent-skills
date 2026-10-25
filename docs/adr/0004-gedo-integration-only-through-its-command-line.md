---
id: 4
title: Gedo integration only through its command line
status: accepted
date: '2026-10-25'
version: null
subsystems: [cycle, quality-release]
supersedes: []
superseded_by: []
amends: []
amended_by: []
---

# ADR-0004: Gedo integration only through its command line

## Context

Gedo (Levix-Digital/gedo) is the entity-level collaboration overlay the protocol needs for parallel agents. Both are Levix products with separate open source futures and a paid platform that joins them in a visual and cloud layer. Gedo is a library today; its command line arrives in v0.4.0.

## Decision

The protocol integrates with Gedo only through Gedo's command line with `--json` output (`gedo doctor`, `radar`, `delta`, `stream` and later commands), behind `gedo.enabled` in `.dev-cycle.json`. It never links Gedo's code, and the two repositories stay separate. Problems found in real use are recorded with `gedo-feedback`.

## Alternatives Considered

- **Merge the repositories into one product**: a single release train, but each engine loses its own adoption path, and the open-core model needs two free engines.
- **Link Gedo as a library** (Python bindings): tighter integration, but couples the protocol's releases to Gedo's build and platforms.

## Consequences

- Each project evolves and releases on its own; the protocol depends on a documented JSON contract.
- No Gedo-backed feature ships before the Gedo version that provides its command.
