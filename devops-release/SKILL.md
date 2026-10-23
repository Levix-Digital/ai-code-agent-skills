---
name: devops-release
description: The DevOps role of the development cycle - the project's branching model chosen from three presets (integration with main and dev, trunk with main alone, release-branches with release/X.Y), short-lived feature/fix/docs/chore branches and hotfixes, pull request CI policy, semantic version bumps, release candidates (vX.Y.Z-rc.N pre-releases for manual acceptance), the user's release gate (the merge into main, or the promotion of the accepted candidate) with automatic final tag and release, CI failure handling, rollback, and telemetry/observability requirements in plans and tests, with workflow templates and checkers for the CI policy and published releases. Use when choosing the branching model, creating branches or pull requests, changing CI/CD workflows, bumping versions, cutting a release candidate, following CI or a release, handling a hotfix, or planning logging and observability.
---

# 🚀 DevOps and Release Skill (`devops-release`)

> **Paths**: `<skills_root>` is the folder that contains this skill (the parent of this `SKILL.md`'s folder). Use the real absolute path when running the commands.

The DevOps role of the development cycle. It owns **how changes travel from a branch to a published release**, and the observability that lets the team see the software working. Settings come from the `devops` section of `.dev-cycle.json`:

```json
{ "devops": { "branching": "integration", "integration_branch": "dev", "production_branch": "main",
              "release_branch_prefix": "release", "releases_repo": "org/app-releases", "release_candidates": true,
              "version_files": ["Cargo.toml", "version.txt"], "observability": "cli" } }
```

| Phase of the cycle | What this skill does |
|---|---|
| Plan | States the CI, release and observability impact: workflows to change, logs and diagnostics to add, how they will be tested. |
| Implementation | Works on a short-lived branch from the model's work base. |
| Pull request | Opens the PR into the work base and follows CI until it is green, fixing failures. |
| Release candidate | Bumps the version, tags `vX.Y.Z-rc.N` on the model's candidate branch, follows the pipeline and verifies the pre-release. |
| Release | After **the user's release gate** (merge into production, or promotion of the accepted candidate), follows the final tag and release until every asset is verified. |
| Tests | Checks that the planned logs, diagnostics and telemetry appear. |

---

## 🌿 Branching Model

Each project chooses one of three presets in `devops.branching`, asked once by `/dev-cycle` (detected from the repository: `release/*` branches, a `dev` or `develop` branch, or `main` alone). A missing key means `integration`. Each preset answers the same questions, so the gates know where the release gate is:

| | `integration` (default) | `trunk` | `release-branches` |
|---|---|---|---|
| Long-lived branches | `main` (`production_branch`), `dev` (`integration_branch`) | `main` | `main`, `release/X.Y` (`release_branch_prefix`) |
| Work base: work branches from, pull requests into | `dev` | `main` | `main`; fixes of a release in acceptance or maintenance: `release/X.Y`, then forward into `main` |
| Release candidate `vX.Y.Z-rc.N` tagged on | `dev` | `main` | `release/X.Y`, cut from `main` after the version bump |
| Release gate (the user) | merges the pull request `dev` → `main`; `tag-on-merge.yml` creates `vX.Y.Z` and the release | runs `promote.yml` for the accepted candidate | runs `promote.yml` for the accepted candidate |
| Hotfix | `hotfix/<slug>` from `main` into `main`, then `main` merged back into `dev` | `fix/<slug>` into `main`; when `main` holds unreleased work, through `hotfix/X.Y.Z` cut from the release tag | `fix/<slug>` into `release/X.Y`, then forward into `main` |
| Fits | Products with manual acceptance that want `main` to hold only released code | Small teams and continuous delivery; work merged during acceptance goes into the next candidate (hide unfinished work behind flags) | Products that maintain several versions at once |

```powershell
python "<skills_root>/devops-release/branching.py" show        # the project's model: branches, candidate, release gate, hotfix
python "<skills_root>/devops-release/branching.py" rc-branch   # the branch the next candidate is tagged on (release/X.Y from the version)
```

Short-lived branches, whatever the model:

| Branch | Purpose |
|---|---|
| `feat/<slug>` | A feature |
| `fix/<slug>` | A bug fix |
| `docs/<slug>`, `chore/<slug>`, `refactor/<slug>`, `test/<slug>`, `ci/<slug>` | Documentation, maintenance, refactoring, tests, CI |
| `hotfix/<slug>` (`hotfix/X.Y.Z` under `trunk`) | Urgent fix of released code (see the table above) |

**Environment branches** (`staging`, `qa`, `prod`) are not a model: they drift apart and build the same code several times. Environments are deployment targets of one artifact, chosen by tag or approval.

Rules:
- One change per branch; branches live days, not weeks; rebase on or merge in the branch they target often.
- Never commit directly to a long-lived branch; always a pull request.
- Delete branches after merge.
- Commit messages follow Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `ci:`, `chore:`, `perf:`), with a scope when it helps (`fix(storage): ...`). The prefix of the branch matches the main commit type.
- **Authorship**: commits, tags, plans, pull requests, reviews, review comments and release notes are authored by the identity configured in the repository's Git (`user.name`, `user.email`). They never mention an AI, a model or a coding agent (protocol principle 9): no `Co-Authored-By` trailer, session link or "generated with" footer, and no such mention in titles, descriptions or comments. Where the agent's tool adds attribution on its own, turn it off in the tool's settings (for Claude Code, `attribution` in `.claude/settings.json`; `/dev-cycle` proposes it) and remove any that still appears. When the environment's Git identity is the agent's, set the repository's own identity first (`git config user.name` / `user.email`, as the user says).
- Branch protection on every long-lived branch (`release/*` included): pull requests required, required status checks green, no force pushes.
- **Corrections go through the pull request**: once a pull request is open, every correction of that change is a commit on its branch, answered on the thread that asked for it (`dev-cycle-protocol` Phase 7, `pr_sync.py`).

---

## ✅ Pull Request CI Policy

- The test workflow runs on **pull requests into every branch that receives them** (`integration`: `dev` and `main`; `trunk`: `main`; `release-branches`: `main` and `release/**`), and on pushes to the branches work is merged into. A pull request without checks cannot be merged.
- The checks run the project's `test_commands` on every supported platform.
- Optional external components that tests need (runtimes, models) are provisioned in CI so those tests are mandatory there.
- Concurrency: cancel superseded runs of the same pull request; a cancelled run on a push to a long-lived branch is fine when a newer push contains it.
- **CI failures are fixed, never ignored**: read the failed logs (`gh run view <id> --log-failed`), reproduce locally, fix on the branch, push, and follow the new run to green. Re-running without a change is only for a demonstrated infrastructure flake, and the flake is reported.

Check a repository against this policy:

```powershell
python "<skills_root>/devops-release/check_ci_policy.py" [--dir <project>]
```

---

## 🔢 Versioning

Semantic versioning, bumped **in the work base before the release candidate** (`release-branches`: in `main` before `release/X.Y` is cut; a patch in `release/X.Y` itself), in every file of `devops.version_files`:
- **Major**: breaking changes for users (commands, file formats, APIs, data that needs migration without compatibility).
- **Minor**: new features, schema migrations that stay compatible, notable behavior changes.
- **Patch**: bug fixes and small compatible improvements.

```powershell
python "<skills_root>/devops-release/release_status.py" version     # the version in the files, checked for consistency
python "<skills_root>/devops-release/release_status.py" next-rc     # the next rc tag for that version
```

---

## 🧪 Release Candidates

When `devops.release_candidates` is `true` (default):

1. The candidate branch (`branching.py rc-branch`) is green and its version bumped. With `release-branches`, cut `release/X.Y` from `main` first (`git branch release/1.4 main && git push -u origin release/1.4`) and protect it.
2. Tag the candidate on that branch and push it:
   ```bash
   git tag -a v1.4.0-rc.1 -m "Release candidate v1.4.0-rc.1" <candidate branch> && git push origin v1.4.0-rc.1
   ```
3. The release workflow builds the packages and publishes a **pre-release** (never the "latest" release, so normal users never receive a candidate). Follow the run to the end, then verify:
   ```powershell
   python "<skills_root>/devops-release/release_status.py" verify --tag v1.4.0-rc.1
   ```
4. `quality-assurance` starts the manual test run for the candidate; the user installs the candidate and marks the results.
5. Failures are fixed through pull requests into the candidate branch and produce `rc.2`, `rc.3`, ... until the acceptance gate passes (`release-branches`: each fix also goes forward into `main`).

## 🔒 The Release Gate: the User's Decision

- When the candidate passes acceptance, run gate 8 (`dev-cycle-protocol`) and hand the release to the user with the release notes highlights (user-visible changes, upgrade steps, known limitations).
- **The agent never merges into the production branch, never runs the promote workflow, never pushes a final tag by hand and never publishes a final release.** The user's action is the approval:

  | Model | The user's action | What follows |
  |---|---|---|
  | `integration` | Merges the pull request from the integration branch into the production branch (the agent opens it) | `templates/tag-on-merge.yml` reads the version, creates `vX.Y.Z` and calls the release |
  | `trunk`, `release-branches` | Runs the promote workflow with the accepted candidate (`gh workflow run promote.yml -f candidate=vX.Y.Z-rc.N`, or Actions > Promote), approved in the `release` environment | `templates/promote.yml` tags the candidate's own commit `vX.Y.Z` and calls the release, so what was accepted is what is released |

- Follow the workflow to the end and verify the published release:
  ```powershell
  python "<skills_root>/devops-release/release_status.py" verify --tag v1.4.0
  ```
- The cycle is complete only when the release is verified in the releases repository (`devops.releases_repo`). If it is not configured and cannot be found in the workflows, ask the user.
- Under `release-branches`, a patch of an older line never becomes the "latest" release (`make_latest: legacy` in `templates/release.yml`).

**GitHub Actions limitation**: a tag pushed with the default `GITHUB_TOKEN` does not start other workflows. The workflow that creates the final tag (tag-on-merge or promote) must either run the release in the same workflow (a reusable workflow call) or push the tag with a dedicated token (`secrets.RELEASE_TOKEN`). `check_ci_policy.py` flags the broken combination, and flags a tag-on-merge job under `trunk` or `release-branches`, where it would release every merge.

**Promote environment**: create the `release` environment with the user as required reviewer (Settings > Environments), so even a started promotion waits for the user's approval.

## 🩹 Hotfixes and Rollback

- **Hotfix**: fix with a regression test and bump the patch version, on the path of the model (table in "Branching Model"):
  - `integration`: branch `hotfix/<slug>` from the production branch, optionally cut an `rc` from it, pull request into the production branch (the user's merge releases it), then merge the production branch back into the integration branch.
  - `trunk`: a `fix/<slug>` pull request into `main`, candidate, promotion. When `main` already holds work that must not ship, create `hotfix/X.Y.Z` from the release tag, open the fix as a pull request into it (add `hotfix/**` to the CI pull request branches), tag the candidate there, promote it, then bring the fix into `main` by pull request.
  - `release-branches`: a `fix/<slug>` pull request into `release/X.Y`, candidate on it, promotion, then the fix forward into `main` (and into newer release branches that need it).
- **Rollback**: if a release is broken, revert the change through a pull request on the branch it was released from and release the next patch version; never fix forward directly on a long-lived branch without review.

---

## 📈 Observability and Telemetry

Planned in Phase 2 and tested before the candidate. Scale to `devops.observability`:

| Profile | Required |
|---|---|
| `cli` | Structured log file per component with levels; a diagnostics command (versions, paths, configuration, connectivity); actionable error messages with the log path; no secrets in logs. |
| `service` | Structured logs with request ids; metrics (latency, throughput, errors, saturation); health and readiness endpoints; traces across service calls; alerts for error rate and availability. |
| `desktop` | Log files in the platform's log folder; a "copy diagnostics" action; crash reports with the user's consent. |
| `library` | No logging by default, or through the host's logging facade; errors with context. |
| `none` | Nothing beyond actionable errors. |

For every change, the plan answers: what new failure modes exist, how an operator notices them (log line, status output, metric), and which test proves the signal appears. Telemetry that leaves the user's machine is opt-in, documented in the README, and never includes document content or credentials.

---

## ⚙️ Commands

| Command | What it does |
|---|---|
| `branching.py show [--json]` | The project's branching model: long-lived branches, where work branches start and pull requests go, where candidates are tagged, the release gate and the hotfix path. |
| `branching.py rc-branch [--version V]` | The branch the next candidate is tagged on (`release/X.Y` from the version under `release-branches`). |
| `check_ci_policy.py [--dir D]` | Checks the workflows for the branching model: tests on pull requests into every branch that receives them; pre-releases for `-rc` tags; the release gate (tag-on-merge under `integration`, a promote workflow with an environment otherwise, and no tag-on-merge there) and the `GITHUB_TOKEN` trigger limitation; `contents: write` for releasing; version files exist. Exit code 1 on errors. |
| `release_status.py version` | Reads the version from `devops.version_files` (`Cargo.toml`, `package.json`, `pyproject.toml`, `version.txt`, ...) and fails when they disagree. |
| `release_status.py next-rc` | Next `vX.Y.Z-rc.N` for the current version; fails when the final `vX.Y.Z` tag already exists (bump first). |
| `release_status.py verify --tag T [--asset PATTERN ...]` | Checks with `gh` that the release exists in `devops.releases_repo`, is a pre-release exactly when the tag has `-rc`, has assets, and has every asset pattern given. |

Workflow templates in [`templates/`](templates/): `ci.yml` (pull request CI, with the branch lists of each model), `release.yml` (build and publish, pre-release for `-rc` tags), `tag-on-merge.yml` (`integration`: final tag and release when the production branch receives the merge) and `promote.yml` (`trunk`, `release-branches`: final tag on the accepted candidate's commit and release, run by the user).
