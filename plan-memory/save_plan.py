"""
Save Plan Utility - plan-memory Skill
Saves an uncompressed, comprehensive architectural plan or roadmap into ChromaDB.
Supports plan lineage via --supersedes to automatically deprecate older plans.
"""
import os
import sys
import argparse
import datetime
import re
import chromadb

from _common import get_db_path, normalize_project

def slugify(text: str) -> str:
    text = text.lower().strip()
    text = re.sub(r'[^\w\s-]', '', text)
    text = re.sub(r'[\s_-]+', '_', text)
    return text.strip('_')

def save_plan(
    project: str,
    title: str,
    content: str,
    plan_id: str = None,
    session_id: str = None,
    version: str = None,
    tags: str = None,
    milestones: str = None,
    status: str = "active",
    parent_plan_id: str = None,
    supersedes: str = None,
):
    if sys.stdout.encoding != 'utf-8':
        try:
            sys.stdout.reconfigure(encoding='utf-8')
        except Exception:
            pass

    db_path = get_db_path()
    os.makedirs(db_path, exist_ok=True)

    client = chromadb.PersistentClient(path=db_path)
    col = client.get_or_create_collection(
        name="project_plans",
        metadata={"description": "Persistent vector database for complete, uncompressed project architecture plans and roadmaps"}
    )

    now_iso = datetime.datetime.now().isoformat()
    pid = plan_id or f"{slugify(project)}_{slugify(title)}"

    # Handle lineage / superseding
    if supersedes:
        old_res = col.get(ids=[supersedes])
        if old_res and old_res["ids"]:
            old_meta = old_res["metadatas"][0] or {}
            old_doc = old_res["documents"][0]
            old_status = old_meta.get("status", "active")
            old_meta["status"] = "superseded"
            old_meta["superseded_by"] = pid
            old_meta["updated_at"] = now_iso
            notes = old_meta.get("status_notes", "")
            old_meta["status_notes"] = f"{notes} [{now_iso}] Superseded by {pid}".strip()
            col.upsert(ids=[supersedes], documents=[old_doc], metadatas=[old_meta])
            print(f"📦 Plan Lineage: Successfully marked '{supersedes}' as SUPERSEDED (was {old_status.upper()})")
        else:
            print(f"⚠️ Warning: Target plan to supersede '{supersedes}' not found in database.", file=sys.stderr)

    metadata = {
        "project": str(project),
        "title": str(title),
        "plan_id": str(pid),
        "session_id": str(session_id or "unknown"),
        "version": str(version or "unversioned"),
        "status": str(status or "active"),
        "created_at": now_iso,
        "updated_at": now_iso,
        "tags": str(tags or ""),
        "milestones": str(milestones or ""),
        "completed_milestones": "",
        "parent_plan_id": str(parent_plan_id or ""),
        "supersedes": str(supersedes or ""),
        "superseded_by": "",
        "status_notes": "",
        "content_length": len(content),
    }

    col.upsert(
        ids=[pid],
        documents=[content],
        metadatas=[metadata]
    )

    print(f"✅ Plan successfully saved and indexed in ChromaDB!")
    print(f"   • Plan ID    : {pid}")
    print(f"   • Project    : {project}")
    print(f"   • Title      : {title}")
    print(f"   • Session ID : {metadata['session_id']}")
    print(f"   • Version    : {metadata['version']}")
    print(f"   • Status     : {metadata['status'].upper()}")
    print(f"   • Size       : {len(content)} characters (100% uncompressed)")
    if supersedes:
        print(f"   • Supersedes : {supersedes}")
    if milestones:
        print(f"   • Milestones : {milestones}")
    print(f"   • Total plans in DB: {col.count()}\n")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Save uncompressed architecture plan or roadmap to ChromaDB")
    parser.add_argument("--project", type=str, required=True, help="Project name, normalized to kebab-case (e.g. any-context)")
    parser.add_argument("--title", type=str, required=True, help="Plan title / headline")
    parser.add_argument("--content", type=str, default=None, help="Inline markdown content")
    parser.add_argument("--file", type=str, default=None, help="Path to markdown/text file containing the plan")
    parser.add_argument("--id", type=str, default=None, help="Explicit unique Plan ID (auto-generated if omitted)")
    parser.add_argument("--session-id", type=str, default=None, help="Conversation ID where the plan was conceived")
    parser.add_argument("--version", type=str, default=None, help="Target software version (e.g. v0.30.37)")
    parser.add_argument("--tags", type=str, default=None, help="Comma-separated keywords/tags")
    parser.add_argument("--milestones", type=str, default=None, help="Comma-separated milestone targets")
    parser.add_argument("--status", type=str, default="active", help="Plan status (active, in_progress, completed, superseded)")
    parser.add_argument("--parent-id", type=str, default=None, help="Parent plan ID for hierarchical breakdown")
    parser.add_argument("--supersedes", type=str, default=None, help="Older Plan ID that this new plan supersedes")

    args = parser.parse_args()

    plan_content = ""
    if args.file:
        with open(args.file, "r", encoding="utf-8") as f:
            plan_content = f.read()
    elif args.content:
        plan_content = args.content
    else:
        print("❌ Error: You must supply either --file <path> or --content <text>", file=sys.stderr)
        sys.exit(1)

    save_plan(
        project=normalize_project(args.project),
        title=args.title,
        content=plan_content,
        plan_id=args.id,
        session_id=args.session_id,
        version=args.version,
        tags=args.tags,
        milestones=args.milestones,
        status=args.status,
        parent_plan_id=args.parent_id,
        supersedes=args.supersedes,
    )
