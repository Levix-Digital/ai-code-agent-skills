"""
Roadmap store - plan-memory Skill
Reads and writes the project roadmap file (default: docs/ROADMAP.md).
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "dev-cycle-protocol"))
from project_config import load_project_config  # noqa: E402

from _roadmap import Roadmap, RoadmapError  # noqa: E402
from _store import repo_root  # noqa: E402

TEMPLATE = os.path.join(HERE, "templates", "ROADMAP_TEMPLATE.md")


class RoadmapFile:
    """The roadmap file of one project."""

    def __init__(self, start_dir: str = None):
        self.config = load_project_config(repo_root(start_dir))
        rel = self.config.section("roadmap").get("file") or os.path.join("docs", "ROADMAP.md")
        self.path = os.path.join(self.config.root, rel)

    @property
    def exists(self) -> bool:
        return os.path.isfile(self.path)

    @property
    def rel(self) -> str:
        return os.path.relpath(self.path, self.config.root).replace("\\", "/")

    def load(self, create: bool = False) -> Roadmap:
        if self.exists:
            with open(self.path, "r", encoding="utf-8", newline="") as f:
                return Roadmap(f.read())
        if not create:
            raise RoadmapError(f"no roadmap at {self.rel}; the first 'add-release' creates it")
        with open(TEMPLATE, "r", encoding="utf-8") as f:
            return Roadmap(f.read())

    def save(self, roadmap: Roadmap):
        """Atomic write; refuses a file whose structure it could not read, so nothing is lost."""
        if roadmap.errors:
            raise RoadmapError(f"{self.rel} has structural problems; fix them by hand first:\n  - "
                               + "\n  - ".join(roadmap.errors))
        os.makedirs(os.path.dirname(self.path), exist_ok=True)
        tmp = self.path + ".tmp"
        with open(tmp, "w", encoding="utf-8", newline="") as f:
            f.write(roadmap.render())
        os.replace(tmp, self.path)
