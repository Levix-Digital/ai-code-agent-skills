"""
Save Incremental Turns - Session Memory Skill
Saves a 20-turn conversation block of the active session into ChromaDB, scoped to a project.
"""
import argparse
import datetime
import os
import sys

from _common import chunk_id, get_collection, resolve_project, setup_stdout


def save_turn_block(project: str, start_turn: int, end_turn: int, turns_text: str, session_id: str = None):
    setup_stdout()
    col = get_collection()

    date_str = datetime.date.today().isoformat()
    sid = session_id or date_str
    doc_id = chunk_id(project, sid, start_turn, end_turn)

    doc_content = f"=== [{project}] CURRENT SESSION TURNS {start_turn} TO {end_turn} (Date: {date_str}) ===\n\n{turns_text}"
    metadata = {
        "project": project,
        "type": "current_session_chunk",
        "session_id": sid,
        "date": date_str,
        "start_turn": start_turn,
        "end_turn": end_turn,
        "topic": f"current_session_turns_{start_turn}_{end_turn}"
    }

    col.upsert(ids=[doc_id], documents=[doc_content], metadatas=[metadata])

    print(f"✅ [{project}] Incremental turns {start_turn}-{end_turn} saved into ChromaDB! (Doc ID: {doc_id})")
    print(f"Entries for project '{project}': {len(col.get(where={'project': project})['ids'])}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Save 20-turn incremental block to ChromaDB session memory")
    parser.add_argument("--project", type=str, default=None, help="Project name (default: git repository root name)")
    parser.add_argument("--start", type=int, required=True, help="Starting turn index")
    parser.add_argument("--end", type=int, required=True, help="Ending turn index")
    parser.add_argument("--content", type=str, default=None, help="Integral text content of the turns")
    parser.add_argument("--file", type=str, default=None, help="Path to a text/markdown file with the turns")
    parser.add_argument("--session-id", type=str, default=None, help="Optional session identifier")
    args = parser.parse_args()

    if args.file and os.path.exists(args.file):
        with open(args.file, "r", encoding="utf-8") as f:
            content = f.read()
    elif args.content:
        content = args.content
    else:
        print("Error: Either --content or --file must be specified.")
        sys.exit(1)

    save_turn_block(resolve_project(args.project), args.start, args.end, content, args.session_id)
