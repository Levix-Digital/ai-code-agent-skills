---
name: dev-cycle
description: The /dev-cycle command - the single entry point of the development cycle. It diagnoses the repository (configuration, branching model and branches, product baseline, plans in progress, release candidates, blocking bugs, roadmap, Spec Kit), proposes where to start in the recommended order, sets up what is missing after the user confirms, and hands over to product inception (a new product from an idea) or to the cycle (a request in an existing project). Use when the user types /dev-cycle, starts working on a project, gives a product idea, or asks where to start or how the project stands.
argument-hint: ["<product idea or request>"]
---

# 🧭 Development Cycle Command (`/dev-cycle`)

> **Paths**: `<skills_root>` is the folder that contains this skill (the parent of this `SKILL.md`'s folder). Use the real absolute path when running the commands.

The one command the user needs: the protocol guides the user, not the other way around. It diagnoses first, asks only when there is a real decision, and then follows [`dev-cycle-protocol`](../dev-cycle-protocol/SKILL.md). Talk to the user in their language.

## Arguments

`$ARGUMENTS`: optional. In a new repository it is the **product idea**; in an existing one it is a **request** (a feature, a fix, a question about the project).

## Steps

1. **Diagnose** (it only reads):
   ```powershell
   python "<skills_root>/dev-cycle-protocol/onboard.py" diagnose --json [--request "<the text>"] [--agent claude-code]
   ```
   Pass `--agent claude-code` when the command runs in Claude Code, so the diagnosis can check its attribution settings.
   The result has `mode` (`new` or `existing`), `summary`, `ask`, `options` (in the recommended order) and `recommended`.
   When the session started outside a repository, the diagnosis uses the only repository below it and says so in the summary: work from that repository from then on. With more than one, `mode` is `choose` and the options are the repositories: ask which one is the project and run the diagnosis again with `--dir <it>`.
2. **Nothing to decide** (`ask` is false): tell the user the summary in one line and go on:
   - `new`: [`product-inception`](../product-inception/SKILL.md) Phase 0 with the idea;
   - `existing`: `dev-cycle-protocol` Phase 1 with the request (full or short path as usual).
3. **Something to decide** (`ask` is true): show the summary and the options as a numbered list, mark the recommended one, and wait for the user's choice. For a list of initial setup items, the user may answer "all" or pick items; do them in the listed order. Never start the request while a plan in progress, a pending release candidate, an open blocking bug or a bug not triaged is listed above it, unless the user chooses so.
4. **Do what was chosen** (table below), then run the diagnosis again and continue with what comes next, until the user reaches the work they came for.

| Option (`key`) | What to do |
|---|---|
| `plan:<id>` | Resume the plan: `get_plan --id <id>` on the engine (`"<skills_root>/dev-cycle-protocol/bin/dev-cycle" plan-memory get_plan --id <id>`), `check_milestone.py --list`, continue from the next milestone (Phase 4 onwards). |
| `pr:<number>` | The pull request on its platform: `pr_sync.py status --pr <number>`; then `dev-cycle-protocol` Phase 7: fix and resolve the small threads, bring the large ones to the user, answer questions, fix CI and conflicts. |
| `rc:<run>` | Release candidate: `"<skills_root>/dev-cycle-protocol/bin/dev-cycle" quality-assurance manual_tests summary --run <run>` and `failed --run <run>`; failures go back to Phase 4; when all pass, gate rc and then gate 8 (Phases 9 and 10). |
| `bugs` | Phase 1 with the blocking bugs: show them, the user decides which to promote (`backlog.py promote`). |
| `triage` | Show the open bugs not triaged and why; with the user, give each a severity (`backlog.py set <ID> --severity <s>`) and schedule the ones that do not block in the next release at the latest (`roadmap assign <ID> --release <vX.Y.Z> [--move]` on the engine), or promote them. |
| `git` | Ask, then `git init` with the production branch (`main`). |
| `config` | `onboard.py propose-config`, show the proposal, let the user change it, write only what they confirm: save it to a file and `onboard.py write-config --file <it>`. The proposal carries the detected branching model (`devops.branching`): present the choice as in `branching` below before writing. The architecture style is chosen later with `software-design` (or in Phase 0). |
| `gitignore` | The project's `.gitignore` keeps files of the protocol out of Git (an allowlist `.gitignore`, a `docs/` entry), so the decisions in them would be lost in the next clone. Show the ignored paths and the proposed entries; after the user confirms, add the entries to `.gitignore` and commit it with the configuration. If the user prefers another layout, change the paths in `.dev-cycle.json` instead. |
| `branching` | Asked once. Show the three models in a short table (`devops-release`, "Branching Model": work base, candidate branch, release gate, what each fits) and recommend the detected one. Record the answer: `write-config` with `{"devops": {"branching": "<model>"}}` (with `integration_branch` for `integration` when it is not `dev`). For `integration` without its branch, the next item creates it; for `trunk` and `release-branches`, offer to add `templates/promote.yml` and the `release` environment when the project publishes releases. |
| `attribution` | Ask, then `onboard.py set-attribution`: it turns off the automatic attribution in `.claude/settings.json` (commit trailer, pull request footer, session link); commit the file. Remove any attribution that still appears in a pull request description. |
| `branch` | `integration` model only. Ask, then `onboard.py create-branch`; offer to push it (`git push -u origin <branch>`). |
| `speckit-ask` | Explain in two sentences what Spec Kit does in the cycle (the detail of features: specification, clarification, plan, tasks, implementation; at a version pinned per project; needs uv) and recommend it for products that will grow, not for small tools. Yes: `speckit_setup.py init`, then `constitution.py generate` (Phase 0 does it when the baseline is not there yet). No: record the decision so it is not asked again: `write-config` with `{"speckit": {"enabled": false}}`. |
| `speckit-adopt` | `speckit_setup.py adopt`. If `constitution.py check` says the constitution is hand-written: show each of its principles, propose where it goes (constraints and non-functional requirements to `TECH_SPEC.md`, decisions to ADRs, general design rules as a suggestion for `software-design`), apply what the user confirms, then `constitution.py generate --replace-hand-written` (a copy is kept). |
| `speckit-fix` | Fix each reported problem: version drift (`speckit_setup.py upgrade --to <version>` after the user agrees), preset or branch prefix (`adopt` again or `upgrade`), stale constitution (`constitution.py generate`). |
| `inception` | `product-inception` Phase 0 with the idea (ask for it when there is none). |
| `adoption` | `product-inception` in adoption mode. |
| `release` | Phase 1 from the open items of the current release (`roadmap current` on the engine). |
| `request` | Phase 1 with the request. |

## Rules

- The diagnosis never changes anything. Every write (configuration, branch, Spec Kit, constitution) happens after the user's confirmation, and the configuration is only added to, never overwritten.
- Spec Kit and the branching model are asked about at most once per project: the answers are recorded in `.dev-cycle.json`.
- Commits made while setting up follow `devops-release` (Conventional Commits, authored by the repository's Git identity); the configuration is committed as `chore: dev-cycle configuration`.
- When `onboard.py` reports findings (a file it could not read), show them; they are not blocking unless the user's choice needs that file.
