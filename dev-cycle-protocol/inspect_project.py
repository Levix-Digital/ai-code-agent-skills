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

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from project_config import branch_summary, load_project_config

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

def backlog_profile(cwd: str) -> dict:
    """Open backlog items per type and the bugs that come before new work."""
    try:
        sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "plan-memory"))
        from _backlog_store import BacklogFile
        bf = BacklogFile(cwd)
        if not bf.exists:
            return {"file": bf.rel, "exists": False}
        backlog = bf.load()
        return {
            "file": bf.rel,
            "exists": True,
            "open": {kind: len(backlog.open_items(kind)) for kind in ("bug", "feature", "improvement", "adjustment", "epic")},
            "blocking_bugs": [f"{b.id} ({b.rank}) {b.title}" for b in backlog.blocking_bugs(bf.blocking_severities)],
        }
    except Exception as e:
        return {"error": str(e)}


def roadmap_profile(cwd: str) -> dict:
    """The current release of the roadmap and its open items."""
    try:
        sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "plan-memory"))
        from _roadmap_store import RoadmapFile
        from _backlog_store import BacklogFile
        import roadmap as roadmap_cli
        rf = RoadmapFile(cwd)
        if not rf.exists:
            return {"file": rf.rel, "exists": False}
        bf = BacklogFile(cwd)
        data = roadmap_cli.current_data(rf.load(), bf.load() if bf.exists else None)
        release = data["release"]
        return {"file": rf.rel, "exists": True,
                "current": f"{release['version']}: {release['name']}" if release else None,
                "open_items": [f"{d['id']} ({d['rank']}) {d['title']}" for d in data["open_items"]],
                "unscheduled": len(data["unscheduled"])}
    except Exception as e:
        return {"error": str(e)}


def product_profile(cfg) -> dict:
    """Phase 0 baseline: which product documents exist."""
    product = cfg.section("product")
    return {key: {"file": product.get(key), "exists": bool(product.get(key)) and os.path.isfile(os.path.join(cfg.root, product.get(key)))}
            for key in ("vision", "tech_spec")}


def speckit_profile(cwd: str) -> dict:
    try:
        import speckit_setup
        return speckit_setup.status_data(cwd)
    except Exception as e:
        return {"error": str(e)}


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
        "config_manifest": None,
        "architecture": {},
        "test_commands": [],
        "devops": {},
        "config_problems": [],
        "backlog": {},
        "roadmap": {},
        "product": {},
        "speckit": {},
    }

    # Project configuration (.dev-cycle.json, shared by every skill)
    try:
        cfg = load_project_config(cwd)
        profile["config_manifest"] = cfg.manifest_path
        profile["project_name"] = cfg.project_name
        profile["architecture"] = cfg.section("architecture")
        profile["test_commands"] = list(cfg.get("test_commands") or [])
        profile["devops"] = cfg.section("devops")
        profile["config_problems"] = cfg.validate()
        profile["backlog"] = backlog_profile(cwd)
        profile["roadmap"] = roadmap_profile(cwd)
        profile["product"] = product_profile(cfg)
        profile["speckit"] = speckit_profile(cwd)
    except Exception as e:
        cfg = None
        profile["config_problems"] = [f"could not read .dev-cycle.json: {e}"]

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

    # Configured documentation paths win over detection
    if cfg and cfg.manifest_path:
        for section, key in (("docs", "user_doc"), ("docs", "tec_doc"), ("tests", "manual_tests")):
            value = cfg.section(section).get(key)
            if value and os.path.exists(os.path.join(cwd, value)):
                profile["doc_files"][key] = value

    # Configured test commands win over detected runners
    if profile["test_commands"]:
        profile["test_runners"] = list(profile["test_commands"])

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
    arch = profile.get("architecture", {})
    devops = profile.get("devops", {})
    print(f"   • Config         : {profile.get('config_manifest') or 'none (.dev-cycle.json recommended)'}")
    print(f"   • Architecture   : {arch.get('style') or 'not set (architecture.style in .dev-cycle.json)'}")
    if devops:
        rc = "on" if devops.get("release_candidates") else "off"
        print(f"   • Branching      : {branch_summary(devops)} (release candidates: {rc})")
        print(f"   • Releases Repo  : {devops.get('releases_repo') or 'not set'}")
    for problem in profile.get("config_problems", []):
        print(f"   ⚠️ Config: {problem}")
    print("-" * 85)
    backlog = profile.get("backlog") or {}
    if backlog.get("exists"):
        counts = ", ".join(f"{n} {kind}" for kind, n in backlog["open"].items())
        print(f"📥 BACKLOG ({backlog['file']}): open {counts}")
        for bug in backlog["blocking_bugs"]:
            print(f"   🐞 Comes before new work: {bug}")
        print("   Triage with the user: promote, re-prioritize or drop (plan-memory/backlog.py summary).")
    elif backlog.get("file"):
        print(f"📥 BACKLOG: none yet ({backlog['file']} is created by the first /backlog or backlog.py add)")
    elif backlog.get("error"):
        print(f"📥 BACKLOG: could not be read ({backlog['error']})")
    roadmap = profile.get("roadmap") or {}
    if roadmap.get("exists"):
        print(f"🗺️ ROADMAP ({roadmap['file']}): current release {roadmap.get('current') or 'none (every release is released)'}")
        for item in roadmap.get("open_items", []):
            print(f"   · {item}")
        if roadmap.get("unscheduled"):
            print(f"   ℹ️ {roadmap['unscheduled']} open backlog item(s) in no release (plan-memory/roadmap.py current)")
    elif roadmap.get("file"):
        print(f"🗺️ ROADMAP: none yet ({roadmap['file']}; Phase 0 or plan-memory/roadmap.py add-release)")
    elif roadmap.get("error"):
        print(f"🗺️ ROADMAP: could not be read ({roadmap['error']})")
    product = profile.get("product") or {}
    if product:
        state = ", ".join(f"{d['file']} {'✅' if d['exists'] else '❌'}" for d in product.values())
        print(f"🌱 PRODUCT BASELINE: {state}" + ("" if all(d["exists"] for d in product.values())
                                                else " (Phase 0: product-inception)"))
    speckit = profile.get("speckit") or {}
    if speckit.get("enabled"):
        print(f"🧩 SPEC KIT: {speckit.get('pinned') or 'version not set'} ({speckit.get('integration')}, "
              f"branches {speckit.get('branch_prefix') or '?'}/NNN-slug)")
        for problem in speckit.get("problems", []):
            print(f"   ⚠️ {problem}")
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
