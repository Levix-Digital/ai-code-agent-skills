"""
Save Plan Utility - plan-memory Skill
Saves an uncompressed, comprehensive architectural plan or roadmap as
<repo>/docs/plans/<plan_id>.md (YAML frontmatter + Markdown body) and indexes it.
Supports plan lineage via --supersedes to automatically deprecate older plans.
Saving an existing plan ID updates it, keeping its author, creation date and milestone progress.
The skills run the engine's `plan-memory save_plan` (IMP-005); this module stays for backlog.py,
which promotes items with save_plan(), until backlog is translated (M32).
"""
import argparse
import os
import sys

from _backlog_store import sync_plan_items
from _store import KINDS, Plan, PlanStore, STATUSES, now_iso, setup_stdout, slugify_id, split_csv


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


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Save an uncompressed architecture plan or roadmap to docs/plans")
    parser.add_argument("--title", type=str, required=True, help="Plan title / headline")
    parser.add_argument("--content", type=str, default=None, help="Inline markdown content")
    parser.add_argument("--file", type=str, default=None, help="Path to markdown/text file containing the plan")
    parser.add_argument("--project", type=str, default=None, help="Project name (default: .dev-cycle.json project_name or repository folder name)")
    parser.add_argument("--dir", type=str, default=None, help="Project directory (default: current; the Git root is used)")
    parser.add_argument("--id", type=str, default=None, help="Explicit unique Plan ID (auto-generated if omitted)")
    parser.add_argument("--session-id", type=str, default=None, help="Conversation ID where the plan was conceived")
    parser.add_argument("--version", type=str, default=None, help="Target software version (e.g. v0.30.37)")
    parser.add_argument("--tags", type=str, default=None, help="Comma-separated keywords/tags")
    parser.add_argument("--milestones", type=str, default=None, help="Comma-separated milestone targets")
    parser.add_argument("--status", type=str, default="active", choices=STATUSES, help="Plan status")
    parser.add_argument("--parent-id", type=str, default=None, help="Parent plan ID for hierarchical breakdown")
    parser.add_argument("--supersedes", type=str, default=None, help="Older Plan ID that this new plan supersedes")
    parser.add_argument("--kind", type=str, default=None, choices=KINDS, help="Plan kind (default: feature)")
    parser.add_argument("--backlog-items", type=str, default=None, help="Comma-separated backlog item ids the plan delivers")
    parser.add_argument("--spec-dir", type=str, default=None, help="Spec Kit folder with the plan's detail (e.g. specs/001-photo-upload)")
    args = parser.parse_args()

    if args.file:
        with open(args.file, "r", encoding="utf-8") as f:
            plan_content = f.read()
    elif args.content:
        plan_content = args.content
    else:
        print("❌ Error: You must supply either --file <path> or --content <text>", file=sys.stderr)
        sys.exit(1)

    save_plan(
        store=PlanStore(args.dir, args.project),
        title=args.title,
        content=plan_content,
        plan_id=args.id,
        session_id=args.session_id,
        version=args.version,
        tags=args.tags,
        milestones=args.milestones,
        status=args.status,
        parent_plan_id=args.parent_id,
        supersedes=args.supersedes,
        kind=args.kind,
        backlog_items=args.backlog_items,
        spec_dir=args.spec_dir,
    )
