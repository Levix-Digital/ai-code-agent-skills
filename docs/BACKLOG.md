# Backlog

Work captured for later planning: bugs, features, improvements and adjustments, grouped under epics when the product is planned as a whole. Capturing an item takes no discovery; when the user decides to work on items, they are promoted into a plan (`plan-memory/backlog.py promote`), where the discovery and the planning happen. Open bugs come before new work.

Each item: `### <ID>: <title>` with the fields `Severity` (bugs: critical, high, medium, low) or `Priority` (others: high, medium, low), an optional `Epic`, `Status` (open, planned, in_progress), `Found`, `Plan` and `Description`. Epics (`EPIC-NNN`) group items of the complete product; they have no `Plan` and are never promoted: their items are. The roadmap (`docs/ROADMAP.md`) says which items go into which release. Closed items (done, dropped, duplicate, superseded by better defined items) move to `Closed` with a `Closed` note and keep their ids, which are never reused. Edit this file with `plan-memory/backlog.py` or by hand, and check it with `backlog.py validate`.

## Bugs

### BUG-003: Ignored protocol files reported only partially on Windows
- **Severity**: medium
- **Status**: open
- **Found**: 2026-10-27, 2026-10-27, first CI run on Windows (PR #22)
- **Plan**: -
- **Description**: onboard.py sends the list of protocol files to 'git check-ignore --stdin' in text mode. On Windows each newline becomes CRLF, Git looks for 'path\r' and only the last path matches, so the diagnosis and write-config warn about one ignored file instead of all of them (the BUG-002 protection). Send the input as bytes.

## Features

### FEAT-001: Pull request review on Bitbucket, Azure DevOps and Gitea
- **Priority**: medium
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
- **Description**: Lite (dev-cycle, backlog and pull request review, no Phase 0, release candidates or Spec Kit), Standard (plans, design checks, technical docs, Spec Kit for feature plans) and Full (phases 0 to 10, Spec Kit, release candidates, Gedo when enabled), in the README and as a choice in /dev-cycle. Spec Kit is on by default in Standard and Full (see FEAT-009); bug plans and the short path never use it.

### FEAT-007: Proprietary binary-only distribution with Levix licenses
- **Priority**: medium
- **Epic**: EPIC-006
- **Status**: open
- **Found**: 2026-10-25, 2026-10-25, platform strategy discussion; redefined 2026-10-28, protocol as a commercial product; decided 2026-10-28, same model as Aurora Dev Suite (its ADR-0005)
- **Plan**: -
- **Description**: The protocol follows the decision every Levix product shares (Aurora Dev Suite ADR-0005): proprietary, freemium, distributed only as binaries. The source repository becomes private; the signed dcp binary and installers are published to a separate public releases repository (the AnyContext pattern). Every crate has publish = false and the proprietary license notice; dependencies stay permissive (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode-3.0), checked with cargo-deny. Licenses (one per plan, signed by Levix, verified offline) and the verified download catalog come from the levix-platform components once they exist; until then the engine has no license check. The free tier and paid plans are open, as for the other products. Replaces the open source launch; the license, not hidden instructions, is the protection (FEAT-009). Record the decision as an ADR in this repository when the item is implemented.

### FEAT-008: Non-overlapping milestone scopes at gate 3 with gedo scope
- **Priority**: high
- **Epic**: EPIC-004
- **Status**: open
- **Found**: 2026-10-25, Gedo integration contract 2026-10-25
- **Plan**: -
- **Description**: When gedo.enabled and Gedo has the scope capability: at gate 3, check with gedo scope --json that the milestones marked to run in parallel do not share entities or their dependency closure; overlapping milestones run in sequence. Needs Gedo v0.5.0 (FEAT-133 there) and Gedo support for the project's language.

### FEAT-009: Protocol served by the dcp engine over MCP, without skills
- **Priority**: high
- **Epic**: EPIC-006
- **Status**: open
- **Found**: 2026-10-28, 2026-10-28, protocol as a commercial product discussion; redefined 2026-10-29, no skills
- **Plan**: -
- **Description**: The protocol has three parts and no skills. (1) The engine: the dcp binary holds all the logic, a state machine that keeps the cycle state, enforces phase order and gates deterministically, generates files from embedded templates and returns only the current step's instructions. (2) The agent's interface: dcp mcp, a local stdio MCP server over the same code (typed tools such as dcp_start, dcp_next and dcp_gate_check, no network, offline, in cloud sessions, on Claude Code, Codex, Gemini and Copilot); the installer registers it in every supported agent it finds, with no manual step. The command line stays for Aurora Dev Suite, CI, people, and as the fallback for an agent without MCP. (3) The reminder that makes the agent work through dcp, written by dcp init in layers: a line in AGENTS.md and CLAUDE.md, the MCP server instructions, /dev-cycle as an MCP prompt, and session-start hooks in the agents that support them. Adoption is verifiable and enforced outside the agent: the agent's first action is dcp_start, which returns a status line with real state (project, phase, plan) that the agent must show the user; dcp status and Aurora show whether a session is really active, so an agent that only claims to use the protocol is exposed; and dcp init adds a CI check so a pull request without the cycle's trail (plan approved at gate 3, gates recorded) fails the merge gate. GitHub Spec Kit is orchestrated, not embedded: it stays a third-party tool installed in the project at the pinned version with its own commands, dcp tells the agent when to run them (feature plans of the full path only) and checks the result at gate 3 deterministically, and the protocol's own part (preset sections, generated constitution, checks) lives in dcp. Spec Kit is on by default: the DCP installation asks the user once to install uv and Spec Kit (recommended); uv goes in at user level without admin rights, at a pinned version with a verified checksum, on Windows, macOS, Linux and cloud sessions; dcp init then installs Spec Kit in the project without asking again. When the user declines or there is no network, dcp continues without Spec Kit, says why, and dcp status shows it off; it never blocks the cycle. Inside each phase the agent implements freely; the engine only enforces the order of work and the gates. Instructions stay readable and explain their reasons (hiding them would make the agent worse); the binary is signed so the protocol cannot be altered by the user. Projects adjust it only through a closed set of settings validated by the engine (onboarding level of FEAT-006, optional phases, providers); no custom templates, phases or extension points, and mandatory gates and principles are not configurable. Each phase becomes an explicit contract kept as data in the engine (inputs, artifacts to produce, done criteria, gate checks), the single source for both validation and the agent's instructions. Incremental cycle context: when a phase passes its gate, its structured handoff (decisions, constraints, artifacts by path, open questions) is validated and added to the cycle context, which the engine returns with every step: recent phases in full, older ones compacted, artifacts referenced rather than inlined. The context is stored in the project's repository, so it survives new sessions, a change of agent and cloud sessions, and later gives each role (FEAT-002) and Gedo stream (FEAT-003) only the slice it needs. Acceptance: on a set of sample projects the cycle run through dcp matches or beats the current skills (FEAT-015 metrics: gates passed first time, complete artifacts, rework, tokens); only then are the skills removed. After v2.10.0, with Spec Kit.

### FEAT-010: Infrastructure as code step in CI and releases
- **Priority**: medium
- **Epic**: EPIC-007
- **Status**: open
- **Found**: 2026-10-28, 2026-10-28, protocol improvements discussion
- **Plan**: -
- **Description**: An optional infrastructure step owned by the DevOps role, set in .dev-cycle.json (infra: off, or a tool from the supported list and its path), with every supported tool from the start: Terraform/OpenTofu, Helm, Pulumi, Kustomize, CloudFormation and Bicep. Onboarding offers it when it finds infrastructure files. Pull request CI only checks, never changes: format and validation, lint, security scan (Checkov or Trivy) and the plan or rendered manifests published for review. Apply is tied to the existing gates: the release candidate applies to staging and the promotion to production, always with the user's approval. Credentials stay in the CI provider's secrets (OIDC preferred) and the protocol never handles them. Plans state the infrastructure impact in their DevOps section. Against the known failure modes of agent-written infrastructure (insecure defaults, outdated provider syntax, every developer and agent designing it differently, drift, cost, secrets): a project infrastructure baseline (modules, naming, tags, environments, remote state) recorded in an ADR or the TECH_SPEC and checked at gate 3 and in code review; pinned provider and tool versions, validated against those versions; secret scanning (gitleaks) and the rule that the agent never receives cloud credentials; a cost estimate on the pull request (Infracost CLI); scheduled drift detection, a periodic plan that opens a bug when the real environment differs from the code; and policy as code (OPA/Conftest) for the organization's own rules. Free tools only. Workflow templates for GitHub Actions, GitLab CI, Azure Pipelines and Bitbucket Pipelines.

### FEAT-011: Dependency and supply chain guard
- **Priority**: high
- **Epic**: EPIC-008
- **Status**: open
- **Found**: 2026-10-28, 2026-10-28, gaps of the cycle with coding agents discussion
- **Plan**: -
- **Description**: Criticality 1 of the agent gaps. Agents invent packages that do not exist (slopsquatting), add dependencies the change does not need and pick vulnerable or outdated versions. A dependency policy in the plan and gate 3 (every new dependency justified, existing alternatives checked), a check that each new package exists in its registry and is not a fresh or near-name lookalike, a vulnerability audit in pull request CI with the stack's free tools (cargo audit, npm audit, pip-audit, govulncheck, Trivy), license checks against the project's allowed list, and an SBOM attached to every release. Free tools only, on the main CI providers.

### FEAT-012: Static security analysis in CI
- **Priority**: high
- **Epic**: EPIC-008
- **Status**: open
- **Found**: 2026-10-28, 2026-10-28, gaps of the cycle with coding agents discussion
- **Plan**: -
- **Description**: Criticality 4 of the agent gaps. The review's security item gets deterministic tools: Semgrep OSS rules and the stack's security linters (bandit, gosec, clippy, eslint security rules) in pull request CI, with findings at blocking level failing the merge gate. Free tools only (CodeQL stays out: paid on private repositories), on the main CI providers.

### FEAT-013: Operations phase: production feedback into the backlog
- **Priority**: high
- **Epic**: EPIC-001
- **Status**: open
- **Found**: 2026-10-28, 2026-10-28, gaps of the cycle with coding agents discussion
- **Plan**: -
- **Description**: Criticality 5 of the agent gaps. The cycle ends at the release today. A phase after the release closes the loop: production errors and incidents become triaged bugs in the backlog (bugs first applies), every incident has a short blameless postmortem whose actions become backlog items, and plans state the signals (logs, metrics, alerts) that tell whether the release works. Provider-neutral: it reads what the project already uses, it does not impose a monitoring tool.

### FEAT-014: Review brief at the user's gates
- **Priority**: medium
- **Epic**: EPIC-001
- **Status**: open
- **Found**: 2026-10-28, 2026-10-28, gaps of the cycle with coding agents discussion
- **Plan**: -
- **Description**: Criticality 6 of the agent gaps. With agents the user approves code they did not write and may not understand (comprehension debt). Each user gate (plan approval, acceptance, release) gets a short generated brief: what changed and why, the risks, the decisions taken, and the files and behaviors that deserve careful review, linked to the plan and the pull request.

### FEAT-015: Metrics of the cycle
- **Priority**: medium
- **Epic**: EPIC-001
- **Status**: open
- **Found**: 2026-10-28, 2026-10-28, gaps of the cycle with coding agents discussion
- **Plan**: -
- **Description**: Criticality 7 of the agent gaps. The cycle records, in the project's repository, gates passed first time or rejected, rework (review rounds, reopened milestones, reverted releases), time per phase and, where the agent reports them, tokens spent. Three sources: (1) what Git, the pull requests, the plans (status_log, milestone done_at) and the backlog already record, computed retroactively by an extractor, so the history before this item counts; (2) what is lost today (gates rejected and why, rework reasons, phase start and end), written automatically by the engine as an event at the end of every phase and gate to an append-only log in the repository (for example .dev-cycle/metrics/events.jsonl), with no manual step and the same on every agent and environment; (3) tokens and cost, through an optional importer per agent for the main 3-4 agents, never required for the cycle to run. Baseline: the Git and pull request history of projects built before or without the protocol; the engine decides what counts as before and after the protocol and reports the period the baseline covers. A summary per release shows whether the protocol is improving delivery. Required before FEAT-009, whose acceptance compares the MCP cycle with the current skills on these numbers.

### FEAT-016: Performance budgets and regression benchmarks
- **Priority**: low
- **Epic**: EPIC-008
- **Status**: open
- **Found**: 2026-10-28, 2026-10-28, gaps of the cycle with coding agents discussion
- **Plan**: -
- **Description**: Criticality 8 of the agent gaps. Plans state performance budgets for the paths they touch (latency, memory, binary or bundle size), and pull request CI runs the project's benchmarks and fails when a budget is exceeded or a regression passes a set threshold. Uses the stack's own benchmark tools.

### FEAT-017: Metrics and settings commands for Aurora Dev Suite
- **Priority**: medium
- **Epic**: EPIC-001
- **Status**: open
- **Found**: 2026-10-28, 2026-10-28, cycle metrics discussion
- **Plan**: -
- **Description**: The graphical dashboard and settings live only in Aurora Dev Suite, which reads the engine through its command line with versioned JSON (Aurora ADR-0003 and ADR-0006: Aurora uses the engine only through the dcp binary, never reads the engine's files, and has no stand-in; without a capability the feature is unavailable in Aurora). The protocol has no interface of its own and does not duplicate Aurora. The engine offers: (1) metrics --json (schema dcp.metrics, with schema_version; the FEAT-015 metrics per project, with an optional --release and the list of releases for trends): each metric has its unit and its origin (recorded event or computed after the fact), the baseline marker and the period it covers come from the engine, and tokens are absent, not zero, when the agent does not report them. The retroactive computation lives in the engine, so it serves the agent and the command line too. (2) config get --json (schema dcp.config; the command name is final, with no alias): for every setting its current value, allowed values, default, one-line description, type (enum, boolean, text, list), whether it is read-only (mandatory gates and principles are shown as fixed) and its group, so Aurora builds the form from the schema. (3) config set (one key, or several with --json), written to .dev-cycle.json through the same validation as the engine, so the file stays the single source of truth; --dry-run --json validates and returns, without writing, the before and after of each key and the file diff; validation errors come back as JSON with the key, the message and the accepted values. No setting holds secrets such as provider tokens; if one ever does, config get does not return its value. The engine does not commit: after config set the user commits .dev-cycle.json and opens the pull request. (4) The config and metrics capabilities in version --json. (5) A static HTML report (report) for users of the engine alone and for CI artifacts. The agent and the command line keep working without Aurora. Aurora's side is FEAT-027 Project settings and FEAT-028 Cycle metrics in its EPIC-004; when talking to Aurora, cite these items as DCP FEAT-0NN, since Aurora uses the same ids for its own items.

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
- **Status**: in_progress
- **Found**: 2026-10-27, 2026-10-27, Rust migration discussion
- **Plan**: ai_code_agent_skills_translate_the_scripts_to_rust_with_identical_behavior
- **Description**: Translate every Python script of the skills into one Rust binary with no change in behavior: same commands, flags, stdout, stderr, exit codes and files written, proven by comparing the Python and Rust outputs on the same fixtures. One script per pull request: when the outputs are identical, the SKILL.md calls the Rust command and the Python script is deleted. When no script is left, the Python requirements and tests are removed.

### IMP-006: Test integrity checks
- **Priority**: high
- **Epic**: EPIC-008
- **Status**: open
- **Found**: 2026-10-28, 2026-10-28, gaps of the cycle with coding agents discussion
- **Plan**: -
- **Description**: Criticality 2 of the agent gaps. Never moving on with a failing test does not stop an agent from weakening a test, mocking everything or writing a test that asserts nothing. Pull request CI reports coverage of the changed lines with a minimum the project sets; mutation testing runs on the modules the plan marks as critical (cargo-mutants, Stryker, mutmut, go-mutesting); changing or deleting an existing test or assertion needs a justification in the pull request, checked by the review and the merge gate.

### IMP-007: Change scope checked against the plan
- **Priority**: high
- **Epic**: EPIC-008
- **Status**: open
- **Found**: 2026-10-28, 2026-10-28, gaps of the cycle with coding agents discussion
- **Plan**: -
- **Description**: Criticality 3 of the agent gaps. Agents refactor outside the plan and pull requests grow. The merge gate compares the diff with the files and modules the plan's milestones declare (allowed_scopes of IMP-004): changes outside them need a plan update or become backlog items. A pull request size limit in the configuration warns and asks to split.

## Adjustments

### ADJ-001: Engine binary renamed to dcp
- **Priority**: high
- **Epic**: EPIC-006
- **Status**: open
- **Found**: 2026-10-28, 2026-10-28, Aurora Dev Suite integration discussion
- **Plan**: -
- **Description**: The engine's binary and launchers are called dcp (the name Aurora Dev Suite and the Levix products use) instead of dev-cycle: the binary, bin/dcp and bin/dcp.ps1, the release assets, and every SKILL.md and doc that calls the engine. The /dev-cycle command keeps its name. Done right after the migration (IMP-005 stays a pure translation), before the first release of the versioned JSON contract that Aurora reads; projects already using the protocol keep working through the launcher.

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
- **Description**: Execution without manual installs, standard-library search, onboarding in three levels, the protocol served by the signed engine and its proprietary, binary-only distribution.

### EPIC-007: Infrastructure as code
- **Priority**: medium
- **Status**: open
- **Found**: 2026-10-28, 2026-10-28, protocol improvements discussion
- **Description**: Infrastructure checked in pull request CI and applied through the release gates, with the project's infrastructure as code tool on the main CI providers.

### EPIC-008: Guardrails for agent-written code
- **Priority**: high
- **Status**: open
- **Found**: 2026-10-28, 2026-10-28, gaps of the cycle with coding agents discussion
- **Description**: Deterministic checks against the failure modes of code written by agents: invented or vulnerable dependencies, misleading tests, changes outside the plan, insecure code and performance regressions.

## Closed

### BUG-004: AI attribution footers reached pull requests past the principle 9 guard
- **Severity**: high
- **Status**: done
- **Found**: 2026-10-27, pull requests #16 to #23
- **Plan**: -
- **Closed**: 2026-10-27, guard covers Markdown links, emphasis, agent links, the attribution marker and Git identities; text posted through other tools is checked and read back
- **Description**: Pull request bodies and comments carried attribution footers (a "Generated by" line linking to the coding agent) and an attribution block with an agent's thread link. They were posted through tools other than pr_sync.py, so the guard never ran, and the guard itself missed them: the Markdown link and the italic underscores hid the agent's name from the attribution pattern, and only session links were flagged. Gate 8 did not look at commit identities either.

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
