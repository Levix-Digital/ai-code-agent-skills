import json
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SKILL = os.path.dirname(HERE)
sys.path.insert(0, SKILL)

from check_ci_policy import run as check_ci  # noqa: E402
from release_status import evaluate_release, next_rc_tag, project_version, read_version_file  # noqa: E402

sys.path.insert(0, os.path.join(SKILL, "..", "dev-cycle-protocol"))
from project_config import load_project_config  # noqa: E402

TEMPLATES = os.path.join(SKILL, "templates")


def repo(tmp_path, devops=None, workflows=None, files=None):
    (tmp_path / ".dev-cycle.json").write_text(json.dumps({"devops": devops or {}}), encoding="utf-8")
    wf_dir = tmp_path / ".github" / "workflows"
    wf_dir.mkdir(parents=True, exist_ok=True)
    for name, content in (workflows or {}).items():
        (wf_dir / name).write_text(content, encoding="utf-8")
    for rel, content in (files or {}).items():
        (tmp_path / rel).parent.mkdir(parents=True, exist_ok=True)
        (tmp_path / rel).write_text(content, encoding="utf-8")
    return str(tmp_path)


def template(name):
    with open(os.path.join(TEMPLATES, name), encoding="utf-8") as f:
        return f.read()


# --- check_ci_policy ---------------------------------------------------------------

def test_templates_pass_the_policy(tmp_path):
    ci = template("ci.yml").replace('echo "replace with the project\'s test_commands" && exit 1', "cargo test --workspace")
    d = repo(tmp_path, {"version_files": ["version.txt"]},
             {"ci.yml": ci, "release.yml": template("release.yml"), "tag-on-merge.yml": template("tag-on-merge.yml")},
             {"version.txt": "1.2.3\n"})
    errors, warnings = check_ci(d)
    assert errors == [] and warnings == []


def test_missing_pr_ci_into_integration_branch(tmp_path):
    wf = "on:\n  pull_request:\n    branches: [main]\njobs:\n  t:\n    runs-on: ubuntu-latest\n    steps:\n      - run: cargo test\n"
    errors, _ = check_ci(repo(tmp_path, {}, {"ci.yml": wf}))
    assert any("pull requests into 'dev'" in e for e in errors)
    assert not any("pull requests into 'main'" in e for e in errors)


def test_pull_request_without_branch_filter_covers_all(tmp_path):
    wf = "on: [pull_request]\njobs:\n  t:\n    runs-on: ubuntu-latest\n    steps:\n      - run: pytest\n"
    errors, _ = check_ci(repo(tmp_path, {}, {"ci.yml": wf}))
    assert not any("pull requests into" in e for e in errors)


def test_release_without_prerelease_and_permissions(tmp_path):
    rel = ("on:\n  push:\n    tags: ['v*']\njobs:\n  r:\n    runs-on: ubuntu-latest\n    steps:\n"
           "      - uses: softprops/action-gh-release@v2\n        with:\n          generate_release_notes: true\n")
    errors, _ = check_ci(repo(tmp_path, {}, {"release.yml": rel}))
    assert any("contents: write" in e for e in errors)
    assert any("must publish pre-releases" in e for e in errors)
    errors, _ = check_ci(repo(tmp_path, {"release_candidates": False}, {"release.yml": rel}))
    assert not any("pre-releases" in e for e in errors)


def test_prerelease_flag_computed_in_env_is_accepted(tmp_path):
    rel = """on:
  push:
    tags: ['v*']
permissions:
  contents: write
jobs:
  r:
    runs-on: ubuntu-latest
    env:
      IS_RC: ${{ contains(github.ref_name, '-rc') }}
    steps:
      - uses: softprops/action-gh-release@v2
        with:
          prerelease: ${{ env.IS_RC == 'true' }}
"""
    errors, _ = check_ci(repo(tmp_path, {}, {"release.yml": rel}))
    assert not any("pre-releases" in e for e in errors)


def test_tag_pushed_with_github_token_cannot_trigger_release(tmp_path):
    tagger = ("on:\n  push:\n    branches: [main]\npermissions:\n  contents: write\njobs:\n  t:\n    runs-on: ubuntu-latest\n"
              "    steps:\n      - run: |\n          git tag -a v1 -m v1\n          git push origin v1\n")
    errors, _ = check_ci(repo(tmp_path, {}, {"tag.yml": tagger}))
    assert any("does not trigger the release workflow" in e for e in errors)
    with_pat = tagger.replace("git push origin v1", "git push https://x:${{ secrets.RELEASE_TOKEN }}@github.com/o/r v1")
    errors, _ = check_ci(repo(tmp_path, {}, {"tag.yml": with_pat}))
    assert not any("does not trigger" in e for e in errors)


def test_missing_version_file_and_no_workflows(tmp_path):
    d = str(tmp_path)
    (tmp_path / ".dev-cycle.json").write_text(json.dumps({"devops": {"version_files": ["Cargo.toml"]}}), encoding="utf-8")
    errors, _ = check_ci(d)
    assert any("'Cargo.toml' does not exist" in e for e in errors)
    assert any("no GitHub Actions workflows" in e for e in errors)


# --- release_status ----------------------------------------------------------------

def test_read_version_from_supported_files(tmp_path):
    files = {
        "Cargo.toml": '[package]\nname = "a"\nversion = "1.2.3"\n',
        "ws/Cargo.toml": '[workspace]\nmembers = []\n[workspace.package]\nversion = "1.2.3"\n',
        "root/Cargo.toml": '[workspace]\nmembers = ["a"]\n',
        "package.json": '{"name": "a", "version": "1.2.3"}',
        "pyproject.toml": '[project]\nname = "a"\nversion = "1.2.3"\n',
        "version.txt": "v1.2.3\n",
    }
    for rel, content in files.items():
        (tmp_path / rel).parent.mkdir(parents=True, exist_ok=True)
        (tmp_path / rel).write_text(content, encoding="utf-8")
    for rel in files:
        expected = None if rel == "root/Cargo.toml" else "1.2.3"
        assert read_version_file(str(tmp_path / rel)) == expected, rel


def test_project_version_consistency(tmp_path):
    repo(tmp_path, {"version_files": ["version.txt", "Cargo.toml"]}, {},
         {"version.txt": "1.2.3", "Cargo.toml": '[package]\nversion = "1.2.4"\n'})
    version, problems = project_version(load_project_config(str(tmp_path)))
    assert version is None and any("disagree" in p for p in problems)
    (tmp_path / "Cargo.toml").write_text('[package]\nversion = "1.2.3"\n', encoding="utf-8")
    version, problems = project_version(load_project_config(str(tmp_path)))
    assert version == "1.2.3" and problems == []


def test_next_rc_tag():
    assert next_rc_tag("1.4.0", ["v1.3.0", "v1.3.1-rc.1"]) == ("v1.4.0-rc.1", None)
    assert next_rc_tag("1.4.0", ["v1.4.0-rc.1", "v1.4.0-rc.2", "v1.3.9-rc.7"]) == ("v1.4.0-rc.3", None)
    tag, problem = next_rc_tag("1.4.0", ["v1.4.0-rc.1", "v1.4.0"])
    assert tag is None and "already released" in problem


def test_next_rc_cli_with_git_tags(tmp_path):
    if not shutil.which("git"):
        return
    repo(tmp_path, {"version_files": ["version.txt"]}, {}, {"version.txt": "2.0.0"})
    git = lambda *a: subprocess.run(["git", *a], cwd=tmp_path, capture_output=True, text=True)  # noqa: E731
    git("init", "-q")
    git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "init")
    git("tag", "v2.0.0-rc.1")
    script = os.path.join(SKILL, "release_status.py")
    res = subprocess.run([sys.executable, script, "next-rc", "--dir", str(tmp_path)], capture_output=True, text=True, encoding="utf-8")
    assert res.returncode == 0 and res.stdout.strip() == "v2.0.0-rc.2"


def test_evaluate_release():
    assets = [{"name": "app-linux.tar.gz"}, {"name": "app-windows.zip"}]
    assert evaluate_release("v1.0.0", {"isPrerelease": False, "assets": assets}, ["*.zip"]) == []
    assert evaluate_release("v1.0.0-rc.1", {"isPrerelease": True, "assets": assets}, []) == []
    problems = evaluate_release("v1.0.0-rc.1", {"isPrerelease": False, "isDraft": True, "assets": []}, ["*.dmg"])
    text = "\n".join(problems)
    assert "draft" in text and "should be a pre-release" in text and "no assets" in text and "*.dmg" in text
    assert any("not a pre-release" in p for p in evaluate_release("v1.0.0", {"isPrerelease": True, "assets": assets}, []))
