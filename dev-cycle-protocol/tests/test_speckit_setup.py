import json
import os
import subprocess
import sys

import pytest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))

import speckit_setup as ss  # noqa: E402

GIT_CONFIG = """# Git Branching Workflow Extension Configuration
branch_numbering: sequential
branch_template: ""
branch_prefix: ""
init_commit_message: "[Spec Kit] Initial commit"
"""


def make_project(tmp_path, speckit=None, initialized=True, version="1.2.3", prefix="feat", preset=True):
    root = tmp_path / "proj"
    root.mkdir()
    subprocess.run(["git", "init", "-q"], cwd=root, check=True)
    if speckit is not None:
        (root / ".dev-cycle.json").write_text(json.dumps({"project_name": "proj", "speckit": speckit}), encoding="utf-8")
    if initialized:
        spec = root / ".specify"
        (spec / "extensions" / "git").mkdir(parents=True)
        (spec / "extensions" / "git" / "git-config.yml").write_text(
            GIT_CONFIG.replace('branch_prefix: ""', f'branch_prefix: "{prefix}"'), encoding="utf-8")
        (spec / "init-options.json").write_text(json.dumps({"speckit_version": version}), encoding="utf-8")
        if preset:
            (spec / "presets" / ss.PRESET_ID).mkdir(parents=True)
            (spec / "presets" / ss.PRESET_ID / "preset.yml").write_text("preset: {}\n", encoding="utf-8")
    return root


def test_versions_are_exact_release_tags():
    assert ss.parse_version("v1.2.3") == (1, 2, 3)
    for bad in ("1.2.3", "latest", "main", "v1.2", "v1.2.3-rc.1"):
        with pytest.raises(ss.SetupError):
            ss.parse_version(bad)
    assert ss.newest(["v1.2.3", "v1.10.0", "v1.9.9", "v2.0.0-rc.1", "main"]) == "v1.10.0"
    assert ss.newest(["main"]) is None


def test_latest_release_reads_remote_tags():
    out = "\n".join(f"abc\trefs/tags/{t}" for t in ("v1.1.0", "v1.2.3", "v1.2.10", "v2.0.0-rc.1"))
    assert ss.latest_release(lambda cmd, cwd=None: (0, out)) == "v1.2.10"
    with pytest.raises(ss.SetupError):
        ss.latest_release(lambda cmd, cwd=None: (128, "network"))


def test_cli_is_always_called_at_the_pinned_version():
    cmd = ss.init_cmd("v1.2.3", "claude", "sh")
    assert cmd[:4] == ["uvx", "--from", f"git+{ss.SPECKIT_REPO}@v1.2.3", "specify"]
    assert cmd[4:] == ["init", "--here", "--force", "--integration", "claude", "--script", "sh",
                       "--extension", "git", "--non-interactive", "--ignore-agent-tools"]
    assert ss.preset_add_cmd("v1.2.3")[-3:] == ["add", "--dev", ss.PRESET_DIR]
    upgrade = ss.upgrade_cmds("v1.3.0", "codex", "ps")
    assert [c[4:6] for c in upgrade] == [["integration", "upgrade"], ["extension", "update"], ["preset", "update"]]
    assert all("@v1.3.0" in c[2] for c in upgrade)
    with pytest.raises(ss.SetupError):
        ss.specify_cmd("latest")


def test_script_commands_per_type():
    assert ss.branch_script_cmd("sh", "x", "d")[0] == "bash"
    assert ss.branch_script_cmd("ps", "x", "d")[0] == "pwsh"
    assert ss.branch_script_cmd("py", "x", "d")[0] == sys.executable
    assert ss.setup_plan_cmd("sh")[-1] == "--json" and ss.setup_plan_cmd("ps")[-1] == "-Json"


def test_branch_prefix_is_read_and_written(tmp_path):
    root = make_project(tmp_path, prefix="")
    assert ss.read_branch_prefix(str(root)) == ""
    ss.set_branch_prefix(str(root), "feat")
    assert ss.read_branch_prefix(str(root)) == "feat"
    text = (root / ".specify" / "extensions" / "git" / "git-config.yml").read_text(encoding="utf-8")
    assert text.count("branch_prefix:") == 1 and "branch_numbering: sequential" in text


def test_speckit_section_is_merged_into_dev_cycle_json(tmp_path):
    root = make_project(tmp_path, speckit={"integration": "codex"}, initialized=False)
    path = ss.write_speckit_config(str(root), {"enabled": True, "version": "v1.2.3"})
    data = json.loads(open(path, encoding="utf-8").read())
    assert data["project_name"] == "proj"
    assert data["speckit"] == {"integration": "codex", "enabled": True, "version": "v1.2.3"}


def test_dev_cycle_json_is_created_when_missing(tmp_path):
    root = make_project(tmp_path, initialized=False)
    path = ss.write_speckit_config(str(root), {"enabled": True})
    assert os.path.basename(path) == ".dev-cycle.json"


def test_installed_version_prefers_integration_json(tmp_path):
    root = make_project(tmp_path)
    assert ss.installed_version(str(root)) == "v1.2.3"
    (root / ".specify" / "integration.json").write_text(json.dumps({"version": "1.3.0"}), encoding="utf-8")
    assert ss.installed_version(str(root)) == "v1.3.0"


def test_status_reports_a_consistent_project(tmp_path):
    root = make_project(tmp_path, speckit={"enabled": True, "version": "v1.2.3", "integration": "claude", "script": "sh"})
    data = ss.status_data(str(root))
    assert data["problems"] == []
    assert data["installed"] == "v1.2.3" and data["preset"] and data["branch_prefix"] == "feat"


def test_status_reports_drift(tmp_path):
    root = make_project(tmp_path, speckit={"enabled": True, "version": "v1.3.0"}, prefix="", preset=False)
    problems = " | ".join(ss.status_data(str(root))["problems"])
    assert "pinned v1.3.0 but the project was set up with v1.2.3" in problems
    assert "preset 'dev-cycle' is not installed" in problems
    assert "branch_prefix" in problems


def test_status_of_an_enabled_but_missing_setup(tmp_path):
    root = make_project(tmp_path, speckit={"enabled": True}, initialized=False)
    problems = ss.status_data(str(root))["problems"]
    assert any("not initialized" in p for p in problems) and any("version is not set" in p for p in problems)


def test_disabled_speckit_has_no_problems(tmp_path):
    root = make_project(tmp_path, initialized=False)
    assert ss.status_data(str(root))["problems"] == []


def test_command_files_found_for_skills_and_commands(tmp_path):
    for rel in ("claude/.claude/skills/speckit-plan/SKILL.md", "codex/.codex/prompts/speckit.plan.md",
                "codex/.codex/prompts/speckit.tasks.md"):
        path = tmp_path / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("x", encoding="utf-8")
    assert set(ss.find_command_files(str(tmp_path / "claude"))) == {"plan"}
    assert set(ss.find_command_files(str(tmp_path / "codex"))) == {"plan", "tasks"}


def test_verified_version_only_moves_forward(tmp_path, monkeypatch):
    compat = tmp_path / "compat.json"
    compat.write_text(json.dumps({"verified": "v1.2.3"}), encoding="utf-8")
    monkeypatch.setattr(ss, "COMPAT_FILE", str(compat))
    assert not ss.record_verified("v1.2.0", "claude", "sh")
    assert ss.record_verified("v1.3.0", "claude", "sh")
    assert ss.verified_version() == "v1.3.0"


def test_adaptation_document_lists_the_failed_checks(tmp_path, monkeypatch):
    monkeypatch.setattr(ss, "ADAPTATION_DIR", str(tmp_path / "docs" / "protocol-updates"))
    result = {"version": "v2.0.0", "integration": "claude", "script": "sh", "passed": False,
              "checks": [{"name": "Core templates", "ok": True, "detail": ""},
                         {"name": "Branch created as feat/NNN-slug", "ok": False, "detail": "got None"}]}
    path = ss.write_adaptation_draft(result, "v1.2.3")
    text = open(path, encoding="utf-8").read()
    assert path.endswith("speckit-v2.0.0.md")
    assert "Branch created as feat/NNN-slug**: got None" in text and "Core templates" not in text
    assert "keep using v1.2.3" in text


def test_new_project_falls_back_to_the_verified_version(tmp_path, monkeypatch):
    monkeypatch.setattr(ss, "ADAPTATION_DIR", str(tmp_path / "updates"))
    monkeypatch.setattr(ss, "verified_version", lambda: "v1.2.3")
    monkeypatch.setattr(ss, "latest_release", lambda runner=None: "v2.0.0")
    monkeypatch.setattr(ss, "compatibility_check", lambda *a, **k: {
        "version": "v2.0.0", "integration": "claude", "script": "sh", "passed": False,
        "checks": [{"name": "Agent commands", "ok": False, "detail": "missing: plan"}]})
    assert ss.choose_version(None, "claude", "sh") == "v1.2.3"
    assert (tmp_path / "updates" / "speckit-v2.0.0.md").is_file()


def test_new_project_takes_a_compatible_latest(tmp_path, monkeypatch):
    compat = tmp_path / "compat.json"
    compat.write_text(json.dumps({"verified": "v1.2.3"}), encoding="utf-8")
    monkeypatch.setattr(ss, "COMPAT_FILE", str(compat))
    monkeypatch.setattr(ss, "latest_release", lambda runner=None: "v1.3.0")
    monkeypatch.setattr(ss, "compatibility_check", lambda *a, **k: {
        "version": "v1.3.0", "integration": "claude", "script": "sh", "passed": True, "checks": []})
    assert ss.choose_version(None, "claude", "sh") == "v1.3.0"
    assert ss.verified_version() == "v1.3.0"


def test_a_requested_version_is_used_as_is():
    assert ss.choose_version("v1.2.0", "claude", "sh") == "v1.2.0"


def test_init_refuses_an_initialized_project(tmp_path, capsys):
    root = make_project(tmp_path, speckit={"enabled": True, "version": "v1.2.3"})
    assert ss.main(["init", "--dir", str(root), "--dry-run"]) == 1
    assert "already initialized" in capsys.readouterr().out


def test_upgrade_refuses_an_older_version(tmp_path, capsys):
    root = make_project(tmp_path, speckit={"enabled": True, "version": "v1.2.3"})
    assert ss.main(["upgrade", "--dir", str(root), "--to", "v1.2.0", "--dry-run"]) == 1
    assert "not newer" in capsys.readouterr().out


@pytest.mark.skipif(os.environ.get("SPECKIT_ONLINE_TESTS") != "1", reason="network: set SPECKIT_ONLINE_TESTS=1")
def test_verified_version_passes_the_real_check():
    result = ss.compatibility_check(ss.verified_version(), "claude", ss.default_script())
    assert result["passed"], result["checks"]
