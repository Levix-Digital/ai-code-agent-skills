---
name: gedo-feedback
description: Records what goes wrong with Gedo (the Levix collaboration overlay on Git) during real work with parallel threads or developers - missed or false collisions, merges that fall back to text, contract breaks that slip through, slowness, crashes, setup problems and workarounds - as entries in the project and, with the user's confirmation, as issues in the Gedo repository. Off until the project uses Gedo (gedo.enabled in .dev-cycle.json). Use when a Gedo command fails or misbehaves, when work had to go around Gedo, and at the review and release phases of a project that uses Gedo.
---

# 🧪 Gedo Feedback (`gedo-feedback`)

> **Paths**: `<skills_root>` is the folder that contains this skill (the parent of this `SKILL.md`'s folder). Use the real absolute path when running the commands.

[Gedo](https://github.com/Levix-Digital/gedo) is Levix's collaboration overlay on Git: semantic merges, collisions found while they are born, intents, an inbox per symbol and stacked pull requests. The development cycle uses it to run parallel threads (or developers) safely, and it is still maturing. This skill turns every problem Gedo shows in real work into input for Gedo's own backlog. It is **temporary**: when Gedo is mature, turn it off (`gedo.feedback: false`) or remove it.

## When It Is On

Recording needs both keys of the `gedo` section of `.dev-cycle.json` (`dev-cycle-protocol/project_config.py`):

| Key | Meaning | Default |
|---|---|---|
| `gedo.enabled` | The project uses Gedo | `false` |
| `gedo.feedback` | Record Gedo feedback while Gedo is used | `true` |
| `gedo.feedback_dir` | Folder of the entries | `docs/gedo-feedback` |
| `gedo.repo` | Repository that receives the issues | `Levix-Digital/gedo` |

While it is off, `add` and `issue` refuse with exit code 2 and say why; `status`, `list`, `show` and `validate` always work. Never turn it on by yourself: `gedo.enabled` is the user's decision.

## What to Record

Anything negative, not only errors. One entry per distinct problem:

| Kind | When |
|---|---|
| `missed-collision` | Two threads or developers changed the same entity and `gedo radar` (or intents) said nothing, or said it too late |
| `false-collision` | A collision reported where the changes were independent |
| `merge-fallback` | The merge driver fell back to Git's textual merge (say why, when Gedo says it) |
| `wrong-merge` | A merge without conflict markers that is wrong |
| `contract-miss` | A changed contract broke a caller and the post-merge check did not report it |
| `identity` | A node identity lost, split or merged wrongly (`gedo trace` shows a broken history) |
| `performance` | An operation slower than Gedo's requirements (publish, merge driver, hooks, shadow merge) |
| `crash` | A Gedo command, hook or the daemon failed |
| `data-loss` | Work or Gedo data lost or corrupted |
| `setup` | Installation, `gedo init`, hooks, worktrees or ephemeral environments |
| `interface` | Unclear output, missing `--json`, prompts in non-interactive use, wrong exit codes |
| `workaround` | Something done around Gedo because it could not do it |
| `other` | Anything else |

Severity: `critical` (data loss, corruption, Git blocked), `high` (a main feature fails, no workaround), `medium` (fails with a workaround), `low` (cosmetic, rare).

## Steps

1. **Record it when it happens**, before working around it, so nothing is lost:
   ```powershell
   python "<skills_root>/gedo-feedback/gedo_feedback.py" add --kind <kind> --title "<short title>" --severity <s> `
     --scenario "<what was being done and how the work was split between threads>" `
     --expected "<what Gedo should have done>" --actual "<what it did>" `
     [--reproduction "<minimal steps>"] [--workaround "<what was done instead>"] [--command "<gedo or git command>"] `
     [--gedo-version <v>] [--found-in "<branch, pull request>"] [--related "<Gedo proposal or backlog id>"] `
     [--diagnostics-file <log excerpt>] [--doctor]
   ```
   `--doctor` attaches `gedo doctor --json`; `--diagnostics-file` attaches the last 200 lines of a log (Gedo's own logs are in `.git/gedo/logs/`). An open entry with the same kind and title is refused: add to it instead (edit its file), unless it is really a different case (`--allow-duplicate`).
2. **Write in English**, faithfully, without guessing causes inside Gedo. `--related` points to Gedo's backlog items (`FEAT-074`) or to the proposals Levix sent to Gedo (`G1` to `G12`) when one clearly applies.
3. **Keep proprietary code out**: the reproduction is minimal and synthetic; never paste the project's code, secrets, customer data or internal URLs. The issue never carries the `Found` field (branch or pull request of this project).
4. **Parallel threads**: each thread records its own entries (one file per entry, so threads never conflict); the orchestrator thread reviews them before the pull request phase and merges duplicates.
5. **Commit** the entries with the change they were found in, or on their own as `docs(gedo-feedback): <title>`.

## Reporting to Gedo

Issues leave the project, so **ask the user first** (once, for the list of entries to report):

```powershell
python "<skills_root>/gedo-feedback/gedo_feedback.py" list
python "<skills_root>/gedo-feedback/gedo_feedback.py" issue <ID> --body-file <tmp>/issue.md
gh issue create --repo Levix-Digital/gedo --title "<title it printed>" --body-file <tmp>/issue.md
python "<skills_root>/gedo-feedback/gedo_feedback.py" reported <ID> --issue <issue URL>
```

`issue` refuses text that attributes work to an AI (protocol principle 9). Without `gh`, create the issue with the platform's API or ask the user to. When Gedo fixes it, or it turns out not to be a Gedo problem: `close <ID> --reason "<fixed in vX.Y.Z | not a Gedo problem: ...>"`.

## In the Cycle

- **Any phase**: a Gedo command fails or misbehaves → record it (step 1), then go on.
- **Phase 7 (pull request)**: list the open entries the change produced and link them in the pull request description.
- **Phase 10 (release)**: show the user the open entries and ask which to report as issues.
- `validate` checks every entry; run it before committing entries.

## Commands

| Command | What it does |
|---|---|
| `status [--json]` | Whether recording is on, and the entry counts |
| `add ...` | Records a new entry (on only) |
| `list [--status S] [--kind K] [--all] [--json]` | Open and reported entries by default |
| `show <ID> [--json]` | One entry |
| `issue <ID> [--body-file F] [--json]` | Title and body of the Gedo issue for an open entry (on only) |
| `reported <ID> --issue <URL>` | Marks the entry reported |
| `close <ID> --reason R` | Closes the entry |
| `validate` | Checks every entry; exit code 1 on problems |

All commands accept `--dir <project>`.
