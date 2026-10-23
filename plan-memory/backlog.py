"""
Backlog - plan-memory Skill
The first level of planning: capture bugs, features, improvements and adjustments in the
project backlog (default: docs/BACKLOG.md) without discovery, group them under epics, list
and triage them, and close them. Writes the file; never commits.

Usage:
    python backlog.py add --type bug|feature|improvement|adjustment|epic --title T --description D
                          [--severity S] [--priority P] [--epic EPIC-NNN] [--found-in "v1.2.0, PR #12"]
    python backlog.py add --from-failed --run <rc> [--severity S]    (failed manual tests deferred as bugs)
    python backlog.py list [--type T] [--status S] [--epic EPIC-NNN] [--all] [--json]
    python backlog.py summary [--json]
    python backlog.py show <ID> [--json]
    python backlog.py set <ID> [--title T] [--description D] [--severity S] [--priority P] [--status S] [--plan ID] [--epic ID|-]
    python backlog.py close <ID> [--ref "PR #12"]
    python backlog.py drop <ID> --reason R
    python backlog.py duplicate <ID> --of <ID>
    python backlog.py supersede <ID> --by <ID>[,<ID>...]    (replaced by different, better defined items)
    python backlog.py reopen <ID>
    python backlog.py promote <ID> [<ID> ...] [--title T] [--id PLAN_ID] [--version V] | --into <PLAN_ID>
    python backlog.py validate
All commands accept --dir <project> (default: current directory; the Git root is used).
"""
import argparse
import json
import os
import re
import sys

from _backlog import (CLOSED_STATUSES, OPEN_STATUSES, PRIORITIES, SEVERITIES, TYPES,
                      Backlog, BacklogError)
from _backlog_store import BacklogFile, sync_plan_items
from _store import PlanStore, now_iso, setup_stdout, today
from save_plan import save_plan

HERE = os.path.dirname(os.path.abspath(__file__))
PLAN_TEMPLATES = {
    "bug": os.path.join(HERE, "templates", "BUG_PLAN_TEMPLATE.md"),
    "feature": os.path.join(HERE, "templates", "FEATURE_PLAN_TEMPLATE.md"),
}
ITEMS_SECTION = "## Backlog items"


def line_of(item) -> str:
    rank = item.rank or "?"
    plan = f"  → {item.plan}" if item.plan else ""
    epic = f"  [{item.epic}]" if item.epic else ""
    return f"{item.id:<9} [{item.status}] ({rank}) {item.title}{epic}{plan}"


def epic_progress(backlog: Backlog, epic) -> str:
    items = backlog.epic_items(epic.id)
    done = len([i for i in items if i.closed])
    return f"{done}/{len(items)} items closed"


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
                extra = f"  ({epic_progress(backlog, item)})" if item.type == "epic" else ""
                print(f"   {line_of(item)}{extra}")


def add_failed_manual_tests(bf: BacklogFile, run: str, severity: str = None) -> list:
    """One bug per manual test marked Fail in `run`, skipping tests already captured for that run."""
    sys.path.insert(0, os.path.join(HERE, "..", "quality-assurance"))
    from manual_tests import load_suite
    try:
        suite = load_suite(bf.config.root)
    except FileNotFoundError as e:
        raise BacklogError(str(e))
    failed = [t for t in suite.select(run) if t.status == "Fail"]
    if not failed:
        print(f"ℹ️ No manual test is marked Fail in run {run}.")
        return []
    backlog = bf.load(create=True)
    captured = " ".join(i.get("Description") for i in backlog.items)
    added = []
    for t in failed:
        marker = f"Manual test {t.id} failed in run {run}"
        if marker in captured:
            print(f"ℹ️ {t.id} is already in the backlog for run {run}.")
            continue
        notes = suite.notes_text(t)
        description = f"{marker} ({t.feature})." + (f" Notes: {notes}" if notes else "")
        item = backlog.add("bug", f"{t.id} fails: {t.title}", description, f"{today()}, {run}, {t.id}",
                           severity=severity or "high")
        added.append(item)
    if added:
        bf.save(backlog)
        for item in added:
            print(f"✅ {item.id}: {item.title} ({item.rank}) added to {bf.rel}")
        if not severity:
            print("   Severity defaults to high for failed acceptance tests; adjust with 'backlog.py set <ID> --severity <s>'.")
    return added


def render_items(items: list) -> str:
    """The items as they are copied into a plan body."""
    blocks = []
    for item in items:
        lines = [f"### {item.id}: {item.title}"]
        for name in ("Severity", "Priority", "Found", "Description"):
            if item.get(name):
                lines.append(f"- **{name}**: {item.get(name)}")
        blocks.append("\n".join(lines))
    return "\n\n".join(blocks)


def insert_items(body: str, items_text: str) -> str:
    """Adds items at the end of the plan's 'Backlog items' section (or a new one at the end)."""
    start = body.find(ITEMS_SECTION)
    if start == -1:
        return f"{body.rstrip()}\n\n{ITEMS_SECTION}\n\n{items_text}\n"
    m = re.compile(r"^## ", re.M).search(body, start + len(ITEMS_SECTION))
    end = m.start() if m else len(body)
    return f"{body[:end].rstrip()}\n\n{items_text}\n\n{body[end:].lstrip()}".rstrip() + "\n"


def promote(bf: BacklogFile, ids: list, title: str = None, into: str = None, plan_id: str = None,
            version: str = None, store: PlanStore = None):
    """Creates a draft plan for open backlog items (or adds them to `into`) and links both ways."""
    backlog = bf.load()
    items = []
    for item_id in ids:
        item = backlog.find(item_id)
        if item.closed:
            raise BacklogError(f"{item.id} is closed ({item.status}); reopen it first")
        if item.type == "epic":
            members = ", ".join(i.id for i in backlog.epic_items(item.id, include_closed=False)) or "none open"
            raise BacklogError(f"{item.id} is an epic; promote its items instead ({members})")
        if item.plan:
            raise BacklogError(f"{item.id} is already promoted into plan '{item.plan}'")
        if item in items:
            raise BacklogError(f"{item.id} is listed twice")
        items.append(item)
    if not items:
        raise BacklogError("name at least one backlog item")
    store = store or PlanStore(bf.config.root)
    all_bugs = all(i.type == "bug" for i in items)

    if into:
        plan = store.get(into)
        if not plan:
            raise BacklogError(f"plan '{into}' not found in {store.plans_dir}")
        if plan.meta.get("status") in ("completed", "superseded"):
            raise BacklogError(f"plan '{into}' is {plan.meta['status']}; promote into an open plan")
        plan.meta["backlog_items"] = plan.backlog_items + [i.id for i in items]
        if plan.kind == "bug" and not all_bugs:
            plan.meta["kind"] = "feature"
        plan.meta["updated_at"] = now_iso()
        plan.body = insert_items(plan.body, render_items(items))
        plan.add_log(store.author(), f"Backlog items added: {', '.join(i.id for i in items)}")
        store.save(plan)
        store.sync_index()
        print(f"✅ Added {', '.join(i.id for i in items)} to plan '{into}' ({plan.path})")
    else:
        if not title:
            if len(items) > 1:
                raise BacklogError("promoting several items needs --title")
            title = items[0].title
        kind = "bug" if all_bugs else "feature"
        with open(PLAN_TEMPLATES[kind], "r", encoding="utf-8") as f:
            body = f.read().replace("{title}", title).replace("{items}", render_items(items))
        plan = save_plan(store, title, body, plan_id=plan_id, version=version, status="draft",
                         kind=kind, backlog_items=",".join(i.id for i in items))
    for message in sync_plan_items(bf.config.root, plan):
        print(message)
    return plan


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
        for w in backlog.warnings():
            print(f"⚠️ {w}")
        print(f"\n{'✅' if not problems else '❌'} backlog: {len(backlog.items)} item(s), {len(problems)} problem(s).")
        return 1 if problems else 0

    if args.command == "add" and args.from_failed:
        if not args.run:
            raise BacklogError("--from-failed needs --run <rc>")
        add_failed_manual_tests(bf, args.run, args.severity)
        return 0

    if args.command == "add":
        if args.type is None or args.title is None or args.description is None:
            raise BacklogError("add needs --type, --title and --description (or --from-failed --run <rc>)")
        backlog = bf.load(create=True)
        found = today() + (f", {args.found_in.strip()}" if args.found_in and args.found_in.strip() else "")
        item = backlog.add(args.type, args.title, args.description, found, args.severity, args.priority, args.epic)
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
        items = [i for i in items if (not args.type or i.type == args.type) and (not args.status or i.status == args.status)
                 and (not args.epic or i.epic == args.epic.upper())]
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
        item = backlog.update(args.id, args.title, args.description, args.severity, args.priority, args.status, args.plan,
                              args.epic)
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
    elif args.command == "supersede":
        by = [i.strip() for i in args.by.split(",") if i.strip()]
        item = backlog.supersede(args.id, by, today())
        message = f"✅ {item.id} closed as superseded by {', '.join(item.superseded_by)}"
        if item.plan:
            message += f"\n⚠️ {item.id} was in plan '{item.plan}'; update the plan (backlog.py promote <new ids> --into {item.plan})."
    elif args.command == "promote":
        promote(bf, args.ids, args.title, args.into, args.id, args.version)
        return 0
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
    p.add_argument("--type", choices=list(TYPES))
    p.add_argument("--title")
    p.add_argument("--description")
    p.add_argument("--severity", choices=SEVERITIES, help="Required for bugs")
    p.add_argument("--priority", choices=PRIORITIES, help="Other types (default: medium)")
    p.add_argument("--epic", default=None, help="Epic the item belongs to (EPIC-NNN)")
    p.add_argument("--found-in", default=None, help="Where it was found: version, pull request, test")
    p.add_argument("--from-failed", action="store_true", help="Capture the manual tests marked Fail in --run as bugs")
    p.add_argument("--run", default=None, help="Release candidate run, with --from-failed")

    p = command("list", "List items (open ones by default)")
    p.add_argument("--type", choices=list(TYPES))
    p.add_argument("--status", choices=OPEN_STATUSES + CLOSED_STATUSES)
    p.add_argument("--epic", default=None, help="Only the items of this epic")
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
    p.add_argument("--epic", help="Epic id ('-' clears it)")

    p = command("close", "Close an item as done")
    p.add_argument("id")
    p.add_argument("--ref", default=None, help="Pull request or commit that delivered it")

    p = command("drop", "Close an item that will not be done")
    p.add_argument("id")
    p.add_argument("--reason", required=True)

    p = command("duplicate", "Close an item as a duplicate of another")
    p.add_argument("id")
    p.add_argument("--of", required=True)

    p = command("supersede", "Close an item replaced by different, better defined items")
    p.add_argument("id")
    p.add_argument("--by", required=True, help="Comma-separated ids of the items that replace it")

    p = command("reopen", "Move a closed item back to its section as open")
    p.add_argument("id")

    p = command("promote", "Create a draft plan for open items (or add them to an open plan)")
    p.add_argument("ids", nargs="+")
    p.add_argument("--title", help="Plan title (default: the item's title; required for several items)")
    p.add_argument("--id", help="Plan id (default: generated from the title)")
    p.add_argument("--version", help="Target version")
    p.add_argument("--into", help="Add the items to this existing plan instead")

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
