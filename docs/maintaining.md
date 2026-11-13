# Maintaining the Repository

**Add a skill**
1. Create `<skill-name>/SKILL.md` with `name` and `description` frontmatter. The description is what makes agents load it, so say when to use it.
2. Add `!/<skill-name>/` to [`.gitignore`](../.gitignore) (allowlist).
3. To ship it in the cloud plugin, add `./<skill-name>` to `skills` in [`.claude-plugin/plugin.json`](../.claude-plugin/plugin.json) and bump `version`.
4. Add tests for every command: unit tests in its crate, command tests in `crates/dev-cycle/tests` and parity cases in `tests/parity`.
5. Run `install.ps1` / `install.sh` again, then commit and push.

**Run the tests**
```bash
dev-cycle-protocol/bin/dev-cycle dev-cycle-protocol speckit_setup check --version <verified>   # real Spec Kit compatibility check (network); <verified> from speckit-preset/compat.json
cargo test                       # the Rust workspace (crates/), with the command tests in crates/dev-cycle/tests
cargo build && cargo run -p parity -- run   # every command still gives the results of its golden file
```

**Change a command**: the engine was translated from the Python scripts with no behavior change (IMP-005, ADR-0007), and the parity cases keep that behavior.
1. Add or change the command in its crate, with tests, and add parity cases in `tests/parity/cases/*.json` (arguments, a fixture from `tests/parity/fixtures`, masks for clocks) for a new command or a new behavior.
2. Run `cargo run -p parity -- run`: stdout, stderr, exit code and files written must match the golden files. When a behavior change is intended (a decided backlog item), record the new golden files from the engine with `cargo run -p parity -- record <name>` and review their diff in the pull request.
3. Bump the engine version (`dev-cycle-protocol/bin/engine-version` and `version` in `Cargo.toml`); CI refuses an engine change without it. On merge, the Engine workflow publishes release `engine-vX.Y.Z` with a binary per target.

**Release the skills** (trunk model, `devops-release`): a product release is `vX.Y.Z` of `.claude-plugin/plugin.json`, the repository at its tag.
1. Bump `version` in `.claude-plugin/plugin.json` by pull request into `main`. When the engine changed, its `engine-vX.Y.Z` release must be published first (the Engine workflow of that merge).
2. Tag the candidate on `main` (`release_status next-rc` gives the tag) and push it: the Release workflow publishes it as a pre-release, after checking the version file and the engine release.
3. Start the candidate's manual test run (`manual_tests start-run --run vX.Y.Z-rc.N`), commit it by pull request, and run the tests from a clone of the tag.
4. After gates `rc` and `8`, the maintainer runs Actions > Promote with the accepted candidate (`gh workflow run promote.yml -f candidate=vX.Y.Z-rc.N`). The `release` environment holds it for the maintainer's approval; it tags the candidate's commit `vX.Y.Z` and publishes the release, marked latest.

**Where releases are published**: engine and product releases go to the public releases repository [Levix-Digital/dcp-releases](https://github.com/Levix-Digital/dcp-releases), named in the repository variable `RELEASES_REPOSITORY`, with the secret `RELEASES_TOKEN` (a fine-grained token with Contents: read and write on that repository only). Without the variable, the workflows publish to this repository. A product release there carries the source archives `dev-cycle-protocol.tar.gz` and `dev-cycle-protocol.zip` (the same names in every release, for `releases/latest/download/...` links) and their `SHA256SUMS`, since the releases repository has no source. Actions > Mirror releases copies the releases this repository already has to the releases repository; it only copies what is missing, so it can be run again.

**How the skills run the engine**: `"<skills_root>/dev-cycle-protocol/bin/dev-cycle" <skill> <script> [args]` in sh shells (Linux, macOS, Git Bash on Windows) and `& "<skills_root>/dev-cycle-protocol/bin/dev-cycle.ps1" <skill> <script> [args]` in PowerShell. The launcher uses `DEV_CYCLE_BIN` when set, else the binary of its `engine-version` from the cache (`$LOCALAPPDATA`, `$XDG_CACHE_HOME` or `~/.cache`, under `dev-cycle/`), else downloads it from the `engine-vX.Y.Z` release in the releases repository (or, for versions published before it, in this repository) and checks it against `SHA256SUMS`, else builds it with `cargo` when it sits in a checkout. It sets `DEV_CYCLE_SKILLS_ROOT` to its skills folder: commands that read other skills' files (the constitution reads the SKILL.md files and the style guides; `speckit_setup` reads the preset and its verified version and drafts adaptation documents) read them there, as the scripts did. Without it, the binary looks for the checkout it was built in.

**Conventions**
- Write in English.
- Keep data out of skill folders: shared data in the project repository, personal data in `~/.ai-code-agent-memory/<skill>` with an environment variable override.
- Refer to commands as `"<skills_root>/dev-cycle-protocol/bin/dev-cycle" <skill> <command>` and to other skills by relative links (`../plan-memory/SKILL.md`).

**Personal / machine-specific skills** stay out of this repository, in `~/.ai-code-agent-skills-personal`, linked with `install.ps1 -Source`.

**Update on another machine**
```bash
git -C ~/.ai-code-agent-skills pull
```
Links pick up the changes at once. Rerun the install script only when skills were added. Cloud sessions get the new plugin version at their next start.
