"""
Backlog - plan-memory Skill
The first level of planning: capture bugs, features, improvements and adjustments in the
project backlog (default: docs/BACKLOG.md) without discovery, list and triage them, and
close them. Writes the file; never commits.

Usage:
    python backlog.py add --type bug|feature|improvement|adjustment --title T --description D
                          [--severity S] [--priority P] [--found-in "v1.2.0, PR #12"]
    python backlog.py list [--type T] [--status S] [--all] [--json]
    python backlog.py summary [--json]
    python backlog.py show <ID> [--json]
    python backlog.py set <ID> [--title T] [--description D] [--severity S] [--priority P] [--status S] [--plan ID]
    python backlog.py close <ID> [--ref "PR #12"]
    python backlog.py drop <ID> --reason R
    python backlog.py duplicate <ID> --of <ID>
    python backlog.py reopen <ID>
    python backlog.py validate
All commands accept --dir <project> (default: current directory; the Git root is used).
"""
import argparse
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "dev-cycle-protocol"))
from project_config import load_project_config  # noqa: E402

from _backlog import (CLOSED_STATUSES, OPEN_STATUSES, PRIORITIES, SEVERITIES, TYPES,  # noqa: E402
                      Backlog, BacklogError)
from _store import repo_root, setup_stdout, today  # noqa: E402

TEMPLATE = os.path.join(HERE, "templates", "BACKLOG_TEMPLATE.md")


class BacklogFile:
    """The backlog file of one project."""

    def __init__(self, start_dir: str = None):
        self.config = load_project_config(repo_root(start_dir))
        section = self.config.section("backlog")
        self.path = os.path.join(self.config.root, section.get("file") or os.path.join("docs", "BACKLOG.md"))
        self.blocking_severities = section.get("blocking_severities") or []

    @property
    def exists(self) -> bool:
        return os.path.isfile(self.path)

    def load(self, create: bool = False) -> Backlog:
        if self.exists:
            with open(self.path, "r", encoding="utf-8", newline="") as f:
                return Backlog(f.read())
        if not create:
            raise BacklogError(f"no backlog at {self.rel}; the first 'add' creates it")
        with open(TEMPLATE, "r", encoding="utf-8") as f:
            return Backlog(f.read())

    def save(self, backlog: Backlog):
        """Atomic write; refuses a file whose structure it could not read, so nothing is lost."""
        structural = backlog.errors + [f"unknown section '## {name}'" for _, name in backlog.unknown_sections]
        if structural:
            raise BacklogError(f"{self.rel} has structural problems; fix them by hand first:\n  - " + "\n  - ".join(structural))
        os.makedirs(os.path.dirname(self.path), exist_ok=True)
        tmp = self.path + ".tmp"
        with open(tmp, "w", encoding="utf-8", newline="") as f:
            f.write(backlog.render())
        os.replace(tmp, self.path)

    @property
    def rel(self) -> str:
        return os.path.relpath(self.path, self.config.root).replace("\\", "/")


def line_of(item) -> str:
    rank = item.rank or "?"
    plan = f"  → {item.plan}" if item.plan else ""
    return f"{item.id:<9} [{item.status}] ({rank}) {item.title}{plan}"


def summary_data(backlog: Backlog, blocking: list) -> dict:
    data = {"types": {}, "blocking_severities": blocking,
            "blocking_bugs": [i.id for i in backlog.blocking_bugs(blocking)],
            "closed": len([i for i in backlog.items if i.closed])}
    for item_type in TYPES:
        items = backlog.open_items(item_type)
        data["types"][item_type] = {s: len([i for i in items if i.status == s]) for s in OPEN_STATUSES}
    return data


def print_summary(backlog: Backlog, bf: BacklogFile):
    data = summary_data(backlog, bf.blocking_severities)
    print(f"📋 Backlog: {bf.rel}")
    for item_type, counts in data["types"].items():
        total = sum(counts.values())
        detail = ", ".join(f"{n} {s}" for s, n in counts.items() if n)
        print(f"   {TYPES[item_type][1]:<13}: {total}" + (f" ({detail})" if detail else ""))
    print(f"   Closed       : {data['closed']}")
    blocking = backlog.blocking_bugs(bf.blocking_severities)
    if blocking:
        print(f"\n🐞 Open bugs that come before new work ({', '.join(bf.blocking_severities)}):")
        for item in blocking:
            print(f"   {line_of(item)}")
    for item_type in TYPES:
        items = [i for i in backlog.open_items(item_type) if i not in blocking]
        if items:
            print(f"\n{TYPES[item_type][1]}:")
            for item in items:
                print(f"   {line_of(item)}")


def cmd(args) -> int:
    bf = BacklogFile(args.dir)

    if args.command == "validate":
        if not bf.exists:
            print(f"ℹ️ No backlog at {bf.rel} (nothing to validate).")
            return 0
        backlog = bf.load()
        problems = backlog.validate()
        for p in problems:
            print(f"❌ {p}")
        print(f"\n{'✅' if not problems else '❌'} backlog: {len(backlog.items)} item(s), {len(problems)} problem(s).")
        return 1 if problems else 0

    if args.command == "add":
        backlog = bf.load(create=True)
        found = today() + (f", {args.found_in.strip()}" if args.found_in and args.found_in.strip() else "")
        item = backlog.add(args.type, args.title, args.description, found, args.severity, args.priority)
        bf.save(backlog)
        print(f"✅ {item.id}: {item.title} ({item.rank}) added to {bf.rel}")
        return 0

    if not bf.exists:
        if args.command in ("list", "summary"):
            if getattr(args, "json", False):
                print(json.dumps([] if args.command == "list" else summary_data(Backlog(""), bf.blocking_severities)))
            else:
                print(f"ℹ️ No backlog at {bf.rel} yet.")
            return 0
        raise BacklogError(f"no backlog at {bf.rel}")
    backlog = bf.load()

    if args.command == "summary":
        if args.json:
            print(json.dumps(summary_data(backlog, bf.blocking_severities), indent=2))
        else:
            print_summary(backlog, bf)
        return 0

    if args.command == "list":
        items = backlog.items if args.all else backlog.open_items()
        items = [i for i in items if (not args.type or i.type == args.type) and (not args.status or i.status == args.status)]
        if args.json:
            print(json.dumps([i.to_dict() for i in items], indent=2, ensure_ascii=False))
        else:
            for item in items:
                print(line_of(item))
            if not items:
                print("ℹ️ No matching items.")
        return 0

    if args.command == "show":
        item = backlog.find(args.id)
        if args.json:
            print(json.dumps(item.to_dict(), indent=2, ensure_ascii=False))
        else:
            print(f"{item.id}: {item.title}")
            for name in item.fields:
                print(f"  {name}: {item.get(name)}")
        return 0

    if args.command == "set":
        item = backlog.update(args.id, args.title, args.description, args.severity, args.priority, args.status, args.plan)
        message = f"✅ {item.id} updated: [{item.status}] ({item.rank}) {item.title}"
    elif args.command == "close":
        item = backlog.close(args.id, "done", today() + (f", {args.ref.strip()}" if args.ref else ""))
        message = f"✅ {item.id} closed as done"
    elif args.command == "drop":
        if not args.reason.strip():
            raise BacklogError("dropping an item needs a reason")
        item = backlog.close(args.id, "dropped", f"{today()}, {args.reason.strip()}")
        message = f"✅ {item.id} dropped"
    elif args.command == "duplicate":
        original = backlog.find(args.of)
        if original.id == args.id.upper():
            raise BacklogError("an item cannot duplicate itself")
        item = backlog.close(args.id, "duplicate", f"{today()}, duplicate of {original.id}")
        message = f"✅ {item.id} closed as duplicate of {original.id}"
    elif args.command == "reopen":
        item = backlog.reopen(args.id)
        message = f"✅ {item.id} reopened"
    else:
        raise BacklogError(f"unknown command '{args.command}'")
    bf.save(backlog)
    print(message)
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Capture, list and close items of the project backlog")
    sub = parser.add_subparsers(dest="command", required=True)

    def command(name, help_text):
        p = sub.add_parser(name, help=help_text)
        p.add_argument("--dir", default=None, help="Project directory (default: current; the Git root is used)")
        return p

    p = command("add", "Capture a new item")
    p.add_argument("--type", required=True, choices=list(TYPES))
    p.add_argument("--title", required=True)
    p.add_argument("--description", required=True)
    p.add_argument("--severity", choices=SEVERITIES, help="Required for bugs")
    p.add_argument("--priority", choices=PRIORITIES, help="Other types (default: medium)")
    p.add_argument("--found-in", default=None, help="Where it was found: version, pull request, test")

    p = command("list", "List items (open ones by default)")
    p.add_argument("--type", choices=list(TYPES))
    p.add_argument("--status", choices=OPEN_STATUSES + CLOSED_STATUSES)
    p.add_argument("--all", action="store_true", help="Include closed items")
    p.add_argument("--json", action="store_true")

    p = command("summary", "Counts per type and the bugs that come before new work")
    p.add_argument("--json", action="store_true")

    p = command("show", "Show one item")
    p.add_argument("id")
    p.add_argument("--json", action="store_true")

    p = command("set", "Change fields of an open item")
    p.add_argument("id")
    p.add_argument("--title")
    p.add_argument("--description")
    p.add_argument("--severity", choices=SEVERITIES)
    p.add_argument("--priority", choices=PRIORITIES)
    p.add_argument("--status", choices=OPEN_STATUSES)
    p.add_argument("--plan", help="Plan id ('-' clears it)")

    p = command("close", "Close an item as done")
    p.add_argument("id")
    p.add_argument("--ref", default=None, help="Pull request or commit that delivered it")

    p = command("drop", "Close an item that will not be done")
    p.add_argument("id")
    p.add_argument("--reason", required=True)

    p = command("duplicate", "Close an item as a duplicate of another")
    p.add_argument("id")
    p.add_argument("--of", required=True)

    p = command("reopen", "Move a closed item back to its section as open")
    p.add_argument("id")

    command("validate", "Check the backlog format")
    return parser


def main(argv: list = None) -> int:
    setup_stdout()
    args = build_parser().parse_args(argv)
    try:
        return cmd(args)
    except BacklogError as e:
        print(f"❌ {e}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
