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
- **Description**: The protocol has three parts and no skills. (1) The engine: the dcp binary holds all the logic, a state machine that keeps the cycle state, enforces phase order and gates deterministically, generates files from embedded templates and returns only the current step's instructions. (2) The agent's interface: dcp mcp, a local stdio MCP server over the same code (typed tools such as dcp_start, dcp_next and dcp_gate_check, no network, offline, in cloud sessions, on Claude Code, Codex, Gemini and Copilot); the installer registers it in every supported agent it finds, with no manual step. The command line stays for Aurora Dev Suite, CI, people, and as the fallback for an agent without MCP. (3) The reminder that makes the agent work through dcp, written by dcp init in layers: a line in AGENTS.md and CLAUDE.md, the MCP server instructions, /dev-cycle as an MCP prompt, and session-start hooks in the agents that support them. Adoption is verifiable and enforced outside the agent: the agent's first action is dcp_start, which returns a status line with real state (project, phase, plan) that the agent must show the user; dcp status and Aurora show whether a session is really active, so an agent that only claims to use the protocol is exposed; and dcp init adds a CI check so a pull request without the cycle's trail (plan approved at gate 3, gates recorded) fails the merge gate. GitHub Spec Kit is orchestrated, not embedded: it stays a third-party tool installed in the project at the pinned version with its own commands, dcp tells the agent when to run them (feature plans of the full path only) and checks the result at gate 3 deterministically, and the protocol's own part (preset sections, generated constitution, checks) lives in dcp. Spec Kit is on by default: the DCP installation asks the user once to install uv and Spec Kit (recommended); uv goes in at user level without admin rights, at a pinned version with a verified checksum, on Windows, macOS, Linux and cloud sessions; dcp init then installs Spec Kit in the project without asking again. When the user declines or there is no network, dcp continues without Spec Kit, says why, and dcp status shows it off; it never blocks the cycle. Inside each phase the agent implements freely; the engine only enforces the order of work and the gates. Instructions stay readable and explain their reasons (hiding them would make the agent worse); the binary is signed so the protocol cannot be altered by the user. Projects adjust it only through a closed set of settings validated by the engine (onboarding level of FEAT-006, optional phases, providers); no custom templates or phases, and mandatory gates and principles are not configurable. The only extension point is the provider contract of FEAT-018, which can add items and gates but never skip, weaken or approve the protocol's own. Each phase becomes an explicit contract kept as data in the engine (inputs, artifacts to produce, done criteria, gate checks), the single source for both validation and the agent's instructions. Incremental cycle context: when a phase passes its gate, its structured handoff (decisions, constraints, artifacts by path, open questions) is validated and added to the cycle context, which the engine returns with every step: recent phases in full, older ones compacted, artifacts referenced rather than inlined. The context is stored in the project's repository, so it survives new sessions, a change of agent and cloud sessions, and later gives each role (FEAT-002) and Gedo stream (FEAT-003) only the slice it needs. Acceptance: on a set of sample projects the cycle run through dcp matches or beats the current skills (FEAT-015 metrics: gates passed first time, complete artifacts, rework, tokens); only then are the skills removed. After v2.10.0, with Spec Kit.

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

### FEAT-017: Settings commands for Aurora Dev Suite
- **Priority**: medium
- **Epic**: EPIC-001
- **Status**: open
- **Found**: 2026-10-28, 2026-10-28, cycle metrics discussion; split 2026-11-01, the metrics commands moved to FEAT-022
- **Plan**: -
- **Description**: The settings screen lives only in Aurora Dev Suite, which reads the engine through its command line with versioned JSON (Aurora ADR-0003 and ADR-0006: Aurora uses the engine only through the dcp binary, never reads the engine's files, and has no stand-in; without a capability the feature is unavailable in Aurora). The protocol has no interface of its own and does not duplicate Aurora. The engine offers: (1) config get --json --dir <root> (schema dcp.config, MAJOR 1; the command name is final, with no alias): an opaque revision of the file, the file read or null, and for every setting its key, group, one-line description, type (enum, boolean, text, integer, list), current value, default, allowed values, whether null is accepted, whether it is read-only and why, and findings. (2) config set --json [--dry-run] --dir <root> --revision <revision> --value <key>=<JSON value>: all or nothing, through the same validation as the engine, so .dev-cycle.json stays the single source of truth; exit 0 whenever the request was understood, including refusals; the reply has the before and after of each changed key, the file diff and errors (unknown_key, read_only, invalid_value, stale_revision, invalid_file) with the key, the message and the accepted values; a revision different from the file's is refused. (3) The config capability in version --json (the command itself comes with FEAT-019). Contracts: Aurora's specs/007-project-settings/contracts in aurora-dev-suite, with its sample replies as the acceptance check, plus the changes proposed to Aurora on 2026-11-01 and pending its answer: a required field set (the key is present in the file), since an explicit speckit.enabled false is a decision and a missing key means it was never asked; --value <key>=null removes the key; structured settings such as drift_rules are read-only, and readers treat an unknown type as read-only so new types can come in a MINOR version; read-only settings may be display-only, not stored in the file. Engine rules: keys managed by other commands (speckit.enabled and speckit.version, written by speckit init and upgrade) are read-only with a reason naming the command; set moves a legacy flat key it touches into its section, and keeps the key order and indentation so the diff shows only the change. No setting holds secrets such as provider tokens; if one ever does, config get does not return its value. The engine does not commit: after config set the user commits .dev-cycle.json and opens the pull request. The agreed schemas and samples are copied into this repository and run as contract tests. Aurora's side is FEAT-027 Project settings; when talking to Aurora, cite this item as DCP FEAT-017, since Aurora uses the same ids for its own items.

### FEAT-018: Extension point for providers
- **Priority**: medium
- **Epic**: EPIC-001
- **Status**: open
- **Found**: 2026-10-29, 2026-10-29, Aurora Dev Suite request for cross-project integrations (Aurora EPIC-010, ADR-0007)
- **Plan**: -
- **Description**: A generic extension point in the dcp engine that knows nothing about any one provider, so Aurora Dev Suite (and later Gedo) can join the cycle through processes and JSON only, with nothing specific to a coding agent or a Git host. (1) External subcommands, like Git and Cargo: dcp <name> runs dcp-<name> from PATH with the remaining arguments. (2) Providers registered at user level, outside the project repository (for example ~/.config/dcp/extensions.json), written by the provider's installer; nothing is committed to projects. Only registered providers run at cycle points, never any dcp-* found on PATH. (3) Cycle points with a JSON contract: the engine runs dcp-<name> hook <point> --json at discovery (start of Phase 1 and the /dev-cycle diagnosis), after-merge (a pull request merged into the work base or a followed branch, with commit and branch) and before-release-gate (before gate 8). The answer is a list of items (schema dcp.extension.v1): id, title, needs (none, judgment for whoever runs the protocol, user), blocking, detail and suggested commands. The diagnosis shows them; a blocking item holds the gate like a blocking bug until it is handled or the user accepts the risk. Suggested commands are shown, never run by the engine. (4) Gates declared by providers (Aurora declares integration), presented to the user like the protocol's own gates and recorded through the provider (dcp-<name> gate <id> approve|reject --json); the protocol never decides for the user. (5) One line in the protocol text at Phases 1 and 10 and in the diagnosis: run the registered providers (dcp extensions run <point> --json) and handle their items, an instruction of the protocol that holds for any agent and for a person at a terminal. (6) dcp extensions list --json, so Aurora and the diagnosis know what is installed. Limits: a provider only adds items, warnings and its own gates; it never skips, weakens or approves the protocol's gates and principles (FEAT-009), the engine never writes to the repository on a provider's behalf, and there is no platform automation (workflows, pipelines, webhooks). Acceptance: with no provider registered dcp behaves exactly as today; a test provider returning a blocking item makes gate 8 fail with the item's message and pass with the risk accepted; a provider that fails, times out or returns invalid JSON becomes a warning and never blocks the cycle. Aurora's side is its EPIC-010 and ADR-0007; when talking to Aurora, cite this item as DCP FEAT-018.

### FEAT-019: Diagnosis and version commands for Aurora Dev Suite
- **Priority**: high
- **Epic**: EPIC-001
- **Status**: open
- **Found**: 2026-10-30, 2026-10-30, Aurora Dev Suite contract request for its v0.2.0 portfolio (Aurora FEAT-016)
- **Plan**: -
- **Description**: Aurora Dev Suite shows each registered project's cycle state and reads it only through the dcp binary (Aurora ADR-0003 and ADR-0006). The engine offers: (1) version --json (schema dcp.version, schema_version 1.x): the field version and capabilities as a list of {name, schema_major}, listing diagnose with schema_major 1; later items (FEAT-017 config, FEAT-022 metrics) add their capabilities to it. (2) diagnose --json --dir <root>: exit code 0 and one JSON reply on standard output; any other exit code is a failure explained on standard error; warnings may go to standard error with exit 0. (3) The reply follows schema dcp.diagnose, schema_version 1.x (MINOR adds optional fields only, MAJOR breaks, readers ignore unknown fields), as agreed with Aurora (specs/002-dcp-adapter/contracts/dcp-diagnose.schema.json in aurora-dev-suite). Required fields: schema, schema_version, root (absolute repository root), summary (one line); next {ask, recommended (option key or null), options [{key, title, reason?}]} with the diagnosis keys (plan:<id>, pr:<number>, rc:<run>, bugs, triage, release, request); release {version, name, open_items [item]} or null (the current roadmap release); plans [{id, title, status, milestones_done, milestones_total}] (active or in progress); release_candidate {run, version, not_run, failed, released} or null (latest run); pull_requests [{number, title, summary, needs_action, threads_for_user}] (empty plus a finding when the platform cannot be read); bugs {blocking [item], untriaged [item and reason]}; findings [string] (what could not be read and why, never blocking for the reader); item = {id, rank (priority or severity, or null), title}. It is the data the diagnosis already collects, as typed fields instead of preformatted strings; the agent's diagnosis keeps its current text. Setup-only data (configuration problems, ignored files, Spec Kit, agent attribution) stays out of version 1 and can come later as optional fields. Acceptance: the reply validates against the agreed JSON Schema on sample projects, including one without a platform connection. Built after the Rust migration (IMP-005), on the dcp binary of ADJ-001. When talking to Aurora, cite this item as DCP FEAT-019.

### FEAT-020: Security assessment: the product attacked at the release candidate
- **Priority**: high
- **Epic**: EPIC-009
- **Status**: open
- **Found**: 2026-11-01, 2026-11-01, security step in the cycle discussion
- **Plan**: -
- **Description**: A new role skill, security-assessment, attacks the running product and writes a vulnerability report that the next cycle starts from. FEAT-011 (dependencies), FEAT-012 (static analysis), FEAT-010 (infrastructure) and secret scanning check the code; this item checks the deployed product. (1) When: at the release candidate (Phase 8), against the candidate deployed to staging, and on a schedule, since new vulnerabilities appear without code changes. Never production. Targets come only from an allowlist in .dev-cycle.json, enforced at the network level (sandbox), not only in instructions; the attacker never receives real credentials. (2) Scope, with free command line tools that run locally and on the main CI providers: dynamic scanning (OWASP ZAP, Nuclei), API fuzzing from the project's OpenAPI description (Schemathesis) and the container image scan (Trivy). On top of them, a pentest run by an agent following the OWASP Web Security Testing Guide, which tests the threats of the threat model (FEAT-021); a finding of it counts only with a reproducible proof (request and response). (3) Model-agnostic attacker: by default the coding agent the project already uses; optionally a local open model through an OpenAI-compatible endpoint (Ollama, vLLM, llama.cpp), so target data and findings never leave the machine. No model or framework is fixed by the protocol. The first step of the implementation is an evaluation of candidate models and open pentest frameworks against intentionally vulnerable applications (OWASP Juice Shop, DVWA), comparing real findings, false positives, time and cost; its result sets the documented recommendation. (4) Report in docs/security/ (a Markdown summary and SARIF per run). Each finding becomes a backlog bug with its severity taken from its CVSS score, so bugs first applies and the next discovery starts from them; findings are deduplicated by fingerprint, and an accepted risk is recorded with a reason and an expiry date. (5) Gate 8 fails while a critical or high finding of the release is open, unless the user explicitly accepts the risk with a reason, as with --accept-open-bugs. Deterministic tools carry the gate; the model only adds hypotheses and proof. (6) A product with nothing to attack (a library or a command line tool) sets the step off. Built after the Rust migration (IMP-005).

### FEAT-021: Light threat model in the plan
- **Priority**: medium
- **Epic**: EPIC-009
- **Status**: open
- **Found**: 2026-11-01, 2026-11-01, security step in the cycle discussion
- **Plan**: -
- **Description**: In Phase 2, a plan whose change touches the attack surface (authentication, permissions, user input, sensitive data, external integrations) carries a short STRIDE threat model: who could abuse the change and how. The product's threat model lives in docs/security/threat-model.md and is updated by each such plan; short-path fixes and documentation changes skip it. Every threat listed becomes an acceptance criterion, a test or a control in the plan, or it is not listed, so the model does not turn into paperwork. Code review (Phase 7) checks that the promised controls are in the code, and the security assessment (FEAT-020) tests the threats as hypotheses; each finding of the report says whether its threat was foreseen or missed by the model. Gate 3 checks that a plan touching the attack surface has the section. It finds logic flaws that scanners cannot, such as access to another user's data or an endpoint without a permission check, while fixing them is still cheap.

### FEAT-022: Metrics commands for Aurora Dev Suite
- **Priority**: medium
- **Epic**: EPIC-001
- **Status**: open
- **Found**: 2026-11-01, 2026-11-01, split from FEAT-017 after the review of Aurora Dev Suite's requests
- **Plan**: -
- **Description**: The metrics dashboard lives only in Aurora Dev Suite, which reads the engine only through the dcp binary with versioned JSON (Aurora ADR-0003 and ADR-0006). The engine offers: (1) metrics --json (schema dcp.metrics, with schema_version; the FEAT-015 metrics per project, with an optional --release and the list of releases for trends): each metric has its unit and its origin (recorded event or computed after the fact), the baseline marker and the period it covers come from the engine, and tokens are absent, not zero, when the agent does not report them. The retroactive computation lives in the engine, so it serves the agent and the command line too. (2) The metrics capability in version --json. (3) A static HTML report (report) for users of the engine alone and for CI artifacts. The metric definitions are protocol rules, so the protocol writes the dcp.metrics MAJOR 1 contract together with the FEAT-015 plan and sends it to Aurora for review (proposed to Aurora on 2026-11-01); the agreed schema and samples run as contract tests in this repository. Built with or after FEAT-015. Aurora's side is FEAT-028 Cycle metrics; when talking to Aurora, cite this item as DCP FEAT-022.

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

### IMP-008: Attribution settings for every supported coding agent
- **Priority**: medium
- **Epic**: EPIC-005
- **Status**: open
- **Found**: 2026-10-29, 2026-10-29, Aurora Dev Suite question on the agent settings folder in the diagnosis
- **Plan**: -
- **Description**: The diagnosis checks automatic attribution only for Claude Code: set-attribution writes .claude/settings.json and the protocol keeps that file in the repository, so every clone, including cloud sessions, starts with attribution off. Committing it serves one agent and ties the project to none: other agents ignore the folder. Do the same for the other supported agents (Codex, Gemini, Copilot): find which of them add attribution on their own (commit trailers, author or committer, pull request footers, session links) and have a project-level setting that turns it off; for each one that does, the diagnosis checks it, set-attribution writes it and protocol_files keeps it in the repository, like Claude Code. Agents without such a setting rely on the principle 9 guard and the gate 8 check of commits. A shared project settings file is committed; personal ones (such as .claude/settings.local.json) stay out. Settings folders are kept in the repository on purpose, so a project that excludes them locally gets the diagnosis item until they are committed.

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

### EPIC-009: Product security
- **Priority**: high
- **Status**: open
- **Found**: 2026-11-01, 2026-11-01, security step in the cycle discussion
- **Description**: The running product is attacked at the release candidate and on a schedule, the threats of each change are modeled in its plan, and every finding comes back as a triaged bug that the next cycle starts from.

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
