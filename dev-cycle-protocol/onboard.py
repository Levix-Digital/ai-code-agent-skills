"""
Onboard - dev-cycle-protocol Skill
The diagnosis behind `/dev-cycle`: reads the repository (configuration, branches, product
baseline, plans, release candidate, bugs, roadmap, Spec Kit) and prints where to start, in
the recommended order. It only reads; every change happens after the user's choice.

Usage:
    python onboard.py diagnose [--request "<text>"] [--dir <project>] [--json]
    python onboard.py propose-config [--dir <project>] [--json]
    python onboard.py write-config --file <proposal.json> [--dir <project>]
    python onboard.py create-branch [--dir <project>]      (the integration branch, from the production branch)
"""
import argparse
import json
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SKILLS_ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(SKILLS_ROOT, "plan-memory"))
sys.path.insert(0, os.path.join(SKILLS_ROOT, "quality-assurance"))
from _onboard import recommendation  # noqa: E402
from project_config import MANIFEST_NAME, load_project_config  # noqa: E402

SOURCE_EXT = {".py", ".rs", ".go", ".ts", ".tsx", ".js", ".jsx", ".java", ".kt", ".cs", ".cpp", ".cc", ".c", ".h",
              ".hpp", ".swift", ".rb", ".php", ".dart", ".scala", ".vue", ".svelte"}
SKIP_DIRS = {".git", ".specify", ".claude", ".github", "node_modules", "docs", "tests", "test", "specs", ".venv",
             "venv", "__pycache__", "dist", "build", "target", ".idea", ".vscode"}
RC_RE = re.compile(r"^(v\d+\.\d+\.\d+)-rc\.(\d+)$")


def git(root: str, *args) -> tuple:
    try:
        res = subprocess.run(["git", *args], cwd=root, capture_output=True, text=True, encoding="utf-8", errors="replace")
        return res.returncode, res.stdout.strip()
    except FileNotFoundError:
        return 127, ""


def project_root(start_dir: str = None) -> str:
    start = os.path.abspath(start_dir or os.getcwd())
    code, out = git(start, "rev-parse", "--show-toplevel")
    return os.path.abspath(out) if code == 0 and out else start


def has_code(root: str, limit: int = 3) -> bool:
    found = 0
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS and not d.startswith(".")]
        for name in filenames:
            if os.path.splitext(name)[1].lower() in SOURCE_EXT:
                found += 1
                if found >= limit:
                    return True
    return found > 0


def branch_exists(root: str, name: str) -> bool:
    return any(git(root, "rev-parse", "--verify", "--quiet", ref)[0] == 0
               for ref in (f"refs/heads/{name}", f"refs/remotes/origin/{name}"))


TEST_COMMANDS = {"pytest": "python -m pytest", "cargo test": "cargo test", "npm test": "npm test",
                 "go test ./...": "go test ./..."}
VERSION_FILES = ["pyproject.toml", "package.json", "Cargo.toml", "setup.cfg", "VERSION", "version.txt"]


def detect_config(root: str) -> dict:
    """A .dev-cycle.json filled from what can be detected. The architecture style is left out:
    it is chosen with software-design."""
    from inspect_project import detect_project_profile
    profile = detect_project_profile(root)
    production = "main"
    for name in ("main", "master"):
        if branch_exists(root, name):
            production = name
            break
    data = {"project_name": os.path.basename(root)}
    commands = [TEST_COMMANDS[r] for r in profile.get("test_runners", []) if r in TEST_COMMANDS]
    if commands:
        data["test_commands"] = commands
    docs = profile.get("doc_files") or {}
    if docs.get("user_doc") or docs.get("tec_doc"):
        data["docs"] = {k: docs[k] for k in ("user_doc", "tec_doc") if docs.get(k)}
    if docs.get("manual_tests"):
        data["tests"] = {"manual_tests": docs["manual_tests"]}
    data["devops"] = {"integration_branch": "dev", "production_branch": production,
                      "version_files": [f for f in VERSION_FILES if os.path.isfile(os.path.join(root, f))]}
    return data


def missing_keys(existing: dict, proposal: dict, prefix: str = "") -> dict:
    """The part of `proposal` that `existing` does not set (one level into sections)."""
    out = {}
    for key, value in proposal.items():
        if key not in existing:
            out[key] = value
        elif isinstance(value, dict) and isinstance(existing.get(key), dict):
            sub = {k: v for k, v in value.items() if k not in existing[key]}
            if sub:
                out[key] = sub
    return out


def merge_config(existing: dict, additions: dict) -> tuple:
    """(merged, conflicts): additions are only added; a key the file already sets is never changed."""
    merged = json.loads(json.dumps(existing))
    conflicts = []
    for key, value in additions.items():
        if key not in merged:
            merged[key] = value
        elif isinstance(value, dict) and isinstance(merged[key], dict):
            for k, v in value.items():
                if k not in merged[key]:
                    merged[key][k] = v
                elif merged[key][k] != v:
                    conflicts.append(f"{key}.{k}")
        elif merged[key] != value:
            conflicts.append(key)
    return merged, conflicts


def propose_config(root: str) -> dict:
    cfg = load_project_config(root)
    existing = raw_manifest(cfg)
    proposal = missing_keys(existing, detect_config(root))
    return {"file": cfg.manifest_path or os.path.join(root, MANIFEST_NAME), "exists": bool(cfg.manifest_path),
            "additions": proposal}


def write_config(root: str, additions: dict) -> tuple:
    """Writes the confirmed additions. Returns (path, added keys). Raises ValueError on conflicts."""
    cfg = load_project_config(root)
    existing = raw_manifest(cfg)
    merged, conflicts = merge_config(existing, additions)
    if conflicts:
        raise ValueError("the file already sets " + ", ".join(conflicts) + "; edit those keys by hand")
    path = cfg.manifest_path or os.path.join(root, MANIFEST_NAME)
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(json.dumps(merged, indent=2, ensure_ascii=False) + "\n")
    added = [k if not isinstance(v, dict) else f"{k}.{{{', '.join(v)}}}" for k, v in missing_keys(existing, merged).items()]
    return path, added


def create_integration_branch(root: str) -> str:
    cfg = load_project_config(root)
    devops = cfg.section("devops")
    integration, production = devops.get("integration_branch") or "dev", devops.get("production_branch") or "main"
    if branch_exists(root, integration):
        raise ValueError(f"'{integration}' already exists")
    base = production if git(root, "rev-parse", "--verify", "--quiet", f"refs/heads/{production}")[0] == 0 else f"origin/{production}"
    code, out = git(root, "branch", integration, base)
    if code != 0:
        raise ValueError(f"could not create '{integration}' from '{base}': {out}")
    return f"{integration} (from {base}); push it with: git push -u origin {integration}"


def raw_manifest(cfg) -> dict:
    if not cfg.manifest_path:
        return {}
    try:
        with open(cfg.manifest_path, "r", encoding="utf-8") as f:
            data = json.load(f)
        return data if isinstance(data, dict) else {}
    except (OSError, ValueError):
        return {}


def collect(root: str) -> dict:
    """The state of the repository, as read by the diagnosis (see _onboard.py)."""
    state = {"findings": []}
    is_git = git(root, "rev-parse", "--is-inside-work-tree")[0] == 0
    commits = 0
    if is_git:
        code, out = git(root, "rev-list", "--count", "HEAD")
        commits = int(out) if code == 0 and out.isdigit() else 0
    _, branch = git(root, "rev-parse", "--abbrev-ref", "HEAD") if commits else (0, "")
    _, status = git(root, "status", "--porcelain") if is_git else (0, "")
    state["repo"] = {"git": is_git, "commits": commits, "branch": branch,
                     "dirty": len([l for l in status.splitlines() if l.strip()]), "has_code": has_code(root)}

    cfg = load_project_config(root)
    state["config"] = {"exists": bool(cfg.manifest_path),
                       "problems": [p for p in cfg.validate() if not p.startswith("unknown top-level key")]}
    devops = cfg.section("devops")
    state["branches"] = {"integration": devops.get("integration_branch"), "production": devops.get("production_branch"),
                         "integration_exists": branch_exists(root, devops.get("integration_branch") or "dev") if commits else False,
                         "production_exists": branch_exists(root, devops.get("production_branch") or "main") if commits else False}

    product = cfg.section("product")
    index = (cfg.section("architecture_docs") or {}).get("index") or cfg.section("docs").get("tec_doc")
    baseline = {"vision": os.path.isfile(os.path.join(root, product.get("vision") or "docs/product/VISION.md")),
                "tech_spec": os.path.isfile(os.path.join(root, product.get("tech_spec") or "docs/product/TECH_SPEC.md")),
                "style": cfg.section("architecture").get("style"),
                "tec_doc": bool(index) and os.path.isfile(os.path.join(root, index)), "epics": 0, "roadmap": False}

    backlog = None
    try:
        from _backlog_store import BacklogFile
        bf = BacklogFile(root)
        if bf.exists:
            backlog = bf.load()
            baseline["epics"] = len(backlog.open_items("epic"))
            state["bugs"] = [f"{b.id} ({b.rank}) {b.title}" for b in backlog.blocking_bugs(bf.blocking_severities)]
    except Exception as e:
        state["findings"].append(f"backlog: could not be read ({e})")

    try:
        from _roadmap_store import RoadmapFile
        import roadmap as roadmap_cli
        rf = RoadmapFile(root)
        if rf.exists:
            baseline["roadmap"] = True
            data = roadmap_cli.current_data(rf.load(), backlog)
            release = data["release"]
            if release:
                state["roadmap"] = {"current": f"{release['version']} {release['name']}",
                                    "open_items": [f"{d['id']} ({d['rank']}) {d['title']}" for d in data["open_items"]]}
    except Exception as e:
        state["findings"].append(f"roadmap: could not be read ({e})")
    state["baseline"] = baseline

    try:
        from _store import PlanStore
        store = PlanStore(root)
        state["plans"] = [{"id": p.meta["plan_id"], "title": p.meta.get("title"), "status": p.meta.get("status"),
                           "done": len(p.done_milestones), "total": len(p.milestones)}
                          for p in store.load_all() if p.meta.get("status") in ("active", "in_progress")]
    except Exception as e:
        state["findings"].append(f"plans: could not be read ({e})")

    state["rc"] = rc_state(root, cfg)
    state["speckit"] = speckit_state(root, cfg)
    return state


def rc_state(root: str, cfg):
    """The latest release candidate run of the manual test suite, or None."""
    path = os.path.join(root, cfg.section("tests").get("manual_tests") or "tests/MANUAL_TESTS.md")
    if not os.path.isfile(path):
        return None
    from manual_tests import load_suite
    suite = load_suite(root)
    runs = {t.run for t in suite.tests if t.run and RC_RE.match(t.run)}
    if not runs:
        return None

    def key(run):
        m = RC_RE.match(run)
        return tuple(int(x) for x in m.group(1)[1:].split(".")) + (int(m.group(2)),)

    run = max(runs, key=key)
    version = RC_RE.match(run).group(1)
    tests = suite.select(run)
    released = bool(git(root, "tag", "-l", version)[1]) if os.path.isdir(os.path.join(root, ".git")) else False
    return {"run": run, "version": version, "not_run": len([t for t in tests if not t.status]),
            "failed": len([t for t in tests if t.status in ("Fail", "Blocked")]), "released": released}


def speckit_state(root: str, cfg) -> dict:
    """undecided (never asked), declined (explicit false), enabled, or unmanaged (.specify without the protocol)."""
    raw = (raw_manifest(cfg).get("speckit") or {})
    decided = cfg.is_set("speckit", "enabled")
    initialized = os.path.isdir(os.path.join(root, ".specify"))
    out = {"uv": bool(shutil.which("uvx")), "problems": [], "installed": None}
    if raw.get("enabled") is True:
        import speckit_setup
        data = speckit_setup.status_data(root)
        out.update({"state": "enabled", "problems": data["problems"], "installed": data["installed"]})
        if initialized:
            import constitution
            out["problems"] += constitution.check(root)
        return out
    if initialized and not (decided and raw.get("enabled") is False):
        import speckit_setup
        out.update({"state": "unmanaged", "installed": speckit_setup.installed_version(root)})
        return out
    out["state"] = "declined" if decided else "undecided"
    return out


def render(rec: dict, state: dict) -> str:
    lines = [f"🧭 {rec['summary']}"]
    if rec["ask"]:
        for n, opt in enumerate(rec["options"], 1):
            star = " (recommended)" if opt["key"] == rec["recommended"] else ""
            lines.append(f"   {n}. {opt['title']}{star}" + (f" — {opt['reason']}" if opt["reason"] else ""))
    for finding in state.get("findings") or []:
        lines.append(f"   ⚠️ {finding}")
    repo = state.get("repo") or {}
    if repo.get("dirty"):
        lines.append(f"   ℹ️ {repo['dirty']} uncommitted change(s) on '{repo.get('branch')}'.")
    return "\n".join(lines)


def main(argv: list = None) -> int:
    if sys.stdout.encoding != "utf-8":
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass
    parser = argparse.ArgumentParser(description="Diagnose a repository for the development cycle (/dev-cycle)")
    sub = parser.add_subparsers(dest="command", required=True)
    p = sub.add_parser("diagnose", help="Where to start, in the recommended order")
    p.add_argument("--request", default=None, help="The user's text after /dev-cycle (an idea or a request)")
    p.add_argument("--dir", default=None)
    p.add_argument("--json", action="store_true")
    p = sub.add_parser("propose-config", help="The .dev-cycle.json keys that can be detected and are missing")
    p.add_argument("--dir", default=None)
    p.add_argument("--json", action="store_true")
    p = sub.add_parser("write-config", help="Write confirmed additions to .dev-cycle.json (never overwrites)")
    p.add_argument("--dir", default=None)
    p.add_argument("--file", required=True, help="JSON file with the additions the user confirmed")
    p = sub.add_parser("create-branch", help="Create the integration branch from the production branch")
    p.add_argument("--dir", default=None)
    args = parser.parse_args(argv)

    root = project_root(args.dir)
    if args.command == "propose-config":
        data = propose_config(root)
        if args.json:
            print(json.dumps(data, indent=2, ensure_ascii=False))
        elif not data["additions"]:
            print(f"✅ {MANIFEST_NAME} already has every key that can be detected.")
        else:
            print(f"📝 Proposed {'additions to' if data['exists'] else 'new'} {data['file']} (written only after you confirm):")
            print(json.dumps(data["additions"], indent=2, ensure_ascii=False))
            print("ℹ️ architecture.style is chosen with software-design, not detected.")
        return 0
    if args.command == "write-config":
        try:
            with open(args.file, "r", encoding="utf-8") as f:
                additions = json.load(f)
            if not isinstance(additions, dict):
                raise ValueError("the file must hold a JSON object")
            path, added = write_config(root, additions)
        except (OSError, ValueError) as e:
            print(f"❌ {e}")
            return 1
        print(f"✅ {path}: added {', '.join(added) or 'nothing'}")
        return 0
    if args.command == "create-branch":
        try:
            print(f"✅ Created {create_integration_branch(root)}")
        except ValueError as e:
            print(f"❌ {e}")
            return 1
        return 0

    state = collect(root)
    rec = recommendation(state, (args.request or "").strip() or None)
    if args.json:
        print(json.dumps({**rec, "state": state}, indent=2, ensure_ascii=False))
    else:
        print(render(rec, state))
    return 0


if __name__ == "__main__":
    sys.exit(main())
