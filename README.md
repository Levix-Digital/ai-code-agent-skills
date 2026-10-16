# AI Code Agent Skills

Development skills in the open `SKILL.md` format, shared by every coding agent on the machine (Claude Code, Codex, Gemini, Copilot and others) and by Claude Code cloud sessions through a plugin.

| Skill | What it does |
|---|---|
| [`dev-cycle-protocol`](#dev-cycle-protocol) | 8-step development lifecycle with an explicit approval gate, Dual-Doc, manual tests and release follow-through |
| [`plan-memory`](#plan-memory) | Keeps complete architecture plans as versioned files in each project (`docs/plans`), shared by the team, with milestones, lineage, search and drift detection |
| [`session-memory`](#session-memory) | Recovers and consolidates each developer's own session context per project (local ChromaDB) |
| [`folder-sync`](#folder-sync) | Incremental RAG over any folder: `/sync` to index, `/folder` to ask |

Agents load a skill on their own when the task matches its `description`. You can also ask for one by name ("use plan-memory to save this plan").

---

## Contents

- [How the repository works](#how-the-repository-works)
- [Setup](#setup)
- [Claude Code cloud sessions (plugin)](#claude-code-cloud-sessions-plugin)
- [Skills](#skills)
- [Project configuration: `.dev-cycle.json`](#project-configuration-dev-cyclejson)
- [Maintaining the repository](#maintaining-the-repository)
- [Troubleshooting](#troubleshooting)

---

## How the repository works

```
~/.ai-code-agent-skills/                 this repository (source of truth)
├── .claude-plugin/                      Claude Code marketplace + plugin manifests
├── dev-cycle-protocol/  plan-memory/  session-memory/  folder-sync/
├── install.ps1 / install.sh             link the skills into each agent
└── requirements.txt                     Python dependencies of the scripts

~/.claude/skills/<skill>        ─┐
~/.agents/skills/<skill>         ├─ links (junctions on Windows) to the folders above
~/.gemini/config/skills/<skill> ─┘

~/.ai-code-agent-memory/                 local data, never versioned
├── plan-memory/db                       search index of the plans (disposable)
└── session-memory/db                    this developer's session memory

<project>/docs/plans/*.md                plans: versioned and shared in each project
```

- **One copy of each skill.** Agents read the files through links, so an edit in this repository is seen by every agent at once.
- **No data in this repository.** It can be versioned, cloned and shipped as a plugin without carrying data.
- **Shared vs. personal memory.** Plans belong to the project and are shared: they live as files in each project repository and go through review like code. Session memory belongs to each developer and stays in their local `~/.ai-code-agent-memory`.
- **Allowlist `.gitignore`.** Only the files listed in [`.gitignore`](.gitignore) are versioned; databases, virtual environments and `.env` files never are.
- **Paths in `SKILL.md`.** Commands are written as `<skills_root>/<skill>/script.py`, where `<skills_root>` is the folder that holds the skill. The agent replaces it with the real path.

---

## Setup

Requirements: Git, Python 3.10+ and `pip`.

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

Other targets and options:
```powershell
.\install.ps1 -Targets "$HOME\.copilot\skills"              # extra agent
.\install.ps1 -Source "$HOME\.ai-code-agent-skills-personal" # link skills from another folder
.\install.ps1 -Exclude folder-sync                           # skip a skill
```
```bash
./install.sh ~/.copilot/skills
SOURCE=~/.ai-code-agent-skills-personal ./install.sh
EXCLUDE="folder-sync" ./install.sh
```

The scripts are idempotent and safe:
- an existing link to the same skill is kept (`=`);
- a new link is created (`+`);
- an existing entry that is not a link to the source is reported and **never overwritten** (`!`);
- a target folder that is itself a link is skipped (replace it with a real folder to link skills individually).

Restart the agent (or open a new session) after installing so it picks up the skills.

---

## Claude Code cloud sessions (plugin)

Cloud sessions do not see your local folders. For them, this repository is also a Claude Code marketplace (`levix-agent-skills`) with the `dev-skills` plugin, which ships the four skills.

Add to the project's `.claude/settings.json` and push it to the branch the cloud session uses:

```json
{
  "extraKnownMarketplaces": {
    "levix-agent-skills": {
      "source": { "source": "github", "repo": "Levix-Digital/ai-code-agent-skills" }
    }
  },
  "enabledPlugins": {
    "dev-skills@levix-agent-skills": true
  }
}
```

- In the plugin, skill names are prefixed: `dev-skills:session-memory`.
- The Claude GitHub app must have access to this private repository.
- In the cloud, plans come with the project clone (the index is built on first use, which needs `pip install -r requirements.txt` and downloads a small embedding model). Session memory is written to the session's own environment and does not reach your local database.
- Locally, a project with the plugin enabled shows each skill twice (from `~/.claude/skills` and from the plugin). Both read the same files.

---

## Skills

### dev-cycle-protocol

The governance protocol for AI pair-programming. It defines seven engineering pillars (modularity, hexagonal architecture with surface parity, Dual-Doc, cumulative manual tests, persistent plans, secret hygiene, release follow-through) and an 8-step cycle:

1. **Discovery** — read-only analysis; logs first; check existing plans.
2. **Complete final plan** — 8-section blueprint, drift check, saved with `plan-memory`.
3. **Explicit approval gate** — nothing is implemented until the user approves ("Approved", "Aprovado", "Go").
4. **Modular implementation** — core domain first, then storage, then adapters.
5. **Automated tests** — 100% green on the native runner, regression test for every bug fix.
6. **Dual-Doc update** — `README.md` (UserDoc) and `TECDOC.md` (TecDoc with ADRs and Mermaid).
7. **Manual test record** — new scenario added to `tests/MANUAL_TESTS.md`, nothing truncated.
8. **Closure** — version bump, PR, CI watched until green, merge, tag, release verified in the releases repository.

Scripts:
```bash
python <skills_root>/dev-cycle-protocol/inspect_project.py [--dir .] [--json]
python <skills_root>/dev-cycle-protocol/verify_gate.py --gate 3 --project <project> [--dir .]
python <skills_root>/dev-cycle-protocol/verify_gate.py --gate 8 --project <project> [--dir .]
```
- `inspect_project.py` detects languages, manifests, test runners, documentation files and Git state.
- `verify_gate.py --gate 3` (before implementing): clean working tree, active plan in `plan-memory`, no secrets tracked or staged.
- `verify_gate.py --gate 8` (before releasing): UserDoc, TecDoc and manual tests exist, no secrets, clean tree.

Per-project settings go in [`.dev-cycle.json`](#project-configuration-dev-cyclejson). Full protocol: [dev-cycle-protocol/SKILL.md](dev-cycle-protocol/SKILL.md).

### plan-memory

Keeps every multi-step plan **complete and uncompressed**, shared by everyone on the project:

- **Plan files**: `docs/plans/<plan_id>.md` in the project repository, with YAML frontmatter (status, version, author, milestones, lineage, status log) and the plan body. They are committed and reviewed like code. Change the folder with `plans_dir` in `.dev-cycle.json`.
- **Local index**: a ChromaDB index for semantic search and drift detection, synced from the files on every command (including teammates' changes after `git pull`). It is disposable: `rebuild_index.py` recreates it.

| Task | Command |
|---|---|
| Save or update a plan | `save_plan.py --title "<t>" --file plan.md [--version v1.2.0] [--milestones "A,B,C"] [--supersedes <old_id>]` |
| List plans | `list_plans.py [--status active]` |
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
- **The scripts never commit.** Commit the plan after approval (Step 3) and its milestone/status changes with the work that completes them.
- **Drift detection** combines similarity to the active plan, milestone coverage and order, and the project's own `drift_rules` (see [`.dev-cycle.json`](#project-configuration-dev-cyclejson)).
- **Index location**: `~/.ai-code-agent-memory/plan-memory/db` (override with `PLAN_MEMORY_DB`).

Details: [plan-memory/SKILL.md](plan-memory/SKILL.md).

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

Optional file at a project's root, read by `dev-cycle-protocol` and `plan-memory`. Every key is optional.

```json
{
  "project_name": "my-app",
  "core_path": "src/core",
  "doc_files": {
    "user_doc": "README.md",
    "tec_doc": "TECDOC.md",
    "manual_tests": "tests/MANUAL_TESTS.md"
  },
  "test_commands": ["cargo test --workspace"],
  "surfaces": ["cli", "rest-api", "mcp"],
  "canonical_datastore": "data/app.db",
  "releases_repo": "my-org/my-app-releases",
  "plans_dir": "docs/plans",
  "drift_rules": [
    { "tech": "langchain", "conflict_msg": "The baseline plan removes LangChain in favor of a native implementation." },
    {
      "tech": "parallel database",
      "negative_markers": ["single source", "settings.db"],
      "proposal_allow": [],
      "conflict_msg": "Proposal hints at a parallel database, violating the single source of truth."
    }
  ]
}
```

| Key | Meaning |
|---|---|
| `project_name` | Project name (normalized to kebab-case); default: the repository folder name |
| `core_path` | Where the domain core lives |
| `doc_files` | UserDoc, TecDoc and manual tests paths |
| `test_commands` | Commands that run the automated tests |
| `surfaces` | Interfaces that must stay in parity |
| `canonical_datastore` | The single source of truth database |
| `releases_repo` | Repository where releases must be published (Step 8) |
| `plans_dir` | Folder of the plan files (default `docs/plans`) |
| `drift_rules` | Technologies or patterns the project's plans reject |

**How a drift rule works.** A conflict is raised when:
1. the active plan mentions `tech` together with a rejection marker; and
2. the proposal mentions `tech` without an allowed phrase.

When `negative_markers` and `proposal_allow` are omitted, bilingual defaults are used (`remove`, `replace`, `without`, `remover`, `substituir`, `sem`, ... and `without <tech>`, `remove <tech>`, `sem <tech>`, ...).

---

## Maintaining the repository

**Add a skill**
1. Create `<skill-name>/SKILL.md` with `name` and `description` frontmatter. The description is what makes agents load it, so say when to use it.
2. Add `!/<skill-name>/` to [`.gitignore`](.gitignore) (allowlist).
3. To ship it in the cloud plugin, add `./<skill-name>` to `skills` in [`.claude-plugin/plugin.json`](.claude-plugin/plugin.json) and bump `version`.
4. Run `install.ps1` / `install.sh` again, then commit and push.

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
| `plan-memory` lists no plans | Run it inside the project repository (or pass `--dir`); plans are read from `docs/plans` (or `plans_dir`). |
| A memory script finds nothing for a project | Run `read_memory.py --list-projects` or `list_plans.py`; the project may be stored under another name. Names are normalized (`AnyContext` = `any-context`), but `anycontext` cannot be split. |
| `detect_drift.py` reports `Drift Rules: none` | Run it from the project folder (or pass `--dir`) and add `drift_rules` to `.dev-cycle.json`. |
| Cloud session has no `dev-skills:*` skills | Check the branch has `.claude/settings.json`, the GitHub app can access this repository, and `/plugin` for load errors. |
