---
title: Design, Quality and Release
summary: The architecture style checks, test strategy and manual test runs, branching models, CI policy and releases.
code: [software-design, quality-assurance, devops-release]
---

# Design, Quality and Release

The architecture style checks, test strategy and manual test runs, branching models, CI policy and releases.

## Responsibilities

Owns how code is structured and checked, how it is tested and how it is released: the architecture style, the test strategy and manual test runs, the branching models, the CI policy, versions and release candidates.

## Structure

- `software-design/`: universal principles, style guides and `check_design.py` (style, core path, forbidden core imports).
- `quality-assurance/manual_tests.py`: validates `tests/MANUAL_TESTS.md`, starts release candidate runs and gates them.
- `devops-release/branching.py`: the `integration`, `trunk` and `release-branches` models; `check_ci_policy.py`: workflow policy; `release_status.py`: versions and next candidate tag; `templates/`: CI, tag-on-merge, promote and release workflows.

## Behavior

The branching model decides where work starts, where pull requests go, which branch a release candidate is tagged on and which action of the user releases. Gate `rc` needs every manual test of the run to pass; gate 8 runs the docs, design, README and CI checks before the release gate.

## Interfaces

Called by the cycle in Phases 2, 4, 5, 8 to 10 and by gates `rc` and 8.

## Decisions

- [ADR-0001](../adr/0001-skills-in-the-open-skill-md-format-with-python-scripts.md): checkers as Python scripts
- [ADR-0004](../adr/0004-gedo-integration-only-through-its-command-line.md): Gedo checks join the gates only through its command line
- [ADR-0006](../adr/0006-gedo-contract-discovered-by-capability-with-safe-fallback-pe.md): a missing Gedo capability skips that check safely and is recorded as feedback
