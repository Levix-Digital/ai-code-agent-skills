"""
Roadmap - plan-memory Skill
The releases of the product in delivery order and the backlog items each one delivers
(default: docs/ROADMAP.md). The roadmap is the single source of the item-to-release
assignment; the backlog holds the items. Writes the file; never commits.

Usage:
    python roadmap.py add-release <vX.Y.Z> --name N --goal G [--target T] [--before V | --after V]
    python roadmap.py set-release <vX.Y.Z> [--name N] [--goal G] [--target T] [--status planned|in_progress|released]
    python roadmap.py move-release <vX.Y.Z> --before V | --after V
    python roadmap.py assign <ID> [<ID> ...] --release <vX.Y.Z> [--move]   (an epic adds its open items)
    python roadmap.py unassign <ID> [<ID> ...]
    python roadmap.py show [--json]
    python roadmap.py current [--json]          (the open items of the first release not released)
    python roadmap.py validate
All commands accept --dir <project> (default: current directory; the Git root is used).
"""
import argparse
import json
import sys

from _backlog import BacklogError
from _backlog_store import BacklogFile
from _roadmap import STATUSES, RoadmapError
from _roadmap_store import RoadmapFile
from _store import setup_stdout, today


def load_backlog(start_dir: str):
    bf = BacklogFile(start_dir)
    return bf.load() if bf.exists else None


def expand(backlog, ids: list, roadmap, move: bool) -> list:
    """Backlog ids to assign: an epic becomes its open items (those not in another release, unless `move`)."""
    if backlog is None:
        raise RoadmapError("there is no backlog; capture the items first (backlog.py add)")
    out = []
    for item_id in ids:
        item = backlog.find(item_id)
        if item.type == "epic":
            members = [i for i in backlog.epic_items(item.id, include_closed=False)
                       if move or roadmap.release_of(i.id) is None]
            if not members:
                print(f"ℹ️ {item.id} has no open item to assign.")
            out += [i.id for i in members]
            continue
        if item.closed:
            raise RoadmapError(f"{item.id} is closed ({item.status})")
        out.append(item.id)
    return list(dict.fromkeys(out))


def describe(backlog, item_id: str) -> dict:
    if backlog is None:
        return {"id": item_id, "title": None, "status": None, "type": None, "rank": None}
    try:
        item = backlog.find(item_id)
    except BacklogError:
        return {"id": item_id, "title": None, "status": "missing", "type": None, "rank": None}
    return {"id": item.id, "title": item.title, "status": item.status, "type": item.type,
            "rank": item.rank, "closed": item.closed, "epic": item.epic}


def item_line(d: dict) -> str:
    mark = "✓" if d.get("closed") else "·"
    title = "(not in the backlog)" if d["status"] == "missing" else d["title"] or ""
    return f"{mark} {d['id']:<9} [{d['status'] or '?'}] ({d['rank'] or '?'}) {title}"


def current_data(roadmap, backlog) -> dict:
    release = roadmap.current()
    data = {"release": release.to_dict() if release else None, "open_items": [], "unscheduled": []}
    if backlog is None:
        return data
    order = {i.id: n for n, i in enumerate(backlog.open_items())}
    if release:
        items = [describe(backlog, i) for i in release.items]
        data["open_items"] = sorted([d for d in items if not d.get("closed") and d["status"] != "missing"],
                                    key=lambda d: order.get(d["id"], len(order)))
    assigned = roadmap.assigned
    data["unscheduled"] = [i.id for i in backlog.open_items() if i.type != "epic" and i.id not in assigned]
    return data


def warnings(roadmap, backlog) -> list:
    notes = []
    if backlog is None:
        return notes
    for release in roadmap.releases:
        for item_id in release.items:
            d = describe(backlog, item_id)
            if release.status == "released" and d["status"] in ("open", "planned", "in_progress"):
                notes.append(f"{release.version} is released but {item_id} is still {d['status']}")
            if release.status != "released" and d["status"] in ("dropped", "duplicate"):
                notes.append(f"{item_id} in {release.version} was closed as {d['status']}; unassign it")
            if release.status != "released" and d["status"] == "superseded":
                replacements = [r for r in backlog.find(item_id).superseded_by if roadmap.release_of(r) is None]
                hint = (f"; assign {', '.join(replacements)} (roadmap.py assign {' '.join(replacements)} --release {release.version})"
                        if replacements else "")
                notes.append(f"{item_id} in {release.version} was superseded; unassign it{hint}")
    return notes


def cmd(args) -> int:
    rf = RoadmapFile(args.dir)

    if args.command == "add-release":
        roadmap = rf.load(create=True)
        release = roadmap.add_release(args.version, args.name, args.goal, args.target, args.before, args.after)
        rf.save(roadmap)
        print(f"✅ {release.version}: {release.name} added to {rf.rel}")
        return 0

    if not rf.exists:
        if args.command in ("show", "current", "validate"):
            if getattr(args, "json", False):
                print(json.dumps([] if args.command == "show" else {"release": None, "open_items": [], "unscheduled": []}))
            else:
                print(f"ℹ️ No roadmap at {rf.rel} yet.")
            return 0
        raise RoadmapError(f"no roadmap at {rf.rel}; the first 'add-release' creates it")
    roadmap = rf.load()
    backlog = load_backlog(args.dir)

    if args.command == "validate":
        problems = roadmap.validate(backlog)
        for p in problems:
            print(f"❌ {p}")
        for w in warnings(roadmap, backlog):
            print(f"⚠️ {w}")
        if backlog is None and roadmap.assigned:
            print("⚠️ No backlog file: item ids not checked.")
        print(f"\n{'✅' if not problems else '❌'} roadmap: {len(roadmap.releases)} release(s), {len(problems)} problem(s).")
        return 1 if problems else 0

    if args.command == "show":
        data = [{**r.to_dict(), "items": [describe(backlog, i) for i in r.items]} for r in roadmap.releases]
        if args.json:
            print(json.dumps(data, indent=2, ensure_ascii=False))
            return 0
        print(f"🗺️ Roadmap: {rf.rel}")
        for r in data:
            done = len([i for i in r["items"] if i.get("closed")])
            when = f", released {r['released']}" if r["released"] else (f", target {r['target']}" if r["target"] else "")
            print(f"\n{r['version']}: {r['name']} [{r['status']}{when}] ({done}/{len(r['items'])} items closed)")
            print(f"   Goal: {r['goal']}")
            for d in r["items"]:
                print(f"   {item_line(d)}")
        return 0

    if args.command == "current":
        data = current_data(roadmap, backlog)
        if args.json:
            print(json.dumps(data, indent=2, ensure_ascii=False))
            return 0
        if not data["release"]:
            print("ℹ️ Every release in the roadmap is released; add the next one (roadmap.py add-release).")
        else:
            r = data["release"]
            print(f"🎯 Current release {r['version']}: {r['name']} [{r['status']}]")
            print(f"   Goal: {r['goal']}")
            for d in data["open_items"]:
                print(f"   {item_line(d)}")
            if not data["open_items"]:
                print("   No open item left: the release is ready for its candidate (dev-cycle-protocol Phase 8).")
        if data["unscheduled"]:
            print(f"ℹ️ {len(data['unscheduled'])} open backlog item(s) in no release: {', '.join(data['unscheduled'])}")
        return 0

    if args.command == "set-release":
        release = roadmap.update_release(args.version, args.name, args.goal, args.target, args.status,
                                         released_on=today() if args.status == "released" else None)
        message = f"✅ {release.version}: {release.name} [{release.status}]"
        if args.status == "released":
            still_open = [i for i in release.items if describe(backlog, i)["status"] in ("open", "planned", "in_progress")]
            if still_open:
                message += f"\n⚠️ Still open in this release: {', '.join(still_open)}; move them (assign --move) or close them."
    elif args.command == "move-release":
        release = roadmap.move_release(args.version, args.before, args.after)
        message = f"✅ {release.version} moved"
    elif args.command == "assign":
        ids = expand(backlog, args.ids, roadmap, args.move)
        added = roadmap.assign(ids, args.release, args.move)
        message = f"✅ {args.release}: added {', '.join(added)}" if added else f"ℹ️ Nothing to add to {args.release}."
    elif args.command == "unassign":
        removed = roadmap.unassign(args.ids)
        message = f"✅ Removed {', '.join(removed)} from the roadmap"
    else:
        raise RoadmapError(f"unknown command '{args.command}'")
    rf.save(roadmap)
    print(message)
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Plan the releases of the product and the backlog items each one delivers")
    sub = parser.add_subparsers(dest="command", required=True)

    def command(name, help_text):
        p = sub.add_parser(name, help=help_text)
        p.add_argument("--dir", default=None, help="Project directory (default: current; the Git root is used)")
        return p

    p = command("add-release", "Add a release (at the end, or --before / --after another)")
    p.add_argument("version")
    p.add_argument("--name", required=True, help="Short name, for example MVP")
    p.add_argument("--goal", required=True, help="What the release achieves for its users")
    p.add_argument("--target", default=None, help="Target date or period (free text)")
    p.add_argument("--before", default=None)
    p.add_argument("--after", default=None)

    p = command("set-release", "Change a release")
    p.add_argument("version")
    p.add_argument("--name")
    p.add_argument("--goal")
    p.add_argument("--target", help="'' clears it")
    p.add_argument("--status", choices=STATUSES)

    p = command("move-release", "Reorder a release")
    p.add_argument("version")
    p.add_argument("--before", default=None)
    p.add_argument("--after", default=None)

    p = command("assign", "Put backlog items (or an epic's open items) into a release")
    p.add_argument("ids", nargs="+")
    p.add_argument("--release", required=True)
    p.add_argument("--move", action="store_true", help="Move items that are in another release")

    p = command("unassign", "Take items out of the roadmap")
    p.add_argument("ids", nargs="+")

    p = command("show", "Every release with its items")
    p.add_argument("--json", action="store_true")

    p = command("current", "The open items of the current release (the first one not released)")
    p.add_argument("--json", action="store_true")

    command("validate", "Check the roadmap format and its ids against the backlog")
    return parser


def main(argv: list = None) -> int:
    setup_stdout()
    args = build_parser().parse_args(argv)
    try:
        return cmd(args)
    except (RoadmapError, BacklogError) as e:
        print(f"❌ {e}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
