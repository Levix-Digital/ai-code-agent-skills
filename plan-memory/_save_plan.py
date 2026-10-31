"""
Save plan - plan-memory Skill (shared module)
Saves an uncompressed, comprehensive architectural plan or roadmap as
<repo>/docs/plans/<plan_id>.md (YAML frontmatter + Markdown body) and indexes it.
Supports plan lineage via --supersedes to automatically deprecate older plans.
Saving an existing plan ID updates it, keeping its author, creation date and milestone progress.
The command is the engine's `plan-memory save_plan` (IMP-005); this module stays for backlog.py,
which promotes items with save_plan(), until backlog is translated (M32).
"""
import os
import sys

from _backlog_store import sync_plan_items
from _store import KINDS, Plan, PlanStore, now_iso, setup_stdout, slugify_id, split_csv


def save_plan(
    store: PlanStore,
    title: str,
    content: str,
    plan_id: str = None,
    session_id: str = None,
    version: str = None,
    tags: str = None,
    milestones: str = None,
    status: str = "active",
    parent_plan_id: str = None,
    supersedes: str = None,
    kind: str = None,
    backlog_items: str = None,
    spec_dir: str = None,
):
    """Writes the plan file and returns the Plan. `spec_dir` links the Spec Kit folder whose
    artifacts hold the plan's detail (the plan file is then the envelope)."""
    setup_stdout()
    pid = plan_id or f"{slugify_id(store.project)}_{slugify_id(title)}"
    author = store.author()
    ts = now_iso()

    existing = store.get(pid)
    if existing:
        meta = existing.meta
        old_done = {m["name"]: m for m in existing.milestones}
        action = "updated"
    else:
        meta = {"plan_id": pid, "author": author, "created_at": ts, "status_log": []}
        old_done = {}
        action = "created"

    meta.update({
        "project": store.project,
        "title": title,
        "status": status or meta.get("status") or "active",
        "updated_at": ts,
    })
    if version:
        meta["version"] = version
    if session_id:
        meta["session_id"] = session_id
    if tags is not None:
        meta["tags"] = split_csv(tags)
    if parent_plan_id:
        meta["parent_plan_id"] = parent_plan_id
    if supersedes:
        meta["supersedes"] = supersedes
    if kind:
        if kind not in KINDS:
            raise ValueError(f"kind '{kind}' is not one of: {', '.join(KINDS)}")
        meta["kind"] = kind
    if backlog_items is not None:
        meta["backlog_items"] = [i.upper() for i in split_csv(backlog_items)]
    if spec_dir is not None:
        spec_dir = spec_dir.strip().replace("\\", "/").rstrip("/")
        meta["spec_dir"] = spec_dir or None
        if spec_dir and not os.path.isdir(os.path.join(store.root, spec_dir)):
            print(f"⚠️ Warning: spec folder '{spec_dir}' does not exist (yet) in {store.root}.", file=sys.stderr)
    if milestones is not None:
        meta["milestones"] = [old_done.get(name, {"name": name, "done": False}) for name in split_csv(milestones)]
    for key in ("version", "session_id", "supersedes", "superseded_by", "parent_plan_id"):
        meta.setdefault(key, None)
    meta.setdefault("tags", [])
    meta.setdefault("milestones", [])

    plan = Plan(meta, content)
    plan.add_log(author, f"Plan {action}", status=meta["status"])
    store.save(plan)

    # Handle lineage / superseding
    if supersedes:
        old = store.get(supersedes)
        if old:
            old_status = old.meta.get("status", "active")
            old.meta["status"] = "superseded"
            old.meta["superseded_by"] = pid
            old.meta["updated_at"] = ts
            old.add_log(author, f"Superseded by {pid}", status="superseded")
            store.save(old)
            print(f"📦 Plan Lineage: marked '{supersedes}' as SUPERSEDED (was {old_status.upper()})")
        else:
            print(f"⚠️ Warning: plan to supersede '{supersedes}' not found in {store.plans_dir}.", file=sys.stderr)

    store.sync_index()
    item_messages = sync_plan_items(store.root, plan)

    print(f"✅ Plan {action}: {plan.path}")
    print(f"   • Plan ID    : {pid}")
    print(f"   • Project    : {store.project}")
    print(f"   • Title      : {title}")
    print(f"   • Kind       : {plan.kind}")
    if plan.backlog_items:
        print(f"   • Backlog    : {', '.join(plan.backlog_items)}")
    if plan.spec_dir:
        print(f"   • Spec Kit   : {plan.spec_dir} ({len(store.spec_files(plan))} artifact(s))")
    print(f"   • Author     : {meta.get('author')}")
    print(f"   • Version    : {meta.get('version') or '-'}")
    print(f"   • Status     : {meta['status'].upper()}")
    print(f"   • Size       : {len(content)} characters (100% uncompressed)")
    if supersedes:
        print(f"   • Supersedes : {supersedes}")
    if meta["milestones"]:
        print(f"   • Milestones : {', '.join(m['name'] for m in meta['milestones'])}")
    for message in item_messages:
        print(f"   {message}")
    print("   👉 Commit the plan file with the change it describes (dev-cycle-protocol Phase 3).\n")
    return plan

