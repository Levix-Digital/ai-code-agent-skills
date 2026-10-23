# Backlog

Work captured for later planning: bugs, features, improvements and adjustments, grouped under epics when the product is planned as a whole. Capturing an item takes no discovery; when the user decides to work on items, they are promoted into a plan (`plan-memory/backlog.py promote`), where the discovery and the planning happen. Open bugs come before new work.

Each item: `### <ID>: <title>` with the fields `Severity` (bugs: critical, high, medium, low) or `Priority` (others: high, medium, low), an optional `Epic`, `Status` (open, planned, in_progress), `Found`, `Plan` and `Description`. Epics (`EPIC-NNN`) group items of the complete product; they have no `Plan` and are never promoted: their items are. The roadmap (`docs/ROADMAP.md`) says which items go into which release. Closed items (done, dropped, duplicate, superseded by better defined items) move to `Closed` with a `Closed` note and keep their ids, which are never reused. Edit this file with `plan-memory/backlog.py` or by hand, and check it with `backlog.py validate`.

## Bugs

## Features

### FEAT-001: Pull request review on Bitbucket, Azure DevOps and Gitea
- **Priority**: medium
- **Status**: open
- **Found**: 2026-10-23, pull request review plan discussion
- **Plan**: -
- **Description**: The pull request review flow starts with GitHub and GitLab. Analyze and add adapters for Bitbucket, Azure DevOps and Gitea (reading threads, comments, suggestions, reviews and CI; replying, resolving and posting reviews).

## Improvements

## Adjustments

## Epics

## Closed

### IMP-001: Triage section in the bug plan template
- **Priority**: high
- **Status**: done
- **Found**: 2026-10-23, PR #5 discussion
- **Plan**: -
- **Closed**: 2026-10-23, Triage section in the bug plan template
- **Description**: Borrow the assess step of Spec Kit's bug extension: before the fix is planned, a bug plan first judges whether the report is really a bug (reproduces, expected behavior, not a misuse or a feature request) and locates the suspected code paths. Add a 'Triage' section to the bug plan template (plan-memory/templates/BUG_PLAN_TEMPLATE.md) and the matching rule in quality-assurance.
