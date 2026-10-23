# Maintaining the Repository

**Add a skill**
1. Create `<skill-name>/SKILL.md` with `name` and `description` frontmatter. The description is what makes agents load it, so say when to use it.
2. Add `!/<skill-name>/` to [`.gitignore`](../.gitignore) (allowlist).
3. To ship it in the cloud plugin, add `./<skill-name>` to `skills` in [`.claude-plugin/plugin.json`](../.claude-plugin/plugin.json) and bump `version`.
4. Add tests under `<skill-name>/tests` for every script.
5. Run `install.ps1` / `install.sh` again, then commit and push.

**Run the tests**
```bash
python -m pytest plan-memory/tests dev-cycle-protocol/tests product-inception/tests software-design/tests user-docs/tests quality-assurance/tests devops-release/tests
SPECKIT_ONLINE_TESTS=1 python -m pytest dev-cycle-protocol/tests/test_speckit_setup.py   # real Spec Kit compatibility check (network)
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
