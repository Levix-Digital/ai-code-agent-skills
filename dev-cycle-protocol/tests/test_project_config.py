import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SKILL = os.path.dirname(HERE)
sys.path.insert(0, SKILL)

from project_config import ARCHITECTURE_STYLES, find_manifest, load_project_config  # noqa: E402


def write_manifest(folder, data):
    path = folder / ".dev-cycle.json"
    path.write_text(json.dumps(data), encoding="utf-8")
    return path


def test_defaults_without_manifest(tmp_path):
    cfg = load_project_config(str(tmp_path))
    assert cfg.manifest_path is None
    assert cfg.project_name == tmp_path.name
    assert cfg.section("devops")["integration_branch"] == "dev"
    assert cfg.section("devops")["production_branch"] == "main"
    assert cfg.section("devops")["release_candidates"] is True
    assert cfg.section("docs")["user_doc"] == "README.md"
    assert cfg.section("tests")["manual_tests"] == "tests/MANUAL_TESTS.md"
    assert cfg.section("plans")["dir"] == "docs/plans"
    assert cfg.section("backlog") == {"file": "docs/BACKLOG.md", "blocking_severities": ["critical", "high"]}
    assert cfg.section("architecture")["style"] is None
    assert cfg.validate() == []


def test_sections_override_defaults(tmp_path):
    write_manifest(tmp_path, {
        "project_name": "demo",
        "architecture": {"style": "layered"},
        "devops": {"integration_branch": "develop", "release_candidates": False},
        "test_commands": ["pytest"],
    })
    cfg = load_project_config(str(tmp_path))
    assert cfg.project_name == "demo"
    assert cfg.section("architecture")["style"] == "layered"
    assert cfg.section("architecture")["surfaces"] == []
    assert cfg.section("devops")["integration_branch"] == "develop"
    assert cfg.section("devops")["production_branch"] == "main"
    assert cfg.section("devops")["release_candidates"] is False
    assert cfg.get("test_commands") == ["pytest"]
    assert cfg.validate() == []


def test_legacy_flat_keys_are_mapped(tmp_path):
    write_manifest(tmp_path, {
        "core_path": "src/core",
        "surfaces": ["cli"],
        "releases_repo": "org/releases",
        "plans_dir": "plans",
        "doc_files": {"user_doc": "USER.md", "tec_doc": "ARCH.md", "manual_tests": "MT.md"},
    })
    cfg = load_project_config(str(tmp_path))
    assert cfg.section("architecture")["core_path"] == "src/core"
    assert cfg.section("architecture")["surfaces"] == ["cli"]
    assert cfg.section("devops")["releases_repo"] == "org/releases"
    assert cfg.section("plans")["dir"] == "plans"
    assert cfg.section("docs") == {"user_doc": "USER.md", "tec_doc": "ARCH.md", "readme_max_lines": 400}
    assert cfg.section("tests")["manual_tests"] == "MT.md"
    assert "core_path" in cfg.legacy_keys_used
    assert "doc_files.user_doc" in cfg.legacy_keys_used


def test_new_location_wins_over_legacy_key(tmp_path):
    write_manifest(tmp_path, {
        "releases_repo": "org/old",
        "devops": {"releases_repo": "org/new"},
    })
    assert load_project_config(str(tmp_path)).section("devops")["releases_repo"] == "org/new"


def test_validation_reports_problems(tmp_path):
    write_manifest(tmp_path, {
        "architecture": {"style": "spaghetti"},
        "devops": {"observability": "telepathy", "release_candidates": "yes"},
        "test_commands": "cargo test",
        "backlog": {"blocking_severities": ["urgent"]},
        "mystery": 1,
    })
    problems = load_project_config(str(tmp_path)).validate()
    text = "\n".join(problems)
    assert "architecture.style 'spaghetti'" in text
    assert "devops.observability 'telepathy'" in text
    assert "release_candidates" in text
    assert "test_commands must be a list" in text
    assert "unknown top-level key 'mystery'" in text
    assert "backlog.blocking_severities" in text


def test_manifest_found_from_subdirectory(tmp_path):
    write_manifest(tmp_path, {"project_name": "root-project"})
    nested = tmp_path / "a" / "b"
    nested.mkdir(parents=True)
    assert find_manifest(str(nested)) == str(tmp_path / ".dev-cycle.json")
    assert load_project_config(str(nested)).project_name == "root-project"


def test_migrate_moves_legacy_keys_and_keeps_the_rest(tmp_path):
    write_manifest(tmp_path, {
        "project_name": "demo",
        "core_path": "src",
        "doc_files": {"manual_tests": "tests/MT.md"},
        "releases_repo": "org/rel",
        "drift_rules": [{"tech": "x", "conflict_msg": "no x"}],
        "architecture_docs": {"max_lines": 500},
    })
    migrated = load_project_config(str(tmp_path)).migrated()
    assert migrated == {
        "project_name": "demo",
        "architecture": {"core_path": "src"},
        "tests": {"manual_tests": "tests/MT.md"},
        "devops": {"releases_repo": "org/rel"},
        "architecture_docs": {"max_lines": 500},
        "drift_rules": [{"tech": "x", "conflict_msg": "no x"}],
    }
    assert load_project_config(str(tmp_path)).legacy_keys_used  # file itself is untouched


def test_cli_migrate_write_and_validate(tmp_path):
    write_manifest(tmp_path, {"core_path": "src", "architecture": {"style": "hexagonal"}})
    script = os.path.join(SKILL, "project_config.py")
    res = subprocess.run([sys.executable, script, "--dir", str(tmp_path), "--migrate", "--write"],
                         capture_output=True, text=True, encoding="utf-8")
    assert res.returncode == 0, res.stdout + res.stderr
    data = json.loads((tmp_path / ".dev-cycle.json").read_text(encoding="utf-8"))
    assert data == {"architecture": {"style": "hexagonal", "core_path": "src"}}
    res = subprocess.run([sys.executable, script, "--dir", str(tmp_path), "--validate"],
                         capture_output=True, text=True, encoding="utf-8")
    assert res.returncode == 0, res.stdout + res.stderr


def test_known_architecture_styles():
    assert ARCHITECTURE_STYLES == ["hexagonal", "layered", "mvc", "pipeline", "modular-monolith", "other"]


def test_product_roadmap_and_speckit_defaults(tmp_path):
    cfg = load_project_config(str(tmp_path))
    assert cfg.section("roadmap") == {"file": "docs/ROADMAP.md"}
    assert cfg.section("product")["vision"] == "docs/product/VISION.md"
    assert cfg.section("product")["tech_spec"] == "docs/product/TECH_SPEC.md"
    assert cfg.section("speckit") == {"enabled": False, "version": None, "integration": "claude",
                                      "script": None, "branch_prefix": "feat"}
    assert cfg.validate() == []


def test_speckit_validation(tmp_path):
    write_manifest(tmp_path, {"speckit": {"enabled": True, "version": "latest", "script": "bat"}})
    problems = " | ".join(load_project_config(str(tmp_path)).validate())
    assert "speckit.version 'latest' must be an exact release tag" in problems
    assert "speckit.script 'bat'" in problems
    write_manifest(tmp_path, {"speckit": {"enabled": True}})
    assert any("must be set when speckit.enabled" in p for p in load_project_config(str(tmp_path)).validate())
    write_manifest(tmp_path, {"speckit": {"enabled": True, "version": "v1.2.3", "script": "ps"}})
    assert load_project_config(str(tmp_path)).validate() == []


def test_branching_defaults_and_validation(tmp_path):
    devops = load_project_config(str(tmp_path)).section("devops")
    assert devops["branching"] == "integration" and devops["release_branch_prefix"] == "release"
    write_manifest(tmp_path, {"devops": {"branching": "trunk"}})
    assert load_project_config(str(tmp_path)).validate() == []
    write_manifest(tmp_path, {"devops": {"branching": "gitflow", "release_branch_prefix": "rel ease"}})
    text = "\n".join(load_project_config(str(tmp_path)).validate())
    assert "devops.branching 'gitflow'" in text and "release_branch_prefix" in text
