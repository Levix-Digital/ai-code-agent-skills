"""
Finalize Session - Session Memory Skill
Consolidates the active session's full text and previous master summary of a project into a
single, comprehensive master summary ("<project>::master_summary") and cleans up that
project's temporary chunk records. Other projects are never touched.
"""
import argparse
import datetime
import os
import sys

from _common import get_collection, master_id, resolve_project, setup_stdout


def finalize_session(project: str, new_master_summary: str, version: str = None, keep_legacy: bool = False):
    setup_stdout()
    col = get_collection()
    date_str = datetime.date.today().isoformat()

    # 1. Update the project's single Master Summary
    metadata = {
        "project": project,
        "type": "master_summary",
        "last_updated": date_str,
        "topic": "consolidated_master_summary"
    }
    if version:
        metadata["version"] = version
    col.upsert(ids=[master_id(project)], documents=[new_master_summary], metadatas=[metadata])
    print(f"✅ [{project}] Master Summary consolidated and updated in ChromaDB (Doc ID: '{master_id(project)}')!")

    # 2. Remove this project's temporary turn chunks if not keeping legacy
    if not keep_legacy:
        chunks = col.get(where={"$and": [{"project": project}, {"type": "current_session_chunk"}]})
        if chunks["ids"]:
            col.delete(ids=chunks["ids"])
            print(f"🧹 [{project}] Cleaned up {len(chunks['ids'])} temporary session turn chunk(s) (now consolidated into master summary).")

    print(f"Entries for project '{project}': {len(col.get(where={'project': project})['ids'])} | Total entries: {col.count()}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Finalize session and update the project's consolidated master summary in ChromaDB")
    parser.add_argument("--project", type=str, default=None, help="Project name (default: git repository root name)")
    parser.add_argument("--summary", type=str, default=None, help="Full consolidated master summary text")
    parser.add_argument("--file", type=str, default=None, help="Path to text/markdown file containing master summary")
    parser.add_argument("--version", type=str, default=None, help="Optional project version the summary refers to")
    parser.add_argument("--keep-legacy", action="store_true", help="Do not delete raw turn chunks")
    args = parser.parse_args()

    if args.file and os.path.exists(args.file):
        with open(args.file, "r", encoding="utf-8") as f:
            summary = f.read()
    elif args.summary:
        summary = args.summary
    else:
        print("Error: Either --summary or --file must be specified.")
        sys.exit(1)

    finalize_session(resolve_project(args.project), summary, args.version, args.keep_legacy)
