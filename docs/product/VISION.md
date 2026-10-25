# Dev Cycle Protocol

A development cycle for coding agents: open `SKILL.md` skills and deterministic checks that take a request from idea to verified release, with the user deciding at every gate. It works with any agent that reads `SKILL.md`, on a local machine or in a cloud session, and grows into coordinated parallel agents when the project uses Gedo.

## Problem

Coding agents made writing code several times faster, and the bottleneck moved to everything around the code:

- **Collisions**: several agents and developers editing the same repository break each other's signatures and fight over the same files and lockfiles.
- **Architectural drift**: agents without deterministic governance cross layers, duplicate abstractions and leave the technical docs and decisions outdated within weeks.
- **Review fatigue**: agent pull requests mix refactoring, formatting and new logic in diffs too large to review safely.
- **No traceability**: nobody can show which changes went through a specification, a design check and an acceptance test.

Today each developer copes with prompts, personal checklists and manual review. Every agent works differently, nothing is checked deterministically, and the process lives in the chat instead of the repository.

## Target Users

- **Levix Digital developers (primary, now)**: build Levix products (Gedo first) with agents in several environments; want one cycle that guides them from idea to release and keeps decisions in the repository.
- **Developers and small teams using coding agents (after the open source launch)**: work with Claude Code, Codex, Gemini, Copilot or similar; want structure, gates and review without adopting a platform or paying for a service.
- **Engineering teams and companies (through the Levix platform, a separate product)**: need coordination across machines, managed infrastructure and compliance evidence on top of the same engine.

## Value Proposition

- One entry command (`/dev-cycle`) that diagnoses the repository and says where to start; the user decides, the protocol guides.
- Deterministic gates (`0`, `3`, `rc`, `8`) and a merge gate instead of trusting the agent's word.
- Roles (product, planner, architect, reviewer, QA, DevOps, writers) with their own skill and, from v3.0.0, their own context, so the one who reviews is not the one who wrote.
- Decisions, plans, backlog and roadmap as files in the repository, shared by every agent and developer.
- No lock-in: plain Markdown and Python, any agent, any environment, GitHub and GitLab today.
- With Gedo, parallel agents per milestone over entity-level streams; without Gedo, the same cycle runs in sequence.

## Business Model

Open-core. The Dev Cycle Protocol and Gedo are the two free engines and stay separate repositories. The paid Levix platform (Team Cloud as a service, Enterprise self-hosted) joins them in a visual and cloud layer: managed relay, real-time pull request service, synced memory, team dashboard and compliance reports. The protocol stays internal and unlicensed until the open source launch (FEAT-007), then opens under a permissive license. Price per developer is a hypothesis of the platform, not of this product.

## Success Metrics

- **Lead time**: median time from an approved plan (gate 3) to the merge goes down release after release; baseline measured in v2.9.0.
- **Rework after gates**: fewer than 1 in 5 pull requests needs a second review round for a blocking finding.
- **Escaped bugs**: no critical or high bug found after a release that its release candidate should have caught.
- **Zero-friction install**: every skill runs on a clean Python 3.10+ without `pip install` (target for v2.9.0).
- **Adoption (after the open source launch)**: number of repositories with a `.dev-cycle.json`.

## Scope

- **Cycle and gates**: the `/dev-cycle` command, phases 0 to 10, gates and the merge gate, configuration in `.dev-cycle.json`.
- **Product and planning**: product inception, backlog with epics, roadmap of releases, versioned plans with milestones and their dependencies.
- **Pull requests and review**: the pull request as the channel, the protocol's code review, on GitHub, GitLab, Bitbucket, Azure DevOps and Gitea.
- **Multi-agent orchestration**: each role in its own context; with Gedo, parallel milestones over streams and deep integration of Gedo's checks into the gates.
- **Agents and environments**: runners for the main coding agents (Claude Code, Codex, Gemini, Copilot) on a local machine and in cloud sessions.
- **Zero-friction adoption**: execution without manual installs, a search engine from the standard library, onboarding in three levels and the open source launch.
- **Quality, release and documentation**: test strategy and manual test runs, branching models, CI policy, releases, living technical docs and user READMEs.
- **Memory**: plans shared by the team and each developer's session memory.

## Non-Goals

- No paid service required to use the protocol.
- No code specific to one environment: what depends on a provider covers the main three or four.
- No AI attribution in commits, pull requests, reviews or docs (principle 9).
- No dependency on a single coding agent.
- No parallel agents without Gedo: without it, everything runs in sequence.
- No merge with the Gedo repository: integration only through Gedo's command line with `--json`.
- Not the platform: the dashboard, editor extension, managed relay, cloud memory and enterprise features are separate products.

## Risks and Assumptions

- **Gedo timing**: parallel work needs Gedo v0.4.0 (command line) and Gedo support for the project's language (Python in v0.6.0). Mitigation: role isolation in sequence ships first (v3.0.0); nothing in a release depends on an unreleased Gedo version.
- **Agent and platform drift**: agent command lines and platform APIs change (GraphQL refused in some environments, for example). Mitigation: REST first, provider adapters behind one model, tests with recorded responses.
- **Process weight**: the full cycle may feel heavy for small changes. Mitigation: the short path, and onboarding in three levels (FEAT-006).
- **Python environment friction**: users hit missing modules when the agent's Python is not the one with the dependencies. Mitigation: execution through `uv` and a standard-library search fallback (IMP-003).
- **Assumption**: teams will keep decisions and plans in the repository as files rather than in an external tool.

## Open Questions

- When the open source launch happens and under which license (MIT, Apache-2.0 or both); planned with Gedo v0.5.0 or v0.6.0.
- Whether the native verifiers move to Rust next to Gedo; decided in an ADR once Gedo v0.4.0 exists.
