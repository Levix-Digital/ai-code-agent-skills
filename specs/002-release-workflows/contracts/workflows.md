# Contract: release workflows

## release.yml

- Triggers: `push` of a tag matching `v*`; `workflow_call` with input `tag` (string, required).
- Accepts: `^v[0-9]+\.[0-9]+\.[0-9]+(-rc\.[0-9]+)?$`. Anything else fails before publishing, and so does a final `vX.Y.Z` reached by a tag push: only the promotion's call publishes a final release.
- Fails before publishing when `.claude-plugin/plugin.json` `version` ≠ the tag's `X.Y.Z`, or `engine-v<engine-version>` is not a release of the repository.
- Publishes: release named and tagged as the input; `prerelease` for `-rc`; latest only for final tags; body = install line, engine line, generated notes since the last final tag.

## promote.yml

- Trigger: `workflow_dispatch` with input `candidate` (string, optional).
- Environment: `release` (the maintainer's approval).
- With `candidate`: must match `^v[0-9]+\.[0-9]+\.[0-9]+-rc\.[0-9]+$` and exist; the final tag is `vX.Y.Z` on the candidate's commit.
- Without `candidate`: the final tag is `v<plugin.json version>` on the run's commit.
- Fails before tagging when the final tag already exists.
- Then calls `release.yml` with the final tag.
