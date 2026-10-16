"""
Inspect Project Utility - dev-cycle-protocol Skill
Analyzes the current workspace to automatically detect the project tech stack,
build tools, test runners, git health, documentation files, and surfaces.
Project-agnostic: works across Python, Rust, Node/TypeScript, Go, C++, etc.
"""
import os
import sys
import argparse
import json
import subprocess
import glob

def run_cmd(cmd: list[str], cwd: str) -> tuple[int, str]:
    try:
        res = subprocess.run(
            cmd,
            cwd=cwd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace"
        )
        return res.returncode, res.stdout.strip()
    except Exception as e:
        return -1, str(e)

def detect_project_profile(workspace_dir: str = None) -> dict:
    cwd = os.path.abspath(workspace_dir or os.getcwd())

    profile = {
        "project_name": os.path.basename(cwd),
        "workspace_root": cwd,
        "languages": [],
        "frameworks": [],
        "package_manifests": [],
        "test_runners": [],
        "build_tools": [],
        "doc_files": {},
        "git_status": {},
        "config_manifest": None
    }

    # Check for optional .dev-cycle.json manifest
    dev_cycle_config = os.path.join(cwd, ".dev-cycle.json")
    if os.path.exists(dev_cycle_config):
        try:
            with open(dev_cycle_config, "r", encoding="utf-8") as f:
                profile["config_manifest"] = json.load(f)
                if "project_name" in profile["config_manifest"]:
                    profile["project_name"] = profile["config_manifest"]["project_name"]
        except Exception:
            pass

    # Detect Languages & Manifests
    if os.path.exists(os.path.join(cwd, "Cargo.toml")):
        profile["languages"].append("Rust")
        profile["package_manifests"].append("Cargo.toml")
        profile["build_tools"].append("cargo")
        profile["test_runners"].append("cargo test")

    if os.path.exists(os.path.join(cwd, "pyproject.toml")) or os.path.exists(os.path.join(cwd, "setup.py")) or os.path.exists(os.path.join(cwd, "requirements.txt")):
        profile["languages"].append("Python")
        if os.path.exists(os.path.join(cwd, "pyproject.toml")):
            profile["package_manifests"].append("pyproject.toml")
        if os.path.exists(os.path.join(cwd, "setup.py")):
            profile["package_manifests"].append("setup.py")
        profile["build_tools"].append("pip/build")
        if os.path.exists(os.path.join(cwd, "pytest.ini")) or os.path.exists(os.path.join(cwd, "tests")):
            profile["test_runners"].append("pytest")

    if os.path.exists(os.path.join(cwd, "package.json")):
        profile["languages"].append("TypeScript/JavaScript")
        profile["package_manifests"].append("package.json")
        profile["build_tools"].append("npm/pnpm/yarn")
        profile["test_runners"].append("npm test")

    if os.path.exists(os.path.join(cwd, "go.mod")):
        profile["languages"].append("Go")
        profile["package_manifests"].append("go.mod")
        profile["build_tools"].append("go build")
        profile["test_runners"].append("go test ./...")

    # Detect Documentation files
    readme_candidates = glob.glob(os.path.join(cwd, "*README*"), recursive=False)
    if readme_candidates:
        profile["doc_files"]["user_doc"] = os.path.basename(readme_candidates[0])

    tecdoc_candidates = glob.glob(os.path.join(cwd, "*TECDOC*"), recursive=False) or glob.glob(os.path.join(cwd, "docs", "*ARCHITECTURE*"), recursive=False)
    if tecdoc_candidates:
        profile["doc_files"]["tec_doc"] = os.path.relpath(tecdoc_candidates[0], cwd)

    manual_test_candidates = glob.glob(os.path.join(cwd, "tests", "*MANUAL*"), recursive=False) or glob.glob(os.path.join(cwd, "*MANUAL_TESTS*"), recursive=False)
    if manual_test_candidates:
        profile["doc_files"]["manual_tests"] = os.path.relpath(manual_test_candidates[0], cwd)

    # Git Inspection
    git_dir = os.path.join(cwd, ".git")
    if os.path.exists(git_dir):
        code, branch = run_cmd(["git", "rev-parse", "--abbrev-ref", "HEAD"], cwd)
        profile["git_status"]["branch"] = branch if code == 0 else "unknown"

        code, status_out = run_cmd(["git", "status", "--porcelain"], cwd)
        dirty_lines = [l for l in status_out.splitlines() if l.strip()]
        profile["git_status"]["dirty_files_count"] = len(dirty_lines)
        profile["git_status"]["is_clean"] = (len(dirty_lines) == 0)

        code, last_tag = run_cmd(["git", "describe", "--tags", "--abbrev=0"], cwd)
        profile["git_status"]["last_tag"] = last_tag if code == 0 else "none"

        code, ahead_behind = run_cmd(["git", "rev-list", "--left-right", "--count", f"origin/{branch}...{branch}"], cwd)
        if code == 0 and "\t" in ahead_behind:
            parts = ahead_behind.split("\t")
            profile["git_status"]["behind_origin"] = int(parts[0])
            profile["git_status"]["ahead_origin"] = int(parts[1])

    return profile

def print_profile_report(profile: dict):
    if sys.stdout.encoding != 'utf-8':
        try:
            sys.stdout.reconfigure(encoding='utf-8')
        except Exception:
            pass

    p_name = profile["project_name"]
    root = profile["workspace_root"]
    langs = ", ".join(profile["languages"]) or "Generic / Undetermined"
    manifests = ", ".join(profile["package_manifests"]) or "None"
    runners = ", ".join(profile["test_runners"]) or "None detected"
    docs = profile["doc_files"]
    git = profile["git_status"]

    print("=" * 85)
    print(f"🛡️ DEV-CYCLE PROJECT PROFILE: {p_name}")
    print(f"   • Workspace Root : {root}")
    print(f"   • Tech Stack     : {langs}")
    print(f"   • Manifests      : {manifests}")
    print(f"   • Test Runners   : {runners}")
    print("-" * 85)
    print("📚 DOCUMENTATION STANDARD STATUS (Dual-Doc + Manual Tests):")
    print(f"   • UserDoc (Public Guide) : {docs.get('user_doc', '❌ MISSING (README.md recommended)')}")
    print(f"   • TecDoc (Architecture)  : {docs.get('tec_doc', '❌ MISSING (TECDOC.md recommended)')}")
    print(f"   • Manual Tests Suite     : {docs.get('manual_tests', '❌ MISSING (tests/MANUAL_TESTS.md recommended)')}")
    print("-" * 85)
    if git:
        is_clean = git.get("is_clean", False)
        clean_icon = "🟢 Clean" if is_clean else f"⚠️ {git.get('dirty_files_count', 0)} uncommitted changes"
        branch = git.get("branch", "unknown")
        tag = git.get("last_tag", "none")
        print("🌿 GIT REPOSITORY HEALTH:")
        print(f"   • Active Branch  : {branch}")
        print(f"   • Working Tree   : {clean_icon}")
        print(f"   • Current Tag    : {tag}")
        if "ahead_origin" in git:
            print(f"   • Remote Sync    : Ahead: {git['ahead_origin']} | Behind: {git['behind_origin']}")
    else:
        print("🌿 GIT REPOSITORY: ❌ Not a Git workspace.")
    print("=" * 85 + "\n")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Inspect workspace project profile and dev-cycle readiness")
    parser.add_argument("--dir", type=str, default=None, help="Workspace directory (defaults to current dir)")
    parser.add_argument("--json", action="store_true", help="Output raw JSON profile")
    args = parser.parse_args()

    prof = detect_project_profile(args.dir)
    if args.json:
        print(json.dumps(prof, indent=2))
    else:
        print_profile_report(prof)
