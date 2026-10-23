---
plan_id: ai_code_agent_skills_pull_request_review_through_the_platform
project: ai-code-agent-skills
title: Pull request review through the platform
version: v2.4.0
status: in_progress
author: Levix Digital
created_at: '2026-10-23T20:11:03'
updated_at: '2026-10-23T20:32:40'
session_id: null
tags:
- pull-requests
- code-review
- github
- gitlab
- dev-cycle-protocol
supersedes: null
superseded_by: null
parent_plan_id: null
backlog_items: []
milestones:
- name: M1 Model and GitHub reads
  done: true
  done_at: '2026-10-23'
  by: Levix Digital
- name: M2 Writes and the guard
  done: true
  done_at: '2026-10-23'
  by: Levix Digital
- name: M3 GitLab
  done: true
  done_at: '2026-10-23'
  by: Levix Digital
- name: M4 Reviewer and the cycle
  done: false
- name: M5 Documentation and release
  done: false
status_log:
- at: '2026-10-23T20:11:03'
  by: Levix Digital
  status: active
  note: Plan created
- at: '2026-10-23T20:27:28'
  by: Levix Digital
  status: active
  note: Approved by the user
- at: '2026-10-23T20:30:39'
  by: Levix Digital
  note: 'Milestone ''M1 Model and GitHub reads'' completed: _pr_model.py (model, pending, triage hints, markers), GitHub provider through gh (pull request, threads by GraphQL, checks, reviews), pr_sync.py status and open; recorded responses in tests'
- at: '2026-10-23T20:31:18'
  by: Levix Digital
  note: 'Milestone ''M2 Writes and the guard'' completed: _no_ai.py guard (attribution, not subject; code spans and paths ignored; allowed terms); pr_sync reply, resolve, comment, review (marker and level), request-review; gate 8 checks commit messages since the last tag'
- at: '2026-10-23T20:32:40'
  by: Levix Digital
  note: 'Milestone ''M3 GitLab'' completed: GitLabProvider through glab api (merge request, discussions with pagination, approvals, pipeline; reply, resolve, comment, review with diff positions, reviewers), self-hosted hosts; recorded responses in tests'
---

# Pull Request Review Through the Platform

## 1. Goal and decisions

Today Phase 7 opens the pull request and follows CI, but review happens in the chat: comments made on the platform are not read unless the user pastes them. A team works the other way: the pull request (merge request on GitLab) is where the change is reviewed, discussed and corrected, and every correction of that change is a commit on its branch answered on its thread.

This plan makes the pull request the channel of review. The protocol reads review threads, suggestions, reviews and CI from the platform, fixes what is small, brings what is large to the user, answers and resolves on the platform, and runs its own code review on every pull request, while it keeps guiding the user through each step.

Decisions:

- **D1 — The pull request is the source of truth of Phase 7.** After the pull request is open, every correction of the change is a commit on the same branch, and every answer goes to the thread it answers. The agent reads the pull request's state at each sync point (D7), not only what the user says in the chat.
- **D2 — GitHub and GitLab first.** Adapters for GitHub (`gh` CLI) and GitLab (`glab` CLI, gitlab.com or self-hosted). Bitbucket, Azure DevOps and Gitea come later (backlog FEAT-001). The provider is detected from the `origin` remote and can be set in `.dev-cycle.json` (`review.provider`, `review.host`).
- **D3 — The protocol reviews every pull request.** As soon as a pull request is open (and again after pushes that change code), the reviewer role reviews the diff with its checklist (design, tests, documentation, security, the plan's acceptance criteria) and posts the result on the platform as a review with line comments. Each comment starts with a marker (`**[review]**`, `review.marker`) and a level (`blocking`, `suggestion`, `nit`) so it is told apart from a person's review. The review is published with the user's account and never approves: approval stays with people.
- **D4 — Small things are fixed; large ones go to the user.**

  | Thread | What the agent does |
  |---|---|
  | Small and local: nit, rename, missing test, lint, a `suggestion` block | Fixes it, pushes, replies on the thread ("Fixed in `<sha>`") and resolves it |
  | Large: design, interface or data change, several files, unclear intent | Brings it to the user with the context and options; answers on the thread only after the user decides; a deferred ask becomes a backlog item linked in the reply |
  | Question | Answers from the code and the plan |
  | Red CI or a merge conflict | Reads the logs, reproduces, fixes and pushes (as today in `devops-release`) |

  A thread is resolved only after its fix is pushed or the reviewer agrees. The agent never approves or merges into the production branch.
- **D5 — No AI mention, ever (principle 9).** Every text the protocol posts (reviews, replies, descriptions, commit messages) goes through a guard that refuses mentions of an AI, a model or a coding agent; commit messages of a release are checked at gate 8.
- **D6 — One state model for every platform.** The adapters return the same structure: the pull request (number, state, draft, mergeability, branches), checks, reviews, threads (file, line, resolved, outdated, comments, suggestions) and general comments, plus what is **pending**: threads waiting for an answer, changes requested, failed checks, conflicts. The rules (triage hints, pending items) are pure functions over this model.
- **D7 — When the agent reads the pull request.** Some environments receive pull request events (comments, reviews, CI results) as they happen; there the agent reacts at once. Elsewhere nothing tells the agent that someone commented on the platform, so it reads the pull request at fixed moments:
  1. at the start of every session, through the `/dev-cycle` diagnosis, which lists the open pull requests with pending items;
  2. after every push, until CI finishes;
  3. whenever the user says they reviewed or commented ("I left comments on the PR");
  4. optionally on a timer, in agents that can schedule a recurring check.
- **D8 — Open pull requests in the diagnosis.** `/dev-cycle` shows open pull requests of the repository with pending items as an option right after a plan in progress ("PR #12: 3 threads waiting, CI red, 1 design question for you").

Alternatives considered:

- **Keep review in the chat**: loses the team's history and the people who review on the platform. Rejected (D1).
- **One script per platform with its own output**: every rule would be written twice. Rejected (D6).
- **The agent approves its own pull requests after its review**: approval is a person's decision. Rejected (D3).

## 2. Design (`software-design`)

### Components

| Unit | Responsibility |
|---|---|
| `dev-cycle-protocol/_pr_model.py` | The normalized model (D6), pending items, triage hints (suggestion blocks, nit and question markers, size of the ask), the review marker and levels; pure functions |
| `dev-cycle-protocol/_pr_providers.py` | `GitHubProvider` (`gh api`, GraphQL for threads and resolution) and `GitLabProvider` (`glab api`, discussions and notes), each with an injectable command runner; provider detection from the remote |
| `dev-cycle-protocol/_no_ai.py` | The guard of principle 9: finds mentions of an AI, a model or a coding agent in a text (names, "AI-generated", "generated with", co-author trailers), used before anything is posted or committed |
| `dev-cycle-protocol/pr_sync.py` | CLI: `status [--pr N] [--json]` (state and pending items of the current branch's pull request or of `N`), `open [--json]` (open pull requests with pending items), `reply`, `resolve`, `comment`, `review --file <review.json>` (line comments with marker and level), `request-review` |
| `code-review/SKILL.md` (new role skill) | The reviewer: what to check, how to write a comment (level, the problem, why, a concrete fix or `suggestion` block), the review file format, when it runs (D3) |
| `dev-cycle-protocol/onboard.py`, `_onboard.py` | Open pull requests with pending items in the diagnosis (D8); a platform that cannot be reached is a finding |
| `dev-cycle-protocol/verify_gate.py` | Gate 8: commit messages since the last release pass the principle 9 guard |
| `dev-cycle-protocol/project_config.py` | `review` section: `provider` (auto), `host`, `marker`, `auto_review` |

Errors: a missing or unauthenticated CLI is reported with the login command (`gh auth login`, `glab auth login`); the diagnosis keeps working without the platform. Writes are refused when the text fails the guard.

Deliberately not abstracted: no web API clients of our own; the platform CLIs handle authentication and pagination.

## 3. Affected surfaces and files

- New: `dev-cycle-protocol/_pr_model.py`, `_pr_providers.py`, `_no_ai.py`, `pr_sync.py`, tests with recorded platform responses; new skill `code-review/`.
- Changed: `dev-cycle-protocol/SKILL.md` (Phase 7 as a review loop on the platform, the reviewer role), `onboard.py`, `_onboard.py`, `verify_gate.py`, `project_config.py`, `dev-cycle/SKILL.md` (the pull request option), `devops-release/SKILL.md` (review loop and CI follow-through on the platform).
- Changed: `README.md`, `docs/configuration.md`, a guide `docs/pull-requests.md`, `tests/MANUAL_TESTS.md`, `.gitignore` (allowlist), plugin manifests.

## 4. Tests (`quality-assurance`)

Automated (pytest, no network):
- Model: pending items from recorded GitHub and GitLab responses (thread waiting when the last comment is not the author's, changes requested, failed checks, conflicts, outdated threads).
- Triage hints: suggestion blocks, nits, questions, large asks.
- Providers: the exact `gh` / `glab` commands for each read and write; detection from remotes (github.com, gitlab.com, a self-hosted GitLab host); missing CLI and failed authentication messages.
- Guard: names and phrases are found case-insensitively; technical paths (`.claude/settings.json`, `.claude-plugin`) and ordinary words are not; `reply`, `comment`, `review` refuse a text that fails it.
- Review file: marker and level added to every comment; invalid levels and lines outside the diff are rejected.
- Diagnosis: an open pull request with pending items is an option right after a plan in progress; an unreachable platform is a finding.
- Gate 8: a commit message with an AI mention since the last tag blocks.

Manual (`tests/MANUAL_TESTS.md`):
- GitHub: the agent opens a pull request, its review appears with marked line comments; a person leaves a nit, a suggestion and a design question; the agent fixes and resolves the first two and brings the third to the user.
- GitLab: the same flow on a merge request.
- A new session starts with `/dev-cycle` and shows the pending pull request.

## 5. Documentation (`architecture-docs`, `user-docs`)

- `code-review/SKILL.md`; Phase 7 in `dev-cycle-protocol/SKILL.md`; `devops-release/SKILL.md`.
- README (the review loop in Usage, the new skill), `docs/pull-requests.md` (setup of `gh` / `glab`, what the agent does on each kind of thread, when it reads the pull request), `docs/configuration.md` (`review` section).

## 6. CI, release and observability (`devops-release`)

No workflow changes. Version: plugin `2.3.0` → `2.4.0`. `pr_sync.py` prints the platform, the pull request and the command it ran, so a wrong read can be traced.

## 7. Security, migration and rollback

- Tokens stay with the platform CLIs; nothing is stored by the skills. The agent posts only what the guard accepts and never approves or merges into the production branch.
- No migration: projects without a platform CLI keep the current Phase 7.
- Rollback: revert the pull request.

## 8. Execution steps and milestones

- **M1 Model and GitHub reads**: `_pr_model.py`, GitHub provider reads, `pr_sync.py status` and `open`, recorded fixtures, tests.
- **M2 Writes and the guard**: `_no_ai.py`, `reply`, `resolve`, `comment`, `review`, `request-review` on GitHub, gate 8 check, tests.
- **M3 GitLab**: GitLab provider reads and writes, detection, tests.
- **M4 Reviewer and the cycle**: `code-review` skill, Phase 7 review loop, the diagnosis option, `review` configuration, tests.
- **M5 Documentation and release**: README, guide, manual tests, plugin 2.4.0, pull request into `main`.
