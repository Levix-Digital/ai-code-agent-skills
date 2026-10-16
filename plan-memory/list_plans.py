"""
List Plans Utility - plan-memory Skill
Lists the plans in the project's docs/plans folder, optionally filtered by status.
"""
import argparse

from _store import PlanStore, progress_bar, setup_stdout

STATUS_EMOJI = {"active": "🟢", "in_progress": "⚡", "completed": "✅", "superseded": "📦", "draft": "📝", "planned": "🗓️"}


def list_plans(store: PlanStore, status: str = None):
    setup_stdout()
    plans = store.load_all()
    total = len(plans)
    if status:
        plans = [p for p in plans if p.meta.get("status") == status]
    plans.sort(key=lambda p: str(p.meta.get("created_at") or ""), reverse=True)

    print("=" * 90)
    print(f"📋 PLANS OF '{store.project}' (Showing: {len(plans)} of {total}) — {store.plans_dir}")
    print("=" * 90)
    if not plans:
        print("No plans matching the filter criteria were found.")
        return

    for p in plans:
        m = p.meta
        p_status = m.get("status", "active")
        print(f"{STATUS_EMOJI.get(p_status, '📌')} {m.get('title', 'Untitled')}")
        print(f"   • Plan ID    : {m['plan_id']}")
        print(f"   • Version    : {m.get('version') or '-'} | Status: {p_status.upper()} | Size: {len(p.body)} chars")
        print(f"   • Author     : {m.get('author') or '-'} | Created: {m.get('created_at') or '-'}")
        if m.get("supersedes"):
            print(f"   • Supersedes : {m['supersedes']}")
        if m.get("superseded_by"):
            print(f"   • Superseded by: {m['superseded_by']}")
        if p.milestones:
            print(f"   • Progress   : {progress_bar(len(p.done_milestones), len(p.milestones))}")
            print(f"   • Milestones : {', '.join(x['name'] for x in p.milestones)}")
        print("-" * 90)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="List the project's plans (docs/plans)")
    parser.add_argument("--status", type=str, default=None, help="Filter by status (draft, planned, active, in_progress, completed, superseded)")
    parser.add_argument("--project", type=str, default=None, help="Project name (default: from the repository)")
    parser.add_argument("--dir", type=str, default=None, help="Project directory (default: current)")
    args = parser.parse_args()

    list_plans(PlanStore(args.dir, args.project), status=args.status)
