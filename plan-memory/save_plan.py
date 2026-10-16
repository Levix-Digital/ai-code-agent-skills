"""
Save Plan Utility - plan-memory Skill
Saves an uncompressed, comprehensive architectural plan or roadmap as
<repo>/docs/plans/<plan_id>.md (YAML frontmatter + Markdown body) and indexes it.
Supports plan lineage via --supersedes to automatically deprecate older plans.
Saving an existing plan ID updates it, keeping its author, creation date and milestone progress.
"""
import argparse
import sys

from _store import Plan, PlanStore, STATUSES, now_iso, setup_stdout, slugify_id, split_csv


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
):
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

    print(f"✅ Plan {action}: {plan.path}")
    print(f"   • Plan ID    : {pid}")
    print(f"   • Project    : {store.project}")
    print(f"   • Title      : {title}")
    print(f"   • Author     : {meta.get('author')}")
    print(f"   • Version    : {meta.get('version') or '-'}")
    print(f"   • Status     : {meta['status'].upper()}")
    print(f"   • Size       : {len(content)} characters (100% uncompressed)")
    if supersedes:
        print(f"   • Supersedes : {supersedes}")
    if meta["milestones"]:
        print(f"   • Milestones : {', '.join(m['name'] for m in meta['milestones'])}")
    print("   👉 Commit the plan file with the change it describes (dev-cycle-protocol Phase 3).\n")


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
    )
