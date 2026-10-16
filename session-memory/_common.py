"""
Shared helpers - Session Memory Skill
Resolves the ChromaDB location, the collection and the project namespace so every
record carries a `project` metadata field and project-scoped IDs.
"""
import os
import re
import subprocess
import sys

import chromadb

COLLECTION_NAME = "session_history"
DEFAULT_DB_PATH = os.path.join(os.path.expanduser("~"), ".ai-code-agent-memory", "session-memory", "db")


def setup_stdout():
    if sys.stdout.encoding != "utf-8":
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass


def slugify(text: str) -> str:
    text = text.lower().strip()
    text = re.sub(r"[^\w\s-]", "", text)
    text = re.sub(r"[\s_]+", "-", text)
    return text.strip("-")


def resolve_project(explicit: str = None) -> str:
    """--project > SESSION_MEMORY_PROJECT > git repository root name > current folder name."""
    name = explicit or os.environ.get("SESSION_MEMORY_PROJECT")
    if not name:
        try:
            top = subprocess.run(
                ["git", "rev-parse", "--show-toplevel"],
                capture_output=True, text=True, check=True,
            ).stdout.strip()
            name = os.path.basename(top)
        except Exception:
            name = os.path.basename(os.getcwd())
    project = slugify(name)
    if not project:
        print("Error: could not resolve a project name. Pass --project <name>.")
        sys.exit(1)
    return project


def get_db_path() -> str:
    return os.environ.get("SESSION_MEMORY_DB", DEFAULT_DB_PATH)


def get_collection():
    db_path = get_db_path()
    os.makedirs(db_path, exist_ok=True)
    client = chromadb.PersistentClient(path=db_path)
    return client.get_or_create_collection(COLLECTION_NAME)


def master_id(project: str) -> str:
    return f"{project}::master_summary"


def chunk_id(project: str, session_id: str, start_turn: int, end_turn: int) -> str:
    return f"{project}::turns::{session_id}::{start_turn:03d}_to_{end_turn:03d}"
