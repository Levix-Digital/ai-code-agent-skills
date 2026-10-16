"""
List Plans Utility - plan-memory Skill
Lists all architectural plans and roadmaps stored in ChromaDB, optionally filtered by project or status.
"""
import os
import sys
import argparse
import chromadb

from _common import get_db_path, normalize_project

def list_plans(project: str = None, status: str = None):
    if sys.stdout.encoding != 'utf-8':
        try:
            sys.stdout.reconfigure(encoding='utf-8')
        except Exception:
            pass

    db_path = get_db_path()
    if not os.path.exists(db_path):
        print("No plans database found yet.")
        return

    client = chromadb.PersistentClient(path=db_path)
    col = client.get_or_create_collection("project_plans")
    total = col.count()
    if total == 0:
        print("No plans stored in ChromaDB yet.")
        return

    where_filter = {}
    if project:
        where_filter["project"] = project
    if status:
        where_filter["status"] = status

    kwargs = {}
    if len(where_filter) == 1:
        kwargs["where"] = where_filter
    elif len(where_filter) > 1:
        kwargs["where"] = {"$and": [{k: v} for k, v in where_filter.items()]}

    results = col.get(**kwargs)
    ids = results.get("ids", [])
    metas = results.get("metadatas", [])

    print("=" * 90)
    print(f"📋 ARCHITECTURAL PLANS & ROADMAPS IN CHROMADB (Total: {len(ids)} of {total})")
    print("=" * 90)

    if not ids:
        print("No plans matching the filter criteria were found.")
        return

    for pid, meta in zip(ids, metas):
        p_name = meta.get("project", "Unknown")
        p_title = meta.get("title", "Untitled")
        p_ver = meta.get("version", "-")
        p_status = meta.get("status", "active")
        p_date = meta.get("created_at", "-")
        p_sid = meta.get("session_id", "-")
        p_size = meta.get("content_length", 0)
        p_milestones = meta.get("milestones", "")

        p_lineage_sup = meta.get("supersedes", "")
        p_lineage_by = meta.get("superseded_by", "")
        p_completed_ms = [m.strip() for m in meta.get("completed_milestones", "").split(",") if m.strip()]
        all_ms = [m.strip() for m in p_milestones.split(",") if m.strip()]

        status_emoji = {
            "active": "🟢",
            "in_progress": "⚡",
            "completed": "✅",
            "superseded": "📦",
            "draft": "📝"
        }.get(p_status, "📌")

        print(f"{status_emoji} [{p_name}] {p_title}")
        print(f"   • Plan ID    : {pid}")
        print(f"   • Version    : {p_ver} | Status: {p_status.upper()} | Size: {p_size} chars")
        print(f"   • Session ID : {p_sid}")
        print(f"   • Date       : {p_date}")
        if p_lineage_sup:
            print(f"   • Supersedes : {p_lineage_sup}")
        if p_lineage_by:
            print(f"   • Superseded by: {p_lineage_by}")
        if all_ms:
            ratio = len(p_completed_ms) / len(all_ms)
            bar_len = int(round(16 * ratio))
            bar = "█" * bar_len + "░" * (16 - bar_len)
            print(f"   • Progress   : [{bar}] {ratio*100:.1f}% ({len(p_completed_ms)}/{len(all_ms)} Marcos)")
            print(f"   • Milestones : {p_milestones}")
        print("-" * 90)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="List architectural plans stored in ChromaDB")
    parser.add_argument("--project", type=str, default=None, help="Filter by project name")
    parser.add_argument("--status", type=str, default=None, help="Filter by status (active, in_progress, completed, superseded)")
    args = parser.parse_args()

    list_plans(project=normalize_project(args.project), status=args.status)
