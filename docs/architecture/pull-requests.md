---
title: Pull Request Sync
summary: Reads and answers pull requests on GitHub and GitLab through one model, and holds the merge while something blocks it.
code: [dev-cycle-protocol/pr_sync.py, dev-cycle-protocol/_pr_model.py, dev-cycle-protocol/_pr_providers.py, code-review]
---

# Pull Request Sync

Reads and answers pull requests on GitHub and GitLab through one model, and holds the merge while something blocks it.

## Responsibilities

Owns Phase 7 on the platform: reading the pull request (merge request on GitLab) with its threads, comments, reviews and checks; saying what is pending; posting replies, resolutions, comments and the protocol's review; and the merge gate. The review's content belongs to `code-review`.

## Structure

- `dev-cycle-protocol/_pr_model.py`: the platform-independent model, pending items, triage hints, merge blockers, post format with markers.
- `dev-cycle-protocol/_pr_providers.py`: `GitHubProvider` (`gh api`, REST; GraphQL only for thread resolution) and `GitLabProvider` (`glab api`), behind an injectable command runner.
- `dev-cycle-protocol/pr_sync.py`: the commands `status`, `open`, `gate-merge`, `reply`, `resolve`, `comment`, `review`, `request-review`.

## Behavior

Every posted text carries the `[review]` or `[reply]` marker and passes the attribution guard. A `[review]` `blocking` thread holds the merge until it is resolved; a reply alone does not release it. When a platform cannot say whether a thread is resolved, the thread counts as open and the gate says why. The gate also warns when the base branch does not require resolved conversations.

## Interfaces

- Called by the cycle (diagnosis, Phase 7, merge gate) and by `code-review`.
- Calls `gh` and `glab` with the user's authentication.

## Decisions

- [ADR-0003](../adr/0003-pull-requests-through-the-platform-clis-on-the-rest-api.md): REST first, GraphQL only for thread resolution
