# Research: Candidate and promote workflows for the skills releases

## R1. What a release of the skills contains

- **Decision**: the repository at its tag, with notes; no asset.
- **Rationale**: the skills are installed by cloning and running `install.sh` or `install.ps1`; the engine binaries already have their own releases, downloaded by the launchers.
- **Alternatives considered**: a zip of the skill folders (duplicates GitHub's source archives); bundling the engine binaries (duplicates `engine-vX.Y.Z` and grows each release by six binaries).

## R2. How the final tag starts the release

- **Decision**: `promote.yml` creates the tag and calls `release.yml` as a reusable workflow in the same run.
- **Rationale**: a tag pushed with `GITHUB_TOKEN` does not start other workflows; the call needs no extra secret (`devops-release`, "GitHub Actions limitation").
- **Alternatives considered**: a `RELEASE_TOKEN` personal access token (a secret to rotate, rejected).

## R3. Release notes

- **Decision**: GitHub's generated notes from the last final `vX.Y.Z` (found with `git tag --sort=-v:refname`), with the install command and the engine version on top; the whole history for the first release.
- **Rationale**: without `previous_tag_name`, GitHub picks the previous release, which can be an `engine-v*` release; naming the last final tag keeps every candidate of a version on the same changes.
- **Alternatives considered**: a committed release notes file per version (history in the repository, against "documentation describes today").

## R4. Latest release

- **Decision**: candidates `make_latest: false`, final releases `true`.
- **Rationale**: trunk model only, so a final release is always the newest; `legacy` compares by date and version and could be confused by the engine releases created after it.
- **Alternatives considered**: `legacy`, as in the template (kept for release branches, which this repository does not use).

## R5. Preconditions checked by the release

- **Decision**: tag format, `plugin.json` version equal to the tag's, and the `engine-v<engine-version>` release exists.
- **Rationale**: a release whose launchers point at a missing engine would fail on first use for every user; the version check is the same `release_status` asks for.
