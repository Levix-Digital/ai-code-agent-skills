# Maintaining the Repository

**Add a skill**
1. Create `<skill-name>/SKILL.md` with `name` and `description` frontmatter. The description is what makes agents load it, so say when to use it.
2. Add `!/<skill-name>/` to [`.gitignore`](../.gitignore) (allowlist).
3. To ship it in the cloud plugin, add `./<skill-name>` to `skills` in [`.claude-plugin/plugin.json`](../.claude-plugin/plugin.json) and bump `version`.
4. Add tests under `<skill-name>/tests` for every script.
5. Run `install.ps1` / `install.sh` again, then commit and push.

**Run the tests**
```bash
python -m pytest plan-memory/tests dev-cycle-protocol/tests quality-assurance/tests devops-release/tests
SPECKIT_ONLINE_TESTS=1 python -m pytest dev-cycle-protocol/tests/test_speckit_setup.py   # real Spec Kit compatibility check (network)
cargo test                       # the Rust workspace (crates/)
cargo build && cargo run -p parity -- run   # Python scripts and their Rust translations give the same results
```

**Translate a script to Rust** (IMP-005, ADR-0007): the translation changes no behavior.
1. Add cases for the script in `tests/parity/cases/*.json` (arguments, a fixture from `tests/parity/fixtures`, masks for clocks) and record what the Python script does: `cargo run -p parity -- record <name>`.
2. Write the command in the `dev-cycle` binary, set the case's `rust` to `["<skill>", "<script>"]` and run `cargo run -p parity -- run`: stdout, stderr, exit code and files written must be identical.
3. In the same pull request, switch the script's lines in the `SKILL.md` files and its callers to the Rust command, and delete the Python script; the golden files keep proving the behavior.
4. Bump the engine version (`dev-cycle-protocol/bin/engine-version` and `version` in `Cargo.toml`); CI refuses an engine change without it. On merge, the Engine workflow publishes release `engine-vX.Y.Z` with a binary per target.

**How the skills run the engine**: `"<skills_root>/dev-cycle-protocol/bin/dev-cycle" <skill> <script> [args]` in sh shells (Linux, macOS, Git Bash on Windows) and `& "<skills_root>/dev-cycle-protocol/bin/dev-cycle.ps1" <skill> <script> [args]` in PowerShell. The launcher uses `DEV_CYCLE_BIN` when set, else the binary of its `engine-version` from the cache (`$LOCALAPPDATA`, `$XDG_CACHE_HOME` or `~/.cache`, under `dev-cycle/`), else downloads it from the release and checks it against `SHA256SUMS`, else builds it with `cargo` when it sits in a checkout. It sets `DEV_CYCLE_SKILLS_ROOT` to its skills folder: commands that read other skills' files (the constitution reads the SKILL.md files and the style guides; `speckit_setup` reads the preset and its verified version and drafts adaptation documents) read them there, as the scripts did. Without it, the binary looks for the checkout it was built in.

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
