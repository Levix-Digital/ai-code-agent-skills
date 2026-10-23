import json
import os
import subprocess
import sys

import pytest

HERE = os.path.dirname(os.path.abspath(__file__))
SKILL = os.path.dirname(HERE)
sys.path.insert(0, SKILL)

from branching import BranchingModel, detect  # noqa: E402

SCRIPT = os.path.join(SKILL, "branching.py")


def model(**devops):
    return BranchingModel.from_devops(devops)


def test_integration_is_the_default():
    m = model()
    assert (m.name, m.integration, m.production, m.gate) == ("integration", "dev", "main", "merge")
    assert m.pr_targets == ["dev", "main"] and m.rc_branch("1.4.0") == "dev"
    assert model(integration_branch="develop").rc_branch() == "develop"


def test_trunk_works_on_the_production_branch():
    m = model(branching="trunk", integration_branch="dev", production_branch="trunk")
    assert m.integration == "trunk" and m.long_lived == ["trunk"]
    assert m.pr_targets == ["trunk"] and m.rc_branch("1.4.0") == "trunk" and m.gate == "promote"


def test_release_branches_cut_a_branch_per_minor_version():
    m = model(branching="release-branches")
    assert m.integration == "main" and m.gate == "promote"
    assert m.pr_targets == ["main", "release/*"]
    assert m.rc_branch("1.4.0") == "release/1.4" and m.rc_branch("v2.0.3") == "release/2.0"
    assert model(branching="release-branches", release_branch_prefix="maint").release_branch("3.1.0") == "maint/3.1"
    with pytest.raises(ValueError):
        m.rc_branch(None)


def test_unknown_model_is_refused():
    with pytest.raises(ValueError, match="gitflow"):
        model(branching="gitflow")


def test_every_model_describes_each_question():
    for name in ("integration", "trunk", "release-branches"):
        data = model(branching=name).describe()
        assert set(data) >= {"long_lived", "work", "pull_requests_into", "release_candidate", "release_gate", "hotfix"}


def test_detect_from_branch_names():
    assert detect(["main", "origin/release/1.2"])["model"] == "release-branches"
    found = detect(["main", "origin/develop", "feat/x"])
    assert found["model"] == "integration" and found["integration_branch"] == "develop"
    assert detect(["main", "origin/main", "fix/y"])["model"] == "trunk"
    assert detect([])["model"] is None


def run(tmp_path, *args):
    res = subprocess.run([sys.executable, SCRIPT, *args, "--dir", str(tmp_path)], capture_output=True, text=True,
                         encoding="utf-8")
    return res.returncode, res.stdout


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
