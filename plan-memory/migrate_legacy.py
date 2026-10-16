"""
Migrate Legacy Plans - plan-memory Skill
One-time migration from the old ChromaDB-only storage (collection `project_plans`)
to plan files in the repository (docs/plans/<plan_id>.md), reconciling plan files
that already exist there:

- plan only in the database          -> new file with the database metadata
- file with the same ID as a plan    -> database metadata; body from whichever is newer
- file matching a plan's content     -> renamed (git mv) to the database ID, which the
  under another ID                      lineage fields reference; then as above
- file with no matching plan         -> kept, with minimal metadata (--orphan-status)

The source database is never modified. The local search index is rebuilt at the end.
"""
import argparse
import datetime
import difflib
import os
import re
import subprocess
import sys

import chromadb

from _store import Plan, PlanStore, now_iso, read_plan, setup_stdout, split_csv, split_frontmatter

LEGACY_COLLECTION = "project_plans"
NOTE_RE = re.compile(r"\[(\d{4}-\d{2}-\d{2}T[\d:.]+)\]\s*(.*?)(?=\s*\[\d{4}-\d{2}-\d{2}T|\Z)", re.S)


def iso_seconds(value: str):
    if not value:
        return None
    try:
        return datetime.datetime.fromisoformat(value).replace(microsecond=0).isoformat()
    except ValueError:
        return value


def git_date(root: str, path: str, first: bool = False):
    args = ["git", "log", "--format=%cI", "--", path]
    try:
        out = subprocess.run(args, cwd=root, capture_output=True, text=True, check=True).stdout.split()
    except Exception:
        return None
    if not out:
        return None
    value = out[-1] if first else out[0]
    return datetime.datetime.fromisoformat(value).replace(tzinfo=None).isoformat()


def file_meta_from_db(m: dict, project: str, author: str) -> dict:
    done = set(split_csv(m.get("completed_milestones")))
    log = [{"at": iso_seconds(at), "note": note.strip()} for at, note in NOTE_RE.findall(m.get("status_notes") or "")]

    def opt(key, empty=("", "unknown", "unversioned")):
        v = m.get(key)
        return None if v in empty or v is None else v

    return {
        "plan_id": m["plan_id"],
        "project": project,
        "title": m.get("title") or m["plan_id"],
        "version": opt("version"),
        "status": m.get("status") or "active",
        "author": author,
        "created_at": iso_seconds(m.get("created_at")),
        "updated_at": iso_seconds(m.get("updated_at")),
        "session_id": opt("session_id"),
        "tags": split_csv(m.get("tags")),
        "supersedes": opt("supersedes"),
        "superseded_by": opt("superseded_by"),
        "parent_plan_id": opt("parent_plan_id"),
        "milestones": [{"name": n, "done": n in done} for n in split_csv(m.get("milestones"))],
        "status_log": log,
    }


def first_heading(body: str, default: str) -> str:
    for line in body.splitlines():
        if line.startswith("# "):
            return line[2:].strip()
    return default


def similarity(a: str, b: str) -> float:
    return difflib.SequenceMatcher(None, a[:3000], b[:3000]).ratio()


def migrate(store: PlanStore, source_path: str, orphan_status: str, threshold: float, dry_run: bool):
    setup_stdout()
    tag = "(dry-run) " if dry_run else ""
    author = store.author()

    col = chromadb.PersistentClient(path=source_path).get_collection(LEGACY_COLLECTION)
    res = col.get(where={"project": store.project})
    db = {i: (d, dict(m or {}, plan_id=i)) for i, d, m in zip(res["ids"], res["documents"], res["metadatas"])}
    print(f"Source DB: {len(db)} plan(s) for project '{store.project}' | Target: {store.plans_dir}")

    files = {}
    if os.path.isdir(store.plans_dir):
        for name in sorted(os.listdir(store.plans_dir)):
            if name.endswith(".md"):
                path = os.path.join(store.plans_dir, name)
                with open(path, "r", encoding="utf-8") as f:
                    _, body = split_frontmatter(f.read())
                files[name[:-3]] = (path, body.strip())
    print(f"Existing plan files: {len(files)}\n")

    # 1. Pair files with database plans (same ID, else by content)
    pairs, orphans, claimed = {}, [], set(files) & set(db)
    for fid, (path, body) in files.items():
        if fid in db:
            pairs[fid] = (fid, path, body)
            continue
        candidates = [(similarity(db[i][0].strip(), body), i) for i in db if i not in claimed]
        score, best = max(candidates, default=(0.0, None))
        if best and score >= threshold:
            claimed.add(best)
            pairs[best] = (fid, path, body)
            print(f"{tag}rename  {fid}.md -> {best}.md  (content match {score:.2f})")
        else:
            orphans.append((fid, path, body))

    # 2. Write every database plan as a file
    written = 0
    for pid, (db_body, m) in db.items():
        meta = file_meta_from_db(m, store.project, author)
        body = db_body.strip()
        if pid in pairs:
            fid, path, file_body = pairs[pid]
            if file_body != body:
                file_date = git_date(store.root, path) or ""
                if file_date >= (meta["updated_at"] or ""):
                    body = file_body
                    choice = f"file (last commit {file_date[:10]})"
                else:
                    choice = f"database (updated {meta['updated_at'][:10]})"
                print(f"{tag}body    {pid}: differs, keeping {choice}")
            if fid != pid and not dry_run:
                subprocess.run(["git", "mv", path, store.path_for(pid)], cwd=store.root, check=True)
        meta["status_log"].append({"at": now_iso(), "by": author, "note": "Migrated from the local plan-memory database to docs/plans"})
        if not dry_run:
            store.save(Plan(meta, body))
        written += 1

    # 3. Files with no matching database plan
    for fid, path, body in orphans:
        created = git_date(store.root, path, first=True)
        meta = {
            "plan_id": fid, "project": store.project, "title": first_heading(body, fid), "version": None,
            "status": orphan_status, "author": author, "created_at": created, "updated_at": git_date(store.root, path),
            "session_id": None, "tags": [], "supersedes": None, "superseded_by": None, "parent_plan_id": None,
            "milestones": [],
            "status_log": [{"at": now_iso(), "by": author, "status": orphan_status,
                            "note": "Plan file had no entry in the plan-memory database; metadata added during migration"}],
        }
        print(f"{tag}orphan  {fid}.md -> status '{orphan_status}'")
        if not dry_run:
            store.save(Plan(meta, body))

    print(f"\n{tag}{written} plan(s) from the database, {len(orphans)} orphan file(s): {written + len(orphans)} plan files.")
    if not dry_run:
        store.sync_index(rebuild=True, quiet=False)
        bad = [p.meta["plan_id"] for p in store.load_all() for k in ("supersedes", "superseded_by", "parent_plan_id")
               if p.meta.get(k) and not os.path.isfile(store.path_for(p.meta[k]))]
        if bad:
            print(f"⚠️ Lineage references to missing plans in: {sorted(set(bad))}")
        print("👉 Review the changes in docs/plans and commit them.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Migrate plans from the old plan-memory database to docs/plans files")
    parser.add_argument("--source", type=str, default=None, help="Legacy ChromaDB folder (default: the plan-memory database path)")
    parser.add_argument("--project", type=str, default=None, help="Project name (default: from the repository)")
    parser.add_argument("--dir", type=str, default=None, help="Project directory (default: current)")
    parser.add_argument("--orphan-status", type=str, default="completed", help="Status for plan files with no database entry")
    parser.add_argument("--threshold", type=float, default=0.9, help="Content similarity needed to pair a file with a plan under another ID")
    parser.add_argument("--dry-run", action="store_true", help="Only show what would be done")
    args = parser.parse_args()

    from _common import get_db_path
    migrate(PlanStore(args.dir, args.project), args.source or get_db_path(), args.orphan_status, args.threshold, args.dry_run)
