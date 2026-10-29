# AI Code Agent Skills

Development skills in the open `SKILL.md` format, shared by every coding agent on the machine (Claude Code, Codex, Gemini, Copilot and others) and by Claude Code cloud sessions through a plugin.

## What It Does

| Skill | Role | What it does |
|---|---|---|
| [`dev-cycle`](#dev-cycle) | Command | `/dev-cycle ["<idea or request>"]`: the one entry point; diagnoses the repository, proposes where to start, sets up what is missing after you confirm |
| [`dev-cycle-protocol`](#dev-cycle-protocol) | Orchestrator | Development cycle from product inception to a verified release: phases, the role called in each one, full and short paths, the user's gates, GitHub Spec Kit as optional executor, the shared `.dev-cycle.json` |
| [`product-inception`](#product-inception) | Product | Phase 0: from an idea to an approved product baseline (vision, architecture, technical specification, backlog with epics, roadmap), or adoption of an existing project |
| [`plan-memory`](#plan-memory) | Planner | The project backlog with epics (`docs/BACKLOG.md`), the roadmap of releases (`docs/ROADMAP.md`) and complete plans as versioned files (`docs/plans`), shared by the team, with promotion of backlog items, milestones, lineage, search and drift detection |
| [`backlog`](#backlog) | Command | `/backlog --feature\|--improvement\|--adjustment\|--bug\|--epic "<text>"`: capture an idea or bug for later in one line |
| [`code-review`](#code-review) | Reviewer | Reviews every pull request on GitHub or GitLab with line comments and levels; the agent fixes small threads and brings large ones to you |
| [`software-design`](#software-design) | Architect | Design principles, the project's architecture style with a guide per style, design input for plans, review checklist and core dependency check |
| [`architecture-docs`](#architecture-docs) | Technical documentation | Keeps technical docs current: short index, one living page per subsystem, immutable ADRs, validation and migration of legacy docs |
| [`quality-assurance`](#quality-assurance) | QA | Automated test strategy and one curated manual test suite with Pass/Fail/Blocked runs per release candidate |
| [`devops-release`](#devops-release) | DevOps | The branching model (integration, trunk or release branches), pull request CI, versions, release candidates, the user's release gate, observability |
| [`user-docs`](#user-docs) | Technical writer | End-user READMEs in English with a standard structure, template and checker |
| [`session-memory`](#session-memory) | Session memory | Recovers and consolidates each developer's own session context per project (local ChromaDB) |
| [`gedo-feedback`](gedo-feedback/SKILL.md) | Gedo feedback (temporary) | Off until a project uses [Gedo](https://github.com/Levix-Digital/gedo) (`gedo.enabled`): records what goes wrong with Gedo in real parallel work, one file per entry in `docs/gedo-feedback/`, and turns entries into Gedo issues with your confirmation |
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
├── dev-cycle/  dev-cycle-protocol/  product-inception/  plan-memory/  backlog/
├── software-design/  architecture-docs/  quality-assurance/
├── devops-release/  user-docs/  session-memory/  gedo-feedback/  folder-sync/
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
<project>/docs/BACKLOG.md, ROADMAP.md, product/   backlog, roadmap, vision and technical specification
<project>/.specify/, specs/              Spec Kit setup and feature specs (when enabled)
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
- Optional, for projects that use GitHub Spec Kit: [uv](https://docs.astral.sh/uv/) and network access to GitHub
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

Agents load a skill on their own when the task matches its `description`; you can also ask for one by name ("use plan-memory to save this plan"). For project work, start with **`/dev-cycle`**: with your idea in a new folder, with a request in an existing project, or alone to see where things stand. It diagnoses the repository, says in one line when everything is in order, and otherwise offers the options in the recommended order (a plan in progress, a pending release candidate, blocking bugs, missing setup, the next items of the roadmap). In a repository that never used the cycle it proposes the setup (configuration, `dev` branch, Spec Kit, product baseline) and writes nothing before you confirm. Agents without slash commands: say "start the dev cycle".

0. For a **new product**, start with the idea ("I want an app where families keep their photos"). The agent runs **product inception**: rounds of questions and suggestions, then the vision, the architecture, the technical specification, the backlog of the complete product (epics and their items) and the roadmap from the MVP, each for your approval. An existing project can adopt the same baseline.
1. In a project, describe what you want ("add an export command"), or start from the next items of the roadmap. The agent stays in **Discovery** until you agree on an approach.
2. It writes the **plan** with a section from each role and saves it in `docs/plans`. Reply "Approved" to start, or ask for changes.
3. It implements on a branch, runs the tests, updates the docs and opens a **pull request into `dev`**, following CI until it is green. The pull request is where review happens: the protocol posts its own review there, and the agent reads your comments on the platform, fixes the small ones and asks you about the large ones ([docs/pull-requests.md](docs/pull-requests.md)).
4. When you want to release, it cuts a **release candidate** (`vX.Y.Z-rc.N`) and prepares the manual tests to run. Install the candidate and mark each test Pass, Fail or Blocked in `tests/MANUAL_TESTS.md`; failures come back as fixes.
5. When the candidate passes, it opens the pull request from `dev` into `main`. **Your merge releases it**; the agent follows the release until every asset is published.

Small fixes and documentation changes take the short path: no plan file and no release candidate.

Projects can let **GitHub Spec Kit** do the detail of each feature (specification, clarification, technical plan, tasks, implementation) inside the cycle, while the cycle keeps the gates, the documentation and the releases: [docs/spec-kit.md](docs/spec-kit.md).

Ideas and bugs you do not want to work on yet go to the **backlog**: type `/backlog --feature "<your idea>"` (or `--improvement`, `--adjustment`, `--bug`, `--epic`) and the agent records it in `docs/BACKLOG.md` with an id. The **roadmap** (`docs/ROADMAP.md`) says which items go into which release. Bugs the agent finds outside the current change land there too. At the start of the next cycle the agent shows the backlog; you choose what to plan; open critical, high or medium bugs come before new features, a low bug goes into the next release at the latest, and no known bug stays untriaged.

---

## Claude Code cloud sessions (plugin)

Cloud sessions do not see your local folders. For them, this repository is also a Claude Code marketplace (`levix-agent-skills`) with the `dev-skills` plugin, which ships the development skills. Setup and caveats: [docs/cloud-sessions.md](docs/cloud-sessions.md).

---

## Skills

### dev-cycle

The `/dev-cycle` command, a thin router over `dev-cycle-protocol/onboard.py`: `diagnose` reads the configuration, the branching model and branches, product baseline, plans, the latest release candidate run, blocking bugs, the roadmap and Spec Kit; `propose-config` and `write-config` create or complete `.dev-cycle.json` from what can be detected (adding keys, never overwriting); `create-branch` creates the integration branch (`integration` model). The branching model and Spec Kit are each asked about once per project (the model detected from the branches), and a repository that already uses Spec Kit is adopted (`speckit_setup.py adopt`) with its hand-written constitution moved into the sources before it is replaced. Details: [dev-cycle/SKILL.md](dev-cycle/SKILL.md).

### dev-cycle-protocol

The orchestrator. It owns the phases, the gates and which role runs when; each role skill owns its artifact, rules and checker.

| Phase | Roles |
|---|---|
| 0. Product inception, once per product (user approves the baseline) | product-inception, software-design, architecture-docs, plan-memory |
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

Phase 0 runs once per product, phases 1 to 7 for every change, phases 8 to 10 for every release. Small fixes and documentation-only changes take a short path (phases 1, 4 to 7).

Scripts:
```bash
python <skills_root>/dev-cycle-protocol/inspect_project.py [--dir .] [--json]
<skills_root>/dev-cycle-protocol/bin/dev-cycle dev-cycle-protocol project_config [--json] [--validate] [--migrate [--write]]
python <skills_root>/dev-cycle-protocol/verify_gate.py --gate 0|3|rc|8 [--run vX.Y.Z-rc.N] [--dir .]
python <skills_root>/dev-cycle-protocol/speckit_setup.py status|latest|check|init|upgrade
python <skills_root>/dev-cycle-protocol/constitution.py generate|check
```
- `inspect_project.py` detects languages, manifests, test runners, documentation files and Git state, and shows the configured architecture style, branching model, releases repository, backlog, current release of the roadmap, product baseline and Spec Kit state.
- `project_config` shows, validates or migrates `.dev-cycle.json`. It runs on the native engine: `bin/dev-cycle` in sh shells, `bin/dev-cycle.ps1` in PowerShell (the first run downloads it).
- `verify_gate.py --gate 0` (before the product baseline approval): product documents, architecture style, technical docs, a backlog with epics, a roadmap with a first release, Spec Kit and the constitution when enabled; open questions are listed.
- `verify_gate.py --gate 3` (before plan approval): working tree, active plan, bugs first (a plan that is not a bug plan is blocked while an open critical, high or medium bug, or a bug not triaged, is outside it, unless you accept the risk: `--accept-open-bugs "<reason>"`), with Spec Kit the spec folder (no open clarification), branch and constitution, no secrets tracked or staged.
- `verify_gate.py --gate rc --run <tag>` (candidate acceptance): every manual test of the run is Pass.
- `verify_gate.py --gate 8` (before the release gate: the release pull request, or the promotion of the candidate): documentation and manual test files exist and the checkers of every role pass (`check_docs`, `check_readme`, `manual_tests validate`, `check_design`, `check_ci_policy`, `release_status version`, `backlog.py validate`, `roadmap.py validate`), plus the candidate run with `--run`, no secrets and a clean tree, no open bug untriaged; open blocking bugs are listed for the release notes and open items of the release for you to move or finish.

**Spec Kit** ([GitHub Spec Kit](https://github.com/github/spec-kit)) can do the detail of feature plans in Discovery, Plan and Implementation, at a version pinned per project after a compatibility check, with a constitution generated from the project's rules: [docs/spec-kit.md](docs/spec-kit.md).

Full protocol: [dev-cycle-protocol/SKILL.md](dev-cycle-protocol/SKILL.md).

### product-inception

Phase 0: turns an idea into an approved **product baseline** before the first change. The agent interviews you in rounds of up to five questions, each with a suggested answer, then writes, one at a time and each for your approval: the business document (`docs/product/VISION.md`), the architecture (style, technical docs, founding ADRs), the technical specification (`docs/product/TECH_SPEC.md`), the Spec Kit constitution when used, the backlog of the complete product as epics and items, and the roadmap from the MVP. It ends at gate 0. An existing project can adopt the same baseline: the agent drafts it from the code and asks only what the code cannot answer.

`check_product --init` (native engine: `<skills_root>/dev-cycle-protocol/bin/dev-cycle product-inception check_product`, `bin/dev-cycle.ps1` in PowerShell) creates the two documents from the templates; `check_product` checks sections, placeholders, language and length, with open questions as warnings. Details: [product-inception/SKILL.md](product-inception/SKILL.md).

### plan-memory

Keeps the project's planning in its repository: the **backlog**, where work is captured for later, the **roadmap**, which orders the releases, and **plans**, complete and uncompressed, shared by everyone on the project:

- **Backlog**: `docs/BACKLOG.md` (`backlog.file`), sections Bugs (`BUG-NNN`, with severity), Features (`FEAT-NNN`), Improvements (`IMP-NNN`), Adjustments (`ADJ-NNN`) and Epics (`EPIC-NNN`, with priority), and Closed. An item may belong to an epic (`--epic EPIC-NNN`); epics group the complete product and are never promoted, their items are. `backlog.py promote <ID>` turns items into a draft plan (a bug plan for bugs, with reproduction, root cause and regression test); the items follow the plan and close when it is completed.
- **Roadmap**: `docs/ROADMAP.md` (`roadmap.file`), releases in delivery order (`## vX.Y.Z: <name>` with status, goal and items). `roadmap.py` adds and orders releases, assigns items (an epic adds its open items), shows the current release and validates the ids against the backlog.
- **Plan files**: `docs/plans/<plan_id>.md` in the project repository, with YAML frontmatter (status, version, author, milestones, lineage, status log) and the plan body. They are committed and reviewed like code. Change the folder with `plans.dir` in `.dev-cycle.json`.
- **Local index**: a ChromaDB index for semantic search and drift detection, synced from the files on every command (including teammates' changes after `git pull`). It is disposable: `rebuild_index.py` recreates it.

| Task | Command |
|---|---|
| Save or update a plan | `save_plan.py --title "<t>" --file plan.md [--version v1.2.0] [--milestones "A,B,C"] [--supersedes <old_id>] [--kind bug] [--backlog-items "BUG-001"] [--spec-dir specs/001-x]` |
| Plan releases | `roadmap.py add-release v0.1.0 --name MVP --goal "<g>"`, `assign <ID...> --release v0.1.0`, `current`, `show`, `validate` |
| List plans | `<skills_root>/dev-cycle-protocol/bin/dev-cycle plan-memory list_plans [--status active] [--kind bug]` (`bin/dev-cycle.ps1` in PowerShell) |
| Read a plan in full | `<skills_root>/dev-cycle-protocol/bin/dev-cycle plan-memory get_plan --id <plan_id>` or without `--id` (latest active; `bin/dev-cycle.ps1` in PowerShell) |
| Semantic search | `search_plans.py --query "<text>" [--limit 5] [--all-projects]` |
| Milestones | `check_milestone.py --list` / `--milestone "A" [--notes "..."]` / `--milestone "A" --uncheck` |
| Change status | `update_plan_status.py --id <plan_id> --status completed [--notes "..."]` |
| Check a proposal for drift | `detect_drift.py --proposal "<text>"` or `--file proposal.md [--strict]` |
| Export (JSON or copy) | `<skills_root>/dev-cycle-protocol/bin/dev-cycle plan-memory export_plan --id <plan_id> --format json --output-dir out/` (`bin/dev-cycle.ps1` in PowerShell) |
| Migrate from the old database | `migrate_legacy.py [--source <old_db>] [--dry-run]` |

All scripts are in `<skills_root>/plan-memory/`. They work on the Git repository of the current directory; use `--dir` for another one and `--project` to override the project name.

Plans move through `draft`, `planned`, `active`, `in_progress`, `completed` and `superseded` (`--supersedes` links the old plan); milestones record who completed them and when; the scripts never commit; drift detection combines similarity, milestone coverage and the project's `drift_rules`; the index lives in `~/.ai-code-agent-memory/plan-memory/db` (`PLAN_MEMORY_DB`). Details: [plan-memory/SKILL.md](plan-memory/SKILL.md).

### backlog

The `/backlog` command captures without leaving the conversation: `/backlog --feature "I would like answers exportable to Markdown"`, `/backlog --feature "Album sharing" --in EPIC-002`, `/backlog --epic "Photo organization"`, `/backlog --bug "sync never finishes on network drives" --severity high`, or `/backlog` alone for the summary. The agent writes a short title and a faithful English description, runs `plan-memory/backlog.py add` and answers with one line (`FEAT-003: Export answers to Markdown (medium)`). Priority defaults to `medium`; a bug without `--severity` gets an estimated one, stated in the answer. In agents without slash commands, ask "add to the backlog as a feature: ...". Details: [backlog/SKILL.md](backlog/SKILL.md); backlog commands (`add`, `summary`, `promote`, `close`, ...): [plan-memory/SKILL.md](plan-memory/SKILL.md#-backlog-backlogpy).

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
| New ADR | `<skills_root>/dev-cycle-protocol/bin/dev-cycle architecture-docs new_adr --title "<decision>" [--version v1.2.0] [--subsystem rag] [--supersedes 129] [--amends 118]` |
| New subsystem page | `<skills_root>/dev-cycle-protocol/bin/dev-cycle architecture-docs new_subsystem --name rag --title "Retrieval Pipeline" --summary "<one sentence>" [--code "src/rag"]` |
| Regenerate the index | `<skills_root>/dev-cycle-protocol/bin/dev-cycle architecture-docs build_index` (`--check` to only verify) |
| Validate | `<skills_root>/dev-cycle-protocol/bin/dev-cycle architecture-docs check_docs [--include-plans] [--max-warnings 0]` |
| Migrate a legacy single-file TECDOC | `<skills_root>/dev-cycle-protocol/bin/dev-cycle architecture-docs extract_adrs [--source TECDOC.md] [--dry-run]`, then the migration steps in the SKILL.md |

The `architecture-docs` commands run on the native engine (`bin/dev-cycle.ps1` in PowerShell).

- **`check_docs` fails on**: headings with versions or manual numbering, invalid ADR frontmatter, ADR relations not recorded in both directions, broken links and anchors, an outdated index, and headings not in the configured language.
- **It warns about**: long files, body text that looks like another language, ADRs missing a standard section, and documented source paths that no longer exist.
- **Gate 8** runs it. Once a project has `docs/adr` or `docs/architecture`, errors block the release, so migrate a legacy TECDOC on its own branch and finish it before the next release.
- **Language**: English by default (`architecture_docs.language`). Mark a line with `<!-- lang-ok -->` to exempt a proper name or quote.

Details: [architecture-docs/SKILL.md](architecture-docs/SKILL.md).

### code-review

The reviewer role. Every pull request (merge request on GitLab) is reviewed on its platform against the plan, design, tests, documentation, security and release safety; each finding is a line comment marked `[review]` with a level (`blocking`, `suggestion`, `nit`) and a concrete fix, and the review never approves. `dev-cycle-protocol/pr_sync.py` reads the pull request through `gh` or `glab`, posts replies, resolutions and reviews, and its merge gate (`gate-merge`) fails while a `blocking` finding is open; nothing it posts may mention an AI. Details: [code-review/SKILL.md](code-review/SKILL.md), [docs/pull-requests.md](docs/pull-requests.md).

### software-design

The architect. Universal principles for every project (modularity, separation of responsibilities, SOLID, Clean Code, explicit errors, testability), a **simplicity-first** rule (an abstraction needs a second implementation, a test seam or an external dependency to isolate; design patterns only for a named problem), and the project's architecture style from `architecture.style`, with a guide for `hexagonal`, `layered`, `mvc`, `pipeline` and `modular-monolith`.

| Task | Command |
|---|---|
| Check the style and core dependencies | `<skills_root>/dev-cycle-protocol/bin/dev-cycle software-design check_design [--dir .] [--strict]` (`bin/dev-cycle.ps1` in PowerShell) |

`check_design` validates `architecture.style` and `architecture.core_path`, and flags core files that import anything in `architecture.forbidden_core_imports` (Rust `use` and qualified paths, Python, JavaScript/TypeScript, Go, C/C++, C#/Java imports; test folders are skipped). Details: [software-design/SKILL.md](software-design/SKILL.md).

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
| Validate the suite | `<skills_root>/dev-cycle-protocol/bin/dev-cycle quality-assurance manual_tests validate` (`bin/dev-cycle.ps1` in PowerShell) |
| Start a release candidate run | `manual_tests start-run --run v1.4.0-rc.1 --feature "Web Sources"` (or `--id`, `--all`) |
| Results | `manual_tests summary [--run R]` / `failed [--run R]` / `list [--feature F]` |
| Acceptance gate | `manual_tests gate --run v1.4.0-rc.1` |

Details: [quality-assurance/SKILL.md](quality-assurance/SKILL.md).

### devops-release

The DevOps role. Each project chooses its branching model (`devops.branching`), asked once by `/dev-cycle`: `integration` (default: `main` released, `dev` integration; you merge `dev` into `main`), `trunk` (`main` alone; you promote the accepted candidate) or `release-branches` (`main` and `release/X.Y`, candidates on the release branch; you promote). Work happens on short-lived `feat/`, `fix/`, `docs/`, `chore/` branches with tests on every pull request; versions are bumped before the candidate; release candidates are published as pre-releases; **your merge or promotion is the release gate**, and it creates the final tag and release automatically (the promotion tags the exact commit you accepted). It also plans and tests logs, diagnostics and telemetry (`devops.observability`).

| Task | Command |
|---|---|
| The project's branching model | `<skills_root>/dev-cycle-protocol/bin/dev-cycle devops-release branching show` / `rc-branch` (`bin/dev-cycle.ps1` in PowerShell) |
| Check the workflows against the policy | `<skills_root>/dev-cycle-protocol/bin/dev-cycle devops-release check_ci_policy [--dir .]` (`bin/dev-cycle.ps1` in PowerShell) |
| Version in the version files | `<skills_root>/dev-cycle-protocol/bin/dev-cycle devops-release release_status version` (`bin/dev-cycle.ps1` in PowerShell) |
| Next release candidate tag | `release_status next-rc` |
| Verify a published release | `release_status verify --tag v1.4.0 [--asset "*.zip"]` |

Workflow templates: [devops-release/templates](devops-release/templates) (`ci.yml`, `release.yml`, `tag-on-merge.yml`, `promote.yml`). Details: [devops-release/SKILL.md](devops-release/SKILL.md).

### user-docs

The technical writer. The README is for people who install and use the software: what it is, installation, quick start, usage, configuration, troubleshooting, links and license; always in English, edited in place, with no version sections and no implementation detail. Template: [user-docs/README_TEMPLATE.md](user-docs/README_TEMPLATE.md).

| Task | Command |
|---|---|
| Check the README | `<skills_root>/dev-cycle-protocol/bin/dev-cycle user-docs check_readme [--dir .] [--max-warnings N]` (`bin/dev-cycle.ps1` in PowerShell) |

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

Optional file at a project's root: the project configuration shared by every skill, one section per skill (`architecture`, `docs`, `tests`, `devops`, `plans`, `backlog`, `roadmap`, `product`, `speckit`, `gedo`, `architecture_docs`, `drift_rules`) plus `test_commands`. Every key is optional and older files with flat keys keep working.

```json
{
  "project_name": "my-app",
  "architecture": { "style": "hexagonal", "core_path": "src/core", "forbidden_core_imports": ["my_app_cli"] },
  "test_commands": ["cargo test --workspace"],
  "devops": { "releases_repo": "my-org/my-app-releases", "version_files": ["Cargo.toml"], "observability": "cli" }
}
```

Check it with `~/.ai-code-agent-skills/dev-cycle-protocol/bin/dev-cycle dev-cycle-protocol project_config --validate` (PowerShell: `& ~/.ai-code-agent-skills/dev-cycle-protocol/bin/dev-cycle.ps1 dev-cycle-protocol project_config --validate`).

Full key reference, defaults and drift rules: [docs/configuration.md](docs/configuration.md).

---

## Maintaining the repository

Adding a skill, running the tests, conventions, personal skills and updating other machines: [docs/maintaining.md](docs/maintaining.md).

---

## Troubleshooting

| Symptom | Fix |
|---|---|
| An agent does not list the skills | Check the folder in the table above has links (`ls ~/.agents/skills`), rerun the install script, restart the agent. |
| `! <skill>: ... already exists` | Something else has that name in the agent's folder. Remove or rename it, then rerun. |
| `ModuleNotFoundError: chromadb` / `docx` / `pypdf` | `pip install -r requirements.txt` with the same Python the agent uses. |
| `plan-memory` lists no plans | Run it inside the project repository (or pass `--dir`); plans are read from `docs/plans` (or `plans.dir`). |
| A memory script finds nothing for a project | Run `read_memory.py --list-projects` or `list_plans` (engine); the project may be stored under another name. Names are normalized (`AnyContext` = `any-context`), but `anycontext` cannot be split. |
| `check_docs` reports many errors after `extract_adrs` | Expected: they are the migration work list (translate and restructure ADRs, rewrite the index and subsystem pages). |
| `detect_drift.py` reports `Drift Rules: none` | Run it from the project folder (or pass `--dir`) and add `drift_rules` to `.dev-cycle.json`. |
| Cloud session has no `dev-skills:*` skills | `claude plugin list`. A session that starts outside the repository (Claude Projects, several repositories) never reads the project's `.claude/settings.json`: install with the environment's Setup script ([docs/cloud-sessions.md](docs/cloud-sessions.md)). Otherwise check the branch has `.claude/settings.json`, the GitHub app can access this repository, and `/plugin` for load errors. |

---

## License

Public repository, not yet open source: all rights reserved, no license is granted until the open source launch.
