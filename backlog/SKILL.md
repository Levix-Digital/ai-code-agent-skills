---
name: backlog
description: The /backlog command - capture a feature, improvement, adjustment, bug or epic in the project backlog (docs/BACKLOG.md) in one line, without discovery or planning, or show the backlog summary. Use when the user types /backlog or asks to note, save or add something to the backlog for later.
argument-hint: --feature|--improvement|--adjustment|--bug|--epic "<text>" [--priority high|medium|low] [--severity critical|high|medium|low] [--in EPIC-NNN]
---

# 📥 Backlog Command (`/backlog`)

> **Paths**: `<skills_root>` is the folder that contains this skill (the parent of this `SKILL.md`'s folder). Use the real absolute path when running the commands.

Captures one item in the project backlog, the first level of planning owned by [`plan-memory`](../plan-memory/SKILL.md). Capture only: no discovery, no plan, no code. Promoting, re-prioritizing or dropping items is the user's decision, taken later through [`dev-cycle-protocol`](../dev-cycle-protocol/SKILL.md).

## Arguments

`$ARGUMENTS`

| Form | Effect |
|---|---|
| `--feature "<text>"` | New capability |
| `--improvement "<text>"` | Better behavior of something that exists |
| `--adjustment "<text>"` | Small change: wording, defaults, configuration, cleanup |
| `--bug "<text>"` | Something that does not work as it should |
| `--epic "<text>"` | A capability area of the product that groups items |
| `--in EPIC-NNN` | The epic the item belongs to |
| `--priority high\|medium\|low` | Priority of a feature, improvement or adjustment (default `medium`) |
| `--severity critical\|high\|medium\|low` | Severity of a bug |
| *(no type)* | Show the backlog summary |

Without arguments, or with text but no type, run the summary (or ask which type, in one short question, when there is text).

## Steps

1. Work in the repository the user is working on (the current project).
2. From the user's text, write:
   - a **title**: short, imperative or descriptive, at most about ten words;
   - a **description**: the user's idea, faithfully, in **English** (translate when the user wrote in another language), without adding scope, solutions or details the user did not give.
3. With `--in`, pass the epic as `--epic`; without it, add the item to no epic (do not guess one). An epic itself never has `--in`.
4. For `--bug` without `--severity`, estimate it: `critical` (data loss, security, nothing works), `high` (a main feature fails, no workaround), `medium` (a feature fails with a workaround), `low` (cosmetic, rare). Say that it was estimated. `critical`, `high` and `medium` bugs come before new features, improvements and adjustments; a `low` bug goes into the next release at the latest.
5. Run:
   ```bash
   "<skills_root>/dev-cycle-protocol/bin/dev-cycle" plan-memory backlog add --type <feature|improvement|adjustment|bug|epic> --title "<title>" --description "<description>" [--priority <p>] [--severity <s>] [--epic <EPIC-NNN>] [--found-in "<version, pull request or context, when known>"]
   ```
   Summary: `"<skills_root>/dev-cycle-protocol/bin/dev-cycle" plan-memory backlog summary`. In PowerShell, run `& "<skills_root>/dev-cycle-protocol/bin/dev-cycle.ps1" plan-memory backlog [arguments]` instead.
6. Answer with one line, in the user's language: the id, the title and the priority or severity, for example `FEAT-003: Export answers to Markdown (medium)`, and for an estimated severity add `severity estimated; change it with backlog set BUG-004 --severity <s>`. For a bug that does not block, run the `roadmap assign` that `add` prints (on the engine: `"<skills_root>/dev-cycle-protocol/bin/dev-cycle" plan-memory roadmap assign ...`) when the project has a roadmap (no bug stays untriaged), and say which release it went into.
7. Do not commit unless the user asks; the backlog file is committed with the next change to the project, or on its own as `docs(backlog): <id> <title>`.
