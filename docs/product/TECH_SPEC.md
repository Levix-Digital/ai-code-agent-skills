# Dev Cycle Protocol: Technical Specification

A set of skills in the open `SKILL.md` format, each a module with its instructions and the Python command line scripts that check and change the project's files; agents read the skills and run the scripts. The architecture style is a modular monolith (`architecture.style: modular-monolith`): one repository and one plugin, split into skills by role, each with a small public interface (its `SKILL.md` and its script commands) and a small shared kernel. How each part works is in the technical documentation ([TECDOC.md](../../TECDOC.md)).

## Platforms and Stack

- **Agents**: any agent that reads `SKILL.md` (Claude Code, Codex, Gemini, Copilot and others), linked from one checkout by `install.sh` / `install.ps1`; Claude Code cloud sessions get the skills through the plugin in `.claude-plugin/`.
- **Operating systems**: Windows, macOS and Linux; paths and commands work on all three.
- **Python 3.10 or later** for every script: available wherever agents run, readable by every agent, testable with `pytest`. Scripts use the standard library first.
- **Third-party Python packages** (`requirements.txt`): `chromadb` for the plan and session search indexes and folder RAG, `pyyaml` for frontmatter and CI files, `python-docx` and `pypdf` for folder extraction. IMP-005 (v2.10.0, ADR-0007) translates every script into one Rust binary with identical behavior, script by script, and then removes Python.
- **Platform command lines**: `gh` (GitHub, REST API) and `glab` (GitLab) for pull requests; they keep the user's authentication.
- **GitHub Spec Kit**, optional per project, at a pinned and verified version (v1.2.3 today), run through `uvx`.
- **Gedo**, optional per project (`gedo.enabled`), through its command line with `--json` only, once it has one (v0.4.0); streams for parallel work from v0.5.0.

## Architecture Overview

- **Entry and orchestration**: `dev-cycle` (the command) and `dev-cycle-protocol` (phases, roles, gates, onboarding diagnosis, Spec Kit setup, constitution, pull request sync).
- **Role skills**: `product-inception`, `plan-memory` and `backlog`, `software-design`, `code-review`, `quality-assurance`, `devops-release`, `architecture-docs`, `user-docs`.
- **Support skills**: `session-memory`, `folder-sync`, `gedo-feedback`.
- **Shared kernel**: the project configuration (`project_config.py`, `.dev-cycle.json`) and the attribution guard (`_no_ai.py`, principle 9).
- **Gates** call the role skills' checkers (`verify_gate.py`), never the other way around; skills talk to each other through files in the project (backlog, roadmap, plans, product docs, ADRs) and through their documented commands.

## Non-Functional Requirements

### Performance

- The `/dev-cycle` diagnosis answers in under 5 seconds, platform calls included (4.2 s on this repository, measured 2026-10-25 in a cloud session).
- Every checker runs on the project's files only, without network, except the platform and Spec Kit commands that need it.

### Security and Privacy

- No credentials in the repository or the skills: platform access goes through the user's `gh` and `glab` authentication.
- Secret hygiene is checked at gate 0 and gate 8.
- Session memory and indexes stay on the developer's machine (`~/.ai-code-agent-memory/`), never versioned.
- Principle 9: no AI attribution in anything the protocol writes or posts.

### Availability and Recovery

Not applicable as a service: the protocol runs locally. Its state is the project's files under Git; search indexes are disposable and rebuilt from them.

### Accessibility and Localization

- Skills, docs and every artifact are written in English; the agent talks to the user in the user's language.
- Command line output uses plain text with optional emoji markers; `--json` is available where another tool consumes the output.

## Integrations

| System | Direction | When it is unavailable |
|---|---|---|
| GitHub (`gh`, REST API; GraphQL only for thread resolution) | read and write pull requests, reviews, checks, rules | the diagnosis reports it as a finding; the merge gate fails safe |
| GitLab (`glab`, REST API) | read and write merge requests, discussions, pipelines | same as GitHub |
| GitHub Spec Kit (`uvx`, pinned version) | installs and upgrades the project's Spec Kit | setup stops with a message; the cycle runs without Spec Kit |
| Gedo (command line with `--json`: reads from v0.4.0, streams from v0.5.0) | diagnosis, drift, gates, streams for parallel milestones | no parallelism; the cycle runs in sequence |
| Coding agents (Claude Code, Codex, Gemini, Copilot) | runners start a role in its own context (FEAT-005) | the current agent plays the role in its own context |

## Data

- **Project files** (versioned): `.dev-cycle.json`, `docs/product/`, `docs/BACKLOG.md`, `docs/ROADMAP.md`, `docs/plans/`, `TECDOC.md`, `docs/architecture/`, `docs/adr/`, `tests/MANUAL_TESTS.md`, `.specify/` and `specs/` with Spec Kit.
- **Local data** (never versioned): plan search index and session memory under `~/.ai-code-agent-memory/`.
- **Migrations**: configuration keys are only added, never renamed without a migration (`project_config.py migrate`); file formats stay backward compatible (old plans without milestone dependencies keep working).

## Constraints

- Plain Markdown skills that any agent can read; no agent-specific syntax in the skills.
- No code specific to one environment; provider-dependent features cover the main three or four providers.
- No parallel agents without Gedo.
- Integration with Gedo only through its command line with `--json`; the repositories stay separate.
- No paid service or paid platform feature required (no CodeQL on private repositories, no AI review bot in CI).
- Releases follow `devops-release` with the trunk model; the version lives in `.claude-plugin/plugin.json`.

## Open Questions

None.
