---
id: 7
title: Native Rust engine translated from the Python scripts
status: proposed
date: '2026-10-27'
version: null
subsystems: [cycle, configuration, planning, quality-release, documentation, memory]
supersedes: []
superseded_by: []
amends: [1]
amended_by: []
---

# ADR-0007: Native Rust engine translated from the Python scripts

## Context

ADR-0001 runs every check through Python 3.10+ scripts with third-party packages (`chromadb`, `pyyaml`, `python-docx`, `pypdf`). Agents often run a Python without them: in a Claude Code cloud session on 2026-10-27, `import chromadb` failed and `save_plan.py` stopped after writing the plan file. ADR-0001 kept a compiled tool as the alternative to revisit. The protocol has 40 scripts and 14 shared modules (about 10,500 lines) and 4,000 lines of tests; three skills (architecture-docs, session-memory, folder-sync) have no tests.

## Decision

- The scripts are translated into one Rust binary, built from a Cargo workspace in this repository: one crate per skill, a small kernel crate and the binary crate. The skill folders keep their `SKILL.md` files and templates.
- The translation changes no behavior: same commands and flags, same stdout and stderr, same exit codes, same files read and written. A bug found on the way is kept identical or fixed in both languages, never in Rust only.
- Identity is proven by a parity harness that runs the Python script and the Rust command on the same fixtures and compares everything above byte for byte; the Python outputs are kept as golden files once the script is deleted.
- One script per pull request: when its outputs are identical, its `SKILL.md` lines and callers switch to the Rust command and the Python script is deleted. Shared modules go when their last importer goes. When none is left, `requirements.txt` and the pytest suites are removed.
- Integration with Gedo stays as in ADR-0004 and ADR-0006: through its command line only, no Gedo crates.

## Alternatives Considered

- **Keep Python and run it through `uv` (IMP-003)**: removes manual installs but keeps a Python runtime and package downloads in every environment.
- **Rewrite with new behavior** (other search backend, tree-sitter checks, parallel gates): larger gains, but no way to prove that nothing broke; out of scope by the user's decision.
- **Translate everything in one pull request**: shorter calendar, but a diff too large to review and no working protocol in between.

## Consequences

- No Python, `pip` or virtual environment needed to run the protocol, once the binary reaches the agent (distribution decided before the first swap).
- During the translation the protocol is mixed: Python callers such as `verify_gate.py` call the Rust command for swapped checkers.
- The chromadb scripts need decisions before their turn: tolerance for embedding scores, how Rust reads the existing local store, and PDF text extraction.
- Verify: after each swap the parity cases and the existing tests pass; at the end `/dev-cycle` and gates 0, 3, rc and 8 give the same results on this repository as before.
