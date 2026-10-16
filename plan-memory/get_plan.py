"""
Get Plan Utility - plan-memory Skill
Retrieves and displays the 100% full, uncompressed content of an architectural plan by its Plan ID.
Supports exporting directly to a file.
"""
import os
import sys
import argparse
import chromadb

def get_plan(plan_id: str = None, project: str = None, output_file: str = None):
    if sys.stdout.encoding != 'utf-8':
        try:
            sys.stdout.reconfigure(encoding='utf-8')
        except Exception:
            pass

    db_path = os.path.join(os.path.dirname(__file__), "db")
    if not os.path.exists(db_path):
        print("No plans database found.", file=sys.stderr)
        sys.exit(1)

    client = chromadb.PersistentClient(path=db_path)
    col = client.get_or_create_collection("project_plans")

    doc = None
    meta = None
    target_id = None

    if plan_id:
        res = col.get(ids=[plan_id])
        if res and res["documents"] and len(res["documents"]) > 0:
            doc = res["documents"][0]
            meta = res["metadatas"][0]
            target_id = plan_id
    elif project:
        # Retrieve the latest plan for the given project
        res = col.get(where={"project": project})
        if res and res["documents"] and len(res["documents"]) > 0:
            # Sort by created_at descending if available
            combined = list(zip(res["ids"], res["documents"], res["metadatas"]))
            combined.sort(key=lambda x: (x[2] or {}).get("created_at", ""), reverse=True)
            target_id, doc, meta = combined[0]

    if not doc:
        print(f"❌ Error: Plan not found (plan_id='{plan_id}', project='{project}')", file=sys.stderr)
        sys.exit(1)

    if output_file:
        out_dir = os.path.dirname(os.path.abspath(output_file))
        if out_dir:
            os.makedirs(out_dir, exist_ok=True)
        with open(output_file, "w", encoding="utf-8") as f:
            f.write(doc)
        print(f"✅ Plan '{target_id}' exported successfully to: {output_file}")
    else:
        p_lineage_sup = meta.get("supersedes", "")
        p_lineage_by = meta.get("superseded_by", "")
        p_completed_ms = [m.strip() for m in meta.get("completed_milestones", "").split(",") if m.strip()]
        all_ms = [m.strip() for m in meta.get("milestones", "").split(",") if m.strip()]

        print("=" * 90)
        print(f"📜 FULL ARCHITECTURAL PLAN: {target_id}")
        print(f"   • Project    : {meta.get('project')} | Version: {meta.get('version')} | Status: {meta.get('status', 'active').upper()}")
        print(f"   • Session ID : {meta.get('session_id')}")
        if p_lineage_sup:
            print(f"   • Supersedes : {p_lineage_sup}")
        if p_lineage_by:
            print(f"   • Superseded by: {p_lineage_by}")
        if all_ms:
            ratio = len(p_completed_ms) / len(all_ms)
            bar_len = int(round(16 * ratio))
            bar = "█" * bar_len + "░" * (16 - bar_len)
            print(f"   • Milestones : [{bar}] {ratio*100:.1f}% ({len(p_completed_ms)}/{len(all_ms)} Concluídos)")
        print("=" * 90 + "\n")
        print(doc)
        print("\n" + "=" * 90)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Get full uncompressed architectural plan from ChromaDB")
    parser.add_argument("--id", type=str, default=None, help="Specific Plan ID")
    parser.add_argument("--project", type=str, default=None, help="Get latest plan for project")
    parser.add_argument("--output", type=str, default=None, help="Export plan to a local markdown file")
    args = parser.parse_args()

    if not args.id and not args.project:
        print("❌ Error: Must specify either --id <plan_id> or --project <project_name>", file=sys.stderr)
        sys.exit(1)

    get_plan(plan_id=args.id, project=args.project, output_file=args.output)
