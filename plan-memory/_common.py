"""
Shared helpers - plan-memory Skill
Resolves the ChromaDB location (outside the skill folder) and normalizes project names,
so "AnyContext", "any-context" and "any_context" all refer to the same project.
"""
import os
import re

DEFAULT_DB_PATH = os.path.join(os.path.expanduser("~"), ".ai-code-agent-memory", "plan-memory", "db")


def get_db_path() -> str:
    return os.environ.get("PLAN_MEMORY_DB", DEFAULT_DB_PATH)


def normalize_project(name: str):
    """Kebab-case project name: 'AnyContext' -> 'any-context'. None stays None."""
    if name is None:
        return None
    text = re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "-", name.strip())
    text = text.lower()
    text = re.sub(r"[^\w\s-]", "", text)
    text = re.sub(r"[\s_-]+", "-", text)
    return text.strip("-")
