import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SKILL = os.path.dirname(HERE)
SCRIPT = os.path.join(SKILL, "verify_gate.py")
sys.path.insert(0, SKILL)
sys.path.insert(0, os.path.join(os.path.dirname(SKILL), "plan-memory"))

from _store import PlanStore  # noqa: E402
from test_verify_gate import make_project  # noqa: E402
from verify_gate import run_checker  # noqa: E402

GIT_CONFIG = """# Git Branching Workflow Extension Configuration
branch_numbering: sequential
branch_template: ""
branch_prefix: ""
init_commit_message: "[Spec Kit] Initial commit"
"""

PLAN = """# Implementation Plan: Photo upload

## Summary

Upload photos.

## Dev Cycle Protocol

### Tests (quality-assurance)

Unit tests.
"""

ROADMAP = """# Roadmap

## v1.0.0: Launch
- **Status**: in_progress
- **Goal**: Ship.
- **Items**: FEAT-001, FEAT-002
"""

BACKLOG = """# Backlog

## Bugs

## Features

### FEAT-001: Upload
- **Priority**: high
- **Status**: open
- **Found**: 2026-10-23
- **Plan**: -
- **Description**: Upload.

## Improvements

## Adjustments

## Closed

### FEAT-002: Login
- **Priority**: high
- **Status**: done
- **Found**: 2026-10-01
- **Plan**: login
- **Closed**: 2026-10-10, PR #3
- **Description**: Login.
"""


def git(root, *args):
    subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=root, check=True, capture_output=True)


def speckit_project(tmp_path, branch="feat/001-photo-upload", spec_files=("spec.md", "plan.md", "tasks.md"),
                    spec_text="# Spec\n", plan_text=PLAN, spec_dir="specs/001-photo-upload"):
    root = tmp_path
    git(root, "init", "-q", "-b", "dev")
    git(root, "commit", "-q", "--allow-empty", "-m", "init")
    git(root, "checkout", "-q", "-b", branch)
    cfg = {"project_name": "demo", "architecture": {"style": "layered"},
           "speckit": {"enabled": True, "version": "v1.2.3", "integration": "claude", "script": "sh"}}
    (root / ".dev-cycle.json").write_text(json.dumps(cfg), encoding="utf-8")
    spec = root / ".specify"
    (spec / "extensions" / "git").mkdir(parents=True)
    (spec / "extensions" / "git" / "git-config.yml").write_text(GIT_CONFIG.replace('branch_prefix: ""', 'branch_prefix: "feat"'),
                                                                encoding="utf-8")
    (spec / "init-options.json").write_text(json.dumps({"speckit_version": "1.2.3"}), encoding="utf-8")
    (spec / "presets" / "dev-cycle").mkdir(parents=True)
    (spec / "presets" / "dev-cycle" / "preset.yml").write_text("preset: {}\n", encoding="utf-8")
    (spec / "memory").mkdir()
    code, out = run_checker("dev-cycle-protocol", "constitution.py", ["generate", "--dir", str(root)])
    assert code == 0, out
    folder = root / "specs" / "001-photo-upload"
    folder.mkdir(parents=True)
    texts = {"spec.md": spec_text, "plan.md": plan_text, "tasks.md": "# Tasks\n- [ ] T001 Upload\n"}
    for name in spec_files:
        (folder / name).write_text(texts[name], encoding="utf-8")
    plans = root / "docs" / "plans"
    plans.mkdir(parents=True)
    spec_line = f"spec_dir: {spec_dir}\n" if spec_dir else ""
    (plans / "photo_upload.md").write_text(
        f"---\nplan_id: photo_upload\ntitle: Photo upload\nkind: feature\nstatus: active\n{spec_line}"
        "created_at: '2026-10-23T10:00:00'\n---\n\n# Photo upload\n\nSee the spec folder.\n", encoding="utf-8")
    return root


def gate(root, *args):
    return subprocess.run([sys.executable, SCRIPT, "--dir", str(root), *args],
                          capture_output=True, text=True, encoding="utf-8")


def test_gate_3_passes_with_complete_spec_kit_artifacts(tmp_path):
    res = gate(speckit_project(tmp_path), "--gate", "3")
    assert res.returncode == 0, res.stdout
    assert "Spec Kit artifacts complete in specs/001-photo-upload" in res.stdout
    assert "Constitution (constitution.py check) passed" in res.stdout
    assert "Spec Kit setup (speckit_setup.py status) passed" in res.stdout


def test_gate_3_blocks_missing_tasks_and_open_clarifications(tmp_path):
    root = speckit_project(tmp_path, spec_files=("spec.md", "plan.md"),
                           spec_text="# Spec\n- FR-001 [NEEDS CLARIFICATION: which formats?]\n")
    res = gate(root, "--gate", "3")
    assert res.returncode == 1 and "missing in specs/001-photo-upload: tasks.md" in res.stdout
    (root / "specs" / "001-photo-upload" / "tasks.md").write_text("# Tasks\n", encoding="utf-8")
    res = gate(root, "--gate", "3")
    assert res.returncode == 1 and "spec.md has 1 open [NEEDS CLARIFICATION] marker(s)" in res.stdout


def test_gate_3_blocks_a_stale_constitution(tmp_path):
    root = speckit_project(tmp_path)
    cfg = json.loads((root / ".dev-cycle.json").read_text(encoding="utf-8"))
    cfg["architecture"]["style"] = "hexagonal"
    (root / ".dev-cycle.json").write_text(json.dumps(cfg), encoding="utf-8")
    res = gate(root, "--gate", "3")
    assert res.returncode == 1 and "Constitution (constitution.py check) failed" in res.stdout and "stale" in res.stdout


def test_gate_3_warns_about_branch_preset_and_missing_spec_dir(tmp_path):
    root = speckit_project(tmp_path, branch="photo-upload", plan_text="# Plan\n", spec_dir=None)
    res = gate(root, "--gate", "3")
    assert res.returncode == 0, res.stdout
    assert "has no spec_dir" in res.stdout
    plan = root / "docs" / "plans" / "photo_upload.md"
    plan.write_text(plan.read_text(encoding="utf-8").replace("status: active\n", "status: active\nspec_dir: specs/001-photo-upload\n"),
                    encoding="utf-8")
    res = gate(root, "--gate", "3")
    assert res.returncode == 0, res.stdout
    assert "is not a Spec Kit feature branch (feat/NNN-slug)" in res.stdout
    assert "has no 'Dev Cycle Protocol' sections" in res.stdout


def test_gate_3_ignores_spec_kit_when_disabled(tmp_path):
    root = speckit_project(tmp_path, spec_files=())
    cfg = json.loads((root / ".dev-cycle.json").read_text(encoding="utf-8"))
    cfg["speckit"]["enabled"] = False
    (root / ".dev-cycle.json").write_text(json.dumps(cfg), encoding="utf-8")
    res = gate(root, "--gate", "3")
    assert res.returncode == 0 and "Spec Kit" not in res.stdout


def test_gate_8_lists_open_items_of_the_release(tmp_path):
    d = make_project(tmp_path)
    (tmp_path / "docs").mkdir(exist_ok=True)
    (tmp_path / "docs" / "BACKLOG.md").write_text(BACKLOG, encoding="utf-8")
    (tmp_path / "docs" / "ROADMAP.md").write_text(ROADMAP, encoding="utf-8")
    res = gate(d, "--gate", "8", "--run", "v1.0.0-rc.1")
    assert res.returncode == 0, res.stdout
    assert "Roadmap (plan-memory) passed" in res.stdout
    assert "Items of v1.0.0 still open" in res.stdout and "FEAT-001 [open] Upload" in res.stdout
    res = gate(d, "--gate", "8", "--run", "v2.0.0-rc.1")
    assert "v2.0.0 is not in the roadmap" in res.stdout


def test_plan_envelope_reads_its_spec_kit_artifacts(tmp_path):
    root = speckit_project(tmp_path)
    store = PlanStore(str(root))
    plan = store.get("photo_upload")
    assert plan.spec_dir == "specs/001-photo-upload"
    assert [rel for rel, _ in store.spec_files(plan)] == [
        "specs/001-photo-upload/spec.md", "specs/001-photo-upload/plan.md", "specs/001-photo-upload/tasks.md"]
    full = store.full_text(plan)
    assert full.startswith(plan.body) and "T001 Upload" in full
    before = store.index_sha(plan)
    (root / "specs" / "001-photo-upload" / "tasks.md").write_text("# Tasks\n- [x] T001 Upload\n", encoding="utf-8")
    assert store.index_sha(plan) != before


def test_gate_8_blocks_ai_attribution_in_commit_messages(tmp_path):
    d = make_project(tmp_path)
    git(tmp_path, "init", "-q", "-b", "main")
    git(tmp_path, "add", "-A")
    git(tmp_path, "commit", "-q", "-m", "feat: first\n\nCo-Authored-By: Claude <noreply@anthropic.com>")
    res = gate(d, "--gate", "8")
    assert res.returncode == 1 and "AI attribution in 1 place(s)" in res.stdout and "co-author trailer" in res.stdout
    git(tmp_path, "tag", "v1.0.0")
    git(tmp_path, "commit", "-q", "--allow-empty", "-m", "fix: second")
    res = gate(d, "--gate", "8")
    assert "No AI attribution in the commits since v1.0.0" in res.stdout


def test_gate_8_blocks_an_ai_git_identity(tmp_path):
    d = make_project(tmp_path)
    git(tmp_path, "init", "-q", "-b", "main")
    git(tmp_path, "add", "-A")
    subprocess.run(["git", "-c", "user.name=Claude", "-c", "user.email=noreply@anthropic.com", "commit", "-q", "-m",
                    "feat: first"], cwd=tmp_path, check=True, capture_output=True)
    res = gate(d, "--gate", "8")
    assert res.returncode == 1 and "AI attribution in 1 place(s)" in res.stdout and "AI identity" in res.stdout
