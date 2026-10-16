"""
Migrate Legacy Records - Session Memory Skill
Copies records from an old single-project database (no `project` metadata, unscoped IDs)
into the shared multi-project database, assigning them to --project.
The source database is left untouched.
"""
import argparse

import chromadb

from _common import COLLECTION_NAME, chunk_id, get_collection, get_db_path, master_id, resolve_project, setup_stdout


def migrate(source_path: str, project: str, dry_run: bool):
    setup_stdout()
    src = chromadb.PersistentClient(path=source_path).get_or_create_collection(COLLECTION_NAME)
    records = src.get()
    dst = get_collection()

    for old_id, meta, doc in zip(records["ids"], records["metadatas"], records["documents"]):
        meta = dict(meta or {})
        if meta.get("project"):
            print(f"↷ {old_id}: already has project '{meta['project']}', skipped")
            continue
        meta["project"] = project
        if old_id == "master_summary" or meta.get("type") == "master_summary":
            new_id = master_id(project)
        elif meta.get("type") == "current_session_chunk":
            new_id = chunk_id(project, meta.get("session_id", "legacy"), meta.get("start_turn", 0), meta.get("end_turn", 0))
        else:
            new_id = f"{project}::{old_id}"
        print(f"{'(dry-run) ' if dry_run else ''}{old_id} -> {new_id}")
        if not dry_run:
            dst.upsert(ids=[new_id], documents=[doc], metadatas=[meta])

    print(f"Destination DB: {get_db_path()} | Total entries: {dst.count()}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Migrate legacy single-project session memory into the multi-project database")
    parser.add_argument("--source", type=str, required=True, help="Path to the legacy ChromaDB directory")
    parser.add_argument("--project", type=str, required=True, help="Project the legacy records belong to")
    parser.add_argument("--dry-run", action="store_true", help="Only show what would be migrated")
    args = parser.parse_args()

    migrate(args.source, resolve_project(args.project), args.dry_run)
