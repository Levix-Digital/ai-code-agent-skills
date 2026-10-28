# Roadmap

The releases of the product in delivery order, from the MVP onwards. Each release has a goal and the backlog items (`docs/BACKLOG.md`) it delivers; the roadmap is the only place that says which item goes into which release. Discovery starts from the open items of the current release (the first one not released), after the bugs that come first.

Each release: `## vX.Y.Z: <name>` with the fields `Status` (planned, in_progress, released), an optional `Target`, `Released` (the date, once released), `Goal` and `Items` (backlog ids separated by commas, `-` when empty). Edit this file with `plan-memory/roadmap.py` or by hand, and check it with `roadmap.py validate`.

## v2.9.0: Baseline and zero friction
- **Status**: planned
- **Goal**: The protocol runs on itself with an approved baseline, plans carry milestone dependencies and newcomers pick an onboarding level
- **Items**: IMP-002, IMP-004, FEAT-006

## v2.10.0: Native engine
- **Status**: planned
- **Goal**: Every script runs as a command of one Rust binary with exactly the same behavior, and the protocol needs no Python
- **Items**: IMP-005

## v2.11.0: Guardrails for agent-written code
- **Status**: planned
- **Goal**: Pull requests are blocked on invented or vulnerable dependencies, weakened tests, changes outside the plan and insecure code
- **Items**: FEAT-011, IMP-006, IMP-007, FEAT-012

## v2.12.0: Cycle feedback
- **Status**: planned
- **Goal**: Production problems come back as triaged bugs, the user's gates come with a review brief, the cycle measures itself and performance budgets are enforced
- **Items**: FEAT-013, FEAT-014, FEAT-015, FEAT-016

## v2.13.0: Agents and platforms
- **Status**: planned
- **Goal**: A role can run in its own context on Claude Code, Codex, Gemini or Copilot, pull request review works on Bitbucket, Azure DevOps and Gitea, and the project's infrastructure as code is checked in CI and applied through the release gates
- **Items**: FEAT-005, FEAT-001, FEAT-010

## v2.14.0: Protocol served by the engine
- **Status**: planned
- **Goal**: Agents run the cycle through the engine's local MCP server with minimal skills, the signed engine cannot be altered, projects adjust it only through validated settings, and the commercial license is chosen
- **Items**: FEAT-009, FEAT-007

## v3.0.0: Role isolation
- **Status**: planned
- **Goal**: Plan sections and review lenses are written by separate specialists in sequence, so the one who reviews is not the one who wrote
- **Items**: FEAT-002

## v3.1.0: Gedo checks
- **Status**: planned
- **Goal**: With Gedo v0.4.0 enabled, the diagnosis, drift detection, design checks, review and gate 8 use Gedo's checks through its versioned --json contract, and skip them safely when a capability is missing
- **Items**: FEAT-004

## v3.2.0: Parallel work on Gedo
- **Status**: planned
- **Goal**: With Gedo v0.5.0 enabled (agent streams, local intents and gedo scope: FEAT-129, FEAT-130 and FEAT-133 there), independent milestones and specialists run in parallel on Gedo agent streams after gate 3 confirms their scopes do not overlap
- **Items**: FEAT-003, FEAT-008
