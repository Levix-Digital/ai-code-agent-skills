---
id: 3
title: Pull requests through the platform CLIs on the REST API
status: accepted
date: '2026-10-25'
version: v2.8.1
subsystems: [pull-requests]
supersedes: []
superseded_by: []
amends: []
amended_by: []
---

# ADR-0003: Pull requests through the platform CLIs on the REST API

## Context

The pull request flow used `gh pr list`, `gh pr view` and `gh api graphql`, all on GitHub's GraphQL API. Some environments refuse GraphQL (HTTP 403 in Claude Code cloud sessions), so the diagnosis could not see pull requests and the merge gate failed there (BUG-001). The protocol must behave the same on a local machine, GitHub Enterprise and any cloud environment, without code for one vendor.

## Decision

GitHub is read and written through `gh api` on the REST API for everything REST covers: pull request, reviews, comments, check runs and statuses, replies, line reviews, review requests. GraphQL is tried only for review thread resolution, which REST does not expose. When it is refused, threads are rebuilt from REST review comments with their resolution unknown, `resolve` explains where to resolve the thread, and the merge gate keeps a blocking finding that cannot be read as resolved holding the merge. GitLab uses `glab api` on REST only.

## Alternatives Considered

- **Adapters per environment** (the Claude Code cloud review-thread route, for example): more coverage in one environment, but code that serves a single vendor.
- **Keep GraphQL and fail where it is refused**: the merge gate would not run where the user works.

## Consequences

- One code path for every environment; tests replay recorded REST responses.
- Where GraphQL is refused, resolving a thread is a manual step on the platform; the gate never treats an unknown thread as resolved.
