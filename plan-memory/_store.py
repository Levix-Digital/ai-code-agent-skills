"""
Plan store - plan-memory Skill
Plans live as Markdown files with YAML frontmatter in the project repository
(default: <repo>/docs/plans/<plan_id>.md). The files are the source of truth.
ChromaDB is only a local, disposable search index rebuilt from the files.
"""
import datetime
import hashlib
import json
import os
import re
import subprocess
import sys

import yaml

from _common import get_db_path, normalize_project

INDEX_COLLECTION = "plan_index"
STATUSES = ["draft", "planned", "active", "in_progress", "completed", "superseded"]
KINDS = ["feature", "bug"]   # a plan without `kind` is a feature plan
FIELD_ORDER = [
    "plan_id", "project", "title", "kind", "version", "status", "author", "created_at", "updated_at",
    "session_id", "tags", "supersedes", "superseded_by", "parent_plan_id", "backlog_items", "milestones", "status_log",
]


def setup_stdout():
    if sys.stdout.encoding != "utf-8":
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass


def now_iso() -> str:
    return datetime.datetime.now().replace(microsecond=0).isoformat()


def today() -> str:
    return datetime.date.today().isoformat()


def slugify_id(text: str) -> str:
    text = text.lower().strip()
    text = re.sub(r"[^\w\s-]", "", text)
    text = re.sub(r"[\s_-]+", "_", text)
    return text.strip("_")


# ---------------------------------------------------------------- repository

def repo_root(start_dir: str = None) -> str:
    start = os.path.abspath(start_dir or os.getcwd())
    try:
        out = subprocess.run(["git", "rev-parse", "--show-toplevel"], cwd=start,
                             capture_output=True, text=True, check=True).stdout.strip()
        return os.path.abspath(out)
    except Exception:
        return start


def load_config(root: str) -> dict:
    path = os.path.join(root, ".dev-cycle.json")
    if os.path.isfile(path):
        try:
            with open(path, "r", encoding="utf-8") as f:
                return json.load(f)
        except Exception as e:
            print(f"⚠️ Could not read '{path}': {e}", file=sys.stderr)
    return {}


def git_user(root: str) -> str:
    try:
        return subprocess.run(["git", "config", "user.name"], cwd=root,
                              capture_output=True, text=True, check=True).stdout.strip() or None
    except Exception:
        return None


class PlanStore:
    """Plans of one repository."""

    def __init__(self, start_dir: str = None, project: str = None):
        self.root = repo_root(start_dir)
        self.config = load_config(self.root)
        plans_rel = (self.config.get("plans") or {}).get("dir") or self.config.get("plans_dir") or os.path.join("docs", "plans")
        self.plans_dir = os.path.join(self.root, plans_rel)
        self.project = normalize_project(project or self.config.get("project_name") or os.path.basename(self.root))

    # ------------------------------------------------------------ files

    def path_for(self, plan_id: str) -> str:
        return os.path.join(self.plans_dir, f"{plan_id}.md")

    def load_all(self) -> list:
        if not os.path.isdir(self.plans_dir):
            return []
        plans = []
        for name in sorted(os.listdir(self.plans_dir)):
            if name.endswith(".md"):
                plans.append(read_plan(os.path.join(self.plans_dir, name)))
        return plans

    def get(self, plan_id: str):
        path = self.path_for(plan_id)
        return read_plan(path) if os.path.isfile(path) else None

    def latest(self, plans: list = None):
        """Active / in-progress plans first, then the most recently created."""
        plans = plans if plans is not None else self.load_all()
        if not plans:
            return None
        return sorted(plans, key=lambda p: (p.meta.get("status") in ("active", "in_progress"),
                                            str(p.meta.get("created_at") or "")), reverse=True)[0]

    def save(self, plan) -> None:
        os.makedirs(self.plans_dir, exist_ok=True)
        plan.path = self.path_for(plan.meta["plan_id"])
        write_plan(plan)

    def author(self) -> str:
        return git_user(self.root)

    # ------------------------------------------------------------ index

    def collection(self):
        import chromadb
        db_path = get_db_path()
        os.makedirs(db_path, exist_ok=True)
        client = chromadb.PersistentClient(path=db_path)
        return client.get_or_create_collection(
            name=INDEX_COLLECTION,
            metadata={"description": "Local search index of plan files (docs/plans). Disposable: rebuilt from the files."},
        )

    def sync_index(self, plans: list = None, rebuild: bool = False, quiet: bool = True):
        """Brings the index in line with the plan files of this project. Returns the collection."""
        plans = plans if plans is not None else self.load_all()
        col = self.collection()
        indexed = col.get(where={"project": self.project})
        if rebuild and indexed["ids"]:
            col.delete(ids=indexed["ids"])
            indexed = {"ids": [], "metadatas": []}
        known = {i: (m or {}).get("sha256") for i, m in zip(indexed["ids"], indexed["metadatas"])}

        changed = [p for p in plans if known.get(p.meta["plan_id"]) != p.sha256]
        stale = [i for i in known if i not in {p.meta["plan_id"] for p in plans}]
        if changed:
            col.upsert(
                ids=[p.meta["plan_id"] for p in changed],
                documents=[p.body for p in changed],
                metadatas=[index_metadata(p, self.project) for p in changed],
            )
        if stale:
            col.delete(ids=stale)
        if not quiet or changed or stale:
            print(f"🔄 Plan index synced from {self.plans_dir}: {len(changed)} updated, {len(stale)} removed, {len(plans)} plan(s).",
                  file=sys.stderr)
        return col


# ---------------------------------------------------------------- plan files

class Plan:
    def __init__(self, meta: dict, body: str, path: str = None, raw: str = ""):
        self.meta = meta
        self.body = body
        self.path = path
        self.sha256 = hashlib.sha256(raw.encode("utf-8")).hexdigest() if raw else None

    @property
    def milestones(self) -> list:
        return self.meta.get("milestones") or []

    @property
    def kind(self) -> str:
        return self.meta.get("kind") or "feature"

    @property
    def backlog_items(self) -> list:
        return [str(i) for i in self.meta.get("backlog_items") or []]

    @property
    def done_milestones(self) -> list:
        return [m["name"] for m in self.milestones if m.get("done")]

    def add_log(self, by: str, note: str, status: str = None):
        entry = {"at": now_iso(), "by": by}
        if status:
            entry["status"] = status
        entry["note"] = note
        self.meta.setdefault("status_log", [])
        if self.meta["status_log"] is None:
            self.meta["status_log"] = []
        self.meta["status_log"].append(entry)


def _plain(value):
    """YAML may load timestamps as datetime objects; keep everything as plain strings."""
    if isinstance(value, (datetime.datetime, datetime.date)):
        return value.isoformat()
    if isinstance(value, dict):
        return {k: _plain(v) for k, v in value.items()}
    if isinstance(value, list):
        return [_plain(v) for v in value]
    return value


def split_frontmatter(text: str):
    if text.startswith("---\n"):
        end = text.find("\n---\n", 4)
        if end != -1:
            return yaml.safe_load(text[4:end]) or {}, text[end + 5:].lstrip("\n")
    return None, text


def read_plan(path: str) -> Plan:
    with open(path, "r", encoding="utf-8") as f:
        raw = f.read()
    meta, body = split_frontmatter(raw)
    meta = _plain(meta or {})
    meta.setdefault("plan_id", os.path.splitext(os.path.basename(path))[0])
    meta.setdefault("status", "active")
    milestones = []
    for m in meta.get("milestones") or []:
        milestones.append({"name": str(m), "done": False} if not isinstance(m, dict) else m)
    meta["milestones"] = milestones
    return Plan(meta, body, path, raw)


def write_plan(plan: Plan) -> None:
    ordered = {k: plan.meta[k] for k in FIELD_ORDER if k in plan.meta}
    ordered.update({k: v for k, v in plan.meta.items() if k not in ordered})
    front = yaml.safe_dump(ordered, sort_keys=False, allow_unicode=True, default_flow_style=False, width=4096)
    text = f"---\n{front}---\n\n{plan.body.strip()}\n"
    with open(plan.path, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    plan.sha256 = hashlib.sha256(text.encode("utf-8")).hexdigest()


def index_metadata(plan: Plan, project: str) -> dict:
    m = plan.meta
    return {
        "project": project,
        "plan_id": m["plan_id"],
        "title": str(m.get("title") or ""),
        "version": str(m.get("version") or ""),
        "status": str(m.get("status") or ""),
        "kind": plan.kind,
        "created_at": str(m.get("created_at") or ""),
        "path": plan.path or "",
        "sha256": plan.sha256 or "",
    }


def split_csv(value: str) -> list:
    return [v.strip() for v in (value or "").split(",") if v.strip()]


def progress_bar(done: int, total: int, width: int = 16) -> str:
    ratio = done / total if total else 0.0
    filled = int(round(width * ratio))
    return f"[{'█' * filled}{'░' * (width - filled)}] {ratio * 100:.1f}% ({done}/{total} Milestones)"
