"""
Update Plan Status Utility - plan-memory Skill
Updates the lifecycle status of an architectural plan (e.g. active -> completed, in_progress, superseded).
"""
import os
import sys
import argparse
import datetime
import chromadb

def update_plan_status(plan_id: str, new_status: str, notes: str = None):
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

    res = col.get(ids=[plan_id])
    if not res or not res["documents"] or len(res["documents"]) == 0:
        print(f"❌ Error: Plan ID '{plan_id}' not found.", file=sys.stderr)
        sys.exit(1)

    doc = res["documents"][0]
    meta = res["metadatas"][0] or {}

    old_status = meta.get("status", "active")
    meta["status"] = str(new_status)
    meta["updated_at"] = datetime.datetime.now().isoformat()
    if notes:
        existing_notes = meta.get("status_notes", "")
        meta["status_notes"] = f"{existing_notes} [{meta['updated_at']}] {notes}".strip()

    col.upsert(
        ids=[plan_id],
        documents=[doc],
        metadatas=[meta]
    )

    print(f"✅ Plan status updated successfully!")
    print(f"   • Plan ID    : {plan_id}")
    print(f"   • Project    : {meta.get('project')}")
    print(f"   • Title      : {meta.get('title')}")
    print(f"   • Status     : {old_status.upper()} -> {new_status.upper()}")
    if notes:
        print(f"   • Notes      : {notes}")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Update plan status in ChromaDB")
    parser.add_argument("--id", type=str, required=True, help="Plan ID to update")
    parser.add_argument("--status", type=str, required=True, choices=["active", "in_progress", "completed", "superseded", "draft"], help="New status")
    parser.add_argument("--notes", type=str, default=None, help="Optional progress notes / rationale")
    args = parser.parse_args()

    update_plan_status(plan_id=args.id, new_status=args.status, notes=args.notes)
