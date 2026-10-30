---
title: Spec Kit Integration
summary: Installs, pins, adopts and upgrades GitHub Spec Kit per project and generates its constitution from the baseline.
code: [dev-cycle-protocol/speckit_setup.py, dev-cycle-protocol/constitution.py, crates/dev-cycle-protocol, dev-cycle-protocol/speckit-preset]
---

# Spec Kit Integration

Installs, pins, adopts and upgrades GitHub Spec Kit per project and generates its constitution from the baseline.

## Responsibilities

Owns GitHub Spec Kit in a project: install at a pinned version, adopt an existing install, check compatibility, upgrade, and generate the constitution from the protocol's sources of truth. Spec Kit is the optional executor of the detail of features (specification, clarification, plan, tasks, implementation).

## Structure

- `dev-cycle-protocol/speckit_setup.py`: `status`, `latest`, `check`, `init`, `adopt`, `upgrade`; always runs the pinned CLI through `uvx`.
- `constitution` (engine, `crates/dev-cycle-protocol`; `dev-cycle-protocol/constitution.py` stays for `onboard.py`, which imports it): generates `.specify/memory/constitution.md` from `software-design`, the cycle's principles, the technical specification and the accepted ADRs; never edited by hand.
- `dev-cycle-protocol/speckit-preset/`: the `dev-cycle` preset with the plan addendum and the verified versions (`compat.json`).

## Behavior

The version lives in `speckit.version`; drift between it and the installed one is reported by the diagnosis. Feature branches follow `feat/NNN-slug`. After a change to the baseline or an ADR, the constitution is regenerated.

## Interfaces

Called by `/dev-cycle` setup items and Phase 0. Calls `uvx` and the Spec Kit CLI.

## Decisions

- [ADR-0001](../adr/0001-skills-in-the-open-skill-md-format-with-python-scripts.md): Python scripts that wrap the pinned CLI
