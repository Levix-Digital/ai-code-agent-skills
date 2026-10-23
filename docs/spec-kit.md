# GitHub Spec Kit in the Development Cycle

[GitHub Spec Kit](https://github.com/github/spec-kit) is an optional executor of the detail of features. `dev-cycle-protocol` stays the guide: it decides when Spec Kit runs, keeps the gates and the user's decisions, and covers what Spec Kit does not (product inception, backlog and roadmap, documentation, release candidates, manual acceptance, releases).

## Where It Runs

| Phase | Spec Kit |
|---|---|
| 0. Product inception | `speckit_setup.py init`, then `constitution.py generate` |
| 1. Discovery | `/speckit-specify` creates the branch `feat/NNN-slug` and `specs/NNN-slug/spec.md`; `/speckit-clarify` until no `[NEEDS CLARIFICATION]` is left |
| 2. Plan | `/speckit-plan` (with the protocol's sections), `/speckit-tasks`, `/speckit-analyze`; the plan in `docs/plans` is the envelope of the spec folder (`save_plan.py --spec-dir`) |
| 3. Plan approval | gate 3 checks the spec folder, the branch, the setup and the constitution |
| 4–5. Implementation and tests | `/speckit-implement` executes `tasks.md` under the protocol's rules |
| 6–10 | not used |

Only feature plans of the full path use it; bug plans and the short path never do. Command names are those of the Claude Code integration; other agents use their own form (for example `/speckit.specify`).

## Setup

Requires [uv](https://docs.astral.sh/uv/) and network access to GitHub. In the project:

```bash
python ~/.ai-code-agent-skills/dev-cycle-protocol/speckit_setup.py init
python ~/.ai-code-agent-skills/dev-cycle-protocol/constitution.py generate
```

`init` installs Spec Kit at an exact version through `uvx` (never a global install), with the agent integration (`speckit.integration`, default `claude`), the script type (`speckit.script`: `sh`, or `ps` on Windows), branches `feat/NNN-slug` (`speckit.branch_prefix`) and the protocol's `dev-cycle` preset, which adds the tests, documentation, CI and release, and security and rollback sections to every Spec Kit plan. It records the version in `.dev-cycle.json`. Commit `.specify/`, the agent command files and `.dev-cycle.json`.

`speckit_setup.py status` shows the pinned and installed versions, the preset, the branch prefix and whether uv is available.

## Versions

- Each project is **pinned** to one Spec Kit release (`speckit.version`).
- The protocol records the last release it was verified with in `dev-cycle-protocol/speckit-preset/compat.json`.
- A new project takes the **latest** release when it passes the compatibility check: Spec Kit is initialized in a temporary repository and the agent verifies the version record, templates, agent commands, the `feat/NNN-slug` branch, the preset, feature resolution and the Constitution Check in the plan. The verified version then moves forward.
- When the latest release fails the check, the project pins the last verified version and the agent drafts an **adaptation document** in this repository (`docs/protocol-updates/speckit-vX.Y.Z.md`: what diverged, what the protocol must change) and captures it in this repository's backlog.
- An existing project changes versions only when you ask: `speckit_setup.py upgrade --to vX.Y.Z` runs the same check first, then upgrades the integration, the extensions and the preset, keeping files you edited.

```bash
python ~/.ai-code-agent-skills/dev-cycle-protocol/speckit_setup.py latest
python ~/.ai-code-agent-skills/dev-cycle-protocol/speckit_setup.py check --version vX.Y.Z [--record]
```

## Constitution

`.specify/memory/constitution.md` is generated from the project's sources of truth: software-design's principles and the guide of the project's architecture style, the protocol's principles, the technical specification's requirements and constraints, and the accepted ADRs. Spec Kit's Constitution Check then enforces the same rules as the cycle.

It is never edited by hand: change the source (an ADR, the technical specification, the architecture style), run `constitution.py generate` and commit the result with the change. Its header records a hash of the content and of the sources, so `constitution.py check` (and gates 0 and 3) report a hand edit or a stale file.
