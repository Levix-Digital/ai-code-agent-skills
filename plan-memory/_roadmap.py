"""
Roadmap model - plan-memory Skill
Parses, validates and renders the project roadmap (default: docs/ROADMAP.md): the ordered
releases of the product, each with its goal, status and the backlog items it delivers.
The roadmap is the single source of the item-to-release assignment.
Pure functions over text; reading and writing the file is done by roadmap.py.

Format (see templates/ROADMAP_TEMPLATE.md):
    ## <vX.Y.Z>: <name>
    - **Status**: planned | in_progress | released
    - **Target**: <free text, for example 2026-12>      (optional)
    - **Released**: <date>                               (released only)
    - **Goal**: <text, may continue on indented lines>
    - **Items**: <ID>, <ID>, ...                         (may continue on indented lines; '-' when empty)
"""
import re

STATUSES = ["planned", "in_progress", "released"]
NO_ITEMS = "-"

SECTION_RE = re.compile(r"^##\s+(?!#)(.+?)\s*$")
RELEASE_RE = re.compile(r"^(v\d+\.\d+\.\d+)\s*:\s*(.+?)\s*$")
VERSION_RE = re.compile(r"^v(\d+)\.(\d+)\.(\d+)$")
FIELD_RE = re.compile(r"^-\s+\*\*([A-Za-z ]+)\*\*\s*:\s?(.*)$")
ITEM_ID_RE = re.compile(r"^(BUG|FEAT|IMP|ADJ|EPIC)-\d{3,}$")

FIELD_ORDER = ["Status", "Target", "Released", "Goal", "Items"]


class RoadmapError(ValueError):
    """A request the roadmap cannot satisfy (unknown release, invalid value, wrong state)."""


def version_key(version: str) -> tuple:
    m = VERSION_RE.match(version or "")
    if not m:
        raise RoadmapError(f"'{version}' is not a version like v1.2.0")
    return tuple(int(g) for g in m.groups())


class Release:
    def __init__(self, version: str, name: str, line: int = None):
        self.version, self.name, self.line = version, name, line
        self.fields = {}   # name -> list of lines (first line value, then continuation lines as written)

    def get(self, name: str) -> str:
        lines = self.fields.get(name)
        if not lines:
            return ""
        return " ".join(part.strip() for part in lines if part.strip())

    def set(self, name: str, value: str):
        lines = str(value).replace("\r\n", "\n").split("\n")
        self.fields[name] = [lines[0].strip()] + ["  " + line.strip() if line.strip() else "" for line in lines[1:]]

    @property
    def status(self) -> str:
        return self.get("Status")

    @property
    def items(self) -> list:
        value = self.get("Items")
        if value in ("", NO_ITEMS):
            return []
        return [part.strip().upper() for part in value.split(",") if part.strip()]

    @items.setter
    def items(self, ids: list):
        self.set("Items", ", ".join(ids) if ids else NO_ITEMS)

    def to_dict(self) -> dict:
        return {"version": self.version, "name": self.name, "status": self.status,
                "target": self.get("Target") or None, "released": self.get("Released") or None,
                "goal": self.get("Goal"), "items": self.items}


class Roadmap:
    def __init__(self, text: str):
        self.newline = "\r\n" if "\r\n" in text else "\n"
        self.header = []      # lines before the first release
        self.releases = []
        self.errors = []
        self._parse(text.replace("\r\n", "\n").split("\n"))

    # ------------------------------------------------------------ parsing

    def _parse(self, lines: list):
        release, field = None, None
        for i, line in enumerate(lines):
            line = line.rstrip()
            m_section = SECTION_RE.match(line)
            if m_section:
                field = None
                m_release = RELEASE_RE.match(m_section.group(1))
                if not m_release:
                    self.errors.append(f"line {i + 1}: release heading must be '## vX.Y.Z: <name>'")
                    release = None
                    continue
                release = Release(m_release.group(1), m_release.group(2), i)
                self.releases.append(release)
                continue
            if release is None:
                if not self.releases and not self.errors:
                    self.header.append(line)
                elif line.strip():
                    self.errors.append(f"line {i + 1}: text outside a release")
                continue
            m_field = FIELD_RE.match(line)
            if m_field:
                field = m_field.group(1).strip()
                if field in release.fields:
                    self.errors.append(f"line {i + 1}: {release.version} repeats the field '{field}'")
                release.fields[field] = [m_field.group(2).strip()]
                continue
            if field is not None:
                release.fields[field].append(line)
            elif line.strip():
                self.errors.append(f"line {i + 1}: text in {release.version} outside a '- **Field**:' line")

    # ------------------------------------------------------------ queries

    def find(self, version: str) -> Release:
        for release in self.releases:
            if release.version == version:
                return release
        raise RoadmapError(f"{version}: no such release in the roadmap")

    def release_of(self, item_id: str):
        for release in self.releases:
            if item_id.upper() in release.items:
                return release
        return None

    def current(self):
        """The first release that is not released yet, in roadmap order."""
        for release in self.releases:
            if release.status != "released":
                return release
        return None

    @property
    def assigned(self) -> set:
        return {item for release in self.releases for item in release.items}

    # ------------------------------------------------------------ changes

    def add_release(self, version: str, name: str, goal: str, target: str = None,
                    before: str = None, after: str = None) -> Release:
        version_key(version)
        if any(r.version == version for r in self.releases):
            raise RoadmapError(f"{version} is already in the roadmap")
        if not name.strip():
            raise RoadmapError("the release name must not be empty")
        if not goal.strip():
            raise RoadmapError("the release goal must not be empty")
        release = Release(version, name.strip())
        release.set("Status", "planned")
        if target:
            release.set("Target", target)
        release.set("Goal", goal.strip())
        release.items = []
        self._insert(release, before, after)
        return release

    def _insert(self, release: Release, before: str = None, after: str = None):
        if before and after:
            raise RoadmapError("use --before or --after, not both")
        if before:
            self.releases.insert(self.releases.index(self.find(before)), release)
        elif after:
            self.releases.insert(self.releases.index(self.find(after)) + 1, release)
        else:
            self.releases.append(release)

    def move_release(self, version: str, before: str = None, after: str = None) -> Release:
        if not before and not after:
            raise RoadmapError("say where: --before <version> or --after <version>")
        release = self.find(version)
        if version in (before, after):
            raise RoadmapError("a release cannot move relative to itself")
        self.releases.remove(release)
        try:
            self._insert(release, before, after)
        except RoadmapError:
            self.releases.append(release)
            raise
        return release

    def update_release(self, version: str, name: str = None, goal: str = None, target: str = None,
                       status: str = None, released_on: str = None) -> Release:
        release = self.find(version)
        if name is not None:
            if not name.strip():
                raise RoadmapError("the release name must not be empty")
            release.name = name.strip()
        if goal is not None:
            if not goal.strip():
                raise RoadmapError("the release goal must not be empty")
            release.set("Goal", goal.strip())
        if target is not None:
            if target.strip():
                release.set("Target", target.strip())
            else:
                release.fields.pop("Target", None)
        if status is not None:
            if status not in STATUSES:
                raise RoadmapError(f"status '{status}' is not one of: {', '.join(STATUSES)}")
            release.set("Status", status)
            if status == "released":
                if not released_on:
                    raise RoadmapError("a released release needs its release date")
                release.set("Released", released_on)
            else:
                release.fields.pop("Released", None)
        return release

    def assign(self, ids: list, version: str, move: bool = False) -> list:
        """Adds items to a release. An item in another release moves only with `move`."""
        release = self.find(version)
        if release.status == "released":
            raise RoadmapError(f"{version} is released; plan the items in a later release")
        added = []
        for item_id in [i.upper() for i in ids]:
            if not ITEM_ID_RE.match(item_id):
                raise RoadmapError(f"'{item_id}' is not a backlog id")
            current = self.release_of(item_id)
            if current is release:
                continue
            if current is not None:
                if not move:
                    raise RoadmapError(f"{item_id} is already in {current.version}; use --move to move it")
                if current.status == "released":
                    raise RoadmapError(f"{item_id} was delivered in the released {current.version}")
                current.items = [i for i in current.items if i != item_id]
            release.items = release.items + [item_id]
            added.append(item_id)
        return added

    def unassign(self, ids: list) -> list:
        removed = []
        for item_id in [i.upper() for i in ids]:
            release = self.release_of(item_id)
            if release is None:
                raise RoadmapError(f"{item_id} is not in any release")
            if release.status == "released":
                raise RoadmapError(f"{item_id} was delivered in the released {release.version}")
            release.items = [i for i in release.items if i != item_id]
            removed.append(item_id)
        return removed

    # ------------------------------------------------------------ validation

    def validate(self, backlog=None) -> list:
        """Problems of the roadmap; with a Backlog, also checks the ids against it."""
        problems = list(self.errors)
        seen_versions, seen_items = {}, {}
        released_after_open = False
        open_seen = False
        for release in self.releases:
            where = f"line {release.line + 1}: {release.version}" if release.line is not None else release.version
            if release.version in seen_versions:
                problems.append(f"{where}: duplicate release")
            seen_versions[release.version] = release
            for name in ("Status", "Goal", "Items"):
                if not release.get(name):
                    problems.append(f"{where}: missing '- **{name}**:'")
            if release.status and release.status not in STATUSES:
                problems.append(f"{where}: status must be one of: {', '.join(STATUSES)}")
            if release.status == "released" and not release.get("Released"):
                problems.append(f"{where}: a released release needs '- **Released**: <date>'")
            if release.status != "released" and release.get("Released"):
                problems.append(f"{where}: '- **Released**:' only belongs to a released release")
            if release.status == "released" and open_seen:
                released_after_open = True
            if release.status != "released":
                open_seen = True
            for item_id in release.items:
                if not ITEM_ID_RE.match(item_id):
                    problems.append(f"{where}: '{item_id}' is not a backlog id")
                    continue
                if item_id in seen_items:
                    problems.append(f"{where}: {item_id} is also in {seen_items[item_id]}")
                seen_items.setdefault(item_id, release.version)
                if backlog is not None:
                    try:
                        item = backlog.find(item_id)
                    except ValueError:
                        problems.append(f"{where}: {item_id} is not in the backlog")
                        continue
                    if item.type == "epic":
                        problems.append(f"{where}: {item_id} is an epic; assign its items instead")
        if released_after_open:
            problems.append("a released release comes after one that is not released; order the roadmap by delivery")
        versions = [r.version for r in self.releases if VERSION_RE.match(r.version)]
        if versions != sorted(versions, key=version_key):
            problems.append("releases are not in version order: " + ", ".join(versions))
        return problems

    # ------------------------------------------------------------ rendering

    def render(self) -> str:
        out = list(self.header)
        while out and not out[-1].strip():
            out.pop()
        for release in self.releases:
            out += ["", f"## {release.version}: {release.name}"]
            names = [n for n in FIELD_ORDER if n in release.fields] + [n for n in release.fields if n not in FIELD_ORDER]
            for field in names:
                lines = list(release.fields[field])
                while len(lines) > 1 and not lines[-1].strip():
                    lines.pop()
                first = f"- **{field}**:" + (f" {lines[0]}" if lines[0] else "")
                out += [first] + lines[1:]
        return self.newline.join(out) + self.newline
