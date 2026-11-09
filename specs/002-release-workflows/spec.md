# Feature Specification: Candidate and promote workflows for the skills releases

**Feature Branch**: `feat/002-release-workflows`

**Created**: 2026-11-09

**Status**: Draft

**Input**: IMP-013, "Candidate and promote workflows for the skills releases". Plan: `docs/plans/ai_code_agent_skills_candidate_and_promote_workflows_for_the_skills_releases.md`. Needed to release v2.10.0 through the protocol's own release candidate, acceptance and release gates (decision of 2026-11-01).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - The maintainer gets a release candidate to accept (Priority: P1)

After the version bump reaches `main`, the candidate tag `vX.Y.Z-rc.N` is pushed. The maintainer finds a pre-release with that name, the command to install it and the engine version it uses, installs it from a clone of the tag and runs the manual tests of the candidate.

**Why this priority**: without a candidate there is no acceptance (Phase 9) and no release; it is the first thing v2.10.0 needs.

**Independent Test**: push `v2.10.0-rc.1` on `main` after the bump and check the published pre-release.

**Acceptance Scenarios**:

1. **Given** the version file holds 2.10.0 and the engine release of `engine-version` exists, **When** `v2.10.0-rc.1` is pushed, **Then** a pre-release `v2.10.0-rc.1` is published with the install command, the engine version and the changes since the last final release, and it is not marked latest.
2. **Given** the version file holds another version, **When** a candidate tag is pushed, **Then** nothing is published and the run says which version the file holds and which the tag needs.
3. **Given** the engine release named by `engine-version` is not published, **When** a candidate tag is pushed, **Then** nothing is published and the run says which engine release is missing.

---

### User Story 2 - The maintainer promotes the accepted candidate (Priority: P1)

Once gates `rc` and `8` pass, the maintainer starts the promotion with the accepted candidate's tag and approves it. The candidate's own commit is tagged `vX.Y.Z` and released, marked latest. The agent never starts or approves the promotion.

**Why this priority**: it is the release gate (Phase 10); it ships what was accepted, nothing else.

**Independent Test**: run the promotion for the accepted candidate and check the final tag points at the candidate's commit and the release is the latest.

**Acceptance Scenarios**:

1. **Given** an accepted candidate `v2.10.0-rc.1`, **When** the maintainer runs the promotion with it and approves it, **Then** `v2.10.0` is created on the candidate's commit and released as latest.
2. **Given** a value that is not a candidate tag, or a tag that does not exist, **When** the promotion runs, **Then** it stops before tagging and says why.
3. **Given** `v2.10.0` already exists, **When** the promotion runs again, **Then** it stops before tagging and asks for a new version.
4. **Given** a promotion started by anyone, **When** the maintainer has not approved it, **Then** nothing is tagged.

---

### User Story 3 - A maintainer knows how to cut a release (Priority: P2)

A maintainer reads the repository's maintenance guide and finds the release steps: bump, candidate, acceptance run, promotion.

**Why this priority**: the steps are short but new to this repository; without them the next release depends on memory.

**Independent Test**: follow only the guide to describe the v2.10.0 release.

**Acceptance Scenarios**:

1. **Given** the maintenance guide, **When** a maintainer reads it, **Then** it names the version file, how the candidate is tagged, where the tests are started and how the promotion is run and approved.

---

### Edge Cases

- The first final release has no earlier final release: its notes cover the whole history.
- A second candidate of the same version (`-rc.2`) lists the same changes since the last final release, not since `-rc.1`.
- Engine releases (`engine-vX.Y.Z`) are never product releases: they do not start the product release and are never the previous release for the notes.
- A final release is never published from a tag push: only the promotion creates one.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: A pushed `vX.Y.Z-rc.N` tag MUST publish a pre-release, never marked latest.
- **FR-002**: The release MUST refuse a tag that is not `vX.Y.Z` or `vX.Y.Z-rc.N`, a pushed final tag `vX.Y.Z` (only the promotion publishes one), a tag whose version differs from `.claude-plugin/plugin.json`, and a commit whose engine release (`engine-v<engine-version>`) is not published.
- **FR-003**: The release notes MUST start with the install command for the tag and the engine version, followed by the changes since the last final release.
- **FR-004**: The promotion MUST be started by hand with an existing candidate tag, wait for the maintainer's approval, tag that candidate's commit `vX.Y.Z` and publish it as the latest release in the same run.
- **FR-005**: The promotion MUST refuse an invalid or missing candidate and an existing final tag, before tagging.
- **FR-006**: The workflow policy check (`check_ci_policy`) MUST pass with no warning on this repository.
- **FR-007**: The maintenance guide MUST describe the release steps.

### Key Entities

- **Candidate**: tag `vX.Y.Z-rc.N` on `main` and its pre-release; the thing the maintainer installs and tests.
- **Final release**: tag `vX.Y.Z` on an accepted candidate's commit and its release, the latest.
- **Engine release**: `engine-vX.Y.Z`, the binaries the launchers download; published apart, a precondition of a product release.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: v2.10.0 goes from version bump to published release with no manual step besides the tag push, the acceptance tests and the maintainer's promotion and approval.
- **SC-002**: 100% of the refusals in the acceptance scenarios happen before anything is tagged or published.
- **SC-003**: The released commit of v2.10.0 is the same commit as its accepted candidate.

## Assumptions

- The skills are installed from a clone of the tag; no build or package asset is needed.
- The maintainer creates the `release` environment with themself as required reviewer (repository settings); until then a promotion would not wait for approval.
- The default workflow token is enough: the promotion calls the release in the same run, so no extra secret is needed.
- Only the trunk model applies here: no release branches and no hotfix branches until a release needs one.
