"""
Check Milestone Utility - plan-memory Skill
Tracks granular milestone completion for architectural plans and roadmaps in ChromaDB.
Displays visual progress bars, checklist states, and milestone notes.
"""
import os
import sys
import argparse
import datetime
import chromadb

from _common import get_db_path, normalize_project

def render_progress_bar(completed: int, total: int, width: int = 24) -> str:
    if total <= 0:
        return "[------------------------] 0.0%"
    ratio = min(1.0, max(0.0, completed / total))
    filled_len = int(round(width * ratio))
    bar = "█" * filled_len + "░" * (width - filled_len)
    percent = ratio * 100.0
    return f"[{bar}] {percent:.1f}% ({completed}/{total} Marcos)"

def parse_milestone_list(val: str) -> list[str]:
    if not val:
        return []
    return [m.strip() for m in val.split(",") if m.strip()]

def find_plan(col, plan_id: str = None, project: str = None):
    if plan_id:
        res = col.get(ids=[plan_id])
        if res and res["ids"] and len(res["ids"]) > 0:
            return res["ids"][0], res["documents"][0], res["metadatas"][0]
        return None, None, None
    elif project:
        res = col.get(where={"project": project})
        if res and res["ids"]:
            # Prioritize active or in_progress, sorted by created_at descending
            items = list(zip(res["ids"], res["documents"], res["metadatas"]))
            def sort_key(item):
                st = (item[2] or {}).get("status", "")
                st_prio = 0 if st in ["active", "in_progress"] else 1
                dt = (item[2] or {}).get("created_at", "")
                return (st_prio, -len(dt), dt)
            items.sort(key=sort_key)
            return items[0]
    return None, None, None

def check_milestone(
    plan_id: str = None,
    project: str = None,
    milestone_name: str = None,
    uncheck: bool = False,
    notes: str = None,
    list_only: bool = False,
    auto_complete: bool = False,
):
    if sys.stdout.encoding != 'utf-8':
        try:
            sys.stdout.reconfigure(encoding='utf-8')
        except Exception:
            pass

    db_path = get_db_path()
    if not os.path.exists(db_path):
        print("❌ Error: No plans database found.", file=sys.stderr)
        sys.exit(1)

    client = chromadb.PersistentClient(path=db_path)
    col = client.get_or_create_collection("project_plans")

    pid, doc, meta = find_plan(col, plan_id=plan_id, project=project)
    if not pid:
        target = f"plan_id='{plan_id}'" if plan_id else f"project='{project}'"
        print(f"❌ Error: Plan not found for {target}", file=sys.stderr)
        sys.exit(1)

    meta = meta or {}
    all_milestones = parse_milestone_list(meta.get("milestones", ""))
    completed_milestones = parse_milestone_list(meta.get("completed_milestones", ""))

    if not all_milestones:
        print(f"⚠️ Plan '{pid}' does not have any milestones defined in its metadata.")
        return

    now_iso = datetime.datetime.now().isoformat()
    modified = False

    if milestone_name and not list_only:
        # Match milestone name (case-insensitive / partial match)
        matched_ms = None
        for m in all_milestones:
            if m.lower() == milestone_name.lower():
                matched_ms = m
                break
        if not matched_ms:
            # Partial match
            matches = [m for m in all_milestones if milestone_name.lower() in m.lower()]
            if len(matches) == 1:
                matched_ms = matches[0]
            elif len(matches) > 1:
                print(f"❌ Ambiguous milestone name '{milestone_name}'. Matches: {matches}", file=sys.stderr)
                sys.exit(1)

        if not matched_ms:
            print(f"❌ Milestone '{milestone_name}' not found in plan '{pid}'.", file=sys.stderr)
            print(f"   Available milestones: {', '.join(all_milestones)}")
            sys.exit(1)

        if uncheck:
            if matched_ms in completed_milestones:
                completed_milestones.remove(matched_ms)
                modified = True
                print(f"⏳ Reverted milestone to PENDING: '{matched_ms}'")
            else:
                print(f"ℹ️ Milestone '{matched_ms}' was not completed.")
        else:
            if matched_ms not in completed_milestones:
                completed_milestones.append(matched_ms)
                modified = True
                print(f"✅ Marked milestone as COMPLETED: '{matched_ms}'")
            else:
                print(f"ℹ️ Milestone '{matched_ms}' was already completed.")

        if notes:
            existing_notes = meta.get("status_notes", "")
            meta["status_notes"] = f"{existing_notes} [{now_iso}] Milestone '{matched_ms}': {notes}".strip()

    if modified:
        meta["completed_milestones"] = ",".join(completed_milestones)
        meta["updated_at"] = now_iso

        # If all milestones are completed and auto_complete or status was active
        if len(completed_milestones) == len(all_milestones) and len(all_milestones) > 0:
            if auto_complete or meta.get("status") == "active":
                meta["status"] = "completed"
                print(f"🎉 All milestones completed! Plan status automatically updated to COMPLETED.")
        elif meta.get("status") == "active" and len(completed_milestones) > 0:
            meta["status"] = "in_progress"
            print(f"⚡ Plan status updated to IN_PROGRESS.")

        col.upsert(ids=[pid], documents=[doc], metadatas=[meta])

    # Display status and checklist
    p_status = meta.get("status", "active").upper()
    print("=" * 80)
    print(f"📊 MILESTONE TRACKER: [{meta.get('project', 'Unknown')}] {meta.get('title', pid)}")
    print(f"   • Plan ID : {pid} | Status: {p_status} | Version: {meta.get('version', '-')}")
    print(f"   • Progress: {render_progress_bar(len(completed_milestones), len(all_milestones))}")
    print("-" * 80)
    print("📋 CHECKLIST DE MARCOS:")
    for idx, m in enumerate(all_milestones, 1):
        is_done = m in completed_milestones
        icon = "✅ [X]" if is_done else "⏳ [ ]"
        print(f"   {icon} {idx}. {m}")
    print("=" * 80 + "\n")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Track granular milestone progress for architectural plans in ChromaDB")
    parser.add_argument("--id", type=str, default=None, help="Target Plan ID")
    parser.add_argument("--project", type=str, default=None, help="Project name (targets active plan)")
    parser.add_argument("--milestone", type=str, default=None, help="Milestone name to mark as completed")
    parser.add_argument("--uncheck", action="store_true", help="Revert milestone to pending")
    parser.add_argument("--notes", type=str, default=None, help="Notes or rationale for milestone update")
    parser.add_argument("--list", action="store_true", help="List milestones and progress without modifying")
    parser.add_argument("--auto-complete", action="store_true", help="Automatically set plan status to completed if all milestones are done")

    args = parser.parse_args()

    if not args.id and not args.project:
        print("❌ Error: You must supply either --id <plan_id> or --project <project_name>", file=sys.stderr)
        sys.exit(1)

    check_milestone(
        plan_id=args.id,
        project=normalize_project(args.project),
        milestone_name=args.milestone,
        uncheck=args.uncheck,
        notes=args.notes,
        list_only=args.list,
        auto_complete=args.auto_complete
    )
