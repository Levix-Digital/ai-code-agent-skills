# Manual Tests

Curated manual tests of the skills repository: what the automated tests cannot prove, mainly how an agent follows the skills in a real session. Tests are edited in place and removed when they no longer apply.

How to record a run: in each test of the current run, mark exactly one box of the `Result` line with `x` and, for `Fail` or `Blocked`, describe what happened in `Notes`.

## Installation

### MT-INST-001: Install links every skill into each agent

- **Covers**: install.ps1, install.sh
- **Prerequisites**: the repository cloned at `~/.ai-code-agent-skills`
- **Steps**:
  1. Run `& "$HOME\.ai-code-agent-skills\install.ps1"` (Windows) or `~/.ai-code-agent-skills/install.sh` (macOS/Linux).
  2. Run it a second time.
  3. Open a new agent session and ask "which skills do you have?".
- **Expected**:
  - The first run shows `+` or `=` for `dev-cycle-protocol`, `product-inception`, `plan-memory`, `backlog`, `software-design`, `architecture-docs`, `quality-assurance`, `devops-release`, `user-docs`, `session-memory` and `folder-sync` in every target folder.
  - The second run shows only `=`.
  - The agent lists the ten skills.
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:

## Orchestrated Cycle

### MT-CYCLE-001: A feature request follows the full path

- **Covers**: dev-cycle-protocol, plan-memory, software-design, quality-assurance, devops-release, user-docs
- **Prerequisites**: a sample Git project with `.dev-cycle.json`, a `dev` branch and GitHub Actions
- **Steps**:
  1. Ask the agent for a small feature ("add a `--json` flag to the list command").
  2. Ask one question about the approach before approving.
  3. Approve the plan.
- **Expected**:
  - Before approval the agent edits no files; the question keeps it in Discovery.
  - The plan has the eight sections of the orchestrator's template, including design, tests, documentation and CI/release/observability, and is saved in `docs/plans`.
  - After approval the agent works on a `feat/` branch from `dev`, adds tests, updates the README and docs, opens a pull request into `dev` and follows CI to green.
  - The agent does not merge into `main`.
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:

### MT-CYCLE-002: A small fix takes the short path

- **Covers**: dev-cycle-protocol
- **Prerequisites**: the sample project
- **Steps**:
  1. Ask "fix the typo in the help text of the list command".
- **Expected**:
  - No plan file is created and no release candidate is cut.
  - The fix is made on a `fix/` or `docs/` branch with a pull request into `dev`.
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:

## Backlog

### MT-BACKLOG-001: /backlog captures one item in English

- **Covers**: backlog, plan-memory
- **Prerequisites**: a sample Git project
- **Steps**:
  1. Type `/backlog --feature "eu gostaria de exportar as respostas em Markdown"`.
  2. Type `/backlog --bug "o sync nunca termina em unidades de rede"` without a severity.
  3. Type `/backlog`.
- **Expected**:
  - Step 1 answers with one line such as `FEAT-001: Export answers to Markdown (medium)`; `docs/BACKLOG.md` has the item in English, with no added scope.
  - Step 2 answers with the new `BUG` id and says the severity was estimated.
  - Step 3 shows the summary; no plan file is created, no code changes and nothing is committed.
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:

### MT-BACKLOG-002: Bugs are captured, triaged by the user and come first

- **Covers**: dev-cycle-protocol, plan-memory, quality-assurance
- **Prerequisites**: the sample project with an open `high` bug in `docs/BACKLOG.md`
- **Steps**:
  1. Ask for a small change and let the agent notice an unrelated bug while working (or point one out as out of scope).
  2. Ask for a new feature.
  3. Ask the agent to plan the open bug.
- **Expected**:
  - The unrelated bug is captured with `backlog.py add` and its id is reported; the pull request links it instead of listing loose points to decide.
  - At discovery the agent shows the backlog and asks what to do; it promotes, drops or re-prioritizes nothing on its own.
  - The feature plan does not pass gate 3 while the high bug is open, unless you accept the risk and the reason is recorded in the plan.
  - Planning the bug runs `backlog.py promote` and produces a bug plan with reproduction, root cause and a regression test; the item becomes `planned`.
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:

### MT-BACKLOG-003: Epics, roadmap and superseded items

- **Covers**: backlog, plan-memory
- **Prerequisites**: a sample Git project
- **Steps**:
  1. Type `/backlog --epic "Photo organization"`, then `/backlog --feature "albums" --in EPIC-001` and `/backlog --feature "sharing" --in EPIC-001`.
  2. Ask the agent to plan release v0.1.0 "MVP" with the whole epic, then to show the current release.
  3. Ask the agent to replace the sharing item by two better defined items (share by link, invite relatives).
  4. Ask the agent to promote the epic.
- **Expected**:
  - Step 1 creates `EPIC-001` and two features with `- **Epic**: EPIC-001`.
  - Step 2 creates `docs/ROADMAP.md` with `v0.1.0: MVP` holding both features; `roadmap.py current` lists them.
  - Step 3 captures the two new items, closes the old one as `superseded by` them, and the roadmap (`roadmap.py validate`) asks for the new items to take its place in v0.1.0.
  - Step 4 is refused: the agent offers to promote the epic's items instead.
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:

## Product Inception

### MT-INCEPT-001: A new product from a one-line idea

- **Covers**: product-inception, dev-cycle-protocol, software-design, architecture-docs, plan-memory
- **Prerequisites**: an empty folder
- **Steps**:
  1. Write, in Portuguese: "quero um app onde famílias guardam e compartilham fotos".
  2. Answer the interview rounds (accept some suggestions, change others, leave one question open).
  3. Approve each document as it is shown, then run gate 0.
- **Expected**:
  - The agent asks at most five questions per round, each with a suggestion, and writes no product code.
  - `docs/product/VISION.md` and `docs/product/TECH_SPEC.md` are in English, with every section filled; the open question is listed under Open Questions.
  - The architecture style is set in `.dev-cycle.json`, the technical docs exist with ADRs for the founding decisions.
  - `docs/BACKLOG.md` has epics with their features; `docs/ROADMAP.md` starts with an MVP release.
  - `verify_gate.py --gate 0` passes and lists the open question; the agent asks for the baseline approval and, after it, starts Phase 1 with the first items of the MVP.
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:

### MT-INCEPT-002: An existing project adopts a product baseline

- **Covers**: product-inception
- **Prerequisites**: an existing project with a README and code, without `docs/product`
- **Steps**:
  1. Ask the agent to adopt the project into the cycle with a product baseline.
- **Expected**:
  - The documents are drafted from the code base; the agent asks only what the code cannot answer (users, business model, metrics, priorities).
  - Work in progress becomes backlog items; the roadmap starts at the version after the current one.
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:

## Spec Kit

### MT-SPECKIT-001: A feature goes through Spec Kit inside the cycle

- **Covers**: dev-cycle-protocol, plan-memory
- **Prerequisites**: a sample project with a product baseline, `uv` installed, network access
- **Steps**:
  1. Ask the agent to use Spec Kit in the project.
  2. Ask for the first feature of the current release and answer the clarification questions.
  3. Approve the plan, let the agent implement it and open the pull request.
- **Expected**:
  - Step 1 runs `speckit_setup.py init` (version pinned in `.dev-cycle.json`, preset installed) and `constitution.py generate`.
  - Step 2 creates the branch `feat/001-...` from `dev`, `specs/001-.../spec.md` without `[NEEDS CLARIFICATION]`, then `plan.md` with the "Dev Cycle Protocol" sections, `tasks.md` and the analysis; the plan in `docs/plans` has `spec_dir` and milestones from the task phases.
  - Gate 3 reports the Spec Kit artifacts complete and the constitution current.
  - Implementation follows `tasks.md`; tests, documentation and the pull request follow the protocol as usual.
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:

### MT-SPECKIT-002: An incompatible Spec Kit release falls back to the verified one

- **Covers**: dev-cycle-protocol
- **Prerequisites**: a release newer than `speckit-preset/compat.json` that fails `speckit_setup.py check` (or a temporary edit making the check fail)
- **Steps**:
  1. Run `speckit_setup.py init` in a new project.
- **Expected**:
  - The project pins the last verified version.
  - `docs/protocol-updates/speckit-<version>.md` is drafted in the skills repository with the failed checks, and the agent offers to capture it in that repository's backlog.
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:

### MT-SPECKIT-003: The constitution follows its sources

- **Covers**: dev-cycle-protocol, architecture-docs
- **Prerequisites**: the project of MT-SPECKIT-001
- **Steps**:
  1. Accept a new ADR, then run gate 3 on a new plan.
  2. Edit `.specify/memory/constitution.md` by hand and run gate 3 again.
- **Expected**:
  - Step 1: gate 3 reports the constitution as stale until `constitution.py generate` is run; the regenerated file lists the ADR and its version moves to `1.1.0`.
  - Step 2: gate 3 reports the hand edit; regenerating replaces it.
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:

## Release Candidates

### MT-RC-001: A release candidate is published and its run prepared

- **Covers**: devops-release, quality-assurance
- **Prerequisites**: the sample project with the `devops-release` templates installed and a curated `tests/MANUAL_TESTS.md`
- **Steps**:
  1. Ask the agent to prepare a release.
- **Expected**:
  - The version is bumped in `dev` through a pull request and `release_status.py next-rc` gives the tag used.
  - The `-rc` tag publishes a pre-release (not marked latest) and `release_status.py verify` reports it verified.
  - `manual_tests.py start-run` labels the affected tests with the candidate and the agent explains how to install it and which tests to run.
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:

### MT-RC-002: A failed manual test goes back to a fix and blocks the release

- **Covers**: quality-assurance, dev-cycle-protocol
- **Prerequisites**: MT-RC-001 done
- **Steps**:
  1. Mark one test of the run as `[x] Fail` with a note, the others as Pass.
  2. Tell the agent the tests are done.
- **Expected**:
  - The agent reads the failure with `manual_tests.py failed`, fixes it in `dev` with a regression test and cuts the next candidate.
  - `verify_gate.py --gate rc --run <first rc>` fails and the agent does not open the release pull request for it.
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:

### MT-RC-003: The user's merge releases the accepted candidate

- **Covers**: devops-release
- **Prerequisites**: a candidate whose run is all Pass
- **Steps**:
  1. Let the agent open the pull request from `dev` into `main`.
  2. Merge it yourself.
- **Expected**:
  - `verify_gate.py --gate 8 --run <rc>` passes before the pull request is opened.
  - The merge creates the final tag and release without manual steps.
  - The agent follows the workflow and reports the release verified with `release_status.py verify --tag vX.Y.Z`.
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:
