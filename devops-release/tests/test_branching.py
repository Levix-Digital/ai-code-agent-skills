import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SKILL = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(SKILL, "..", "dev-cycle-protocol", "tests"))

from _engine import run_checker  # noqa: E402


def run(tmp_path, *args):
    return run_checker("devops-release", "branching.py", [*args, "--dir", str(tmp_path)])


def test_cli_show_and_rc_branch(tmp_path):
    (tmp_path / ".dev-cycle.json").write_text(json.dumps({"devops": {
        "branching": "release-branches", "version_files": ["version.txt"]}}), encoding="utf-8")
    (tmp_path / "version.txt").write_text("1.4.0\n", encoding="utf-8")
    code, out = run(tmp_path, "show")
    assert code == 0 and "release-branches" in out and "promote workflow" in out
    code, out = run(tmp_path, "rc-branch")
    assert code == 0 and out.strip() == "release/1.4"
    code, out = run(tmp_path, "rc-branch", "--version", "2.0.0")
    assert out.strip() == "release/2.0"
    code, out = run(tmp_path, "show", "--json")
    assert json.loads(out)["gate"] == "promote"
