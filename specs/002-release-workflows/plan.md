# Implementation Plan: Candidate and promote workflows for the skills releases

**Branch**: `feat/002-release-workflows` | **Date**: 2026-11-09 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/002-release-workflows/spec.md`

## Summary

Two GitHub Actions workflows adapted from `devops-release/templates`: `release.yml` publishes a pre-release for each `vX.Y.Z-rc.N` tag and, when called by the promotion, the final release; `promote.yml` is the maintainer's release gate, held by the `release` environment, which tags the accepted candidate's commit and calls the release in the same run. The release checks the tag, the version file and the engine release before publishing. The maintenance guide gains the release steps.

## Technical Context

**Language/Version**: GitHub Actions YAML, bash on `ubuntu-latest`

**Primary Dependencies**: `actions/checkout@v4`, `softprops/action-gh-release@v2` (already used by `engine.yml`), `gh` and `jq` from the runner image

**Storage**: N/A (tags and releases on GitHub)

**Testing**: `check_ci_policy` on the repository; `actionlint`-style review by reading; the real run of `v2.10.0-rc.1` (quickstart)

**Target Platform**: GitHub-hosted runners

**Project Type**: CI/release configuration of a skills repository

**Performance Goals**: N/A (one job, under a minute)

**Constraints**: no new secret; the default `GITHUB_TOKEN` only; a final tag pushed with it does not start other workflows, so the promotion calls the release itself

**Scale/Scope**: two new workflow files and one documentation section

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **II. Simplicity First**: pass. The workflows are the devops-release templates with the project's version file; no reusable action, no matrix, no packaging step (the release is the repository at its tag).
- **IV.5 Safe delivery**: pass. Candidates and the user's release gate are what this feature adds.
- **IV.6 Security**: pass. No secret; `contents: write` only; the final tag waits for the `release` environment's approval.
- **IV.8 Follow-through**: pass. The agent pushes candidate tags and follows the runs; it never runs the promotion.
- **IV.9 No AI attribution**: pass. Release notes come from pull request titles and the fixed install line.
- **Constraints**: pass. "Releases follow devops-release with the trunk model; the version lives in `.claude-plugin/plugin.json`" is exactly what the workflows implement. No paid feature (environments with required reviewers are free on public repositories, and this repository is public).
- **Architecture decisions**: no ADR contradicted; no new ADR needed (the choice follows the devops-release templates, no alternative was weighed beyond them).

Re-check after design: unchanged.

## Project Structure

### Documentation (this feature)

```text
specs/002-release-workflows/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── workflows.md
├── checklists/
│   └── requirements.md
└── tasks.md
```

### Source Code (repository root)

```text
.github/workflows/
├── ci.yml          # unchanged
├── engine.yml      # unchanged (engine-vX.Y.Z releases)
├── release.yml     # new: pre-release for vX.Y.Z-rc.N, final release when called
└── promote.yml     # new: the maintainer's release gate
docs/maintaining.md # new section "Release the skills"
```

**Structure Decision**: the workflows sit next to `ci.yml` and `engine.yml`; no code outside `.github/workflows` and the guide.

## Complexity Tracking

No violation.

## Dev Cycle Protocol

### Backlog items and release

IMP-013 (EPIC-001), in v2.10.0. Plan envelope: `docs/plans/ai_code_agent_skills_candidate_and_promote_workflows_for_the_skills_releases.md`.

### Tests (quality-assurance)

- `check_ci_policy` passes with no warning (today it warns that no promote workflow exists).
- The quickstart scenarios run for real at v2.10.0: the `-rc.1` pre-release (FR-001 to FR-003) and the promotion (FR-004, SC-003).
- No manual test is added to `tests/MANUAL_TESTS.md`: the suite tests the skills as installed; the release mechanics are checked by the release itself.

### Documentation (architecture-docs, user-docs)

`docs/maintaining.md` gains "Release the skills". README unchanged (installation is already from a clone; it names no version). No ADR.

### CI, release and observability (devops-release)

Two new workflows (above). Version impact: none by itself (v2.10.0 is bumped by its own pull request). Observability: each run prints the version checks and the release notes it publishes.

### Security, migration and rollback

No secret, no data. The promotion is held by the `release` environment with the maintainer as required reviewer (the maintainer creates it). Rollback: revert the pull request; tags and releases already published stay, and a broken release is fixed by the next patch release.
