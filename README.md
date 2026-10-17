# AI Code Agent Skills

Development skills in the open `SKILL.md` format, shared by every coding agent on the machine (Claude Code, Codex, Gemini, Copilot and others) and by Claude Code cloud sessions through a plugin.

## What It Does

| Skill | Role | What it does |
|---|---|---|
| [`dev-cycle-protocol`](#dev-cycle-protocol) | Orchestrator | Development cycle from discovery to a verified release: phases, the role called in each one, full and short paths, the user's gates, the shared `.dev-cycle.json` |
| [`plan-memory`](#plan-memory) | Planner | The project backlog (`docs/BACKLOG.md`) and complete plans as versioned files (`docs/plans`), shared by the team, with promotion of backlog items, milestones, lineage, search and drift detection |
| [`backlog`](#backlog) | Command | `/backlog --feature\|--improvement\|--adjustment\|--bug "<text>"`: capture an idea or bug for later in one line |
| [`software-design`](#software-design) | Architect | Design principles, the project's architecture style with a guide per style, design input for plans, review checklist and core dependency check |
| [`architecture-docs`](#architecture-docs) | Technical documentation | Keeps technical docs current: short index, one living page per subsystem, immutable ADRs, validation and migration of legacy docs |
| [`quality-assurance`](#quality-assurance) | QA | Automated test strategy and one curated manual test suite with Pass/Fail/Blocked runs per release candidate |
| [`devops-release`](#devops-release) | DevOps | Branching, pull request CI, versions, release candidates, the user's merge as release gate, observability |
| [`user-docs`](#user-docs) | Technical writer | End-user READMEs in English with a standard structure, template and checker |
| [`session-memory`](#session-memory) | Session memory | Recovers and consolidates each developer's own session context per project (local ChromaDB) |
| [`folder-sync`](#folder-sync) | Utility | Incremental RAG over any folder: `/sync` to index, `/folder` to ask |

---

## Contents

- [How the repository works](#how-the-repository-works)
- [Setup](#setup)
- [Usage](#usage)
- [Claude Code cloud sessions (plugin)](#claude-code-cloud-sessions-plugin)
- [Skills](#skills)
- [Project configuration: `.dev-cycle.json`](#project-configuration-dev-cyclejson)
- [Maintaining the repository](#maintaining-the-repository)
- [Troubleshooting](#troubleshooting)
- [License](#license)

---

## How the repository works

```
~/.ai-code-agent-skills/                 this repository (source of truth)
├── .claude-plugin/                      Claude Code marketplace + plugin manifests
├── dev-cycle-protocol/  plan-memory/  session-memory/
├── software-design/  architecture-docs/  quality-assurance/
├── devops-release/  user-docs/  folder-sync/
├── docs/plans/                          plans of this repository
├── tests/MANUAL_TESTS.md                manual tests of this repository
├── install.ps1 / install.sh             link the skills into each agent
└── requirements.txt                     Python dependencies of the scripts

~/.claude/skills/<skill>        ─┐
~/.agents/skills/<skill>         ├─ links (junctions on Windows) to the folders above
~/.gemini/config/skills/<skill> ─┘

~/.ai-code-agent-memory/                 local data, never versioned
├── plan-memory/db                       search index of the plans (disposable)
└── session-memory/db                    this developer's session memory

<project>/docs/plans/*.md                plans: versioned and shared in each project
<project>/TECDOC.md, docs/architecture/, docs/adr/   technical docs (architecture-docs)
```

- **One copy of each skill.** Agents read the files through links, so an edit in this repository is seen by every agent at once.
- **No data in this repository.** It can be versioned, cloned and shipped as a plugin without carrying data.
- **Shared vs. personal memory.** Plans belong to the project and are shared: they live as files in each project repository and go through review like code. Session memory belongs to each developer and stays in their local `~/.ai-code-agent-memory`.
- **Allowlist `.gitignore`.** Only the files listed in [`.gitignore`](.gitignore) are versioned; databases, virtual environments and `.env` files never are.
- **Paths in `SKILL.md`.** Commands are written as `<skills_root>/<skill>/script.py`, where `<skills_root>` is the folder that holds the skill. The agent replaces it with the real path.

---

## Requirements

- Git and the GitHub CLI (`gh`) for the release helpers
- Python 3.10 or later with `pip`
- A coding agent that reads `SKILL.md` skills (Claude Code, Codex, Gemini, Copilot or similar)

## Setup

```bash
git clone https://github.com/Levix-Digital/ai-code-agent-skills.git ~/.ai-code-agent-skills
pip install -r ~/.ai-code-agent-skills/requirements.txt
```

Link the skills into the agents.

Windows:
```powershell
& "$HOME\.ai-code-agent-skills\install.ps1"
```

macOS / Linux:
```bash
~/.ai-code-agent-skills/install.sh
```

Default targets:

| Agent | Folder |
|---|---|
| Claude Code | `~/.claude/skills` |
| Codex and other agents that follow the cross-agent standard | `~/.agents/skills` |
| Gemini | `~/.gemini/config/skills` |

Extra agents, another source folder, skipping a skill and how existing entries are handled: [docs/installation.md](docs/installation.md).

Restart the agent (or open a new session) after installing so it picks up the skills.

---

## Usage

Agents load a skill on their own when the task matches its `description`; you can also ask for one by name ("use plan-memory to save this plan"). For project work, start with the orchestrator: it runs the cycle and calls the other roles in each phase.

1. In a project, describe what you want ("add an export command"). The agent stays in **Discovery** until you agree on an approach.
2. It writes the **plan** with a section from each role and saves it in `docs/plans`. Reply "Approved" to start, or ask for changes.
3. It implements on a branch, runs the tests, updates the docs and opens a **pull request into `dev`**, following CI until it is green.
4. When you want to release, it cuts a **release candidate** (`vX.Y.Z-rc.N`) and prepares the manual tests to run. Install the candidate and mark each test Pass, Fail or Blocked in `tests/MANUAL_TESTS.md`; failures come back as fixes.
5. When the candidate passes, it opens the pull request from `dev` into `main`. **Your merge releases it**; the agent follows the release until every asset is published.

Small fixes and documentation changes take the short path: no plan file and no release candidate.

Ideas and bugs you do not want to work on yet go to the **backlog**: type `/backlog --feature "<your idea>"` (or `--improvement`, `--adjustment`, `--bug`) and the agent records it in `docs/BACKLOG.md` with an id. Bugs the agent finds outside the current change land there too. At the start of the next cycle the agent shows the backlog; you choose what to plan, and open critical or high bugs come before new features.

---

## Claude Code cloud sessions (plugin)

Cloud sessions do not see your local folders. For them, this repository is also a Claude Code marketplace (`levix-agent-skills`) with the `dev-skills` plugin, which ships the development skills. Setup and caveats: [docs/cloud-sessions.md](docs/cloud-sessions.md).

---

## Skills

### dev-cycle-protocol

The orchestrator. It owns the phases, the gates and which role runs when; each role skill owns its artifact, rules and checker.

| Phase | Roles |
|---|---|
| 1. Discovery | orchestrator, plan-memory, software-design |
| 2. Plan | plan-memory with a section from every role |
| 3. Plan approval (user) | orchestrator |
| 4. Implementation on a branch | software-design |
| 5. Automated tests | quality-assurance |
| 6. Documentation | architecture-docs, user-docs, quality-assurance |
| 7. Review and pull request into `dev`, CI green | code review, software-design, devops-release |
| 8. Release candidate | devops-release, quality-assurance |
| 9. Acceptance (user runs the manual tests) | quality-assurance |
| 10. Release gate (user merges `dev` into `main`) and verified release | devops-release, plan-memory, session-memory |

Phases 1 to 7 run for every change, phases 8 to 10 for every release. Small fixes and documentation-only changes take a short path (phases 1, 4 to 7).

Scripts:
```bash
python <skills_root>/dev-cycle-protocol/inspect_project.py [--dir .] [--json]
python <skills_root>/dev-cycle-protocol/project_config.py [--json] [--validate] [--migrate [--write]]
python <skills_root>/dev-cycle-protocol/verify_gate.py --gate 3|rc|8 [--run vX.Y.Z-rc.N] [--dir .]
```
- `inspect_project.py` detects languages, manifests, test runners, documentation files and Git state, and shows the configured architecture style, branches and releases repository.
- `project_config.py` shows, validates or migrates `.dev-cycle.json`.
- `verify_gate.py --gate 3` (before plan approval): working tree, active plan, bugs first (a plan that is not a bug plan is blocked while an open critical or high bug is outside it, unless you accept the risk: `--accept-open-bugs "<reason>"`), no secrets tracked or staged.
- `verify_gate.py --gate rc --run <tag>` (candidate acceptance): every manual test of the run is Pass.
- `verify_gate.py --gate 8` (before the release pull request): documentation and manual test files exist and the checkers of every role pass (`check_docs.py`, `check_readme.py`, `manual_tests.py validate`, `check_design.py`, `check_ci_policy.py`, `release_status.py version`, `backlog.py validate`), plus the candidate run with `--run`, no secrets and a clean tree; open blocking bugs are listed for the release notes.

Full protocol: [dev-cycle-protocol/SKILL.md](dev-cycle-protocol/SKILL.md).

### plan-memory

Keeps the project's planning in its repository, at two levels: the **backlog**, where work is captured for later, and **plans**, complete and uncompressed, shared by everyone on the project:

- **Backlog**: `docs/BACKLOG.md` (`backlog.file`), sections Bugs (`BUG-NNN`, with severity), Features (`FEAT-NNN`), Improvements (`IMP-NNN`) and Adjustments (`ADJ-NNN`, with priority), and Closed. `backlog.py promote <ID>` turns items into a draft plan (a bug plan for bugs, with reproduction, root cause and regression test); the items follow the plan and close when it is completed.
- **Plan files**: `docs/plans/<plan_id>.md` in the project repository, with YAML frontmatter (status, version, author, milestones, lineage, status log) and the plan body. They are committed and reviewed like code. Change the folder with `plans.dir` in `.dev-cycle.json`.
- **Local index**: a ChromaDB index for semantic search and drift detection, synced from the files on every command (including teammates' changes after `git pull`). It is disposable: `rebuild_index.py` recreates it.

| Task | Command |
|---|---|
| Save or update a plan | `save_plan.py --title "<t>" --file plan.md [--version v1.2.0] [--milestones "A,B,C"] [--supersedes <old_id>] [--kind bug] [--backlog-items "BUG-001"]` |
| List plans | `list_plans.py [--status active] [--kind bug]` |
| Read a plan in full | `get_plan.py --id <plan_id>` or `get_plan.py` (latest active) |
| Semantic search | `search_plans.py --query "<text>" [--limit 5] [--all-projects]` |
| Milestones | `check_milestone.py --list` / `--milestone "A" [--notes "..."]` / `--milestone "A" --uncheck` |
| Change status | `update_plan_status.py --id <plan_id> --status completed [--notes "..."]` |
| Check a proposal for drift | `detect_drift.py --proposal "<text>"` or `--file proposal.md [--strict]` |
| Export (JSON or copy) | `export_plan.py --id <plan_id> --format json --output-dir out/` |
| Rebuild the local index | `rebuild_index.py` |
| Migrate from the old database | `migrate_legacy.py [--source <old_db>] [--dry-run]` |

All scripts are in `<skills_root>/plan-memory/`. They work on the Git repository of the current directory; use `--dir` for another one and `--project` to override the project name.

- **Statuses**: `draft`, `planned`, `active`, `in_progress`, `completed`, `superseded`. Saving with `--supersedes` marks the old plan as `superseded` and links both.
- **Milestones** record who completed them and when; the first one moves the plan to `in_progress`, the last to `completed`.
- **The scripts never commit.** Commit the plan after approval (Phase 3) and its milestone/status changes with the work that completes them.
- **Drift detection** combines similarity to the active plan, milestone coverage and order, and the project's own `drift_rules` (see [`.dev-cycle.json`](#project-configuration-dev-cyclejson)).
- **Index location**: `~/.ai-code-agent-memory/plan-memory/db` (override with `PLAN_MEMORY_DB`).

Details: [plan-memory/SKILL.md](plan-memory/SKILL.md).

### backlog

The `/backlog` command captures without leaving the conversation: `/backlog --feature "I would like answers exportable to Markdown"`, `/backlog --bug "sync never finishes on network drives" --severity high`, or `/backlog` alone for the summary. The agent writes a short title and a faithful English description, runs `plan-memory/backlog.py add` and answers with one line (`FEAT-003: Export answers to Markdown (medium)`). Priority defaults to `medium`; a bug without `--severity` gets an estimated one, stated in the answer. In agents without slash commands, ask "add to the backlog as a feature: ...". Details: [backlog/SKILL.md](backlog/SKILL.md); backlog commands (`add`, `summary`, `promote`, `close`, ...): [plan-memory/SKILL.md](plan-memory/SKILL.md#-backlog-backlogpy).

### session-memory

Lets a new session pick up where the last one stopped. Per project, it keeps:
- **Master Summary**: one consolidated text with architecture, decisions, releases and the next starting point;
- **Turn checkpoints**: the raw text of each 20-turn block of the current session, as crash protection.

| When | Command |
|---|---|
| Start of a session | `read_memory.py --project <p>` (or `--list-projects`) |
| Every 20 turns | `save_incremental_turns.py --project <p> --start 1 --end 20 --file turns.md` |
| End of a session | `finalize_session.py --project <p> --file summary.md [--version x.y.z]` |
| Migrate an old database | `migrate_legacy.py --source <old_db> --project <p> [--dry-run]` |

All scripts are in `<skills_root>/session-memory/`.

- `finalize_session.py` **replaces** the Master Summary: the new text must keep everything still valid from the old one.
- `finalize_session.py` deletes only that project's turn checkpoints.
- **Project resolution**: `--project`, then `SESSION_MEMORY_PROJECT`, then the Git repository root name, then the current folder name.
- **Database**: `~/.ai-code-agent-memory/session-memory/db` (override with `SESSION_MEMORY_DB`).

Details: [session-memory/SKILL.md](session-memory/SKILL.md).

### architecture-docs

Keeps the technical documentation describing how the system works **today**, instead of growing into a changelog:

| Content | Where | Rule |
|---|---|---|
| How a subsystem works today | `docs/architecture/<subsystem>.md` | Edited in place; outdated text is replaced or removed |
| Why a decision was made | `docs/adr/NNNN-<slug>.md` | One decision per file; immutable once accepted (status and relations only) |
| When something changed | GitHub release notes | Never in the architecture docs |
| Entry point | `TECDOC.md` | Short index; subsystem map and ADR list generated |

| Task | Command |
|---|---|
| New ADR | `new_adr.py --title "<decision>" [--version v1.2.0] [--subsystem rag] [--supersedes 129] [--amends 118]` |
| New subsystem page | `new_subsystem.py --name rag --title "Retrieval Pipeline" --summary "<one sentence>" [--code "src/rag"]` |
| Regenerate the index | `build_index.py` (`--check` to only verify) |
| Validate | `check_docs.py [--include-plans] [--max-warnings 0]` |
| Migrate a legacy single-file TECDOC | `extract_adrs.py [--source TECDOC.md] [--dry-run]`, then the migration steps in the SKILL.md |

All scripts are in `<skills_root>/architecture-docs/`.

- **`check_docs.py` fails on**: headings with versions or manual numbering, invalid ADR frontmatter, ADR relations not recorded in both directions, broken links and anchors, an outdated index, and headings not in the configured language.
- **It warns about**: long files, body text that looks like another language, ADRs missing a standard section, and documented source paths that no longer exist.
- **Gate 8** runs it. Once a project has `docs/adr` or `docs/architecture`, errors block the release, so migrate a legacy TECDOC on its own branch and finish it before the next release.
- **Language**: English by default (`architecture_docs.language`). Mark a line with `<!-- lang-ok -->` to exempt a proper name or quote.

Details: [architecture-docs/SKILL.md](architecture-docs/SKILL.md).

### software-design

The architect. Universal principles for every project (modularity, separation of responsibilities, SOLID, Clean Code, explicit errors, testability), a **simplicity-first** rule (an abstraction needs a second implementation, a test seam or an external dependency to isolate; design patterns only for a named problem), and the project's architecture style from `architecture.style`, with a guide for `hexagonal`, `layered`, `mvc`, `pipeline` and `modular-monolith`.

| Task | Command |
|---|---|
| Check the style and core dependencies | `check_design.py [--dir .] [--strict]` |

`check_design.py` validates `architecture.style` and `architecture.core_path`, and flags core files that import anything in `architecture.forbidden_core_imports` (Rust `use` and qualified paths, Python, JavaScript/TypeScript, Go, C/C++, C#/Java imports; test folders are skipped). Details: [software-design/SKILL.md](software-design/SKILL.md).

### quality-assurance

The QA role. Automated tests on the native runner and in CI, a regression test for every bug fix, and **one curated manual test suite** (`tests.manual_tests`) organized by feature and edited in place, for what cannot be automated:

```markdown
## Web Sources
### MT-WEB-001: Re-sync does not duplicate chunks
- **Covers**: ingestion
- **Steps**:
  1. Run `/sync` twice
- **Expected**:
  - The chunk count is the same
- **Result** (v1.4.0-rc.1): [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:
```

| Task | Command |
|---|---|
| Validate the suite | `manual_tests.py validate` |
| Start a release candidate run | `manual_tests.py start-run --run v1.4.0-rc.1 --feature "Web Sources"` (or `--id`, `--all`) |
| Results | `manual_tests.py summary [--run R]` / `failed [--run R]` / `list [--feature F]` |
| Acceptance gate | `manual_tests.py gate --run v1.4.0-rc.1` |

Details: [quality-assurance/SKILL.md](quality-assurance/SKILL.md).

### devops-release

The DevOps role. Branches `main` (released), `dev` (integration) and short-lived `feat/`, `fix/`, `docs/`, `chore/` branches from `dev`; tests on every pull request into `dev` and `main`; versions bumped in `dev`; release candidates published as pre-releases; **the user's merge into `main` is the release gate**, and it creates the final tag and release automatically. It also plans and tests logs, diagnostics and telemetry (`devops.observability`).

| Task | Command |
|---|---|
| Check the workflows against the policy | `check_ci_policy.py [--dir .]` |
| Version in the version files | `release_status.py version` |
| Next release candidate tag | `release_status.py next-rc` |
| Verify a published release | `release_status.py verify --tag v1.4.0 [--asset "*.zip"]` |

Workflow templates: [devops-release/templates](devops-release/templates) (`ci.yml`, `release.yml`, `tag-on-merge.yml`). Details: [devops-release/SKILL.md](devops-release/SKILL.md).

### user-docs

The technical writer. The README is for people who install and use the software: what it is, installation, quick start, usage, configuration, troubleshooting, links and license; always in English, edited in place, with no version sections and no implementation detail. Template: [user-docs/README_TEMPLATE.md](user-docs/README_TEMPLATE.md).

| Task | Command |
|---|---|
| Check the README | `check_readme.py [--dir .] [--max-warnings N]` |

Details: [user-docs/SKILL.md](user-docs/SKILL.md).

### folder-sync

Turns any folder into a searchable knowledge base without loading its files into the conversation.

| Trigger | Command |
|---|---|
| `/sync` — index the folder (incremental, SHA-256) | `sync.py --dir .` (`--force` to reindex everything) |
| `/folder` — ask about the folder | `query.py --dir . --query "<question>" [--top-k 6] [--json]` |
| `/folder-stats` | `stats.py --dir .` |
| `/folder-clean` — delete the index | `clean.py --dir .` |

All scripts are in `<skills_root>/folder-sync/scripts/`.

- **Supported files**: `.docx`, `.pdf`, `.md`, `.txt`, `.json`, `.csv` and source code.
- **Ignored**: `.git`, `node_modules`, `.venv` and binaries.
- **Google Docs shortcuts**: `.gdoc` files are reported so you can export them.
- **Index location**: `.folder_brain/` inside the indexed folder. Add it to that folder's `.gitignore` if the folder is a repository.

Details: [folder-sync/SKILL.md](folder-sync/SKILL.md).

---

## Project configuration: `.dev-cycle.json`

Optional file at a project's root: the project configuration shared by every skill, one section per skill (`architecture`, `docs`, `tests`, `devops`, `plans`, `backlog`, `architecture_docs`, `drift_rules`) plus `test_commands`. Every key is optional and older files with flat keys keep working.

```json
{
  "project_name": "my-app",
  "architecture": { "style": "hexagonal", "core_path": "src/core", "forbidden_core_imports": ["my_app_cli"] },
  "test_commands": ["cargo test --workspace"],
  "devops": { "releases_repo": "my-org/my-app-releases", "version_files": ["Cargo.toml"], "observability": "cli" }
}
```

```bash
python ~/.ai-code-agent-skills/dev-cycle-protocol/project_config.py --validate
```

Full key reference, defaults and drift rules: [docs/configuration.md](docs/configuration.md).

---

## Maintaining the repository

**Add a skill**
1. Create `<skill-name>/SKILL.md` with `name` and `description` frontmatter. The description is what makes agents load it, so say when to use it.
2. Add `!/<skill-name>/` to [`.gitignore`](.gitignore) (allowlist).
3. To ship it in the cloud plugin, add `./<skill-name>` to `skills` in [`.claude-plugin/plugin.json`](.claude-plugin/plugin.json) and bump `version`.
4. Add tests under `<skill-name>/tests` for every script.
5. Run `install.ps1` / `install.sh` again, then commit and push.

**Run the tests**
```bash
python -m pytest plan-memory/tests dev-cycle-protocol/tests software-design/tests user-docs/tests quality-assurance/tests devops-release/tests
```

**Conventions**
- Write in English.
- Keep data out of skill folders: shared data in the project repository, personal data in `~/.ai-code-agent-memory/<skill>` with an environment variable override.
- Refer to scripts as `<skills_root>/<skill>/...` and to other skills by relative links (`../plan-memory/SKILL.md`).

**Personal / machine-specific skills** stay out of this repository, in `~/.ai-code-agent-skills-personal`, linked with `install.ps1 -Source`.

**Update on another machine**
```bash
git -C ~/.ai-code-agent-skills pull
```
Links pick up the changes at once. Rerun the install script only when skills were added. Cloud sessions get the new plugin version at their next start.

---

## Troubleshooting

| Symptom | Fix |
|---|---|
| An agent does not list the skills | Check the folder in the table above has links (`ls ~/.agents/skills`), rerun the install script, restart the agent. |
| `! <skill>: ... already exists` | Something else has that name in the agent's folder. Remove or rename it, then rerun. |
| `ModuleNotFoundError: chromadb` / `docx` / `pypdf` | `pip install -r requirements.txt` with the same Python the agent uses. |
| `plan-memory` lists no plans | Run it inside the project repository (or pass `--dir`); plans are read from `docs/plans` (or `plans.dir`). |
| A memory script finds nothing for a project | Run `read_memory.py --list-projects` or `list_plans.py`; the project may be stored under another name. Names are normalized (`AnyContext` = `any-context`), but `anycontext` cannot be split. |
| `check_docs.py` reports many errors after `extract_adrs.py` | Expected: they are the migration work list (translate and restructure ADRs, rewrite the index and subsystem pages). |
| `detect_drift.py` reports `Drift Rules: none` | Run it from the project folder (or pass `--dir`) and add `drift_rules` to `.dev-cycle.json`. |
| Cloud session has no `dev-skills:*` skills | Check the branch has `.claude/settings.json`, the GitHub app can access this repository, and `/plugin` for load errors. |

---

## License

Private repository. All rights reserved; no license is granted.
