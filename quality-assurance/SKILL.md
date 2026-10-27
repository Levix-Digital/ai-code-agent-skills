---
name: quality-assurance
description: The QA role of the development cycle - automated test strategy (unit, integration, regression test for every bug fix, run on the native runner and in CI) and one curated manual test suite organized by feature, edited in place, with Pass/Fail/Blocked result lines per run, release candidate run sheets and a script that validates the suite, reports failures and gates the release candidate. Use when planning tests for a change, writing or reviewing tests, curating MANUAL_TESTS.md, preparing a release candidate run, or reading the user's manual test results.
---

# 🧪 Quality Assurance Skill (`quality-assurance`)

> **Paths**: `<skills_root>` is the folder that contains this skill (the parent of this `SKILL.md`'s folder). Use the real absolute path when running the commands.

The QA role of the development cycle. It owns **evidence that the software works**: the automated tests and the curated manual test suite (`tests.manual_tests` in `.dev-cycle.json`, default `tests/MANUAL_TESTS.md`).

| Phase of the cycle | What this skill does |
|---|---|
| Plan | Writes the acceptance criteria and the test plan: automated tests to add, manual tests to add, change or remove. |
| Automated tests | Runs `test_commands` on the native runner; every bug fix gets a regression test; nothing moves on with a failing test. |
| Documentation | Curates the manual suite for the features the change touched. |
| Release candidate | Starts a run for the candidate (`manual_tests.py start-run`) with the affected tests. |
| User acceptance | Reads the user's results (`manual_tests.py failed`), routes failures back to fixes, and gates the candidate (`manual_tests.py gate`). |
| Bugs | Defines what a bug report holds and what proves a bug fixed (see "Bugs"). |

---

## ✅ Automated Tests

- **Run the project's commands** (`test_commands` in `.dev-cycle.json`, otherwise the stack's standard runner: `cargo test`, `pytest`, `npm test`, `go test ./...`). CI must run the same commands.
- **Pyramid**: most tests are fast unit tests of pure logic; integration tests cover boundaries (database, file system, HTTP with local fakes); end-to-end tests only for the main user paths.
- **Every bug fix adds a regression test** that fails without the fix. Write it first when possible.
- **Test behavior, not implementation**: assert on results and observable effects, not on private calls.
- **Error paths are tested**: invalid input, missing files, network failures, permission errors.
- **Deterministic**: no dependence on wall-clock timing margins, test order, real network or the developer's machine state. Use temporary directories and sandbox environment variables.
- **Flaky tests are bugs**: fix or quarantine them with an issue; never re-run until green and move on.
- **Tests that need optional external components** (models, runtimes, services) skip with a clear message locally and are made mandatory in CI where the component is provisioned.
- **Moving forward with a failing or skipped-by-mistake test is forbidden.**

---

## 📝 The Curated Manual Test Suite

Manual tests cover only what cannot be automated: real installation and updates, terminal or GUI look and feel, interaction with real external services, multi-step user journeys on a packaged build.

### Format

Use [`MANUAL_TESTS_TEMPLATE.md`](MANUAL_TESTS_TEMPLATE.md):
- `## <Feature>`: one section per user-facing feature, not per version or per change.
- `### MT-<AREA>-NNN: <title>`: a stable id (area in capitals, three-digit number) and a title saying what the user does and what must happen.
- Fields, in order: `Covers`, `Prerequisites`, `Steps` (numbered, copyable commands), `Expected` (observable results), `Result`, `Notes`.
- `Result` line: `- **Result** (<run>): [ ] Pass [ ] Fail [ ] Blocked`. The run label (for example `v1.4.0-rc.2`) says which run the result belongs to. The user marks exactly one box with `x`.

### Curation rules

1. **One suite, current behavior.** Never add version sections or "Scenario N" blocks at the end; there is no history in this file (Git keeps it).
2. **On every change**, review the tests of every feature the change touches: update steps and expected results in place, remove tests that no longer apply, merge duplicates, and add tests only for behavior not covered yet.
3. **If it can be automated, automate it** and remove the manual test.
4. **Ids are stable**: a test keeps its id while it exists; new tests take the next free number of their area; removed ids are not reused.
5. **Each test is self-contained**: prerequisites stated, no dependence on another test's leftovers.
6. **Expected results are observable** by the tester (screen, output, file, log line), never "works correctly".
7. **English only.**

### Curating a legacy suite

For a cumulative file that grew by appending scenarios:
1. Inventory every scenario and map it to a feature.
2. Check each one against the current code and CLI help: drop what no longer exists, update what changed.
3. Merge scenarios that test the same behavior; move to automated tests what can be automated.
4. Rewrite the rest in the format above with fresh ids and empty results.
5. Run `manual_tests.py validate` until it reports zero errors.

---

## 🚦 Release Candidate Runs

1. When `devops-release` publishes a candidate, start a run with the tests the release must pass: the tests of the features changed since the last release, plus the core smoke tests of the project.
   ```powershell
   python "<skills_root>/quality-assurance/manual_tests.py" start-run --run v1.4.0-rc.1 --feature "Installation" --feature "Web Sources"
   ```
   This labels those tests with the run and clears their boxes. Commit the file so the user tests from it.
2. The user installs the candidate, runs the tests and marks the results.
3. The agent reads the results:
   ```powershell
   python "<skills_root>/quality-assurance/manual_tests.py" failed --run v1.4.0-rc.1
   ```
   Each `Fail` or `Blocked` test, with its notes, becomes a fix in the candidate's branch (`devops-release`: `dev`, `main` or `release/X.Y`, by branching model; with a regression test when it can be automated), followed by a new candidate and a new run (`rc.2`) containing at least the failed tests.
   When the user decides to release without fixing a failure now, capture it in the backlog instead, so it is not lost:
   ```powershell
   python "<skills_root>/plan-memory/backlog.py" add --from-failed --run v1.4.0-rc.1 [--severity medium]
   ```
4. The candidate passes the acceptance gate when every test of the run is `Pass`:
   ```powershell
   python "<skills_root>/quality-assurance/manual_tests.py" gate --run v1.4.0-rc.1
   ```

---

## 🐞 Bugs

Bugs live in the project backlog (`plan-memory/backlog.py`, default `docs/BACKLOG.md`) until they are planned and fixed.

- **Captured bug** (backlog item `BUG-NNN`): a title that names the wrong behavior, the severity, where it was found (version, pull request, manual test) and a description with the symptom: what happens and what should happen. No hypothesis is required at capture time.
- **Severity**: `critical` (data loss, security, nothing works), `high` (a main feature fails, no workaround), `medium` (a feature fails with a workaround), `low` (cosmetic or rare). Open `critical`, `high` and `medium` bugs come before other work (`backlog.blocking_severities`); a `low` bug goes into the next release at the latest (`backlog.bug_deadline`).
- **Triage first**: a bug plan starts by judging whether the report is really a bug: expected vs. actual behavior and where the expectation is defined, a verdict (bug, works as designed, misuse or configuration, environment, feature request, duplicate), the suspected code paths and the severity confirmed. A report that is not a bug is not fixed: the user decides how to close or convert the item.
- **Planned bug** (bug plan from `backlog.py promote`): exact reproduction steps, the real log lines, the confirmed root cause, the fix design and the **regression test that fails before the fix and passes after it**. The plan is not ready for approval while the root cause is still a hypothesis.
- **Fixed**: the regression test is in the suite and green in CI, the affected manual tests are curated, and the item is closed by the plan's completion or `backlog.py close <ID> --ref "PR #N"`.
- **Bugs found while doing something else**: fix them in the same change only when they are in scope and small; otherwise capture them (`backlog.py add --type bug ...`) and report the id to the user.

---

## ⚙️ Commands

```powershell
python "<skills_root>/quality-assurance/manual_tests.py" <command> [--dir <project>] [--file <path>]
```

| Command | What it does |
|---|---|
| `validate` | Format errors: tests outside a feature, bad or duplicate ids, missing fields, malformed result lines, more than one box marked. Exit code 1 on errors. |
| `summary [--run R]` | Counts per feature and status (Pass, Fail, Blocked, not run). |
| `failed [--run R]` | Lists Fail and Blocked tests with their notes. |
| `list [--feature F]` | Lists test ids and titles. |
| `start-run --run R [--feature F ...] [--id ID ...] [--all]` | Labels the selected tests with run `R` and clears their results (writes the file). |
| `gate --run R` | Exit code 0 only when the run has tests and all of them are Pass. |
