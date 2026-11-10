# Dev Cycle Protocol

**The engineering process for your coding agents.**

The agent writes the code. Dev Cycle Protocol makes sure it was done the right way: specification, plan, tests, review and release, with no steps skipped.

## The problem

Coding agents (Claude Code, Codex, Gemini, Copilot) made writing code several times faster, and the bottleneck moved:

- **No process.** Every agent works its own way, and the process lives in the chat instead of the repository.
- **Review becomes impossible.** Huge pull requests that mix refactoring, formatting and new logic.
- **Architecture drifts.** Layers get crossed, abstractions get duplicated and the docs are out of date within weeks.
- **No traceability.** Nobody can show which change went through a specification, a review and an acceptance test.

## The solution

Dev Cycle Protocol is a governance layer on top of the agent. It does not replace the agent or the tools your team already uses. It walks the agent through the same cycle a senior team follows, and checks every step deterministically instead of taking the agent's word for it.

**Idea → Plan → Implementation → Tests → Documentation → Review → Release candidate → Acceptance → Release**

At every gate, you decide. The agent proposes, the protocol checks, you approve.

## What it does today

- **One entry command, `/dev-cycle`.** It diagnoses the repository and tells you where to start: a plan in progress, a blocking bug, missing setup or the next item of the roadmap.
- **Verifiable gates.** The plan is approved before coding, critical bugs come before new features, manual tests pass before a release, and no secret enters the repository.
- **Specialized roles.** Product, planning, architecture, code review, QA, DevOps and documentation, each with its own rules and checker.
- **Review on the pull request itself.** Line comments with a level (blocking, suggestion, nit) and a concrete fix, on GitHub and GitLab.
- **Everything as files in the repository.** Product vision, backlog, roadmap, plans and architecture decisions are versioned and shared by every agent and developer.
- **Specification with GitHub Spec Kit**, optional and built into the cycle.

## Why Dev Cycle Protocol

- **No vendor lock-in.** Works with any model and any agent that reads `SKILL.md` skills, on your machine or in the cloud.
- **Lightweight.** Plain Markdown and one portable binary. No platform to adopt.
- **Open.** Open source (MIT or Apache-2.0) and free to use.
- **You stay in control.** The protocol guides; plan, acceptance and release decisions are always yours.

## Coming soon

- **Guardrails for agent-written code:** pull requests blocked on invented or vulnerable dependencies, weakened tests, changes outside the plan and insecure code.
- **Always-on protocol:** the project turns the protocol on in any agent, and the engine enforces the order of the phases with hooks and a required CI check.
- **A cycle that measures itself:** automatic metrics per phase, production bugs and CI failures coming back as triaged items, and a security attack on every release candidate.
- **Parallel agents** with Gedo, each on its own stream of work, without collisions.

## The vision

Dev Cycle Protocol is the process core of the **Aurora Dev Suite: Dev Cycle Protocol + Spec Kit + Gedo + AnyContext**, a platform that orchestrates development with coding agents from the first idea to a verified release.

## Get started

Clone the repository, run the installer and type `/dev-cycle` in your agent: https://github.com/Levix-Digital/ai-code-agent-skills
