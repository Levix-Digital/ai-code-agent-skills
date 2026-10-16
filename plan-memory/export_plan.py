"""
Export Plan Utility - plan-memory Skill
Exports a plan file (docs/plans/<plan_id>.md) to JSON, or to a Markdown copy
elsewhere (with or without frontmatter). Plans already live in the repository;
use this to hand a plan to another tool or document.
"""
import argparse
import json
import os
import shutil
import sys

from _store import PlanStore, setup_stdout


def export_plan(store: PlanStore, plan_id: str = None, output_path: str = None, output_dir: str = None,
                format_type: str = "markdown", include_frontmatter: bool = True):
    setup_stdout()
    plan = store.get(plan_id) if plan_id else store.latest()
    if not plan:
        print(f"❌ Error: Plan not found (plan_id='{plan_id}', project='{store.project}')", file=sys.stderr)
        sys.exit(1)

    pid = plan.meta["plan_id"]
    ext = ".json" if format_type == "json" else ".md"
    target = os.path.abspath(output_path or os.path.join(output_dir or os.getcwd(), f"{pid}{ext}"))
    if target == os.path.abspath(plan.path):
        print("❌ Error: target is the plan file itself.", file=sys.stderr)
        sys.exit(1)
    os.makedirs(os.path.dirname(target), exist_ok=True)

    if format_type == "json":
        with open(target, "w", encoding="utf-8") as f:
            json.dump({**plan.meta, "content": plan.body}, f, indent=2, ensure_ascii=False)
    elif include_frontmatter:
        shutil.copyfile(plan.path, target)
    else:
        with open(target, "w", encoding="utf-8") as f:
            f.write(plan.body)

    print("=" * 80)
    print("📦 PLAN EXPORTED")
    print(f"   • Plan ID : {pid}")
    print(f"   • Format  : {format_type.upper()}")
    print(f"   • File    : {target}")
    print(f"   • Size    : {os.path.getsize(target)} bytes")
    print("=" * 80 + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Export a plan to JSON or to a Markdown copy")
    parser.add_argument("--id", type=str, default=None, help="Target Plan ID (default: the project's latest active plan)")
    parser.add_argument("--project", type=str, default=None, help="Project name (default: from the repository)")
    parser.add_argument("--dir", type=str, default=None, help="Project directory (default: current)")
    parser.add_argument("--output", type=str, default=None, help="Exact destination file path")
    parser.add_argument("--output-dir", type=str, default=None, help="Directory to save the export")
    parser.add_argument("--format", type=str, default="markdown", choices=["markdown", "json"], help="Output format")
    parser.add_argument("--no-frontmatter", action="store_true", help="Exclude YAML frontmatter from a markdown export")
    args = parser.parse_args()

    export_plan(PlanStore(args.dir, args.project), plan_id=args.id, output_path=args.output, output_dir=args.output_dir,
                format_type=args.format, include_frontmatter=not args.no_frontmatter)
