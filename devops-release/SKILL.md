---
name: devops-release
description: The DevOps role of the development cycle - branching model (main, dev, short-lived feature/fix/docs/chore branches, hotfixes), pull request CI policy, semantic version bumps, release candidates (vX.Y.Z-rc.N pre-releases for manual acceptance), the user's merge into main as the release gate with automatic final tag and release, CI failure handling, rollback, and telemetry/observability requirements in plans and tests, with workflow templates and checkers for the CI policy and published releases. Use when creating branches or pull requests, changing CI/CD workflows, bumping versions, cutting a release candidate, following CI or a release, handling a hotfix, or planning logging and observability.
---

# 🚀 DevOps and Release Skill (`devops-release`)

> **Paths**: `<skills_root>` is the folder that contains this skill (the parent of this `SKILL.md`'s folder). Use the real absolute path when running the commands.

The DevOps role of the development cycle. It owns **how changes travel from a branch to a published release**, and the observability that lets the team see the software working. Settings come from the `devops` section of `.dev-cycle.json`:

```json
{ "devops": { "integration_branch": "dev", "production_branch": "main", "releases_repo": "org/app-releases",
              "release_candidates": true, "version_files": ["Cargo.toml", "version.txt"], "observability": "cli" } }
```

| Phase of the cycle | What this skill does |
|---|---|
| Plan | States the CI, release and observability impact: workflows to change, logs and diagnostics to add, how they will be tested. |
| Implementation | Works on a short-lived branch from the integration branch. |
| Pull request | Opens the PR into the integration branch and follows CI until it is green, fixing failures. |
| Release candidate | Bumps the version, tags `vX.Y.Z-rc.N`, follows the pipeline and verifies the pre-release. |
| Release | After **the user merges** the integration branch into production, follows the final tag and release until every asset is verified. |
| Tests | Checks that the planned logs, diagnostics and telemetry appear. |

---

## 🌿 Branching Model

| Branch | Purpose | Created from | Merged into |
|---|---|---|---|
| `main` (`production_branch`) | Released code; final tags `vX.Y.Z` | — | — |
| `dev` (`integration_branch`) | Integration of finished work; release candidates are cut here | `main` | `main` (by the user, through a pull request) |
| `feat/<slug>` | A feature | `dev` | `dev` |
| `fix/<slug>` | A bug fix | `dev` | `dev` |
| `docs/<slug>`, `chore/<slug>`, `refactor/<slug>`, `test/<slug>`, `ci/<slug>` | Documentation, maintenance, refactoring, tests, CI | `dev` | `dev` |
| `hotfix/<slug>` | Urgent fix of released code | `main` | `main`, then `main` is merged back into `dev` |

Rules:
- One change per branch; branches live days, not weeks; rebase or merge the integration branch in often.
- Never commit directly to `main` or `dev`; always a pull request.
- Delete branches after merge.
- Commit messages follow Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `ci:`, `chore:`, `perf:`), with a scope when it helps (`fix(storage): ...`). The prefix of the branch matches the main commit type.
- **Authorship**: commits, tags, plans, pull requests and release notes are authored by the identity configured in the repository's Git (`user.name`, `user.email`). They never name a coding agent or AI tool as author or co-author: no `Co-Authored-By` trailer, session link or "generated with" footer for the agent. When the environment's Git identity is the agent's, set the repository's own identity first (`git config user.name` / `user.email`, as the user says).
- Branch protection on `main` and `dev`: pull requests required, required status checks green, no force pushes.

---

## ✅ Pull Request CI Policy

- The test workflow runs on **pull requests into both the integration and the production branch**, and on pushes to the integration branch. A pull request without checks cannot be merged.
- The checks run the project's `test_commands` on every supported platform.
- Optional external components that tests need (runtimes, models) are provisioned in CI so those tests are mandatory there.
- Concurrency: cancel superseded runs of the same pull request; a cancelled run on a push to the integration branch is fine when a newer push contains it.
- **CI failures are fixed, never ignored**: read the failed logs (`gh run view <id> --log-failed`), reproduce locally, fix on the branch, push, and follow the new run to green. Re-running without a change is only for a demonstrated infrastructure flake, and the flake is reported.

Check a repository against this policy:

```powershell
python "<skills_root>/devops-release/check_ci_policy.py" [--dir <project>]
```

---

## 🔢 Versioning

Semantic versioning, bumped **in the integration branch before the release candidate**, in every file of `devops.version_files`:
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

1. The integration branch is green and its version bumped.
2. Tag the candidate on the integration branch and push it:
   ```bash
   git tag -a v1.4.0-rc.1 -m "Release candidate v1.4.0-rc.1" && git push origin v1.4.0-rc.1
   ```
3. The release workflow builds the packages and publishes a **pre-release** (never the "latest" release, so normal users never receive a candidate). Follow the run to the end, then verify:
   ```powershell
   python "<skills_root>/devops-release/release_status.py" verify --tag v1.4.0-rc.1
   ```
4. `quality-assurance` starts the manual test run for the candidate; the user installs the candidate and marks the results.
5. Failures are fixed in the integration branch and produce `rc.2`, `rc.3`, ... until the acceptance gate passes.

## 🔒 The Release Gate: the User's Merge

- When the candidate passes acceptance, open the pull request from the integration branch into the production branch, with the release notes highlights (user-visible changes, upgrade steps, known limitations).
- **The agent never merges into the production branch, never pushes a final tag by hand and never publishes a final release.** The user's merge is the approval.
- The merge triggers the final tag `vX.Y.Z` and the release (see `templates/tag-on-merge.yml`). Follow the workflow to the end and verify the published release:
  ```powershell
  python "<skills_root>/devops-release/release_status.py" verify --tag v1.4.0
  ```
- The cycle is complete only when the release is verified in the releases repository (`devops.releases_repo`). If it is not configured and cannot be found in the workflows, ask the user.

**GitHub Actions limitation**: a tag pushed with the default `GITHUB_TOKEN` does not start other workflows. The tag-on-merge workflow must either run the release in the same workflow (a reusable workflow call) or push the tag with a dedicated token (`secrets.RELEASE_TOKEN`). `check_ci_policy.py` flags the broken combination.

## 🩹 Hotfixes and Rollback

- **Hotfix**: branch `hotfix/<slug>` from the production branch, fix with a regression test, bump the patch version, optionally cut an `rc` from the hotfix branch, pull request into the production branch (the user's merge releases it), then merge the production branch back into the integration branch.
- **Rollback**: if a release is broken, revert the merge on the production branch through a pull request and release the next patch version; never fix forward directly on the production branch without review.

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
| `check_ci_policy.py [--dir D]` | Checks the workflows: tests on pull requests into the integration and production branches; pre-releases for `-rc` tags; a tag-on-merge job and the `GITHUB_TOKEN` trigger limitation; `contents: write` for releasing; version files exist. Exit code 1 on errors. |
| `release_status.py version` | Reads the version from `devops.version_files` (`Cargo.toml`, `package.json`, `pyproject.toml`, `version.txt`, ...) and fails when they disagree. |
| `release_status.py next-rc` | Next `vX.Y.Z-rc.N` for the current version; fails when the final `vX.Y.Z` tag already exists (bump first). |
| `release_status.py verify --tag T [--asset PATTERN ...]` | Checks with `gh` that the release exists in `devops.releases_repo`, is a pre-release exactly when the tag has `-rc`, has assets, and has every asset pattern given. |

Workflow templates in [`templates/`](templates/): `ci.yml` (pull request CI), `release.yml` (build and publish, pre-release for `-rc` tags) and `tag-on-merge.yml` (final tag and release when the production branch receives the merge).
