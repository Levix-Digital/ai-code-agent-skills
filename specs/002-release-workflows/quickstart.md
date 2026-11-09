# Quickstart: validate the release workflows

Prerequisites: this feature, the v2.10.0 version bump and the engine release of `engine-version` are in `main`; the `release` environment exists with the maintainer as required reviewer.

1. Policy: `dev-cycle-protocol/bin/dev-cycle devops-release check_ci_policy` → 0 errors, 0 warnings.
2. Candidate: on `main`, `git tag -a v2.10.0-rc.1 -m "Release candidate v2.10.0-rc.1" && git push origin v2.10.0-rc.1` → the Release run passes; release `v2.10.0-rc.1` is a pre-release, not latest, and its notes start with the install line and `engine-v<version>`.
3. Refusal (optional, on a scratch fork): a tag whose version differs from `plugin.json` fails in "Check the tag, the version files and the engine release" and publishes nothing.
4. Promotion (maintainer, after gates rc and 8): Actions > Promote > `candidate: v2.10.0-rc.1` → waits for approval; after it, `v2.10.0` points at the same commit as `v2.10.0-rc.1` (`git rev-list -n 1` of both) and is the latest release.
