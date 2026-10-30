import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from _engine import run_engine  # noqa: E402

README = """# Demo

Demo answers questions about your notes.

## Installation

Run the installer.

## Usage

Run `demo ask`.

## Features

- Answers

## Requirements

- Any OS

## Configuration

None.

## Troubleshooting

None.

## Documentation

See the docs.

## License

MIT.
"""

SUITE = """# Manual Tests

## Basics

### MT-BASIC-001: Demo starts

- **Covers**: cli
- **Steps**:
  1. Run `demo`
- **Expected**:
  - A prompt appears
- **Result** (v1.0.0-rc.1): [{mark}] Pass [ ] Fail [ ] Blocked
- **Notes**:
"""


def make_project(tmp_path, readme=README, mark="x", devops=None):
    (tmp_path / "README.md").write_text(readme, encoding="utf-8")
    (tmp_path / "TECDOC.md").write_text("# Tech\n", encoding="utf-8")
    (tmp_path / "tests").mkdir(exist_ok=True)
    (tmp_path / "tests" / "MANUAL_TESTS.md").write_text(SUITE.format(mark=mark), encoding="utf-8")
    (tmp_path / "version.txt").write_text("1.0.0\n", encoding="utf-8")
    config = {"architecture": {"style": "layered"}, "devops": devops or {"version_files": ["version.txt"]}}
    (tmp_path / ".dev-cycle.json").write_text(json.dumps(config), encoding="utf-8")
    return str(tmp_path)


def gate(project, *args):
    return run_engine("dev-cycle-protocol", "verify_gate", ["--dir", project, *args])


def test_gate_rc_requires_run_and_all_pass(tmp_path):
    d = make_project(tmp_path)
    assert gate(d, "--gate", "rc").returncode == 1
    ok = gate(d, "--gate", "rc", "--run", "v1.0.0-rc.1")
    assert ok.returncode == 0 and "CANDIDATE ACCEPTED" in ok.stdout
    assert gate(d, "--gate", "rc", "--run", "v1.0.0-rc.9").returncode == 1


def test_gate_rc_fails_when_a_test_is_not_pass(tmp_path):
    d = make_project(tmp_path, mark=" ")
    res = gate(d, "--gate", "rc", "--run", "v1.0.0-rc.1")
    assert res.returncode == 1 and "NOT ACCEPTED" in res.stdout


def test_gate_8_passes_on_a_compliant_project(tmp_path):
    d = make_project(tmp_path)
    res = gate(d, "--gate", "8", "--run", "v1.0.0-rc.1")
    assert res.returncode == 0, res.stdout
    assert "README check (user-docs) passed" in res.stdout
    assert "Manual test suite (quality-assurance) passed" in res.stdout
    assert "Version files (devops-release) passed" in res.stdout
    assert "Release candidate run v1.0.0-rc.1" in res.stdout


def test_gate_8_blocks_on_a_changelog_readme(tmp_path):
    d = make_project(tmp_path, readme=README + "\n## What's New\n\n- Faster\n")
    res = gate(d, "--gate", "8")
    assert res.returncode == 1 and "README check (user-docs) failed" in res.stdout


def test_gate_8_blocks_on_failed_candidate_and_bad_versions(tmp_path):
    d = make_project(tmp_path, mark=" ", devops={"version_files": ["version.txt", "missing.toml"]})
    res = gate(d, "--gate", "8", "--run", "v1.0.0-rc.1")
    assert res.returncode == 1
    assert "Version files (devops-release) failed" in res.stdout
    assert "Release candidate run v1.0.0-rc.1 (quality-assurance) failed" in res.stdout


def test_gate_8_without_run_only_warns_about_candidate(tmp_path):
    d = make_project(tmp_path)
    res = gate(d, "--gate", "8")
    assert res.returncode == 0 and "no --run was given" in res.stdout


def test_gate_8_names_the_release_gate_of_the_branching_model(tmp_path):
    d = make_project(tmp_path)
    assert "READY FOR THE RELEASE PULL REQUEST" in gate(d, "--gate", "8", "--run", "v1.0.0-rc.1").stdout
    make_project(tmp_path, devops={"branching": "trunk", "version_files": ["version.txt"]})
    res = gate(d, "--gate", "8", "--run", "v1.0.0-rc.1")
    assert res.returncode == 0, res.stdout
    assert "READY FOR THE PROMOTION (the user runs the promote workflow for v1.0.0-rc.1)" in res.stdout
    assert "Tag v1.0.0-rc.1 not found locally" in res.stdout
