# Backlog

Work captured for later planning: bugs, features, improvements and adjustments, grouped under epics when the product is planned as a whole. Capturing an item takes no discovery; when the user decides to work on items, they are promoted into a plan (`plan-memory/backlog.py promote`), where the discovery and the planning happen. Open bugs come before new work.

Each item: `### <ID>: <title>` with the fields `Severity` (bugs: critical, high, medium, low) or `Priority` (others: high, medium, low), an optional `Epic`, `Status` (open, planned, in_progress), `Found`, `Plan` and `Description`. Epics (`EPIC-NNN`) group items of the complete product; they have no `Plan` and are never promoted: their items are. The roadmap (`docs/ROADMAP.md`) says which items go into which release. Closed items (done, dropped, duplicate, superseded by better defined items) move to `Closed` with a `Closed` note and keep their ids, which are never reused. Edit this file with `plan-memory/backlog.py` or by hand, and check it with `backlog.py validate`.

## Bugs

## Features

### FEAT-001: Pull request review on Bitbucket, Azure DevOps and Gitea
- **Priority**: urgent
- **Epic**: EPIC-003
- **Status**: open
- **Found**: 2026-10-23, pull request review plan discussion
- **Plan**: -
- **Description**: The pull request review flow starts with GitHub and GitLab. Analyze and add adapters for Bitbucket, Azure DevOps and Gitea (reading threads, comments, suggestions, reviews and CI; replying, resolving and posting reviews).

### FEAT-002: Role isolation in sequence
- **Priority**: high
- **Epic**: EPIC-004
- **Status**: open
- **Found**: 2026-10-25, 2026-10-25, platform strategy discussion
- **Plan**: -
- **Description**: Run each role specialist in its own context, one after the other: plan sections in Phase 2 (design, tests, documentation, release) and review lenses in Phase 7 (plan and architecture, complexity and security, tests and docs), with the orchestrator merging the results. No parallelism (ADR-0005); concurrent runs come with Gedo (FEAT-003).

### FEAT-003: Parallel milestones and roles on Gedo streams
- **Priority**: high
- **Epic**: EPIC-004
- **Status**: open
- **Found**: 2026-10-25, 2026-10-25, platform strategy discussion
- **Plan**: -
- **Description**: With gedo.enabled only: resolve the milestone DAG of an approved plan and run independent milestones as parallel workers on Gedo agent streams (gedo stream new --agent with dc.plan, dc.milestone and dc.role labels) within their allowed scopes, with a build lock owned by the protocol for test commands; run the FEAT-002 specialists concurrently. Needs Gedo v0.5.0 (agent streams and local intents) and Gedo support for the project's language. No worktree fallback (ADR-0005).

### FEAT-004: Gedo checks in the protocol's scripts
- **Priority**: high
- **Epic**: EPIC-004
- **Status**: open
- **Found**: 2026-10-25, 2026-10-25, platform strategy discussion
- **Plan**: -
- **Description**: When gedo.enabled: gedo doctor and radar in the diagnosis, symbol-level drift from gedo delta in detect_drift, contract checks from gedo check contracts in check_design, gedo trace in the review, a clean gedo doctor at gate 8 and the delta summary on the pull request. Only through the command line with --json, discovered by capability and skipped safely when missing (ADR-0004, ADR-0006). Needs Gedo v0.4.0.

### FEAT-005: Role runners for the main coding agents
- **Priority**: high
- **Epic**: EPIC-005
- **Status**: open
- **Found**: 2026-10-25, 2026-10-25, platform strategy discussion
- **Plan**: -
- **Description**: Start a role in its own context with the agent configured per role (orchestration.role_runners): Claude Code, Codex, Gemini and Copilot, using the agent's native sub-agents when it has them. Prompt = the plan section, the role's SKILL.md and, with Gedo, the milestone scopes and stream.

### FEAT-006: Onboarding in three levels
- **Priority**: medium
- **Epic**: EPIC-006
- **Status**: open
- **Found**: 2026-10-25, 2026-10-25, platform strategy discussion
- **Plan**: -
- **Description**: Lite (dev-cycle, backlog and pull request review, no Phase 0 or release candidates), Standard (plans, design checks, technical docs) and Full (phases 0 to 10, Spec Kit, release candidates, Gedo when enabled), in the README and as a choice in /dev-cycle.

### FEAT-007: Open source launch
- **Priority**: medium
- **Epic**: EPIC-006
- **Status**: open
- **Found**: 2026-10-25, 2026-10-25, platform strategy discussion
- **Plan**: -
- **Description**: Choose the license (MIT, Apache-2.0 or both), prepare public docs and a contribution guide, and publish together with Gedo v0.5.0 or v0.6.0. Until then the repository stays unlicensed.

### FEAT-008: Non-overlapping milestone scopes at gate 3 with gedo scope
- **Priority**: high
- **Epic**: EPIC-004
- **Status**: open
- **Found**: 2026-10-25, Gedo integration contract 2026-10-25
- **Plan**: -
- **Description**: When gedo.enabled and Gedo has the scope capability: at gate 3, check with gedo scope --json that the milestones marked to run in parallel do not share entities or their dependency closure; overlapping milestones run in sequence. Needs Gedo v0.5.0 (FEAT-133 there) and Gedo support for the project's language.

## Improvements

### IMP-002: Adopt the protocol in this repository (dogfooding)
- **Priority**: high
- **Epic**: EPIC-001
- **Status**: open
- **Found**: 2026-10-25, 2.8.0, /dev-cycle run on this repository
- **Plan**: -
- **Description**: This repository has no .dev-cycle.json, product baseline (VISION, TECH_SPEC, epics) or ROADMAP, so the protocol's own rules (bugs first, low bugs by the next release, gate 8) do not apply to it. Run the /dev-cycle setup: configuration (trunk detected), the Spec Kit decision, and adoption mode for the baseline.

### IMP-004: Milestone dependencies in plans
- **Priority**: high
- **Epic**: EPIC-002
- **Status**: open
- **Found**: 2026-10-25, 2026-10-25, platform strategy discussion
- **Plan**: -
- **Description**: Plan frontmatter milestones gain optional id, depends_on, role and allowed_scopes (backward compatible); check_milestone and gate 3 validate the DAG (no cycles, known ids); Spec Kit [P] tasks map to independent milestones. Prepares FEAT-003; useful in sequence too.

### IMP-005: Translate the scripts to Rust with identical behavior
- **Priority**: high
- **Epic**: EPIC-006
- **Status**: open
- **Found**: 2026-10-27, 2026-10-27, Rust migration discussion
- **Plan**: -
- **Description**: Translate every Python script of the skills into one Rust binary with no change in behavior: same commands, flags, stdout, stderr, exit codes and files written, proven by comparing the Python and Rust outputs on the same fixtures. One script per pull request: when the outputs are identical, the SKILL.md calls the Rust command and the Python script is deleted. When no script is left, the Python requirements and tests are removed.

## Adjustments

## Epics

### EPIC-001: Cycle and gates
- **Priority**: high
- **Status**: open
- **Found**: 2026-10-25, 2026-10-25, platform strategy discussion
- **Description**: The /dev-cycle command, phases 0 to 10, gates and the merge gate, and the project configuration.

### EPIC-002: Product and planning
- **Priority**: high
- **Status**: open
- **Found**: 2026-10-25, 2026-10-25, platform strategy discussion
- **Description**: Product inception, backlog with epics, roadmap of releases and versioned plans with milestones and their dependencies.

### EPIC-003: Pull requests and review
- **Priority**: medium
- **Status**: open
- **Found**: 2026-10-25, 2026-10-25, platform strategy discussion
- **Description**: The pull request as the channel and the protocol's code review on GitHub, GitLab, Bitbucket, Azure DevOps and Gitea.

### EPIC-004: Multi-agent orchestration
- **Priority**: high
- **Status**: open
- **Found**: 2026-10-25, 2026-10-25, platform strategy discussion
- **Description**: Each role in its own context; with Gedo only (ADR-0005), parallel milestones over streams and Gedo's checks in the gates.

### EPIC-005: Agents and environments
- **Priority**: high
- **Status**: open
- **Found**: 2026-10-25, 2026-10-25, platform strategy discussion
- **Description**: Runners for the main coding agents (Claude Code, Codex, Gemini, Copilot) on a local machine and in cloud sessions, with no code specific to one environment.

### EPIC-006: Zero-friction adoption
- **Priority**: high
- **Status**: open
- **Found**: 2026-10-25, 2026-10-25, platform strategy discussion
- **Description**: Execution without manual installs, standard-library search, onboarding in three levels and the open source launch.

## Closed

### IMP-001: Triage section in the bug plan template
- **Priority**: high
- **Status**: done
- **Found**: 2026-10-23, PR #5 discussion
- **Plan**: -
- **Closed**: 2026-10-23, Triage section in the bug plan template
- **Description**: Borrow the assess step of Spec Kit's bug extension: before the fix is planned, a bug plan first judges whether the report is really a bug (reproduces, expected behavior, not a misuse or a feature request) and locates the suspected code paths. Add a 'Triage' section to the bug plan template (plan-memory/templates/BUG_PLAN_TEMPLATE.md) and the matching rule in quality-assurance.

### BUG-001: Pull request flow fails where the platform's GraphQL API is not available
- **Severity**: high
- **Status**: done
- **Found**: 2026-10-25, 2.8.0, /dev-cycle run on this repository
- **Plan**: -
- **Closed**: 2026-10-25, PR #17
- **Description**: The GitHub provider reads pull requests with 'gh pr list/view' and review threads with 'gh api graphql', both GraphQL. Where GraphQL is refused (HTTP 403 seen in Claude Code cloud sessions; other agents, proxies or tokens may do the same), the diagnosis cannot list pull requests and pr_sync status, gate-merge, reply and resolve fail. Fix without code specific to one environment: REST for everything REST covers, GraphQL only for what only it has (thread resolution), and a clear, safe result when that part cannot be read. Must keep working on a local machine, GitHub Enterprise and GitLab.

### BUG-002: Configuration written into a path the project's .gitignore ignores
- **Severity**: medium
- **Status**: done
- **Found**: 2026-10-25, 2.8.0, /dev-cycle run on this repository
- **Plan**: -
- **Closed**: 2026-10-25, PR #17
- **Description**: write-config (and Spec Kit setup) write .dev-cycle.json, .specify/ and specs/ without checking git check-ignore. In a repository whose .gitignore is an allowlist (like this one) the files are never versioned and the decisions are lost in the next clone. Detect it, warn before writing and propose the .gitignore entry.

### IMP-003: Run every script without manual installs
- **Priority**: high
- **Epic**: EPIC-006
- **Status**: superseded
- **Found**: 2026-10-25, 2026-10-25, platform strategy discussion
- **Plan**: -
- **Closed**: 2026-10-27, superseded by IMP-005
- **Description**: Run the skills' scripts through uv (PEP 723 inline metadata or one wrapper) so dependencies install themselves, and add a standard-library SQLite FTS5 (BM25) search backend for plan-memory and session-memory, used when chromadb is missing or memory_backend is sqlite.
