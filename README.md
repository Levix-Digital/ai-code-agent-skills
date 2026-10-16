# AI Code Agent Skills

Skills in the open `SKILL.md` format (one folder per skill, with `name` and `description` frontmatter), shared across coding agents: Claude Code, Codex, Gemini, Copilot and others.

## Skills

| Skill | Purpose |
|---|---|
| `dev-cycle-protocol` | 8-step development lifecycle with explicit approval gates |
| `plan-memory` | Full architecture plans persisted in ChromaDB, per project |
| `session-memory` | 2-layer session memory in ChromaDB, per project |
| `folder-sync` | Incremental folder RAG with ChromaDB |

Data (ChromaDB databases, virtual environments, `.env` files) is never versioned: see [`.gitignore`](.gitignore).

## Local install (all agents)

Links each skill into each agent's skills folder. Default targets: `~/.claude/skills` (Claude Code), `~/.agents/skills` (cross-agent standard location) and `~/.gemini/config/skills` (Gemini).

```powershell
.\install.ps1
.\install.ps1 -Targets "$HOME\.claude\skills","$HOME\.copilot\skills"
```

```bash
./install.sh
./install.sh ~/.claude/skills ~/.copilot/skills
```

The scripts never overwrite an existing entry that is not a link to this repository, and skip a target folder that is itself a link. Run them again after adding a new skill.

## Claude Code in the cloud (plugin)

This repository is also a Claude Code plugin marketplace (`levix-agent-skills`) with the `dev-skills` plugin. To have cloud sessions load the skills, declare it in the project's `.claude/settings.json`:

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

In the plugin, skills are namespaced: `dev-skills:session-memory`. New skills must also be listed in [`.claude-plugin/plugin.json`](.claude-plugin/plugin.json).

## Memory skill data

- `session-memory`: `~/.ai-code-agent-memory/session-memory/db` (override with `SESSION_MEMORY_DB`)
- `plan-memory`: `~/.ai-code-agent-memory/plan-memory/db` (override with `PLAN_MEMORY_DB`)

Both skills normalize project names to kebab-case (`AnyContext` → `any-context`). An all-lowercase concatenated name (`anycontext`) cannot be split: use the name of the project's Git repository root.
