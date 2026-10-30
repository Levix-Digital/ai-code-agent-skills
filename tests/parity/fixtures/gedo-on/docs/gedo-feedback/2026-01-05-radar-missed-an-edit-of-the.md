# Radar missed an edit of the same function

- **Id**: 2026-01-05-radar-missed-an-edit-of-the
- **Kind**: missed-collision
- **Severity**: high
- **Status**: open
- **Gedo version**: 0.4.0
- **Found**: feat/007-private-thing
- **Related**: G3, FEAT-074
- **Recorded**: 2026-01-05

## Scenario

Two threads edited parse_rust in separate worktrees

## Expected

gedo radar reports the collision

## Actual

Nothing reported until the merge
