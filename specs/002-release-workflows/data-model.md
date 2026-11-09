# Data Model: Candidate and promote workflows for the skills releases

| Entity | Identity | Created by | Rules |
|---|---|---|---|
| Candidate | tag `vX.Y.Z-rc.N` on `main`, pre-release of the same name | the agent pushes the tag after the version bump | version `X.Y.Z` equals `.claude-plugin/plugin.json`; engine release of `engine-version` exists; never latest |
| Final release | tag `vX.Y.Z`, release of the same name | `promote.yml`, run and approved by the maintainer | points at the accepted candidate's commit; must not exist yet; latest |
| Engine release | `engine-vX.Y.Z` | `engine.yml` on merge | precondition of a candidate; never latest; never the previous release for notes |

State: version bumped → candidate published → manual run passed (gate `rc`) → gate 8 → promotion approved → final release.
