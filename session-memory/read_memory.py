"""
Read Session Memory - Session Memory Skill
Displays a project's consolidated Master Summary and its active incremental turn chunks.
Use --list-projects to see every project stored in the shared database.
"""
import argparse

from _common import get_collection, get_db_path, master_id, resolve_project, setup_stdout


def list_projects(col):
    counts = {}
    for meta in col.get()["metadatas"]:
        p = (meta or {}).get("project", "<sem projeto>")
        counts[p] = counts.get(p, 0) + 1
    if not counts:
        print("No memory entries found yet.")
    for p, n in sorted(counts.items()):
        print(f"- {p} ({n} entr{'y' if n == 1 else 'ies'})")


def read_project(col, project: str):
    print(f"Project: {project} | DB: {get_db_path()}\n")

    master = col.get(ids=[master_id(project)])
    if master["documents"]:
        print("================================================================================")
        print(f"📜 CONSOLIDATED MASTER SUMMARY [{project}] (All Prior Sessions Unified)")
        print("================================================================================")
        print(master["documents"][0])
        print("\n" + "=" * 80 + "\n")

    chunks = col.get(where={"$and": [{"project": project}, {"type": "current_session_chunk"}]})
    rows = sorted(zip(chunks["ids"], chunks["metadatas"], chunks["documents"]),
                  key=lambda r: (r[1].get("session_id", ""), r[1].get("start_turn", 0)))
    for did, meta, doc in rows:
        print(f"--- Entry: {did} [{meta.get('topic')}] ---")
        print(doc)
        print()

    if not master["documents"] and not rows:
        print(f"No memory entries found for project '{project}'. Existing projects:")
        list_projects(col)


if __name__ == "__main__":
    setup_stdout()
    parser = argparse.ArgumentParser(description="Read a project's session memory from ChromaDB")
    parser.add_argument("--project", type=str, default=None, help="Project name (default: git repository root name)")
    parser.add_argument("--list-projects", action="store_true", help="List projects stored in the database")
    args = parser.parse_args()

    col = get_collection()
    if args.list_projects:
        list_projects(col)
    else:
        read_project(col, resolve_project(args.project))
