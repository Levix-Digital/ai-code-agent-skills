# Claude Code Cloud Sessions (Plugin)

Cloud sessions do not see your local folders. For them, this repository is also a Claude Code marketplace (`levix-agent-skills`) with the `dev-skills` plugin, which ships the fourteen skills.

How to install it depends on where the session starts:

| The session starts | Install with |
|---|---|
| In the project's repository (Claude Code on the web with one repository) | The project's `.claude/settings.json` (below) |
| Outside it (Claude Projects, several repositories, or any session whose working folder is not the project, such as `/home/user`) | The environment's Setup script (below). Claude Code reads `.claude/settings.json`, `CLAUDE.md` and SessionStart hooks only from the folder the session starts in, so the project's settings have no effect there. |

## Setup script

In the cloud environment's settings, Setup script:

```bash
claude plugin marketplace add Levix-Digital/ai-code-agent-skills || true
claude plugin marketplace update levix-agent-skills || true
claude plugin install dev-skills@levix-agent-skills || true
```

Every new container installs the version on this repository's default branch. `|| true` keeps a network failure from stopping the session. In Claude Projects, this repository must be in the project's repositories so the GitHub app can reach it. Check with `claude plugin list`.

### Installed during a session

Skills installed after the session started do not appear until a new session. Until then, read the skills from a clone of this repository (`<clone>/<skill>/SKILL.md`) and run their scripts from there, with `<skills_root>` as the clone.

## Project settings

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

## Notes

- In the plugin, skill names are prefixed: `dev-skills:session-memory`.
- In the cloud, plans come with the project clone (the index is built on first use, which needs `pip install -r requirements.txt` and downloads a small embedding model). Session memory is written to the session's own environment and does not reach your local database.
- When the session starts above the repositories, `/dev-cycle` works from the only repository below it, or asks which one when there are several.
- Locally, a project with the plugin enabled shows each skill twice (from `~/.claude/skills` and from the plugin). Both read the same files.
- Projects that use Spec Kit need [uv](https://docs.astral.sh/uv/) (`pip install uv`) and network access to `github.com` in the cloud environment for `speckit_setup init`, `check` and `upgrade` (engine). The Spec Kit commands themselves run from the files committed in the project (`.specify/` and the agent command files), so a session that only specifies, plans or implements needs neither.
