"""
Search Plans Utility - plan-memory Skill
Performs semantic vector search across all architectural plans and roadmaps in ChromaDB.
"""
import os
import sys
import argparse
import chromadb

from _common import get_db_path, normalize_project

def search_plans(query: str, project: str = None, limit: int = 5):
    if sys.stdout.encoding != 'utf-8':
        try:
            sys.stdout.reconfigure(encoding='utf-8')
        except Exception:
            pass

    db_path = get_db_path()
    if not os.path.exists(db_path):
        print("No plans database found.", file=sys.stderr)
        return

    client = chromadb.PersistentClient(path=db_path)
    col = client.get_or_create_collection("project_plans")
    if col.count() == 0:
        print("No plans stored in ChromaDB yet.")
        return

    where_filter = {}
    if project:
        where_filter["project"] = project

    kwargs = {
        "query_texts": [query],
        "n_results": min(limit, col.count())
    }
    if where_filter:
        kwargs["where"] = where_filter

    results = col.query(**kwargs)

    ids = results.get("ids", [[]])[0]
    docs = results.get("documents", [[]])[0]
    metas = results.get("metadatas", [[]])[0]
    distances = results.get("distances", [[]])[0] if "distances" in results else [0.0] * len(ids)

    print("=" * 90)
    print(f"🔍 SEMANTIC SEARCH IN PLANS: '{query}' (Found: {len(ids)})")
    print("=" * 90)

    if not ids:
        print("No matching plans found.")
        return

    for pid, doc, meta, dist in zip(ids, docs, metas, distances):
        sim = 1.0 / (1.0 + max(0.0, dist))
        p_name = meta.get("project", "Unknown")
        p_title = meta.get("title", "Untitled")
        p_ver = meta.get("version", "-")
        p_status = meta.get("status", "active")
        p_date = meta.get("created_at", "-")
        p_sid = meta.get("session_id", "-")

        print(f"🎯 Score: {sim:.3f} | [{p_name}] {p_title} ({p_ver})")
        print(f"   • Plan ID    : {pid} | Status: {p_status.upper()}")
        print(f"   • Session ID : {p_sid} | Date: {p_date}")
        
        # Snippet preview (first 400 chars)
        preview = doc.strip().replace("\n", " ")
        if len(preview) > 300:
            preview = preview[:300] + "..."
        print(f"   • Preview    : {preview}")
        print("-" * 90)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Search architectural plans in ChromaDB via semantic vector similarity")
    parser.add_argument("--query", type=str, required=True, help="Search query or concept")
    parser.add_argument("--project", type=str, default=None, help="Filter search to specific project")
    parser.add_argument("--limit", type=int, default=5, help="Maximum number of results to return")
    args = parser.parse_args()

    search_plans(query=args.query, project=normalize_project(args.project), limit=args.limit)
