<!-- dev-cycle-constitution
generated-by: dev-cycle-protocol/constitution.py (do not edit; change a source and regenerate)
sources-sha256: 9c9032960db70eeac48366efa26483a162d56287468076ba55e4154088fa8e5d
content-sha256: 457adeb914a788e2b19bb82acba3fcf2b2ee0b7191b420a35946bb98e97de697
-->
# ai-code-agent-skills Constitution

The non-negotiable rules every specification, plan and task of this project follows. It is generated from the project's sources of truth by `dev-cycle-protocol/constitution.py`; change a source and regenerate it, never edit it.

## Core Principles

### I. Design

*Source: software-design*

#### Modularity and separation of responsibilities
- One module, type or function has **one reason to change**. When a description needs "and", split it.
- Group code by what it is about (a domain concept or a feature), not by what kind of file it is, unless the architecture style says otherwise.
- Business rules never live in the user interface, in transport code (HTTP handlers, CLI argument parsing) or in persistence code.
- Dependencies point toward stable code: details (frameworks, databases, network, UI) depend on the core, never the other way around.

#### SOLID, in practice
| Principle | What it means when writing code | Smell that it is violated |
|---|---|---|
| **S**ingle responsibility | Each unit does one job and can be named without "and". | A 600-line file mixing parsing, rules and output. |
| **O**pen/closed | Add behavior by adding a new implementation, not by growing an `if/else` chain in stable code. | Every new provider edits the same `match` in five places. |
| **L**iskov substitution | Any implementation of an interface can replace another without surprises. | An implementation that panics or ignores half the contract. |
| **I**nterface segregation | Small interfaces shaped by what the caller needs. | Callers implementing methods they never use. |
| **D**ependency inversion | High-level code depends on abstractions it owns; adapters implement them. | Core code constructing an HTTP client or a database driver. |

#### Clean Code
- **Names** say what something is or does; no abbreviations a newcomer cannot decode; no `data`, `info`, `manager`, `util` as whole names.
- **Small functions** that do one thing at one level of abstraction; early returns instead of deep nesting.
- **No magic values**: named constants with units in the name (`TIMEOUT_MS`).
- **Comments explain why**, never what; delete commented-out code and dead code.
- **Consistency over preference**: follow the conventions already in the codebase (naming, error types, layout) unless the change is an agreed refactor.
- **Duplication**: remove it when the same rule exists twice; tolerate it when two pieces only look alike today.

#### Explicit error handling
- Never swallow an error (`except: pass`, `let _ =` on a meaningful result, an empty `catch`, a blind `.unwrap()` on input that can fail).
- Errors carry context (what was being done, with which input) and use the project's canonical error types.
- Decide at each boundary: recover, translate into the caller's error type, or propagate. Report to the user once, at the edge.
- Fallbacks are allowed only when they are explicit, logged and visible in status output; a silent fallback that changes behavior is a defect.

#### Testability
- Pure logic is separated from I/O so it can be unit-tested without mocks.
- External services sit behind an interface the tests can replace.
- A structure that is hard to test is a design problem, not a testing problem.

### II. Simplicity First

*Source: software-design*

Principles applied mechanically produce over-engineering. The rule: **the simplest solution that keeps responsibilities separate.**

- Introduce an abstraction (trait, interface, base class, factory) only when at least one of these is true:
  1. there is a **second implementation** now (not "maybe later");
  2. it creates a **test seam** you need today;
  3. it isolates an **external dependency** (framework, service, database, OS API).
- No speculative configuration, plugin systems or generic frameworks for a single use case.
- Prefer composition over inheritance; prefer plain data and functions over class hierarchies.
- Three similar lines are better than a premature helper with five parameters.

#### Design patterns: only when they solve a real problem
| Problem actually present | Pattern that fits |
|---|---|
| Several interchangeable algorithms or providers chosen at runtime | Strategy |
| A third-party or legacy interface that does not match yours | Adapter |
| Construction is complex or depends on configuration | Factory / Builder |
| Behavior changes with an explicit lifecycle state | State machine |
| Actions must be queued, logged, undone or dispatched uniformly | Command |
| Persistence must be swappable or mocked | Repository |
| Several parts react to an event without knowing each other | Observer / event channel |

If you cannot point to the problem in the left column, do not use the pattern. Name the pattern in code or docs only when it helps the reader.

### III. Architecture Style: modular-monolith

*Source: software-design/styles/modular-monolith.md*

##### Structure

- **Modules by business capability**, each with its own folder or package containing its domain, application logic and persistence.
- **Public interface per module**: a small facade or API (functions, service interfaces, events) that other modules use. Everything else is internal.
- **Shared kernel**: only truly shared, stable concepts (IDs, money, time). Keep it small.
- **Integration between modules** through the public interface or domain events, never through another module's tables or internal types.

##### Where things go

| Piece | Location |
|---|---|
| A rule about invoices | `billing` module, internal |
| Another module needs an invoice total | `billing`'s public interface |
| "When an account is closed, cancel subscriptions" | Event published by `accounts`, handled by `billing` |
| Money, IDs, clock | Shared kernel |

##### Common mistakes

- A "common" or "shared" module that grows into a second monolith everything depends on.
- Joins across modules' tables in one query "for performance" without an agreed read model.
- Modules organized by technical layer (`controllers`, `services`, `repositories`) instead of by business capability.
- Public interfaces that expose internal entities, so every change ripples across modules.

### IV. Development Cycle

*Source: dev-cycle-protocol*

1. **Design**: modular code with clear responsibilities, SOLID, Clean Code, explicit errors, patterns only when needed, in the project's architecture style (`software-design`).
2. **Evidence**: every change is proven by automated tests; manual tests cover only what cannot be automated, in one curated suite (`quality-assurance`).
3. **Documentation describes today**: technical docs and README are edited in place; history lives in Git and release notes (`architecture-docs`, `user-docs`).
4. **Plans are shared and complete**: approved plans are versioned files in the project (`plan-memory`).
5. **Safe delivery**: short-lived branches on the project's branching model, CI on every pull request, release candidates, the user's release gate (`devops-release`).
6. **Security**: no secrets in Git, backward-compatible data migrations, a rollback path for every release.
7. **Bugs first**: open `critical`, `high` and `medium` bugs are planned and fixed before new features, improvements and adjustments; a `low` bug goes into the next release at the latest; no known bug stays untriaged (a severity, and a release for the ones that do not block). Only the user can explicitly accept the risk. Nothing found is lost, it is fixed now or captured in the backlog.
8. **Follow-through**: the agent follows CI and releases to the end and fixes what fails; the user's decisions (plan approval, candidate acceptance, the release gate) are never taken by the agent.
9. **No AI attribution, anywhere**: no artifact names an AI, a model or a coding agent as author, co-author, reviewer or source: commits and their trailers, tags, branches, pull and merge requests, reviews and review comments, issues, plans, documentation, release notes and code comments. Everything is authored by the repository's Git identity (`devops-release`). Configuration a project needs for its tools (an agent's settings folder, an integration name) is technical, not attribution.

## Product Requirements and Constraints

*Source: docs/product/TECH_SPEC.md*

### Non-Functional Requirements

#### Performance

- The `/dev-cycle` diagnosis answers in under 5 seconds, platform calls included (4.2 s on this repository, measured 2026-10-25 in a cloud session).
- Every checker runs on the project's files only, without network, except the platform and Spec Kit commands that need it.

#### Security and Privacy

- No credentials in the repository or the skills: platform access goes through the user's `gh` and `glab` authentication.
- Secret hygiene is checked at gate 0 and gate 8.
- Session memory and indexes stay on the developer's machine (`~/.ai-code-agent-memory/`), never versioned.
- Principle 9: no AI attribution in anything the protocol writes or posts.

#### Availability and Recovery

Not applicable as a service: the protocol runs locally. Its state is the project's files under Git; search indexes are disposable and rebuilt from them.

#### Accessibility and Localization

- Skills, docs and every artifact are written in English; the agent talks to the user in the user's language.
- Command line output uses plain text with optional emoji markers; `--json` is available where another tool consumes the output.

### Constraints

- Plain Markdown skills that any agent can read; no agent-specific syntax in the skills.
- No code specific to one environment; provider-dependent features cover the main three or four providers.
- No parallel agents without Gedo.
- Integration with Gedo only through its command line with `--json`; the repositories stay separate.
- No paid service or paid platform feature required (no CodeQL on private repositories, no AI review bot in CI).
- Releases follow `devops-release` with the trunk model; the version lives in `.claude-plugin/plugin.json`.

## Architecture Decisions

Accepted decisions; a plan that contradicts one needs a new ADR that supersedes it.

- ADR-0001: Skills in the open SKILL.md format with Python scripts (`docs/adr/0001-skills-in-the-open-skill-md-format-with-python-scripts.md`)
- ADR-0002: Modular monolith of skills by role (`docs/adr/0002-modular-monolith-of-skills-by-role.md`)
- ADR-0003: Pull requests through the platform CLIs on the REST API (`docs/adr/0003-pull-requests-through-the-platform-clis-on-the-rest-api.md`)
- ADR-0004: Gedo integration only through its command line (`docs/adr/0004-gedo-integration-only-through-its-command-line.md`)
- ADR-0005: Parallel agents only with Gedo (`docs/adr/0005-parallel-agents-only-with-gedo.md`)
- ADR-0006: Gedo contract discovered by capability with safe fallback per check (`docs/adr/0006-gedo-contract-discovered-by-capability-with-safe-fallback-pe.md`)
- ADR-0007: Native Rust engine translated from the Python scripts (`docs/adr/0007-native-rust-engine-translated-from-the-python-scripts.md`)

## Governance

- This constitution supersedes other practices for specifications, plans and tasks. A plan passes the Constitution Check or records each justified violation in its Complexity Tracking table; a lasting exception becomes an ADR.
- Amendments change the source, not this file: an ADR for decisions, the technical specification for requirements and constraints, `.dev-cycle.json` for the architecture style, the skills for principles. Then run `constitution.py generate` and commit the result with the change.
- Gate 3 of the development cycle blocks a plan while this file is stale or edited by hand.

**Version**: 1.2.0 | **Ratified**: 2026-10-25 | **Last Amended**: 2026-10-27
