# Backlog

Work captured for later planning: bugs, features, improvements and adjustments, grouped under epics when the product is planned as a whole. Capturing an item takes no discovery; when the user decides to work on items, they are promoted into a plan (`plan-memory/backlog.py promote`), where the discovery and the planning happen. Open bugs come before new work.

Each item: `### <ID>: <title>` with the fields `Severity` (bugs: critical, high, medium, low) or `Priority` (others: high, medium, low), an optional `Epic`, `Status` (open, planned, in_progress), `Found`, `Plan` and `Description`. Epics (`EPIC-NNN`) group items of the complete product; they have no `Plan` and are never promoted: their items are. The roadmap (`docs/ROADMAP.md`) says which items go into which release. Closed items (done, dropped, duplicate, superseded by better defined items) move to `Closed` with a `Closed` note and keep their ids, which are never reused. Edit this file with `plan-memory/backlog.py` or by hand, and check it with `backlog.py validate`.

## Bugs

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
- **Description**: The protocol has three parts and no skills. (1) The engine: the dcp binary holds all the logic, a state machine that keeps the cycle state, enforces phase order and gates deterministically, generates files from embedded templates and returns only the current step's instructions. (2) The agent's interface: dcp mcp, a local stdio MCP server over the same code (typed tools such as dcp_start, dcp_next and dcp_gate_check, no network, offline, in cloud sessions, on Claude Code, Codex, Gemini and Copilot); the installer registers it in every supported agent it finds, with no manual step. The command line stays for Aurora Dev Suite, CI, people, and as the fallback for an agent without MCP. (3) The reminder that makes the agent work through dcp: the project instruction files and session-start hooks of FEAT-029, now pointing at the MCP tools, plus the MCP server instructions and /dev-cycle as an MCP prompt. Adoption is verifiable and enforced outside the agent (the engine order, Git and agent hooks and the CI trail check of FEAT-030 already exist by then; here they move behind the MCP interface): the agent's first action is dcp_start, which returns a status line with real state (project, phase, plan) that the agent must show the user; dcp status and Aurora show whether a session is really active, so an agent that only claims to use the protocol is exposed; and dcp init adds a CI check so a pull request without the cycle's trail (plan approved at gate 3, gates recorded) fails the merge gate. GitHub Spec Kit is orchestrated, not embedded: it stays a third-party tool installed in the project at the pinned version with its own commands, dcp tells the agent when to run them (feature plans of the full path only) and checks the result at gate 3 deterministically, and the protocol's own part (preset sections, generated constitution, checks) lives in dcp. Spec Kit is on by default: the DCP installation asks the user once to install uv and Spec Kit (recommended); uv goes in at user level without admin rights, at a pinned version with a verified checksum, on Windows, macOS, Linux and cloud sessions; dcp init then installs Spec Kit in the project without asking again. When the user declines or there is no network, dcp continues without Spec Kit, says why, and dcp status shows it off; it never blocks the cycle. Inside each phase the agent implements freely; the engine only enforces the order of work and the gates. Instructions stay readable and explain their reasons (hiding them would make the agent worse); the binary is signed so the protocol cannot be altered by the user. Projects adjust it only through a closed set of settings validated by the engine (onboarding level of FEAT-006, optional phases, providers); no custom templates or phases, and mandatory gates and principles are not configurable. The only extension point is the provider contract of FEAT-018, which can add items and gates but never skip, weaken or approve the protocol's own. Each phase becomes an explicit contract kept as data in the engine (inputs, artifacts to produce, done criteria, gate checks), the single source for both validation and the agent's instructions. Incremental cycle context: when a phase passes its gate, its structured handoff (decisions, constraints, artifacts by path, open questions) is validated and added to the cycle context, which the engine returns with every step: recent phases in full, older ones compacted, artifacts referenced rather than inlined. The context is stored in the project's repository, so it survives new sessions, a change of agent and cloud sessions, and later gives each role (FEAT-002) and Gedo stream (FEAT-003) only the slice it needs. Acceptance: on a set of sample projects the cycle run through dcp matches or beats the current skills (FEAT-015 metrics: gates passed first time, complete artifacts, rework, tokens); only then are the skills removed. After v2.10.0, with Spec Kit.

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
- **Description**: Criticality 5 of the agent gaps. The cycle ends at the release today. A phase after the release closes the loop: production errors and incidents become triaged bugs in the backlog (bugs first applies), every incident has a short blameless postmortem whose actions become backlog items, and plans state the signals (logs, metrics, alerts) that tell whether the release works. Provider-neutral: it reads what the project already uses, it does not impose a monitoring tool. Scheduled intake (2026-11-08, autonomous agents analysis): dcp intake runs as a scheduled job in the project's own CI (nightly, for example), with no agent and no model: it reads the failed runs of the default branch and the issues opened since the last run through the platform API with the token the CI job already has, deduplicates them against the open backlog items, and opens a pull request that only adds triaged bug entries (severity, link, first failing commit), so the developer starts the day accepting or discarding them. The binary comes from the public releases repository or, on premise without internet, from the customer's internal artifact repository; dcp is already in the customer's CI for the FEAT-030 trail check. Templates for GitHub Actions, GitLab CI, Azure Pipelines and Bitbucket Pipelines. Open for the user: whether CI use counts in the freemium license of FEAT-007.

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
- **Description**: The settings screen lives only in Aurora Dev Suite, which reads the engine through its command line with versioned JSON (Aurora ADR-0003 and ADR-0006: Aurora uses the engine only through the dcp binary, never reads the engine's files, and has no stand-in; without a capability the feature is unavailable in Aurora). The protocol has no interface of its own and does not duplicate Aurora. The engine offers: (1) config get --json --dir <root> (schema dcp.config, MAJOR 1; the command name is final, with no alias): an opaque revision of the file, the file read or null, and for every setting its key, group, one-line description, type (enum, boolean, text, integer, list), current value, default, allowed values, whether null is accepted, whether it is read-only and why, and findings. (2) config set --json [--dry-run] --dir <root> --revision <revision> --value <key>=<JSON value>: all or nothing, through the same validation as the engine, so .dev-cycle.json stays the single source of truth; exit 0 whenever the request was understood, including refusals; the reply has the before and after of each changed key, the file diff and errors (unknown_key, read_only, invalid_value, stale_revision, invalid_file) with the key, the message and the accepted values; a revision different from the file's is refused. (3) The config capability in version --json (the command itself comes with FEAT-019). Contracts, agreed with Aurora on 2026-11-01: specs/007-project-settings/contracts in aurora-dev-suite at commit 932750e, with the sample replies in crates/aurora-engines/tests/fixtures/dcp-config/ (get, preview, applied, refused, stale) as the acceptance check. They include a required field set (the key is present in the file), since an explicit speckit.enabled false is a decision and a missing key means it was never asked; --value <key>=null removes the key, and before and after are values in the file, null when absent; type is an open string, readers treat an unknown type as read-only so new types can come in a MINOR version, and structured settings such as drift_rules are read-only with any JSON value; read-only settings may be display-only, not stored in the file (set always false). Engine rules: keys managed by other commands (speckit.enabled and speckit.version, written by speckit init and upgrade) are read-only with a reason naming the command; set moves a legacy flat key it touches into its section, and keeps the key order and indentation so the diff shows only the change. No setting holds secrets such as provider tokens; if one ever does, config get does not return its value. The engine does not commit: after config set the user commits .dev-cycle.json and opens the pull request. The agreed schemas and samples are copied into this repository and run as contract tests. Aurora's side is FEAT-027 Project settings; when talking to Aurora, cite this item as DCP FEAT-017, since Aurora uses the same ids for its own items.

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
- **Description**: The metrics dashboard lives only in Aurora Dev Suite, which reads the engine only through the dcp binary with versioned JSON (Aurora ADR-0003 and ADR-0006). The engine offers: (1) metrics --json (schema dcp.metrics, with schema_version; the FEAT-015 metrics per project, with an optional --release and the list of releases for trends): each metric has its unit and its origin (recorded event or computed after the fact), the baseline marker and the period it covers come from the engine, and tokens are absent, not zero, when the agent does not report them. The retroactive computation lives in the engine, so it serves the agent and the command line too. (2) The metrics capability in version --json. (3) A static HTML report (report) for users of the engine alone and for CI artifacts. The metric definitions are protocol rules, so the protocol writes the dcp.metrics MAJOR 1 contract together with the FEAT-015 plan and sends it to Aurora for review (agreed with Aurora on 2026-11-01); each reply names its project root and release and gives totals that can be added across projects, for Aurora's portfolio view; the agreed schema and samples run as contract tests in this repository. Built with or after FEAT-015. Aurora's side is FEAT-028 Cycle metrics; when talking to Aurora, cite this item as DCP FEAT-022.

### FEAT-023: Context mode in the phase contracts
- **Priority**: high
- **Epic**: EPIC-010
- **Status**: open
- **Found**: 2026-11-01, EGM concept analysis
- **Plan**: -
- **Description**: Each phase contract of the engine (FEAT-009) declares its context mode: full (the writing agent, with plans, memories, AnyContext and the conversation) or isolated (a fresh context that receives only the artifact under review and the files it references, assembled by the engine, never the conversation). An isolated step that cannot start a fresh context (no sub-agent or role runner, FEAT-005) stops and says so; it never falls back to a review in the same context. Applies to the review of the specification and plan (FEAT-024), to the code review of Phase 7 (always in a fresh context, replacing 'when the agent can start a second session') and to the attacker of FEAT-020. The mode is a property of the system, not an instruction the agent may skip. Needs FEAT-009 and FEAT-005. Acceptance: the engine refuses to record an isolated review produced in the writer's context, and the context package of an isolated step contains only the artifact and its references.

### FEAT-024: Independent review of the specification and plan before gate 3
- **Priority**: high
- **Epic**: EPIC-010
- **Status**: open
- **Found**: 2026-11-01, EGM concept analysis
- **Plan**: -
- **Description**: In Phase 2, after /speckit-analyze, the specification, plan and tasks (the plan alone without Spec Kit) are reviewed by isolated reviewers (FEAT-023) in stages: zoom out (root cause, simplest design, reuse, right placement), comprehension (a reader with no background understands it), design critique (failure modes, scale, security, alternatives), zoom out again over what the earlier stages added, and implementation readiness (paths, signatures and test cases are concrete). Reviewers read the Spec Kit artifacts; there is no second specification format. The report goes to the spec folder (review.md) with each reviewer's status and the findings by level (blocking, suggestion, nit). Gate 3 fails while the report is missing or older than the artifacts, a reviewer did not run, or a blocking finding is open, unless the user accepts the risk with a reason recorded in the plan, as with --accept-open-bugs. The short path skips it. FEAT-002 separates who writes each plan section; this item adds reviewers that never saw the writing. Acceptance: on sample projects, plans seeded with known flaws (a missing failure mode, an undecided schema, a reinvented utility) get them reported as blocking, and the cost of each review is measured with FEAT-015. For Aurora Dev Suite, the review status of the active plan goes as an optional field in a MINOR version of dcp.diagnose with the capability in dcp version --json, proposed to Aurora before it is written (Aurora's preference, 2026-11-01).

### FEAT-025: Testable pass rules for specifications and plans
- **Priority**: high
- **Epic**: EPIC-010
- **Status**: open
- **Found**: 2026-11-01, EGM concept analysis
- **Plan**: -
- **Description**: The plan template says what to cover; these rules say when it is good enough. Each rule has one pass test and is applied by the generated constitution, the Spec Kit preset and the reviewers of FEAT-024: Zero-Decision (the tasks of the milestone leave no design choice to whoever implements them: schema, API path, dependency, edge-case handling), Alternatives (a decision with alternatives lists the rejected ones with their reasons and becomes an ADR), Zoom Out (root cause, simplest design, reuse of existing code, no speculative features) and Self-containment (a reader with no conversation history understands the specification). Checkers verify the deterministic parts (an alternatives section for each ADR-worthy decision, no open clarification marker); reviewers judge the rest. Against over-specification, Zero-Decision applies to the tasks of the current milestone, not to the whole product.

### FEAT-026: Documentation drift against the code by Gedo entity identity
- **Priority**: medium
- **Epic**: EPIC-010
- **Status**: open
- **Found**: 2026-11-01, EGM concept analysis
- **Plan**: -
- **Description**: Specifications, plans and architecture pages reference code entities by qualified path together with their file or unit (Rust paths start with crate:: without the crate name, so a path alone can be ambiguous in a workspace). When a plan is approved, the engine records the approval commit and the Gedo identity of each referenced entity; it does not compare stored hashes, since Gedo's structural hash is versioned (hash_format) and a format change would look like drift. Later the engine asks Gedo for the change since the approval commit (gedo entities --json --since <approval commit>), so both sides are computed with the same format. Drift is a referenced entity that changed after the approval while the document did not, or was deleted; a rename or a move is a stale reference the document should follow, not drift of behavior; a reference that does not resolve (not_found, ambiguous) is a finding. Formatting and comment edits are not changes, since the structural hash ignores trivia; the docs say so. References Gedo cannot check (unsupported_language, parse_error, or a partial answer in a shallow clone, per the freshness field of Gedo FEAT-135) are reported as not checked, never as no drift. The check is deterministic and runs in the diagnosis, in Phase 6 and at gate 8; a model only judges whether the changed entities still match the text. It complements detect_drift (proposal against the active plan, the intent) with document against code (the reality). Gate 8 lists the findings; whether they block is a setting. Without Gedo, only references that do not resolve are reported, by text search. Extends FEAT-004, through Gedo's versioned --json contract, discovered by capability. Gedo's side, agreed on 2026-11-01: Gedo FEAT-137 gedo entities (capability entities, schema gedo.entities v1), target Gedo v0.5.0 for Rust and v0.6.0 for TypeScript and Python; replies before Gedo's feature plan starts can still change its shape. For Aurora Dev Suite, the drift findings go as optional fields in a MINOR version of dcp.diagnose with the capability in dcp version --json, proposed to Aurora before they are written (Aurora's preference, 2026-11-01). When talking to Gedo or Aurora, cite this item as DCP FEAT-026.

### FEAT-027: Curated project context map indexed by AnyContext
- **Priority**: medium
- **Epic**: EPIC-011
- **Status**: open
- **Found**: 2026-11-01, EGM concept analysis
- **Plan**: -
- **Description**: architecture-docs gains a bootstrap that writes, bottom up, one index per module with its purpose, interface, invariants and side effects, reviewed in a pull request like any document. Its quality bar is the Re-implementation rule (a reader can reproduce the observable behavior from the document alone), judged by an isolated reviewer (FEAT-023). AnyContext indexes the map with priority over raw files, and the full-context steps of the cycle query AnyContext through its versioned command line contract, discovered by capability and skipped safely when missing; isolated steps never call it. The context stays reviewable and travels with the repository, and retrieval runs over a curated base instead of raw text only. AnyContext's side, agreed on 2026-11-01: AnyContext FEAT-196, a versioned machine contract to index and query a repository workspace, in AnyContext v1.1.0: a workspace bound to the repository and synced without interaction, retrieval without generation (ranked passages with source path, location, score and text), source priority for the map folder (a boost inside AnyContext's relevance ranking rather than a reserved budget, decided when FEAT-196 is planned) and index freshness in every reply (for example the last indexed commit). Nothing in AnyContext v1.0.0 offers the contract yet. When talking to AnyContext, cite this item as DCP FEAT-027.

### FEAT-028: Standard status lines for the current phase, step and gate
- **Priority**: high
- **Epic**: EPIC-001
- **Status**: open
- **Found**: 2026-11-06, protocol status messages discussion
- **Plan**: -
- **Description**: The protocol drives the steps, not the developer (unlike Spec Kit, where the developer types each command), so the developer must always see that the protocol is running and exactly where it is. Today phases exist only in the SKILL text: the engine speaks only at the gates (loose check lines that do not name the gate or the protocol) and in the diagnosis, and each agent announces phases its own way, or not at all. (1) A new engine command, dcp phase enter|exit|wait|pause, prints one line from a fixed template and records the current position (path, phase, milestone or step, gate, what the user is asked for) in the active plan file, so it is versioned and the same locally, in cloud sessions and across agents; verify_gate and the diagnosis print the same header. (2) The template is fixed, in English, identical in every environment and searchable in logs, for example: "[Dev Cycle Protocol] ▶ Started · v2.11.0 · <project> · full path", "[Dev Cycle Protocol] ▶ Phase 4/10 · Implementation · milestone 2/5 \"storage\"", "[Dev Cycle Protocol] ✔ Phase 4/10 · Implementation done · next: Phase 5 · Automated tests", "[Dev Cycle Protocol] 🔒 Gate 3 · Plan approval · ✅ passed (6 checks)", "[Dev Cycle Protocol] ⏸ Waiting for you · Gate 3 · reply \"Approved\" or ask for changes", "[Dev Cycle Protocol] ■ Paused · Phase 7/10 · PR #91 waiting for CI · resume with /dev-cycle". The short path shows its own position (short path 2/5); parallel work on Gedo adds the stream ([Dev Cycle Protocol · <stream>]). (3) Command output does not always reach the developer (cloud sessions, apps), so the skills tell the agent to run the command at every transition and copy its line unchanged as the first line of its message, followed by the explanation in the developer's language. (4) The recorded position is what FEAT-015 writes as phase events and what the FEAT-009 state machine keeps; FEAT-019 diagnose may expose it as an optional field. Done after ADJ-001, since the lines and the commands they name use dcp.

### FEAT-029: Protocol activated by the project itself
- **Priority**: high
- **Epic**: EPIC-001
- **Status**: open
- **Found**: 2026-11-06, protocol always on discussion
- **Plan**: -
- **Description**: Today the cycle starts only when the user types /dev-cycle or when the agent picks the skill from its description; the skills are installed on the machine, not in the project, so nothing in the project's repository tells an agent that the project follows the protocol. Make the project activate it for any agent, locally and in cloud sessions, with no step from the user. (1) Instruction files: the configuration step of /dev-cycle (and dcp init) writes a short block managed by dcp, between markers and updated without touching the rest of the file, in AGENTS.md, CLAUDE.md (importing AGENTS.md), GEMINI.md and .github/copilot-instructions.md, the files the main 3-4 agents read when they open the project. The block says the project follows the Dev Cycle Protocol, that the agent runs dcp status before any change and follows dcp next, and never moves to another phase without the engine; it carries the install command for when dcp is missing, so an agent on a new machine or in a cloud container sets it up on its own. (2) Session-start hooks, in the agents that support them (Claude Code SessionStart and the equivalents of the others, each confirmed when planned): the hook runs dcp status and puts the current phase and next step in the agent's context without relying on the agent reading the file. Hook settings are shared project files, committed like the attribution settings (IMP-008). (3) The diagnosis reports a missing or outdated block or hook, like it does for attribution. Instructions start the agent on the protocol; FEAT-030 is what keeps it there. Pulls forward part (3) of FEAT-009 without MCP. Done after ADJ-001 and FEAT-028.

### FEAT-030: Phase order enforced by the engine
- **Priority**: high
- **Epic**: EPIC-001
- **Status**: open
- **Found**: 2026-11-06, protocol always on discussion
- **Plan**: -
- **Description**: Phases exist only in the skill text and the gates run only when the agent calls them, so nothing stops an agent from writing code before gate 3 or opening a pull request without an approved plan. The agent is guided step by step and cannot skip a phase on its own. (1) The engine owns the order, on the position FEAT-028 records in the active plan: dcp next returns the current step, what it requires and its exit criteria; dcp phase exit passes only when the phase's exit criteria are checked (for example the plan saved and gate 3 approved before Phase 4); dcp phase enter refuses a phase out of the order of the path (full or short); a user gate closes only with the user's approval recorded with the text of their answer. (2) Git hooks, installed by dcp init in a committed folder through core.hooksPath: pre-commit refuses commits of code when the phase does not allow it (before gate 3, for example), pre-push refuses a push without an active plan; any agent, any environment, though --no-verify skips them. (3) Agent hooks where the agent has them (Claude Code PreToolUse and the equivalents of the others, each confirmed when planned): block edits of code files outside the implementation phases and block --no-verify, stopping the agent before the change is made. (4) A required CI check on every pull request, dcp verify-trail, re-checks the cycle's trail (approved plan, phases in order, gates recorded and run again) and fails the merge gate when something was skipped; with branch protection requiring it, a skipped phase means a pull request that cannot be merged. Templates for GitHub Actions, GitLab CI, Azure Pipelines and Bitbucket Pipelines. (5) The status lines of FEAT-028 come from the engine, so an agent that only claims to follow the protocol is exposed. Inside each phase the agent works freely; the engine enforces the order and the gates, not how the work is done. Pulls forward the state machine and the CI check of FEAT-009 without MCP; FEAT-015 writes the same transitions as phase events. Done after FEAT-028.

### FEAT-031: Lessons per role learned from the cycle
- **Priority**: medium
- **Epic**: EPIC-001
- **Status**: open
- **Found**: 2026-11-08, autonomous agents analysis
- **Plan**: -
- **Description**: The same mistake comes back in every new session: a gate rejected yesterday for a missing migration or a review finding already fixed once is repeated because nothing carries it forward (context rot). Each role (the phase skills today, the FEAT-002 specialists later) gets its own lessons, kept in the vector store of IMP-010, so what the release role learns does not mix with what the design role learns. (1) The protocol writes them, never the agent freely: the engine records a lesson when a gate is rejected (with the reason the user gave), when a review finding is blocking, and when the user corrects a phase at one of their gates; each lesson keeps its source (plan, gate, finding, date), the role and a short rule. (2) At the start of each phase the engine returns the role's most relevant lessons with the step's instructions, so a fix made once stays fixed. (3) Lessons are not edited by hand: the user curates them only through engine commands (dcp lessons list, show, forget and restore), every change recorded with who and why and reversible, so a correction can improve the memory but a careless edit cannot corrupt it. (4) Lessons that keep coming back feed the FEAT-015 metrics (rework by cause). Origin: the analysis of AWS Kiro bots (The New Stack, 2026-11-07) on 2026-11-08: per-bot durable memory, "fix a mistake once and it stays fixed"; the idea is taken, Kiro is not adopted (it is tied to one provider). Done after IMP-010 and FEAT-015.

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

### IMP-009: Review synthesis rules
- **Priority**: medium
- **Epic**: EPIC-010
- **Status**: open
- **Found**: 2026-11-01, EGM concept analysis
- **Plan**: -
- **Description**: Whenever the protocol runs several reviewers (FEAT-024, the FEAT-002 lenses, the Phase 7 code review): a reviewer that fails, times out or returns no report is retried once and then marked not run, never counted as zero findings, and a reviewer not run fails the gate it feeds; findings are deduplicated and normalized to blocking, suggestion and nit; optional duplicate reviewers (two independent runs of the same lens) raise recall, a setting off by default because of its cost.

### IMP-010: Vector store on LanceDB with local, remote and repository storage
- **Priority**: medium
- **Epic**: EPIC-005
- **Status**: open
- **Found**: 2026-11-08, autonomous agents analysis
- **Plan**: -
- **Description**: The memory skills (session-memory, plan-memory, folder-sync and the lessons of FEAT-031) use crates/vector-store, a Rust reimplementation of chromadb 1.5's on-disk format kept for compatibility with the old Python scripts, which no longer exist. Replace it with LanceDB: embedded in the dcp binary through its Rust library, no server or separate process, columnar files on disk. One setting, memory.storage, chooses where the store lives: (1) local (default): a folder on the developer's machine, as today; (2) remote: a connection URI to object storage (S3, Azure Blob, Google Cloud Storage or an S3-compatible server such as MinIO inside the customer's network), which keeps the memory in cloud sessions and shares it between the developers of a project; (3) repository (opt-in, off by default, since many customers will not want it in their repository): the records are kept as text (JSONL) in the project and the index is rebuilt locally, because LanceDB's binary files have no useful diff and grow with every commit. A migration command moves existing chromadb stores into LanceDB, read only once and never written again. To check when planned: concurrent writes by several developers in remote mode, the binary size the LanceDB dependencies add, and the default embedding model staying offline. Credentials for remote mode come from the environment or the provider's standard configuration, never from the project's files.

### IMP-011: Spec Kit built into the dcp binary, with no uv or Python
- **Priority**: high
- **Epic**: EPIC-006
- **Status**: open
- **Found**: 2026-11-08, Spec Kit without Python discussion
- **Plan**: -
- **Description**: The engine is pure Rust, but speckit_setup still runs the specify CLI through uvx --from git+https://github.com/github/spec-kit.git@vX.Y.Z (init, preset add and update, integration upgrade, extension update, check) and git ls-remote for the latest tag, so every project that enables Spec Kit needs uv, Python and access to GitHub, which fails on premise and air-gapped. Embedding a Python interpreter in the binary (PyO3, RustPython or a standalone Python) was considered and rejected: tens of MB per platform, fragile with Spec Kit's dependencies, and still Python. Instead each dcp release carries a snapshot of the pinned Spec Kit version: (1) the release CI (the only place uv and Python remain) runs that version's specify init for every supported integration and script type, plus the git extension and the protocol's preset, and captures the generated files and manifests; (2) the snapshot is compressed into the binary, and dcp init writes it natively, offline, the same on Windows, macOS and Linux; (3) upgrades use the sha256 of every file that Spec Kit already writes in .specify/integrations/*.manifest.json: files nobody changed are replaced, files the user edited are kept and reported, and projects installed earlier through uvx upgrade the same way; (4) a parity test in CI compares dcp init and upgrade byte for byte with the real specify of the same version, in the style of IMP-005; (5) a scheduled CI job finds new Spec Kit releases, builds the snapshot, runs the existing compatibility check and opens a pull request that bumps the pinned version, so the protocol keeps up with upstream; (6) the latest version a project can pin is the one in its dcp binary, so an air-gapped customer upgrades Spec Kit by taking a newer dcp from its internal artifact repository; (7) the py script type is removed (sh and ps remain). Spec Kit is MIT: its license and notice ship with the binary and the release assets, compatible with MIT OR Apache-2.0. Out of scope: third-party extensions and presets from Spec Kit's catalog (the protocol uses only the git extension and its own preset). To check when planned: the size the snapshot adds to the binary and how presets resolve templates. Done after the Spec Kit 1.3.2 upgrade and ADJ-001.

### IMP-012: Spec Kit scripts as dcp subcommands, with no bash or PowerShell
- **Priority**: medium
- **Epic**: EPIC-006
- **Status**: open
- **Found**: 2026-11-08, Spec Kit without Python discussion
- **Plan**: -
- **Description**: After IMP-011 the cycle still runs Spec Kit's shell scripts: .specify/scripts (create-new-feature, setup-plan, setup-tasks, check-prerequisites, resolve-template and their common helpers) and the git extension's scripts (create-new-feature-branch, auto-commit, initialize-repo), in bash or, on Windows, in PowerShell 7 (pwsh), which is not installed by default. Each script becomes a dcp subcommand (dcp speckit <script>, names fixed when planned) with exactly the same arguments, JSON output, files written and exit codes as the upstream script of the pinned version, checked by a parity test against both the bash and the PowerShell script; speckit_setup's own calls (branch_script_cmd and setup_plan_cmd) use them too. The snapshot of IMP-011 rewrites the script references in the agent command files to the subcommands, so the agents call dcp and the project needs neither bash nor pwsh, and the scripts are no longer copied into the project; the scheduled upstream job fails the bump when a script changed, until the subcommand follows it. With the subcommands in the engine, FEAT-030 can also record and check them as steps of the cycle. Done after IMP-011.

## Adjustments

### ADJ-001: Engine binary renamed to dcp
- **Priority**: high
- **Epic**: EPIC-006
- **Status**: open
- **Found**: 2026-10-28, 2026-10-28, Aurora Dev Suite integration discussion
- **Plan**: -
- **Description**: The engine's binary and launchers are called dcp (the name Aurora Dev Suite and the Levix products use) instead of dev-cycle: the binary, bin/dcp and bin/dcp.ps1, the release assets, and every SKILL.md and doc that calls the engine. The /dev-cycle command keeps its name. Done right after the migration (IMP-005 stays a pure translation), before the first release of the versioned JSON contract that Aurora reads; projects already using the protocol keep working through the launcher.

### ADJ-002: folder-sync stats output still in Portuguese
- **Priority**: low
- **Status**: open
- **Found**: 2026-11-01, BUG-005 plan
- **Plan**: -
- **Description**: stats.py prints 'Volume total de dados', 'Nunca' and 'Maiores documentos indexados' while every other message of the skill is in English. Translate them (after the Rust migration, which keeps the output identical).

### ADJ-003: Printed texts and templates still name the Python scripts
- **Priority**: medium
- **Epic**: EPIC-006
- **Status**: open
- **Found**: 2026-10-31, Rust migration (IMP-005), kept identical by the pure translation
- **Plan**: -
- **Description**: Engine messages and project templates still tell the user to run scripts that no longer exist, such as plan-memory/backlog.py validate, roadmap.py add-release, speckit_setup.py init, constitution.py generate and gedo_feedback.py. Replace every script name in printed texts, usage lines and templates (BACKLOG, ROADMAP and bug plan templates included) with the engine command, under the dcp name (ADJ-001), in the same release as FEAT-028 so the output is reworked once.

### ADJ-004: Register the Dev Cycle Protocol trademark before the public launch
- **Priority**: medium
- **Epic**: EPIC-006
- **Status**: open
- **Found**: 2026-11-06, levix-admin-hub license proposal
- **Plan**: -
- **Description**: The strategy and distribution document asks the owner to register the trademark of the name Dev Cycle Protocol (and dcp, if the owner decides so) before the public launch, so anyone can fork and reuse the open source code (MIT OR Apache-2.0) while the name stays with Levix Digital. Not a code change: the owner files the registration before the public launch.

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
- **Description**: Execution without manual installs, Spec Kit built into the engine with no uv, Python, bash or PowerShell, standard-library search, onboarding in three levels, the protocol served by the signed engine and the open source distribution under MIT OR Apache-2.0, with the name's trademark registered before the public launch.

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

### EPIC-010: Independent verification
- **Priority**: high
- **Status**: open
- **Found**: 2026-11-01, EGM concept analysis
- **Description**: Artifacts written with full context are checked by reviewers that start without it, against testable pass rules, with the engine enforcing that the review ran, and specifications stay true to the code. Origin: the concept analysis of Google's Elephant-Goldfish Model (EGM, Apache 2.0) on 2026-11-01: the more context the writer has, the stronger its confirmation bias, so withheld context is a verification tool; the ideas are taken, the tool is not adopted (it competes with Spec Kit's specification format).

### EPIC-011: AnyContext integration
- **Priority**: medium
- **Status**: open
- **Found**: 2026-11-01, EGM concept analysis
- **Description**: The cycle's full-context steps take their context from AnyContext through its versioned command line contract, over a curated, versioned map of the project, discovered by capability and skipped safely when missing.

## Closed

### FEAT-007: Proprietary binary-only distribution with Levix licenses
- **Priority**: medium
- **Epic**: EPIC-006
- **Status**: dropped
- **Found**: 2026-10-25, 2026-10-25, platform strategy discussion; redefined 2026-10-28, protocol as a commercial product; decided 2026-10-28, same model as Aurora Dev Suite (its ADR-0005)
- **Plan**: -
- **Closed**: 2026-11-07, dropped by the owner's licensing decision of 2026-11-06 (strategy and distribution document v3): the protocol is open source under MIT OR Apache-2.0 and the repository stays public; license files and the crates' license field added; the trademark of the name is ADJ-004
- **Description**: The protocol follows the decision every Levix product shares (Aurora Dev Suite ADR-0005): proprietary, freemium, distributed only as binaries. The source repository becomes private; the signed dcp binary and installers are published to a separate public releases repository (the AnyContext pattern). Every crate has publish = false and the proprietary license notice; dependencies stay permissive (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode-3.0), checked with cargo-deny. Licenses (one per plan, signed by Levix, verified offline) and the verified download catalog come from the levix-platform components once they exist; until then the engine has no license check. The free tier and paid plans are open, as for the other products. Replaces the open source launch; the license, not hidden instructions, is the protection (FEAT-009). Record the decision as an ADR in this repository when the item is implemented.

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

### BUG-005: folder-sync stats fails on any synced index
- **Severity**: medium
- **Status**: done
- **Found**: 2026-11-01, Rust migration M40 review of stats.py
- **Plan**: ai_code_agent_skills_folder_sync_stats_fails_on_any_synced_index
- **Closed**: 2026-11-01, plan ai_code_agent_skills_folder_sync_stats_fails_on_any_synced_index
- **Description**: stats.py formats the last sync time with datetime.fromtimestamp, but never imports datetime. Every index that has synced at least one file has a last sync time, so /folder-stats stops with NameError instead of showing the statistics. Only an empty or missing index works.

### BUG-006: release_status reads no version from JSON files other than package.json
- **Severity**: medium
- **Status**: done
- **Found**: 2026-11-01, Rust migration acceptance test (gate 8 on this repository)
- **Plan**: ai_code_agent_skills_release_status_reads_no_version_from_json_version_files
- **Closed**: 2026-11-02, plan ai_code_agent_skills_release_status_reads_no_version_from_json_version_files
- **Description**: read_version_file parses only package.json as JSON. Any other JSON version file (.claude-plugin/plugin.json, composer.json, manifest.json) falls to the generic pattern version\s*[=:], which never matches "version": because of the closing quote. release_status version and next-rc fail with 'no version found', and gate 8 is blocked. This repository's own v2.10.0 gate 8 fails on .claude-plugin/plugin.json. Same behavior in the Python script and the engine.

### IMP-005: Translate the scripts to Rust with identical behavior
- **Priority**: high
- **Epic**: EPIC-006
- **Status**: done
- **Found**: 2026-10-27, 2026-10-27, Rust migration discussion
- **Plan**: ai_code_agent_skills_translate_the_scripts_to_rust_with_identical_behavior
- **Closed**: 2026-11-03, plan ai_code_agent_skills_translate_the_scripts_to_rust_with_identical_behavior
- **Description**: Translate every Python script of the skills into one Rust binary with no change in behavior: same commands, flags, stdout, stderr, exit codes and files written, proven by comparing the Python and Rust outputs on the same fixtures. One script per pull request: when the outputs are identical, the SKILL.md calls the Rust command and the Python script is deleted. When no script is left, the Python requirements and tests are removed.

### BUG-003: Ignored protocol files reported only partially on Windows
- **Severity**: medium
- **Status**: done
- **Found**: 2026-10-27, 2026-10-27, first CI run on Windows (PR #22)
- **Plan**: -
- **Closed**: 2026-11-09
- **Description**: onboard.py sends the list of protocol files to 'git check-ignore --stdin' in text mode. On Windows each newline becomes CRLF, Git looks for 'path\r' and only the last path matches, so the diagnosis and write-config warn about one ignored file instead of all of them (the BUG-002 protection). Send the input as bytes.

### IMP-013: Candidate and promote workflows for the skills releases
- **Priority**: high
- **Epic**: EPIC-001
- **Status**: done
- **Found**: 2026-11-09, v2.10.0 release
- **Plan**: ai_code_agent_skills_candidate_and_promote_workflows_for_the_skills_releases
- **Closed**: 2026-11-09, plan ai_code_agent_skills_candidate_and_promote_workflows_for_the_skills_releases
- **Description**: The repository follows the trunk model but no workflow publishes a product release: a vX.Y.Z-rc.N tag gives no pre-release and there is no promote workflow for the user's release gate (check_ci_policy warns). Add release.yml (pre-release for candidates, release for final tags, checks the version file and the engine release) and promote.yml (the user's promotion of an accepted candidate, held by the release environment).
