# Claude Code Cloud Sessions (Plugin)

Cloud sessions do not see your local folders. For them, this repository is also a Claude Code marketplace (`levix-agent-skills`) with the `dev-skills` plugin, which ships the thirteen skills.

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
- Projects that use Spec Kit need [uv](https://docs.astral.sh/uv/) (`pip install uv`) and network access to `github.com` in the cloud environment for `speckit_setup.py init`, `check` and `upgrade`. The Spec Kit commands themselves run from the files committed in the project (`.specify/` and the agent command files), so a session that only specifies, plans or implements needs neither.
