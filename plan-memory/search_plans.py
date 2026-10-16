"""
Search Plans Utility - plan-memory Skill
Semantic vector search across the project's plans. The local index is synced
from docs/plans before every search.
"""
import argparse

from _store import PlanStore, setup_stdout


def search_plans(store: PlanStore, query: str, limit: int = 5, all_projects: bool = False):
    setup_stdout()
    col = store.sync_index()
    where = None if all_projects else {"project": store.project}
    count = len(col.get(where=where)["ids"]) if where else col.count()
    if count == 0:
        print(f"No plans indexed for '{store.project}' ({store.plans_dir}).")
        return

    results = col.query(query_texts=[query], n_results=min(limit, count), where=where)
    ids = results.get("ids", [[]])[0]
    docs = results.get("documents", [[]])[0]
    metas = results.get("metadatas", [[]])[0]
    distances = (results.get("distances") or [[0.0] * len(ids)])[0]

    print("=" * 90)
    print(f"🔍 SEMANTIC SEARCH IN PLANS: '{query}' (Found: {len(ids)})")
    print("=" * 90)
    if not ids:
        print("No matching plans found.")
        return

    for pid, doc, meta, dist in zip(ids, docs, metas, distances):
        sim = 1.0 / (1.0 + max(0.0, dist))
        print(f"🎯 Score: {sim:.3f} | [{meta.get('project')}] {meta.get('title') or 'Untitled'} ({meta.get('version') or '-'})")
        print(f"   • Plan ID    : {pid} | Status: {(meta.get('status') or 'active').upper()}")
        print(f"   • File       : {meta.get('path')}")
        preview = doc.strip().replace("\n", " ")
        if len(preview) > 300:
            preview = preview[:300] + "..."
        print(f"   • Preview    : {preview}")
        print("-" * 90)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Semantic search over the project's plans")
    parser.add_argument("--query", type=str, required=True, help="Search query or concept")
    parser.add_argument("--limit", type=int, default=5, help="Maximum number of results to return")
    parser.add_argument("--project", type=str, default=None, help="Project name (default: from the repository)")
    parser.add_argument("--dir", type=str, default=None, help="Project directory (default: current)")
    parser.add_argument("--all-projects", action="store_true", help="Search every project indexed on this machine")
    args = parser.parse_args()

    search_plans(PlanStore(args.dir, args.project), query=args.query, limit=args.limit, all_projects=args.all_projects)
