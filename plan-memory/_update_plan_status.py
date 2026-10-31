"""
Update Plan Status - plan-memory Skill
Updates the lifecycle status of a plan file (e.g. active -> completed, in_progress, superseded).
The command is the engine's `plan-memory update_plan_status`; this module keeps the function
for the Python callers that remain.
"""
import sys

from _backlog_store import sync_plan_items
from _store import PlanStore, now_iso, setup_stdout


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

