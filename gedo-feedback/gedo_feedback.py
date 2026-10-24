"""
Gedo Feedback - gedo-feedback Skill
Records what goes wrong with Gedo during real work (missed or false collisions, merges that fall
back to text, contract breaks that slip through, slowness, crashes, workarounds) so it becomes
input for Gedo's own backlog. One Markdown file per entry, so parallel threads never conflict.

Off by default: recording needs `gedo.enabled` true in `.dev-cycle.json` (and `gedo.feedback`,
true by default). Reading commands always work.

Usage:
    python gedo_feedback.py status [--json]
    python gedo_feedback.py add --kind K --title T --scenario S --expected E --actual A
                                [--severity S] [--reproduction R] [--workaround W] [--command C]
                                [--gedo-version V] [--found-in F] [--related "G3, FEAT-074"]
                                [--diagnostics-file PATH] [--doctor] [--allow-duplicate]
    python gedo_feedback.py list [--status S] [--kind K] [--all] [--json]
    python gedo_feedback.py show <ID> [--json]
    python gedo_feedback.py issue <ID> [--body-file PATH] [--json]
    python gedo_feedback.py reported <ID> --issue <URL>
    python gedo_feedback.py close <ID> --reason R
    python gedo_feedback.py validate
All commands accept --dir <project> (default: current directory; the folder of .dev-cycle.json is used).
"""
import argparse
import datetime
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "dev-cycle-protocol"))

import _no_ai  # noqa: E402
from project_config import BUG_SEVERITIES, load_project_config  # noqa: E402

KINDS = {
    "missed-collision": "A collision between parallel work that Gedo did not report",
    "false-collision": "A collision reported where there was none",
    "merge-fallback": "The merge driver fell back to Git's textual merge",
    "wrong-merge": "A merge without conflict markers that is wrong",
    "contract-miss": "A contract break that the post-merge check did not report",
    "identity": "A node identity lost, split or merged wrongly",
    "performance": "An operation slower than Gedo's requirements",
    "crash": "A Gedo command or the daemon failed",
    "data-loss": "Work or Gedo data lost or corrupted",
    "setup": "Installation, gedo init, hooks or environment problems",
    "interface": "Unclear output, missing JSON, prompts, exit codes",
    "workaround": "Something done around Gedo because it could not do it",
    "other": "Anything else",
}
STATUSES = ["open", "reported", "closed"]
OPEN_STATUSES = ["open", "reported"]
FIELDS = ["Id", "Kind", "Severity", "Status", "Gedo version", "Command", "Found", "Related", "Issue",
          "Recorded", "Closed"]
SECTIONS = ["Scenario", "Expected", "Actual", "Reproduction", "Workaround", "Diagnostics"]
REQUIRED_SECTIONS = ["Scenario", "Expected", "Actual"]
DIAGNOSTICS_MAX_LINES = 200


class FeedbackError(Exception):
    pass


def setup_stdout():
    if sys.stdout.encoding != "utf-8":
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass


def today() -> str:
    return datetime.date.today().isoformat()


def slugify(text: str, words: int = 6) -> str:
    parts = re.findall(r"[a-z0-9]+", text.lower())
    return "-".join(parts[:words]) or "entry"


def normalize_title(text: str) -> str:
    return " ".join(re.findall(r"[a-z0-9]+", text.lower()))


class Settings:
    """The `gedo` section of `.dev-cycle.json` and whether recording is on."""

    def __init__(self, start_dir: str = None):
        cfg = load_project_config(start_dir)
        self.root = cfg.root
        gedo = cfg.section("gedo")
        self.enabled = gedo.get("enabled") is True
        self.feedback = gedo.get("feedback") is True
        self.dir = os.path.join(self.root, gedo.get("feedback_dir") or "docs/gedo-feedback")
        self.repo = gedo.get("repo") or "Levix-Digital/gedo"
        self.allowed_terms = cfg.section("review").get("allowed_terms") or []

    @property
    def active(self) -> bool:
        return self.enabled and self.feedback

    def why_inactive(self) -> str:
        if not self.enabled:
            return "the project does not use Gedo yet (gedo.enabled is false in .dev-cycle.json)"
        if not self.feedback:
            return "Gedo feedback is turned off (gedo.feedback is false in .dev-cycle.json)"
        return ""

    def require_active(self):
        if not self.active:
            raise FeedbackError(f"Gedo feedback is off: {self.why_inactive()}.")


def render(entry: dict) -> str:
    lines = [f"# {entry['title']}", ""]
    for field in FIELDS:
        value = entry["fields"].get(field)
        if value or field in ("Id", "Kind", "Severity", "Status", "Recorded"):
            lines.append(f"- **{field}**: {value or '-'}")
    for section in SECTIONS:
        text = (entry["sections"].get(section) or "").strip()
        if text:
            lines += ["", f"## {section}", "", text]
    return "\n".join(lines) + "\n"


def parse(text: str, path: str = "") -> dict:
    title = re.search(r"^# (.+)$", text, re.M)
    if not title:
        raise FeedbackError(f"{path}: missing '# <title>' line")
    fields = {m.group(1): m.group(2).strip() for m in re.finditer(r"^- \*\*([^*]+)\*\*: (.*)$", text, re.M)}
    fields = {k: ("" if v == "-" else v) for k, v in fields.items()}
    sections = {}
    parts = re.split(r"^## (.+)$", text, flags=re.M)
    for name, body in zip(parts[1::2], parts[2::2]):
        sections[name.strip()] = body.strip()
    return {"title": title.group(1).strip(), "fields": fields, "sections": sections, "path": path}


class FeedbackStore:
    """The entries of one project: `<feedback_dir>/<id>.md`."""

    def __init__(self, folder: str):
        self.folder = folder

    def path(self, entry_id: str) -> str:
        return os.path.join(self.folder, f"{entry_id}.md")

    def all(self) -> list:
        if not os.path.isdir(self.folder):
            return []
        out = []
        for name in sorted(os.listdir(self.folder)):
            if name.endswith(".md") and name.lower() != "readme.md":
                path = os.path.join(self.folder, name)
                with open(path, "r", encoding="utf-8") as f:
                    out.append(parse(f.read(), path))
        return out

    def get(self, entry_id: str) -> dict:
        path = self.path(entry_id)
        if not os.path.isfile(path):
            raise FeedbackError(f"No Gedo feedback entry '{entry_id}' in {self.folder}")
        with open(path, "r", encoding="utf-8") as f:
            return parse(f.read(), path)

    def write(self, entry: dict):
        os.makedirs(self.folder, exist_ok=True)
        with open(self.path(entry["fields"]["Id"]), "w", encoding="utf-8", newline="\n") as f:
            f.write(render(entry))

    def new_id(self, title: str, date: str) -> str:
        base = f"{date}-{slugify(title)}"
        candidate, n = base, 2
        while os.path.exists(self.path(candidate)):
            candidate, n = f"{base}-{n}", n + 1
        return candidate

    def duplicate_of(self, kind: str, title: str):
        wanted = normalize_title(title)
        for entry in self.all():
            f = entry["fields"]
            if f.get("Status") in OPEN_STATUSES and f.get("Kind") == kind and normalize_title(entry["title"]) == wanted:
                return f.get("Id")
        return None


def validate_entry(entry: dict) -> list:
    problems = []
    name = os.path.basename(entry.get("path") or "") or entry["fields"].get("Id", "?")
    f = entry["fields"]
    if entry.get("path") and f.get("Id") != os.path.splitext(os.path.basename(entry["path"]))[0]:
        problems.append(f"{name}: Id '{f.get('Id')}' does not match the file name")
    if f.get("Kind") not in KINDS:
        problems.append(f"{name}: Kind '{f.get('Kind')}' is not one of: {', '.join(KINDS)}")
    if f.get("Severity") not in BUG_SEVERITIES:
        problems.append(f"{name}: Severity '{f.get('Severity')}' is not one of: {', '.join(BUG_SEVERITIES)}")
    if f.get("Status") not in STATUSES:
        problems.append(f"{name}: Status '{f.get('Status')}' is not one of: {', '.join(STATUSES)}")
    if f.get("Status") == "reported" and not f.get("Issue"):
        problems.append(f"{name}: a reported entry needs its Issue link")
    if f.get("Status") == "closed" and not f.get("Closed"):
        problems.append(f"{name}: a closed entry needs its Closed reason")
    for section in REQUIRED_SECTIONS:
        if not entry["sections"].get(section):
            problems.append(f"{name}: missing section '## {section}'")
    return problems


def run_doctor() -> str:
    """Output of `gedo doctor --json`, or a note saying why it is missing; never fails."""
    try:
        done = subprocess.run(["gedo", "doctor", "--json"], capture_output=True, text=True, timeout=30)
    except FileNotFoundError:
        return "gedo doctor: the gedo command was not found on PATH."
    except subprocess.TimeoutExpired:
        return "gedo doctor: timed out after 30 s."
    output = (done.stdout or "").strip() or (done.stderr or "").strip()
    return f"gedo doctor --json (exit {done.returncode}):\n\n```json\n{output}\n```"


def read_diagnostics(path: str) -> str:
    with open(path, "r", encoding="utf-8", errors="replace") as f:
        lines = f.read().splitlines()
    note = ""
    if len(lines) > DIAGNOSTICS_MAX_LINES:
        note = f"\n\n(last {DIAGNOSTICS_MAX_LINES} of {len(lines)} lines of {os.path.basename(path)})"
        lines = lines[-DIAGNOSTICS_MAX_LINES:]
    return "```text\n" + "\n".join(lines) + "\n```" + note


def issue_text(entry: dict, allowed: list = None) -> tuple:
    """(title, body) of the Gedo issue for an entry, checked against protocol principle 9."""
    f = entry["fields"]
    title = f"[{f.get('Kind')}] {entry['title']}"
    meta = [f"- **Kind**: {f.get('Kind')} ({KINDS.get(f.get('Kind'), '')})", f"- **Severity**: {f.get('Severity')}"]
    for field in ("Gedo version", "Command", "Related"):
        if f.get(field):
            meta.append(f"- **{field}**: {f[field]}")
    meta.append(f"- **Recorded**: {f.get('Recorded')}")
    body = ["Found while using Gedo on real work.", ""] + meta
    for section in SECTIONS:
        if entry["sections"].get(section):
            body += ["", f"## {section}", "", entry["sections"][section]]
    text = "\n".join(body) + "\n"
    _no_ai.check(title + "\n" + text, "the issue", allowed)
    return title, text


# ----------------------------------------------------------------- commands


def cmd_status(settings, store, args):
    entries = store.all()
    counts = {s: sum(1 for e in entries if e["fields"].get("Status") == s) for s in STATUSES}
    data = {"active": settings.active, "reason": settings.why_inactive(), "dir": settings.dir,
            "repo": settings.repo, "counts": counts}
    if args.json:
        print(json.dumps(data, indent=2))
        return 0
    print(f"🧪 Gedo feedback: {'on' if settings.active else 'off'}" + ("" if settings.active else f" ({data['reason']})"))
    print(f"   Entries: {os.path.relpath(settings.dir, settings.root)} "
          f"({counts['open']} open, {counts['reported']} reported, {counts['closed']} closed)")
    print(f"   Issues go to: {settings.repo}")
    return 0


def cmd_add(settings, store, args):
    settings.require_active()
    if args.kind not in KINDS:
        raise FeedbackError(f"--kind must be one of: {', '.join(KINDS)}")
    if not args.allow_duplicate:
        dup = store.duplicate_of(args.kind, args.title)
        if dup:
            raise FeedbackError(f"An open entry with the same kind and title exists: {dup}. "
                                f"Add to it instead, or pass --allow-duplicate.")
    diagnostics = []
    if args.diagnostics_file:
        diagnostics.append(read_diagnostics(args.diagnostics_file))
    if args.doctor:
        diagnostics.append(run_doctor())
    date = today()
    entry = {
        "title": args.title.strip(),
        "fields": {
            "Id": store.new_id(args.title, date), "Kind": args.kind, "Severity": args.severity, "Status": "open",
            "Gedo version": args.gedo_version or "", "Command": args.command or "", "Found": args.found_in or "",
            "Related": args.related or "", "Issue": "", "Recorded": date,
        },
        "sections": {
            "Scenario": args.scenario, "Expected": args.expected, "Actual": args.actual,
            "Reproduction": args.reproduction or "", "Workaround": args.workaround or "",
            "Diagnostics": "\n\n".join(diagnostics),
        },
    }
    store.write(entry)
    print(f"✅ {entry['fields']['Id']}: {entry['title']} ({args.kind}, {args.severity})")
    print(f"   {os.path.relpath(store.path(entry['fields']['Id']), settings.root)}")
    return 0


def summary(entry: dict) -> dict:
    f = entry["fields"]
    return {"id": f.get("Id"), "title": entry["title"], "kind": f.get("Kind"), "severity": f.get("Severity"),
            "status": f.get("Status"), "issue": f.get("Issue") or None, "recorded": f.get("Recorded")}


def cmd_list(settings, store, args):
    entries = store.all()
    if args.status:
        entries = [e for e in entries if e["fields"].get("Status") == args.status]
    elif not args.all:
        entries = [e for e in entries if e["fields"].get("Status") in OPEN_STATUSES]
    if args.kind:
        entries = [e for e in entries if e["fields"].get("Kind") == args.kind]
    rows = [summary(e) for e in entries]
    if args.json:
        print(json.dumps(rows, indent=2))
        return 0
    if not rows:
        print("No Gedo feedback entries.")
    for r in rows:
        issue = f" -> {r['issue']}" if r["issue"] else ""
        print(f"{r['id']}  [{r['status']}] {r['kind']}, {r['severity']}: {r['title']}{issue}")
    return 0


def cmd_show(settings, store, args):
    entry = store.get(args.id)
    if args.json:
        print(json.dumps({**summary(entry), "fields": entry["fields"], "sections": entry["sections"]}, indent=2))
    else:
        print(render(entry), end="")
    return 0


def cmd_issue(settings, store, args):
    settings.require_active()
    entry = store.get(args.id)
    if entry["fields"].get("Status") != "open":
        raise FeedbackError(f"{args.id} is {entry['fields'].get('Status')}; only open entries become issues.")
    title, body = issue_text(entry, settings.allowed_terms)
    if args.body_file:
        with open(args.body_file, "w", encoding="utf-8", newline="\n") as f:
            f.write(body)
    if args.json:
        print(json.dumps({"repo": settings.repo, "title": title, "body": body}, indent=2))
        return 0
    print(f"Repository: {settings.repo}")
    print(f"Title: {title}")
    if args.body_file:
        print(f"Body: {args.body_file}")
        print(f'Create it: gh issue create --repo {settings.repo} --title "{title}" --body-file "{args.body_file}"')
    else:
        print("")
        print(body, end="")
    return 0


def cmd_reported(settings, store, args):
    entry = store.get(args.id)
    if not re.match(r"^https?://\S+$", args.issue):
        raise FeedbackError("--issue must be the issue URL")
    entry["fields"].update({"Status": "reported", "Issue": args.issue})
    store.write(entry)
    print(f"✅ {args.id} reported: {args.issue}")
    return 0


def cmd_close(settings, store, args):
    entry = store.get(args.id)
    entry["fields"].update({"Status": "closed", "Closed": f"{today()}: {args.reason}"})
    store.write(entry)
    print(f"✅ {args.id} closed: {args.reason}")
    return 0


def cmd_validate(settings, store, args):
    problems = []
    try:
        entries = store.all()
    except FeedbackError as e:
        print(f"❌ {e}")
        return 1
    for entry in entries:
        problems += validate_entry(entry)
    for p in problems:
        print(f"❌ {p}")
    if not problems:
        print(f"✅ {len(entries)} Gedo feedback entries valid.")
    return 1 if problems else 0


def build_parser():
    parser = argparse.ArgumentParser(description="Record what goes wrong with Gedo, for Gedo's own backlog")
    parser.add_argument("--dir", default=None, help="Project directory (default: current directory)")
    sub = parser.add_subparsers(dest="action", required=True)

    p = sub.add_parser("status", help="Whether recording is on, and the entry counts")
    p.add_argument("--json", action="store_true")

    p = sub.add_parser("add", help="Record a new entry")
    p.add_argument("--kind", required=True, choices=list(KINDS))
    p.add_argument("--title", required=True)
    p.add_argument("--severity", default="medium", choices=BUG_SEVERITIES)
    p.add_argument("--scenario", required=True, help="What was being done, and how the work was split")
    p.add_argument("--expected", required=True)
    p.add_argument("--actual", required=True)
    p.add_argument("--reproduction", help="Minimal steps, without proprietary code")
    p.add_argument("--workaround", help="What was done instead")
    p.add_argument("--command", help="The Gedo or Git command involved")
    p.add_argument("--gedo-version")
    p.add_argument("--found-in", help="Branch, pull request or context")
    p.add_argument("--related", help="Gedo backlog items or proposals, e.g. 'G3, FEAT-074'")
    p.add_argument("--diagnostics-file", help="A log excerpt to attach (last 200 lines)")
    p.add_argument("--doctor", action="store_true", help="Attach the output of 'gedo doctor --json'")
    p.add_argument("--allow-duplicate", action="store_true")

    p = sub.add_parser("list", help="List entries (open and reported by default)")
    p.add_argument("--status", choices=STATUSES)
    p.add_argument("--kind", choices=list(KINDS))
    p.add_argument("--all", action="store_true")
    p.add_argument("--json", action="store_true")

    p = sub.add_parser("show", help="Show one entry")
    p.add_argument("id")
    p.add_argument("--json", action="store_true")

    p = sub.add_parser("issue", help="Title and body of the Gedo issue for an open entry")
    p.add_argument("id")
    p.add_argument("--body-file", help="Write the body to this file")
    p.add_argument("--json", action="store_true")

    p = sub.add_parser("reported", help="Mark an entry as reported, with its issue URL")
    p.add_argument("id")
    p.add_argument("--issue", required=True)

    p = sub.add_parser("close", help="Close an entry (fixed in Gedo, not a Gedo problem, ...)")
    p.add_argument("id")
    p.add_argument("--reason", required=True)

    sub.add_parser("validate", help="Check every entry")
    return parser


COMMANDS = {"status": cmd_status, "add": cmd_add, "list": cmd_list, "show": cmd_show, "issue": cmd_issue,
            "reported": cmd_reported, "close": cmd_close, "validate": cmd_validate}


def main(argv=None) -> int:
    setup_stdout()
    args = build_parser().parse_args(argv)
    try:
        settings = Settings(args.dir)
        return COMMANDS[args.action](settings, FeedbackStore(settings.dir), args)
    except (FeedbackError, ValueError, OSError) as e:
        print(f"❌ {e}")
        return 2 if isinstance(e, FeedbackError) and "is off" in str(e) else 1


if __name__ == "__main__":
    sys.exit(main())
