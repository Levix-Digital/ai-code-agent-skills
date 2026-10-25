"""
Backlog store - plan-memory Skill
Reads and writes the project backlog file and keeps the items linked to a plan in step
with the plan's status (planned, in progress, done when the plan is completed).
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "dev-cycle-protocol"))
from project_config import load_project_config  # noqa: E402

from _backlog import Backlog, BacklogError  # noqa: E402
from _store import repo_root, today  # noqa: E402

TEMPLATE = os.path.join(HERE, "templates", "BACKLOG_TEMPLATE.md")

# Plan status -> status of its open backlog items. `completed` closes them; `superseded` leaves them.
PLAN_TO_ITEM_STATUS = {"draft": "planned", "planned": "planned", "active": "planned", "in_progress": "in_progress"}


class BacklogFile:
    """The backlog file of one project."""

    def __init__(self, start_dir: str = None):
        self.config = load_project_config(repo_root(start_dir))
        section = self.config.section("backlog")
        self.path = os.path.join(self.config.root, section.get("file") or os.path.join("docs", "BACKLOG.md"))
        self.blocking_severities = section.get("blocking_severities") or []
        self.bug_deadline = section.get("bug_deadline") or "next_release"

    @property
    def exists(self) -> bool:
        return os.path.isfile(self.path)

    @property
    def rel(self) -> str:
        return os.path.relpath(self.path, self.config.root).replace("\\", "/")

    def load(self, create: bool = False) -> Backlog:
        if self.exists:
            with open(self.path, "r", encoding="utf-8", newline="") as f:
                return Backlog(f.read())
        if not create:
            raise BacklogError(f"no backlog at {self.rel}; the first 'add' creates it")
        with open(TEMPLATE, "r", encoding="utf-8") as f:
            return Backlog(f.read())

    def triage(self, backlog: Backlog, exclude=()) -> dict:
        """Open bugs not triaged yet (see _bug_triage), reading the project roadmap when there is one."""
        from _bug_triage import bug_triage
        from _roadmap_store import RoadmapFile
        rf = RoadmapFile(self.config.root)
        roadmap = rf.load() if rf.exists else None
        return bug_triage(backlog, roadmap, self.blocking_severities, self.bug_deadline, exclude)

    def save(self, backlog: Backlog):
        """Atomic write; refuses a file whose structure it could not read, so nothing is lost."""
        structural = backlog.errors + [f"unknown section '## {name}'" for _, name in backlog.unknown_sections]
        if structural:
            raise BacklogError(f"{self.rel} has structural problems; fix them by hand first:\n  - " + "\n  - ".join(structural))
        os.makedirs(os.path.dirname(self.path), exist_ok=True)
        tmp = self.path + ".tmp"
        with open(tmp, "w", encoding="utf-8", newline="") as f:
            f.write(backlog.render())
        os.replace(tmp, self.path)


def sync_plan_items(start_dir: str, plan) -> list:
    """Brings the plan's backlog items in line with its status. Returns messages; never raises."""
    if not plan.backlog_items:
        return []
    pid = plan.meta["plan_id"]
    status = plan.meta.get("status")
    bf = BacklogFile(start_dir)
    if not bf.exists:
        return [f"⚠️ Plan '{pid}' lists backlog items but there is no backlog at {bf.rel}."]
    messages, changed = [], False
    try:
        backlog = bf.load()
        for item_id in plan.backlog_items:
            try:
                item = backlog.find(item_id)
            except BacklogError:
                messages.append(f"⚠️ {item_id} (plan '{pid}') is not in {bf.rel}.")
                continue
            if item.closed:
                continue
            if item.plan and item.plan != pid:
                messages.append(f"⚠️ {item.id} belongs to plan '{item.plan}', not '{pid}'; left unchanged.")
                continue
            if status == "completed":
                backlog.close(item.id, "done", f"{today()}, plan {pid}")
                messages.append(f"✅ Backlog: {item.id} closed as done (plan completed).")
                changed = True
            elif status in PLAN_TO_ITEM_STATUS:
                target = PLAN_TO_ITEM_STATUS[status]
                if item.plan != pid or item.status != target:
                    backlog.update(item.id, plan=pid, status=target)
                    messages.append(f"🔗 Backlog: {item.id} is {target} (plan '{pid}').")
                    changed = True
        if changed:
            bf.save(backlog)
            messages.append(f"👉 Commit {bf.rel} with the plan file.")
    except BacklogError as e:
        messages.append(f"⚠️ Backlog not updated: {e}")
    return messages
