"""
Migrate Legacy Plans - plan-memory Skill
Copies every plan (document, metadata and embedding) from an old database into the shared
database outside the skill folder, normalizing project names. Plan IDs are kept unchanged,
so `supersedes` / `parent_plan_id` lineage keeps working. The source database is left untouched.
"""
import argparse
import sys

import chromadb

from _common import get_db_path, normalize_project

COLLECTION = "project_plans"


def migrate(source_path: str, renames: dict, dry_run: bool):
    if sys.stdout.encoding != "utf-8":
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass

    src = chromadb.PersistentClient(path=source_path).get_or_create_collection(COLLECTION)
    records = src.get(include=["documents", "metadatas", "embeddings"])

    summary = {}
    metas = []
    for meta in records["metadatas"]:
        meta = dict(meta or {})
        old = meta.get("project", "")
        new = normalize_project(old) or "unknown"
        new = renames.get(new, new)
        meta["project"] = new
        metas.append(meta)
        summary[(old, new)] = summary.get((old, new), 0) + 1

    for (old, new), n in sorted(summary.items()):
        print(f"{'(dry-run) ' if dry_run else ''}project '{old}' -> '{new}': {n} plan(s)")

    if not dry_run and records["ids"]:
        dst = chromadb.PersistentClient(path=get_db_path()).get_or_create_collection(
            name=COLLECTION,
            metadata={"description": "Persistent vector database for complete, uncompressed project architecture plans and roadmaps"}
        )
        dst.upsert(ids=records["ids"], documents=records["documents"], metadatas=metas,
                   embeddings=records["embeddings"])
        print(f"Destination DB: {get_db_path()} | Total plans: {dst.count()}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Migrate plans into the shared plan-memory database, normalizing project names")
    parser.add_argument("--source", type=str, required=True, help="Path to the legacy ChromaDB directory")
    parser.add_argument("--rename", action="append", default=[], metavar="OLD=NEW",
                        help="Extra project rename applied after normalization (e.g. anycontext=any-context); repeatable")
    parser.add_argument("--dry-run", action="store_true", help="Only show what would be migrated")
    args = parser.parse_args()

    renames = {}
    for item in args.rename:
        old, _, new = item.partition("=")
        renames[normalize_project(old)] = normalize_project(new)
    migrate(args.source, renames, args.dry_run)
