"""
Update Plan Status Utility - plan-memory Skill
Updates the lifecycle status of a plan file (e.g. active -> completed, in_progress, superseded).
"""
import argparse
import sys

from _backlog_store import sync_plan_items
from _store import PlanStore, STATUSES, now_iso, setup_stdout


def update_plan_status(store: PlanStore, plan_id: str, new_status: str, notes: str = None):
    setup_stdout()
    plan = store.get(plan_id)
    if not plan:
        print(f"❌ Error: Plan ID '{plan_id}' not found in {store.plans_dir}.", file=sys.stderr)
        sys.exit(1)

    meta = plan.meta
    old_status = meta.get("status", "active")
    meta["status"] = new_status
    meta["updated_at"] = now_iso()
    plan.add_log(store.author(), notes or f"Status {old_status} -> {new_status}", status=new_status)
    store.save(plan)
    store.sync_index()
    item_messages = sync_plan_items(store.root, plan)

    print("✅ Plan status updated successfully!")
    print(f"   • Plan ID    : {plan_id}")
    print(f"   • Project    : {meta.get('project', store.project)}")
    print(f"   • Title      : {meta.get('title')}")
    print(f"   • Status     : {old_status.upper()} -> {new_status.upper()}")
    if notes:
        print(f"   • Notes      : {notes}")
    print(f"   • File       : {plan.path}")
    for message in item_messages:
        print(f"   {message}")
    print("   👉 Commit the updated plan file.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Update a plan's lifecycle status")
    parser.add_argument("--id", type=str, required=True, help="Plan ID to update")
    parser.add_argument("--status", type=str, required=True, choices=STATUSES, help="New status")
    parser.add_argument("--notes", type=str, default=None, help="Optional progress notes / rationale")
    parser.add_argument("--project", type=str, default=None, help="Project name (default: from the repository)")
    parser.add_argument("--dir", type=str, default=None, help="Project directory (default: current)")
    args = parser.parse_args()

    update_plan_status(PlanStore(args.dir, args.project), plan_id=args.id, new_status=args.status, notes=args.notes)
