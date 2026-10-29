# Feature Specification: Rust engine with identical behavior

**Feature Branch**: `levix/rust-m<N>-<script>` (one branch per milestone)

**Created**: 2026-10-29 (the plan was approved on 2026-10-27; this envelope covers M17 to M43)

**Status**: In progress

**Input**: IMP-005, "Translate the scripts to Rust with identical behavior". Plan: `docs/plans/ai_code_agent_skills_translate_the_scripts_to_rust_with_identical_behavior.md`.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Projects keep working during and after the migration (Priority: P1)

An agent working on a project that uses the Dev Cycle Protocol runs the skills' documented commands. Whether a command is still a Python script or already runs on the native engine, the agent sees the same output, gets the same exit code and finds the same files written.

**Why this priority**: the protocol is in use by other projects; a migration that changes what they see breaks them ("O Dev cycle deve continuar sendo usado pelos projetos, sem quebrar. Mesmo depois de completamente migrado.").

**Independent Test**: run the swapped command and the Python script it replaces on the same fixtures and compare stdout, stderr, exit code and files byte for byte (the parity cases), then compare gates 0, 3, 8 and rc of the branch with main on every fixture.

**Acceptance Scenarios**:

1. **Given** a script and its Rust command, **When** both run on any parity fixture, **Then** stdout, stderr, the exit code and the files written are identical.
2. **Given** malformed input (wrong types in the frontmatter, broken JSON, missing files), **When** the Python script raises, **Then** the engine exits with the same code, prints the same stdout and ends stderr with the same exception line.
3. **Given** a swapped script, **When** its `SKILL.md`, README and every caller are read, **Then** they name the engine command (`bin/dev-cycle` or `bin/dev-cycle.ps1`) and still work on Linux, macOS and Windows.

---

### User Story 2 - No Python needed at the end (Priority: P2)

Someone installs the skills on a machine without Python and uses every command.

**Why this priority**: it is the point of the migration, but only once every script is translated (M43).

**Independent Test**: after M43, run `/dev-cycle` and gates 0, 3, rc and 8 on this repository and on a sample project with no Python on the PATH, and compare with the last Python release.

**Acceptance Scenarios**:

1. **Given** the last milestone is merged, **When** the repository is searched, **Then** no `*.py` of the skills, no `requirements.txt` and no pytest suite remain, and the parity cases run from their golden files.

---

### Edge Cases

- A Python module still imported by an untranslated script stays until its last importer is gone (for example `_store.py`, `project_config.py`, `branching.py`).
- A bug found during a translation is kept identical in Rust or fixed in both languages, never fixed in Rust only (the pickaxe date of ADR ids is kept on purpose).
- Unicode follows Python 3.13 (Unicode 15.1), through the kernel tables, not Rust's own classes.
- Python 3.10 and 3.13 differ in a few places (`tomllib`): Rust follows 3.13 and the parity cases avoid inputs where the two versions differ.
- PDF text in `folder-sync` may differ from pypdf's (D-pdf), the only accepted exception.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Each translated command MUST accept the same arguments and print argparse's usage, help and error messages exactly.
- **FR-002**: Each translated command MUST produce the same stdout, stderr, exit code and files as the Python script on the same input, on Linux, macOS and Windows.
- **FR-003**: Each milestone MUST be one pull request that translates one script, proves identity, swaps its `SKILL.md` lines and callers, and deletes the script when nothing imports it.
- **FR-004**: The proof MUST include parity cases recorded as golden files (Python 3.13 and 3.10) and a differential fuzz with no differences.
- **FR-005**: Gates 0, 3, 8 and rc MUST give the same output and exit code on the branch as on main for every fixture.
- **FR-006**: The engine MUST carry its own version (`dev-cycle-protocol/bin/engine-version`), raised on every pull request that changes the crates, and be published as `engine-vX.Y.Z` with SHA256SUMS.
- **FR-007**: Local data under `~/.ai-code-agent-memory/` MUST be read and written in the same places and formats (D-store, open before M27).
- **FR-008**: Similarity scores MUST come in the same order and within a stated tolerance (D-embed).

### Key Entities

- **Parity case**: a script, its engine command, arguments, a fixture, environment and the golden outcome (exit code, stdout, stderr, files).
- **Engine**: the `dev-cycle` binary, one crate per skill plus the kernel, reached through the launchers in `dev-cycle-protocol/bin`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 0 differences between Python and Rust in the parity cases and in at least 4,500 fuzz runs per script.
- **SC-002**: 0 differences in gates 0, 3, 8 and rc between each branch and main across all fixtures.
- **SC-003**: CI green on Linux, macOS and Windows for every milestone.
- **SC-004**: After M43, no Python file of the skills remains and a project with no Python installed passes the acceptance run.

## Assumptions

- The reference Python is 3.13; CI also runs 3.10.
- The binary is renamed to `dcp` only after the migration (ADJ-001, v2.11.0).
- Parallel work uses Gedo only; without Gedo, milestones run one after the other.
