"""
Backlog model - plan-memory Skill
Parses, validates and renders the project backlog (default: docs/BACKLOG.md), the first
level of planning: bugs, features, improvements and adjustments captured for later.
Pure functions over text; reading and writing the file is done by backlog.py.

Format (see templates/BACKLOG_TEMPLATE.md):
    ## Bugs | Features | Improvements | Adjustments | Closed
    ### <BUG|FEAT|IMP|ADJ>-NNN: <title>
    - **Severity**: critical | high | medium | low      (bugs)
    - **Priority**: high | medium | low                 (other types)
    - **Status**: open | planned | in_progress          (done | dropped | duplicate under Closed)
    - **Found**: <date>[, <version, pull request or test>]
    - **Plan**: <plan id> | -
    - **Closed**: <date>, <pull request | reason | duplicate of <ID>>   (Closed only)
    - **Description**: <text, may continue on indented lines>
"""
import re

TYPES = {
    # type: (id prefix, section heading)
    "bug": ("BUG", "Bugs"),
    "feature": ("FEAT", "Features"),
    "improvement": ("IMP", "Improvements"),
    "adjustment": ("ADJ", "Adjustments"),
}
CLOSED_SECTION = "Closed"
SECTIONS = [heading for _, heading in TYPES.values()] + [CLOSED_SECTION]
PREFIX_TYPE = {prefix: t for t, (prefix, _) in TYPES.items()}
SECTION_TYPE = {heading: t for t, (_, heading) in TYPES.items()}

SEVERITIES = ["critical", "high", "medium", "low"]
PRIORITIES = ["high", "medium", "low"]
OPEN_STATUSES = ["open", "planned", "in_progress"]
CLOSED_STATUSES = ["done", "dropped", "duplicate"]
NO_PLAN = "-"

SECTION_RE = re.compile(r"^##\s+(?!#)(.+?)\s*$")
ITEM_RE = re.compile(r"^###\s+(?!#)(\S+?):\s*(.+?)\s*$")
ID_RE = re.compile(r"^(BUG|FEAT|IMP|ADJ)-(\d{3,})$")
FIELD_RE = re.compile(r"^-\s+\*\*([A-Za-z ]+)\*\*\s*:\s?(.*)$")
DUPLICATE_RE = re.compile(r"duplicate of\s+((?:BUG|FEAT|IMP|ADJ)-\d{3,})", re.I)

FIELD_ORDER = ["Severity", "Priority", "Status", "Found", "Plan", "Closed", "Description"]


class BacklogError(ValueError):
    """A request the backlog cannot satisfy (unknown id, invalid value, wrong state)."""


class Item:
    def __init__(self, item_id: str, title: str, section: str, line: int = None):
        self.id, self.title, self.section, self.line = item_id, title, section, line
        self.fields = {}   # name -> list of lines (first line value, then continuation lines as written)

    @property
    def type(self):
        m = ID_RE.match(self.id)
        return PREFIX_TYPE.get(m.group(1)) if m else None

    @property
    def number(self) -> int:
        m = ID_RE.match(self.id)
        return int(m.group(2)) if m else 0

    @property
    def closed(self) -> bool:
        return self.section == CLOSED_SECTION

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
    def plan(self):
        value = self.get("Plan")
        return None if value in ("", NO_PLAN) else value

    @property
    def rank(self) -> str:
        """Severity for bugs, priority for everything else."""
        return self.get("Severity") if self.type == "bug" else self.get("Priority")

    def sort_key(self):
        scale = SEVERITIES if self.type == "bug" else PRIORITIES
        rank = self.rank
        return (scale.index(rank) if rank in scale else len(scale), self.number)

    def to_dict(self) -> dict:
        return {
            "id": self.id, "type": self.type, "title": self.title, "section": self.section,
            "severity": self.get("Severity") or None, "priority": self.get("Priority") or None,
            "status": self.status, "found": self.get("Found"), "plan": self.plan,
            "closed": self.get("Closed") or None, "description": self.get("Description"),
        }


class Backlog:
    def __init__(self, text: str):
        self.newline = "\r\n" if "\r\n" in text else "\n"
        lines = text.replace("\r\n", "\n").split("\n")
        self.header = []                         # lines before the first section
        self.sections = {name: [] for name in SECTIONS}
        self.section_notes = {name: [] for name in SECTIONS}  # text between a section heading and its first item
        self.unknown_sections = []
        self.errors = []
        self._parse(lines)

    # ------------------------------------------------------------ parsing

    def _parse(self, lines: list):
        section, item, field = None, None, None
        for i, line in enumerate(lines):
            line = line.rstrip()
            m_section = SECTION_RE.match(line)
            if m_section and not line.startswith("###"):
                name = m_section.group(1)
                item, field = None, None
                if name in self.sections:
                    section = name
                else:
                    section = None
                    self.unknown_sections.append((i, name))
                continue
            if section is None and not self.unknown_sections:
                self.header.append(line)
                continue
            if section is None:
                continue
            if line.startswith("###"):
                field = None
                m_item = ITEM_RE.match(line)
                if not m_item:
                    self.errors.append(f"line {i + 1}: item heading must be '### <ID>: <title>'")
                    item = None
                    continue
                item = Item(m_item.group(1), m_item.group(2), section, i)
                self.sections[section].append(item)
                continue
            if item is None:
                self.section_notes[section].append(line)
                continue
            m_field = FIELD_RE.match(line)
            if m_field:
                field = m_field.group(1).strip()
                if field in item.fields:
                    self.errors.append(f"line {i + 1}: {item.id} repeats the field '{field}'")
                item.fields[field] = [m_field.group(2).strip()]
                continue
            if field is not None:
                item.fields[field].append(line)
            elif line.strip():
                self.errors.append(f"line {i + 1}: text in {item.id} outside a '- **Field**:' line")

    # ------------------------------------------------------------ queries

    @property
    def items(self) -> list:
        return [item for name in SECTIONS for item in self.sections[name]]

    def find(self, item_id: str) -> Item:
        for item in self.items:
            if item.id == item_id.upper():
                return item
        raise BacklogError(f"{item_id}: no such backlog item")

    def next_id(self, item_type: str) -> str:
        prefix = TYPES[item_type][0]
        numbers = [item.number for item in self.items if item.id.startswith(prefix + "-")]
        return f"{prefix}-{max(numbers, default=0) + 1:03d}"

    def open_items(self, item_type: str = None) -> list:
        items = [i for i in self.items if not i.closed and (item_type is None or i.type == item_type)]
        return sorted(items, key=lambda i: (list(TYPES).index(i.type) if i.type in TYPES else len(TYPES), i.sort_key()))

    def blocking_bugs(self, severities: list) -> list:
        return [i for i in self.open_items("bug") if i.get("Severity") in severities]

    # ------------------------------------------------------------ changes

    def add(self, item_type: str, title: str, description: str, found: str,
            severity: str = None, priority: str = None) -> Item:
        if item_type not in TYPES:
            raise BacklogError(f"type '{item_type}' is not one of: {', '.join(TYPES)}")
        if not title.strip():
            raise BacklogError("the title must not be empty")
        if not description.strip():
            raise BacklogError("the description must not be empty")
        item = Item(self.next_id(item_type), title.strip(), TYPES[item_type][1])
        if item_type == "bug":
            if severity not in SEVERITIES:
                raise BacklogError(f"a bug needs a severity: {', '.join(SEVERITIES)}")
            item.set("Severity", severity)
        else:
            priority = priority or "medium"
            if priority not in PRIORITIES:
                raise BacklogError(f"priority '{priority}' is not one of: {', '.join(PRIORITIES)}")
            item.set("Priority", priority)
        item.set("Status", "open")
        item.set("Found", found)
        item.set("Plan", NO_PLAN)
        item.set("Description", description.strip())
        self.sections[item.section].append(item)
        return item

    def update(self, item_id: str, title: str = None, description: str = None, severity: str = None,
               priority: str = None, status: str = None, plan: str = None) -> Item:
        item = self.find(item_id)
        if item.closed and (status or plan):
            raise BacklogError(f"{item.id} is closed; reopen it first")
        if title is not None:
            if not title.strip():
                raise BacklogError("the title must not be empty")
            item.title = title.strip()
        if description is not None:
            item.set("Description", description.strip())
        if severity is not None:
            if item.type != "bug":
                raise BacklogError(f"{item.id} is not a bug; use a priority")
            if severity not in SEVERITIES:
                raise BacklogError(f"severity '{severity}' is not one of: {', '.join(SEVERITIES)}")
            item.set("Severity", severity)
        if priority is not None:
            if item.type == "bug":
                raise BacklogError(f"{item.id} is a bug; use a severity")
            if priority not in PRIORITIES:
                raise BacklogError(f"priority '{priority}' is not one of: {', '.join(PRIORITIES)}")
            item.set("Priority", priority)
        if plan is not None:
            item.set("Plan", plan.strip() or NO_PLAN)
        if status is not None:
            if status not in OPEN_STATUSES:
                raise BacklogError(f"status '{status}' is not one of: {', '.join(OPEN_STATUSES)} "
                                   "(use close, drop or duplicate to close an item)")
            if status != "open" and not item.plan:
                raise BacklogError(f"{item.id} has no plan; an item is '{status}' only once it is promoted into a plan")
            item.set("Status", status)
        return item

    def close(self, item_id: str, status: str, closed: str) -> Item:
        """Moves an item to the Closed section with `status` and the `Closed` note."""
        if status not in CLOSED_STATUSES:
            raise BacklogError(f"status '{status}' is not one of: {', '.join(CLOSED_STATUSES)}")
        item = self.find(item_id)
        if item.closed:
            raise BacklogError(f"{item.id} is already closed ({item.status})")
        self.sections[item.section].remove(item)
        item.section = CLOSED_SECTION
        item.set("Status", status)
        item.set("Closed", closed)
        self.sections[CLOSED_SECTION].append(item)
        return item

    def reopen(self, item_id: str) -> Item:
        item = self.find(item_id)
        if not item.closed:
            raise BacklogError(f"{item.id} is not closed")
        if item.type not in TYPES:
            raise BacklogError(f"{item.id}: unknown id prefix")
        self.sections[CLOSED_SECTION].remove(item)
        item.section = TYPES[item.type][1]
        item.fields.pop("Closed", None)
        item.set("Status", "open")
        item.set("Plan", NO_PLAN)
        self.sections[item.section].append(item)
        return item

    # ------------------------------------------------------------ validation

    def validate(self) -> list:
        problems = list(self.errors)
        for line, name in self.unknown_sections:
            problems.append(f"line {line + 1}: unknown section '## {name}' (expected: {', '.join(SECTIONS)})")
        seen = {}
        ids = {item.id for item in self.items}
        for item in self.items:
            where = f"line {item.line + 1}: {item.id}" if item.line is not None else item.id
            m = ID_RE.match(item.id)
            if not m:
                problems.append(f"{where}: id must be BUG-NNN, FEAT-NNN, IMP-NNN or ADJ-NNN")
                continue
            if item.id in seen:
                problems.append(f"{where}: duplicate id (first at line {seen[item.id] + 1})")
            seen.setdefault(item.id, item.line or 0)
            if not item.closed and SECTION_TYPE.get(item.section) != item.type:
                problems.append(f"{where}: a {item.type} belongs under '## {TYPES[item.type][1]}'")

            required = ["Status", "Found", "Plan", "Description"] + (["Closed"] if item.closed else [])
            required.insert(0, "Severity" if item.type == "bug" else "Priority")
            for name in required:
                if not item.get(name):
                    problems.append(f"{where}: missing '- **{name}**:'")
            if item.type == "bug" and item.get("Severity") and item.get("Severity") not in SEVERITIES:
                problems.append(f"{where}: severity must be one of: {', '.join(SEVERITIES)}")
            if item.type != "bug" and item.get("Priority") and item.get("Priority") not in PRIORITIES:
                problems.append(f"{where}: priority must be one of: {', '.join(PRIORITIES)}")

            allowed = CLOSED_STATUSES if item.closed else OPEN_STATUSES
            if item.status and item.status not in allowed:
                place = "under '## Closed'" if item.closed else "outside '## Closed'"
                problems.append(f"{where}: status {place} must be one of: {', '.join(allowed)}")
            if item.status in ("planned", "in_progress") and not item.plan:
                problems.append(f"{where}: a {item.status} item needs its plan id in '- **Plan**:'")
            if item.status == "duplicate":
                m_dup = DUPLICATE_RE.search(item.get("Closed"))
                if not m_dup:
                    problems.append(f"{where}: a duplicate needs 'duplicate of <ID>' in '- **Closed**:'")
                elif m_dup.group(1).upper() not in ids:
                    problems.append(f"{where}: duplicate of unknown item {m_dup.group(1)}")
        return problems

    # ------------------------------------------------------------ rendering

    def render(self) -> str:
        out = list(self.header)
        while out and not out[-1].strip():
            out.pop()
        for name in SECTIONS:
            out += ["", f"## {name}"]
            notes = list(self.section_notes[name])
            while notes and not notes[0].strip():
                notes.pop(0)
            while notes and not notes[-1].strip():
                notes.pop()
            if notes:
                out += [""] + notes
            for item in self.sections[name]:
                out += ["", f"### {item.id}: {item.title}"]
                names = [n for n in FIELD_ORDER if n in item.fields] + [n for n in item.fields if n not in FIELD_ORDER]
                for field in names:
                    lines = list(item.fields[field])
                    while len(lines) > 1 and not lines[-1].strip():
                        lines.pop()
                    first = f"- **{field}**:" + (f" {lines[0]}" if lines[0] else "")
                    out += [first] + lines[1:]
        return self.newline.join(out) + self.newline
