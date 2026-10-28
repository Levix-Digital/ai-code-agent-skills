---
title: Project Configuration and Shared Kernel
summary: The .dev-cycle.json configuration with its defaults and validation, and the attribution guard every posted text passes.
code: [crates/kernel/src/project_config.rs, crates/kernel/src/py, dev-cycle-protocol/project_config.py, dev-cycle-protocol/_no_ai.py]
---

# Project Configuration and Shared Kernel

The .dev-cycle.json configuration with its defaults and validation, and the attribution guard every posted text passes.

## Responsibilities

Owns the project's configuration file `.dev-cycle.json` (defaults, validation, migration of old keys) and the attribution guard that every text the protocol writes or posts passes (principle 9). Every skill reads its section through this kernel.

## Structure

- `crates/kernel/src/project_config.rs` (engine command `dev-cycle-protocol project_config`): `load_project_config()` finds the manifest from any folder of the repository and merges it over the defaults; `validate()` lists problems; `migrated()` rewrites legacy keys. Translated from `dev-cycle-protocol/project_config.py`, which stays for the Python scripts that import it until they are translated (IMP-005).
- `crates/kernel/src/py`: the Python behavior the translated commands keep: `json` (CPython's decoder messages, `dumps`), `argparse` (prefix matching, help layout, errors), `textwrap`, `os.path`, and `str()`/`repr()` of values.
- `dev-cycle-protocol/_no_ai.py`: `findings()` and `check()` detect AI attribution (trailers, footers, session links, agent names used as authors) while allowing technical references and configured `allowed_terms`.

## Behavior

Sections are optional; a missing section takes its defaults. Keys are only added: `onboard.py write-config` refuses to overwrite a key already set. Unknown top-level keys are reported, not rejected.

## Interfaces

Read by every skill. The guard is called by `pr_sync.py` before posting and by checkers that scan commits and docs.

## Decisions

- [ADR-0002](../adr/0002-modular-monolith-of-skills-by-role.md): the shared kernel stays small and backward compatible
