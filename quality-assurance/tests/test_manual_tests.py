import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SKILL = os.path.dirname(HERE)
sys.path.insert(0, SKILL)

from manual_tests import Suite  # noqa: E402

SUITE = """# Manual Tests

Intro text.

## Installation

### MT-INST-001: Fresh install puts actx on PATH

- **Covers**: installer
- **Prerequisites**: clean machine
- **Steps**:
  1. Run the installer:
     ```bash
     ### not a heading inside code
     ./install.sh
     ```
- **Expected**:
  - `actx --version` prints the version
- **Result** (v1.0.0-rc.1): [x] Pass [ ] Fail [ ] Blocked
- **Notes**:

## Web Sources

### MT-WEB-001: Re-sync does not duplicate chunks

- **Covers**: ingestion
- **Steps**:
  1. Sync twice
- **Expected**:
  - Same chunk count
- **Result** (v1.0.0-rc.1): [ ] Pass [x] Fail [ ] Blocked
- **Notes**: count grew from 120 to 240
  after the second sync

### MT-WEB-002: Unreachable site is reported

- **Covers**: crawler
- **Steps**:
  1. Add an unreachable URL
- **Expected**:
  - A warning in the footer
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:
"""


def write(tmp_path, text=SUITE):
    tests_dir = tmp_path / "tests"
    tests_dir.mkdir(exist_ok=True)
    path = tests_dir / "MANUAL_TESTS.md"
    path.write_text(text, encoding="utf-8")
    return path


def cli(tmp_path, *args):
    script = os.path.join(SKILL, "manual_tests.py")
    return subprocess.run([sys.executable, script, *args, "--dir", str(tmp_path)],
                          capture_output=True, text=True, encoding="utf-8")


def test_parse_features_ids_results_and_notes():
    s = Suite("x.md", SUITE)
    assert [t.id for t in s.tests] == ["MT-INST-001", "MT-WEB-001", "MT-WEB-002"]
    inst, web1, web2 = s.tests
    assert inst.feature == "Installation" and inst.run == "v1.0.0-rc.1" and inst.status == "Pass"
    assert web1.status == "Fail" and s.notes_text(web1) == "count grew from 120 to 240 after the second sync"
    assert web2.run is None and web2.status is None
    assert s.validate() == []


def test_validation_problems():
    bad = SUITE.replace("MT-WEB-002", "MT-WEB-001").replace(
        "- **Result**: [ ] Pass [ ] Fail [ ] Blocked", "- **Result**: [x] Pass [x] Fail [ ] Blocked").replace(
        "- **Covers**: crawler\n", "") + "\n### TEST-9: bad id\n\n- **Result**: Pass\n"
    problems = "\n".join(Suite("x.md", bad).validate())
    assert "duplicate id" in problems
    assert "more than one result box is marked" in problems
    assert "missing field 'Covers'" in problems
    assert "id must look like MT-<AREA>-NNN" in problems
    assert "Result line must be" in problems


def test_test_outside_feature_is_an_error():
    problems = Suite("x.md", "# T\n\n### MT-A-001: x\n- **Covers**: a\n").validate()
    assert any("not under a '## <Feature>' section" in p for p in problems)


def test_summary_failed_and_gate(tmp_path):
    write(tmp_path)
    res = cli(tmp_path, "summary", "--run", "v1.0.0-rc.1")
    assert "2 test(s)" in res.stdout and "Pass 1" in res.stdout and "Fail 1" in res.stdout
    res = cli(tmp_path, "failed", "--run", "v1.0.0-rc.1")
    assert res.returncode == 1 and "MT-WEB-001 [Fail]" in res.stdout and "count grew" in res.stdout
    res = cli(tmp_path, "gate", "--run", "v1.0.0-rc.1")
    assert res.returncode == 1 and "1/2 Pass" in res.stdout
    assert cli(tmp_path, "gate", "--run", "v9.9.9-rc.1").returncode == 1


def test_start_run_labels_clears_and_keeps_other_tests(tmp_path):
    path = write(tmp_path)
    res = cli(tmp_path, "start-run", "--run", "v1.0.0-rc.2", "--feature", "Web Sources")
    assert res.returncode == 0, res.stdout
    s = Suite(str(path), path.read_text(encoding="utf-8"))
    inst, web1, web2 = s.tests
    assert inst.run == "v1.0.0-rc.1" and inst.status == "Pass"          # untouched
    assert web1.run == "v1.0.0-rc.2" and web1.status is None
    assert web2.run == "v1.0.0-rc.2" and web2.status is None
    assert s.notes_text(web1) == ""                                       # old notes cleared
    assert "after the second sync" not in path.read_text(encoding="utf-8")
    assert "### not a heading inside code" in path.read_text(encoding="utf-8")
    assert s.validate() == []


def test_gate_passes_when_all_marked_pass(tmp_path):
    path = write(tmp_path)
    cli(tmp_path, "start-run", "--run", "v1.0.0-rc.2", "--id", "MT-WEB-001")
    text = path.read_text(encoding="utf-8").replace(
        "- **Result** (v1.0.0-rc.2): [ ] Pass [ ] Fail [ ] Blocked",
        "- **Result** (v1.0.0-rc.2): [x] Pass [ ] Fail [ ] Blocked")
    path.write_text(text, encoding="utf-8")
    res = cli(tmp_path, "gate", "--run", "v1.0.0-rc.2")
    assert res.returncode == 0 and "all 1 test(s) Pass" in res.stdout


def test_start_run_rejects_unknown_selection_and_preserves_crlf(tmp_path):
    path = write(tmp_path, SUITE.replace("\n", "\r\n"))
    before = path.read_bytes()
    res = cli(tmp_path, "start-run", "--run", "r1", "--feature", "Nope")
    assert res.returncode == 1 and "Not found: Nope" in res.stdout
    assert path.read_bytes() == before
    assert cli(tmp_path, "start-run", "--run", "r1", "--all").returncode == 0
    assert b"\r\n" in path.read_bytes() and b"\n\n" not in path.read_bytes().replace(b"\r\n", b"")


def test_validate_cli_and_missing_file(tmp_path):
    assert cli(tmp_path, "validate").returncode == 1  # no suite yet
    write(tmp_path)
    res = cli(tmp_path, "validate")
    assert res.returncode == 0 and "3 test(s), 0 problem(s)" in res.stdout
