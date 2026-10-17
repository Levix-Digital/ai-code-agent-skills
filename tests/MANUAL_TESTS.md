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
  - The first run shows `+` or `=` for `dev-cycle-protocol`, `plan-memory`, `backlog`, `software-design`, `architecture-docs`, `quality-assurance`, `devops-release`, `user-docs`, `session-memory` and `folder-sync` in every target folder.
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
