---
name: code-review
description: The reviewer role of the development cycle - reviews every pull request (merge request on GitLab) on its platform with a fixed checklist (the plan's acceptance criteria, design, tests, documentation, security, data and release safety), writes each finding as a line comment with a level (blocking, suggestion, nit), a reason and a concrete fix, posts the review through pr_sync.py with the [review] marker, and never approves. Use when a pull request is opened or receives new pushes, when the user asks for a review, or when reviewing a diff before it is pushed.
---

# 🔍 Code Review Skill (`code-review`)

> **Paths**: `<skills_root>` is the folder that contains this skill (the parent of this `SKILL.md`'s folder). Use the real absolute path when running the commands.

The reviewer of [`dev-cycle-protocol`](../dev-cycle-protocol/SKILL.md) Phase 7. The review lives **on the platform**, as a review of the pull request with line comments, so the team's history stays where a team looks, and people review alongside it. It is published with the user's account, starts every comment with the protocol's marker (`review.marker`, default `[review]`) and never approves: approval is a person's decision.

## When It Runs

- **Every pull request**, as soon as it is open (`review.auto_review`, default on).
- **Again after pushes that change code**, on the new commits only, without repeating findings already made.
- When the user asks for a review, or on a diff before it is pushed.

Review with fresh eyes: read the plan and the diff as a reviewer who did not write the code. When the agent can start a second session, run the review there.

## Checklist

| Area | What to check |
|---|---|
| The plan | Every acceptance criterion and milestone of the plan (or its Spec Kit `tasks.md`) is done; nothing outside the plan slipped in |
| Correctness | Edge cases, empty and large inputs, error paths, concurrency, off-by-one, time zones; the change does what the request asked |
| Design | The `software-design` review checklist and `check_design`; the project's architecture style; no abstraction without its reason |
| Tests | New logic has automated tests; a regression test per bug that fails without the fix; tests assert behavior, not implementation; manual tests curated (`quality-assurance`) |
| Documentation | `docs/architecture` pages and ADRs for structural changes, README for user-visible ones, written for today (`architecture-docs`, `user-docs`) |
| Security and data | No secrets, input validated at the boundary, permissions checked, migrations additive or backward compatible, a rollback path |
| Release | Version impact, CI changes, logs and diagnostics for new failure modes (`devops-release`) |
| Principle 9 | No AI attribution in commits, the description or comments |

## Writing a Finding

One line comment per finding, on the line it is about:

- **Level**: `blocking` (must change before merge: a bug, a missing test for new logic, a security or data risk, a broken rule of the protocol), `suggestion` (better, worth doing in this pull request), `nit` (cosmetic or naming; never blocks).
- **The problem, why it matters, and a concrete fix**; prefer a `suggestion` block when the fix is a few lines:
  ````markdown
  ```suggestion
  if not album.photos:
      return []
  ```
  ````
- Specific and short; no praise padding, no restating the code; one issue per comment.
- Never mention an AI, a model or an agent (protocol principle 9); `pr_sync.py` refuses such text.

The summary says what was reviewed (commits, plan), the count per level, and the one or two things that matter most.

## Posting

Write the review to a file and post it:
```json
{
  "body": "Reviewed 4 commits against the plan. 1 blocking, 2 suggestions, 1 nit. Main point: the empty album case.",
  "comments": [
    {"path": "app/share.py", "line": 42, "level": "blocking", "body": "An empty album raises IndexError here; return [] and add a regression test."}
  ]
}
```
```powershell
python "<skills_root>/dev-cycle-protocol/pr_sync.py" review --file review.json
```
The script adds the marker and the level to every comment and checks principle 9. With no finding, post only the summary.

## After the Review

The author (the agent, or the user) addresses the findings like any other thread (see `dev-cycle-protocol`, Phase 7): small ones fixed and resolved, large ones taken to the user. A finding the author disagrees with gets a reply with the reason; the user decides.
