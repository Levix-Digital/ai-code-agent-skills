"""
Export Plan Utility - plan-memory Skill
Exports uncompressed architectural plans from ChromaDB into markdown files
with structured YAML frontmatter (milestones, lineage, metadata) or JSON format.
"""
import os
import sys
import argparse
import json
import chromadb

from _common import get_db_path, normalize_project

def find_plan(col, plan_id: str = None, project: str = None):
    if plan_id:
        res = col.get(ids=[plan_id])
        if res and res["ids"]:
            return res["ids"][0], res["documents"][0], res["metadatas"][0]
        return None, None, None
    elif project:
        res = col.get(where={"project": project})
        if res and res["ids"]:
            items = list(zip(res["ids"], res["documents"], res["metadatas"]))
            def sort_key(item):
                st = (item[2] or {}).get("status", "")
                st_prio = 0 if st in ["active", "in_progress"] else 1
                dt = (item[2] or {}).get("created_at", "")
                return (st_prio, -len(dt), dt)
            items.sort(key=sort_key)
            return items[0]
    return None, None, None

def export_plan(
    plan_id: str = None,
    project: str = None,
    output_path: str = None,
    output_dir: str = None,
    format_type: str = "markdown",
    include_frontmatter: bool = True,
):
    if sys.stdout.encoding != 'utf-8':
        try:
            sys.stdout.reconfigure(encoding='utf-8')
        except Exception:
            pass

    db_path = get_db_path()
    if not os.path.exists(db_path):
        print("❌ Error: No plans database found.", file=sys.stderr)
        sys.exit(1)

    client = chromadb.PersistentClient(path=db_path)
    col = client.get_or_create_collection("project_plans")

    pid, doc, meta = find_plan(col, plan_id=plan_id, project=project)
    if not pid:
        target = f"plan_id='{plan_id}'" if plan_id else f"project='{project}'"
        print(f"❌ Error: Plan not found for {target}", file=sys.stderr)
        sys.exit(1)

    meta = meta or {}
    all_milestones = [m.strip() for m in meta.get("milestones", "").split(",") if m.strip()]
    completed_milestones = [m.strip() for m in meta.get("completed_milestones", "").split(",") if m.strip()]

    # Determine destination file path
    if output_path:
        target_file = os.path.abspath(output_path)
    else:
        out_base = output_dir or os.path.join(os.getcwd(), "docs", "plans")
        ext = ".json" if format_type == "json" else ".md"
        target_file = os.path.join(os.path.abspath(out_base), f"{pid}{ext}")

    os.makedirs(os.path.dirname(target_file), exist_ok=True)

    if format_type == "json":
        export_data = {
            "plan_id": pid,
            "project": meta.get("project"),
            "title": meta.get("title"),
            "version": meta.get("version"),
            "status": meta.get("status"),
            "session_id": meta.get("session_id"),
            "created_at": meta.get("created_at"),
            "updated_at": meta.get("updated_at"),
            "tags": [t.strip() for t in meta.get("tags", "").split(",") if t.strip()],
            "milestones": [
                {"name": m, "completed": m in completed_milestones}
                for m in all_milestones
            ],
            "lineage": {
                "parent_plan_id": meta.get("parent_plan_id") or None,
                "supersedes": meta.get("supersedes") or None,
                "superseded_by": meta.get("superseded_by") or None,
            },
            "status_notes": meta.get("status_notes", ""),
            "content": doc
        }
        with open(target_file, "w", encoding="utf-8") as f:
            json.dump(export_data, f, indent=2, ensure_ascii=False)
    else:
        # Markdown format
        frontmatter = ""
        if include_frontmatter:
            milestone_yaml = "\n".join([
                f"  - name: \"{m}\"\n    completed: {'true' if m in completed_milestones else 'false'}"
                for m in all_milestones
            ])
            tags_yaml = "\n".join([f"  - \"{t.strip()}\"" for t in meta.get("tags", "").split(",") if t.strip()])
            
            frontmatter = (
                "---\n"
                f"plan_id: \"{pid}\"\n"
                f"project: \"{meta.get('project', '')}\"\n"
                f"title: \"{meta.get('title', '')}\"\n"
                f"version: \"{meta.get('version', '')}\"\n"
                f"status: \"{meta.get('status', 'active')}\"\n"
                f"session_id: \"{meta.get('session_id', '')}\"\n"
                f"created_at: \"{meta.get('created_at', '')}\"\n"
                f"updated_at: \"{meta.get('updated_at', '')}\"\n"
                f"supersedes: \"{meta.get('supersedes', '')}\"\n"
                f"superseded_by: \"{meta.get('superseded_by', '')}\"\n"
                f"tags:\n{tags_yaml if tags_yaml else '  []'}\n"
                f"milestones:\n{milestone_yaml if milestone_yaml else '  []'}\n"
                "---\n\n"
            )

        full_output = frontmatter + doc.strip() + "\n"
        with open(target_file, "w", encoding="utf-8") as f:
            f.write(full_output)

    print("=" * 80)
    print(f"📦 PLAN SNAPSHOT EXPORTED SUCCESSFULLY")
    print(f"   • Plan ID : {pid}")
    print(f"   • Project : {meta.get('project')}")
    print(f"   • Format  : {format_type.upper()}")
    print(f"   • File    : {target_file}")
    print(f"   • Size    : {os.path.getsize(target_file)} bytes")
    print("=" * 80 + "\n")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Export architectural plan from ChromaDB to markdown/json snapshot")
    parser.add_argument("--id", type=str, default=None, help="Target Plan ID")
    parser.add_argument("--project", type=str, default=None, help="Target project (exports active plan)")
    parser.add_argument("--output", type=str, default=None, help="Exact destination file path")
    parser.add_argument("--output-dir", type=str, default=None, help="Directory to save the exported snapshot")
    parser.add_argument("--format", type=str, default="markdown", choices=["markdown", "json"], help="Output format")
    parser.add_argument("--no-frontmatter", action="store_true", help="Exclude YAML frontmatter from markdown export")

    args = parser.parse_args()

    if not args.id and not args.project:
        print("❌ Error: Must specify either --id <plan_id> or --project <project_name>", file=sys.stderr)
        sys.exit(1)

    export_plan(
        plan_id=args.id,
        project=normalize_project(args.project),
        output_path=args.output,
        output_dir=args.output_dir,
        format_type=args.format,
        include_frontmatter=not args.no_frontmatter
    )
