"""
Get Plan Utility - plan-memory Skill
Displays the 100% full, uncompressed content of a plan by its Plan ID,
or the project's latest active plan. Supports copying the body to a file.
"""
import argparse
import os
import sys

from _store import PlanStore, progress_bar, setup_stdout


def get_plan(store: PlanStore, plan_id: str = None, output_file: str = None):
    setup_stdout()
    plan = store.get(plan_id) if plan_id else store.latest()
    if not plan:
        print(f"❌ Error: Plan not found (plan_id='{plan_id}', project='{store.project}') in {store.plans_dir}", file=sys.stderr)
        sys.exit(1)

    m = plan.meta
    if output_file:
        out_dir = os.path.dirname(os.path.abspath(output_file))
        os.makedirs(out_dir, exist_ok=True)
        with open(output_file, "w", encoding="utf-8") as f:
            f.write(plan.body)
        print(f"✅ Plan '{m['plan_id']}' body written to: {output_file}")
        return

    print("=" * 90)
    print(f"📜 FULL ARCHITECTURAL PLAN: {m['plan_id']}")
    print(f"   • Project    : {m.get('project', store.project)} | Version: {m.get('version') or '-'} | Status: {m.get('status', 'active').upper()}")
    print(f"   • Author     : {m.get('author') or '-'} | Created: {m.get('created_at') or '-'} | Updated: {m.get('updated_at') or '-'}")
    print(f"   • File       : {plan.path}")
    if m.get("supersedes"):
        print(f"   • Supersedes : {m['supersedes']}")
    if m.get("superseded_by"):
        print(f"   • Superseded by: {m['superseded_by']}")
    if plan.milestones:
        print(f"   • Milestones : {progress_bar(len(plan.done_milestones), len(plan.milestones))}")
    print("=" * 90 + "\n")
    print(plan.body)
    print("\n" + "=" * 90)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Get a full, uncompressed plan from docs/plans")
    parser.add_argument("--id", type=str, default=None, help="Specific Plan ID (default: the project's latest active plan)")
    parser.add_argument("--project", type=str, default=None, help="Project name (default: from the repository)")
    parser.add_argument("--dir", type=str, default=None, help="Project directory (default: current)")
    parser.add_argument("--output", type=str, default=None, help="Write the plan body to a local markdown file")
    args = parser.parse_args()

    get_plan(PlanStore(args.dir, args.project), plan_id=args.id, output_file=args.output)
