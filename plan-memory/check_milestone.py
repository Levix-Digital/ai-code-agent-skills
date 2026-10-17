"""
Check Milestone Utility - plan-memory Skill
Tracks granular milestone completion in the plan file's frontmatter (docs/plans/<plan_id>.md).
Displays visual progress bars, checklist states, and milestone notes.
"""
import argparse
import sys

from _backlog_store import sync_plan_items
from _store import PlanStore, now_iso, progress_bar, setup_stdout, today


def check_milestone(
    store: PlanStore,
    plan_id: str = None,
    milestone_name: str = None,
    uncheck: bool = False,
    notes: str = None,
    list_only: bool = False,
    auto_complete: bool = False,
):
    setup_stdout()
    plan = store.get(plan_id) if plan_id else store.latest()
    if not plan:
        target = f"plan_id='{plan_id}'" if plan_id else f"project='{store.project}'"
        print(f"❌ Error: Plan not found for {target} in {store.plans_dir}", file=sys.stderr)
        sys.exit(1)

    meta = plan.meta
    pid = meta["plan_id"]
    milestones = plan.milestones
    if not milestones:
        print(f"⚠️ Plan '{pid}' does not have any milestones defined in its frontmatter.")
        return

    modified = False
    if milestone_name and not list_only:
        # Match milestone name (case-insensitive / partial match)
        names = [m["name"] for m in milestones]
        matched = next((n for n in names if n.lower() == milestone_name.lower()), None)
        if not matched:
            matches = [n for n in names if milestone_name.lower() in n.lower()]
            if len(matches) == 1:
                matched = matches[0]
            elif len(matches) > 1:
                print(f"❌ Ambiguous milestone name '{milestone_name}'. Matches: {matches}", file=sys.stderr)
                sys.exit(1)
        if not matched:
            print(f"❌ Milestone '{milestone_name}' not found in plan '{pid}'.", file=sys.stderr)
            print(f"   Available milestones: {', '.join(names)}")
            sys.exit(1)

        author = store.author()
        ms = next(m for m in milestones if m["name"] == matched)
        if uncheck:
            if ms.get("done"):
                ms["done"] = False
                ms.pop("done_at", None)
                ms.pop("by", None)
                modified = True
                print(f"⏳ Reverted milestone to PENDING: '{matched}'")
            else:
                print(f"ℹ️ Milestone '{matched}' was not completed.")
        else:
            if not ms.get("done"):
                ms.update({"done": True, "done_at": today(), "by": author})
                modified = True
                print(f"✅ Marked milestone as COMPLETED: '{matched}'")
            else:
                print(f"ℹ️ Milestone '{matched}' was already completed.")

        if modified or notes:
            action = "reopened" if uncheck else "completed"
            plan.add_log(author, f"Milestone '{matched}' {action}" + (f": {notes}" if notes else ""))
            modified = True

    if modified:
        done = len(plan.done_milestones)
        if done == len(milestones):
            if auto_complete or meta.get("status") in ("active", "in_progress"):
                meta["status"] = "completed"
                plan.add_log(store.author(), "All milestones completed", status="completed")
                print("🎉 All milestones completed! Plan status automatically updated to COMPLETED.")
        elif meta.get("status") == "active" and done > 0:
            meta["status"] = "in_progress"
            print("⚡ Plan status updated to IN_PROGRESS.")
        meta["updated_at"] = now_iso()
        store.save(plan)
        store.sync_index()
        for message in sync_plan_items(store.root, plan):
            print(message)

    print("=" * 80)
    print(f"📊 MILESTONE TRACKER: [{meta.get('project', store.project)}] {meta.get('title', pid)}")
    print(f"   • Plan ID : {pid} | Status: {meta.get('status', 'active').upper()} | Version: {meta.get('version') or '-'}")
    print(f"   • Progress: {progress_bar(len(plan.done_milestones), len(milestones), 24)}")
    print(f"   • File    : {plan.path}")
    print("-" * 80)
    print("📋 MILESTONE CHECKLIST:")
    for idx, m in enumerate(milestones, 1):
        icon = "✅ [X]" if m.get("done") else "⏳ [ ]"
        extra = f"  ({m.get('done_at')}, {m.get('by')})" if m.get("done") and m.get("done_at") else ""
        print(f"   {icon} {idx}. {m['name']}{extra}")
    print("=" * 80 + "\n")
    if modified:
        print("👉 Commit the updated plan file.\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Track granular milestone progress in plan files")
    parser.add_argument("--id", type=str, default=None, help="Target Plan ID (default: the project's latest active plan)")
    parser.add_argument("--project", type=str, default=None, help="Project name (default: from the repository)")
    parser.add_argument("--dir", type=str, default=None, help="Project directory (default: current)")
    parser.add_argument("--milestone", type=str, default=None, help="Milestone name to mark as completed")
    parser.add_argument("--uncheck", action="store_true", help="Revert milestone to pending")
    parser.add_argument("--notes", type=str, default=None, help="Notes or rationale for milestone update")
    parser.add_argument("--list", action="store_true", help="List milestones and progress without modifying")
    parser.add_argument("--auto-complete", action="store_true", help="Automatically set plan status to completed if all milestones are done")
    args = parser.parse_args()

    check_milestone(
        store=PlanStore(args.dir, args.project),
        plan_id=args.id,
        milestone_name=args.milestone,
        uncheck=args.uncheck,
        notes=args.notes,
        list_only=args.list,
        auto_complete=args.auto_complete,
    )
