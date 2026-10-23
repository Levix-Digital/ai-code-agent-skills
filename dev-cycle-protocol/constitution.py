"""
Constitution - dev-cycle-protocol Skill
Generates Spec Kit's constitution (.specify/memory/constitution.md) from the project's
sources of truth, so Spec Kit's Constitution Check enforces the same rules as the protocol:
  - software-design: universal principles, simplicity first, the guide of the project's style
  - dev-cycle-protocol: the cycle's principles
  - the product: non-functional requirements and constraints of the technical specification
  - the accepted ADRs
The file is never edited by hand. Its header records a hash of the generated content and of
the sources; `check` fails when a source changed (stale) or the file was edited (hand edit).

Usage:
    python constitution.py generate [--dir <project>] [--stdout] [--replace-hand-written]
    python constitution.py check    [--dir <project>]

A constitution written by hand (a project that used Spec Kit without the protocol) is never
replaced silently: move its principles into the sources first, then generate with
--replace-hand-written, which keeps a copy in .specify/memory/constitution.hand-written.md.
"""
import argparse
import datetime
import hashlib
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SKILLS_ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(SKILLS_ROOT, "architecture-docs"))
from project_config import load_project_config  # noqa: E402
from _docs import Project  # noqa: E402

CONSTITUTION = os.path.join(".specify", "memory", "constitution.md")
HAND_WRITTEN_COPY = os.path.join(".specify", "memory", "constitution.hand-written.md")
TEMPLATE_MARKERS = ("[PROJECT_NAME]", "[PRINCIPLE_1_NAME]")
HEADER_RE = re.compile(r"\A<!-- dev-cycle-constitution\n(.*?)\n-->\n", re.S)
META_RE = re.compile(r"^([a-z0-9-]+):\s*(.+)$", re.M)
VERSION_LINE_RE = re.compile(r"^\*\*Version\*\*: (\d+)\.(\d+)\.(\d+) \| \*\*Ratified\*\*: (\S+) \| \*\*Last Amended\*\*: (\S+)$", re.M)
STYLE_SECTIONS = ["Structure", "Dependency rule", "Where things go", "Common mistakes"]
ROMAN = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII"]


class ConstitutionError(RuntimeError):
    """The constitution cannot be generated or checked; the message says why."""


def sha(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def read(path: str) -> str:
    with open(path, "r", encoding="utf-8") as f:
        return f.read().replace("\r\n", "\n")


def h2_sections(text: str) -> dict:
    """H2 title (without emoji or decoration) -> body up to the next H2 or a '---' rule."""
    out, title, body = {}, None, []
    for line in text.split("\n"):
        m = re.match(r"^## (?!#)(.+?)\s*$", line)
        if m or line.strip() == "---":
            if title is not None:
                out[title] = "\n".join(body).strip()
            title = re.sub(r"^[^\w(]+", "", m.group(1)).strip() if m else None
            body = []
            continue
        if title is not None:
            body.append(line)
    if title is not None:
        out[title] = "\n".join(body).strip()
    return out


def find_section(sections: dict, prefix: str):
    for title, body in sections.items():
        if title.lower().startswith(prefix.lower()):
            return body
    return None


def demote(text: str, levels: int = 1) -> str:
    return re.sub(r"^(#{2,5}) ", lambda m: "#" * (len(m.group(1)) + levels) + " ", text, flags=re.M)


def strip_comments(text: str) -> str:
    return re.sub(r"<!--.*?-->", "", text, flags=re.S).strip()


def collect_sources(root: str) -> dict:
    """The text of each source the constitution is built from (missing ones are None)."""
    cfg = load_project_config(root)
    design = read(os.path.join(SKILLS_ROOT, "software-design", "SKILL.md"))
    protocol = read(os.path.join(SKILLS_ROOT, "dev-cycle-protocol", "SKILL.md"))
    design_sections, protocol_sections = h2_sections(design), h2_sections(protocol)
    sources = {
        "principles": find_section(design_sections, "Universal Principles"),
        "simplicity": find_section(design_sections, "Simplicity First"),
        "cycle": find_section(protocol_sections, "Principles"),
        "style": None, "style_name": cfg.section("architecture").get("style"),
        "nfr": None, "constraints": None, "tech_spec": None, "adrs": [],
    }
    if not sources["principles"] or not sources["cycle"]:
        raise ConstitutionError("software-design or dev-cycle-protocol SKILL.md no longer has the expected principles sections")
    style = sources["style_name"]
    style_path = os.path.join(SKILLS_ROOT, "software-design", "styles", f"{style}.md") if style else None
    if style_path and os.path.isfile(style_path):
        sections = h2_sections(read(style_path))
        sources["style"] = "\n\n".join(f"#### {name}\n\n{sections[name]}" for name in STYLE_SECTIONS if sections.get(name))
    tech_rel = cfg.section("product").get("tech_spec") or "docs/product/TECH_SPEC.md"
    tech_path = os.path.join(cfg.root, tech_rel)
    if os.path.isfile(tech_path):
        sections = h2_sections(read(tech_path))
        sources["tech_spec"] = tech_rel
        sources["nfr"] = find_section(sections, "Non-Functional Requirements")
        sources["constraints"] = find_section(sections, "Constraints")
    project = Project(cfg.root)
    for adr in project.adrs():
        if adr.meta.get("status") == "accepted":
            sources["adrs"].append((adr.meta.get("id"), adr.meta.get("title") or adr.h1 or "", project.rel(adr.path)))
    return sources


def render_body(name: str, s: dict) -> str:
    """Everything except the header and the version line; its hash is the sources hash."""
    principles = [("Design", "software-design", s["principles"]), ("Simplicity First", "software-design", s["simplicity"])]
    if s["style"]:
        principles.append((f"Architecture Style: {s['style_name']}", f"software-design/styles/{s['style_name']}.md", s["style"]))
    elif s["style_name"]:
        principles.append((f"Architecture Style: {s['style_name']}", ".dev-cycle.json",
                           f"The project follows the `{s['style_name']}` style recorded in `.dev-cycle.json`."))
    principles.append(("Development Cycle", "dev-cycle-protocol", s["cycle"]))

    out = [f"# {name} Constitution", "",
           "The non-negotiable rules every specification, plan and task of this project follows. It is generated from the "
           "project's sources of truth by `dev-cycle-protocol/constitution.py`; change a source and regenerate it, never edit it.",
           "", "## Core Principles", ""]
    for i, (title, source, body) in enumerate(principles):
        out += [f"### {ROMAN[i]}. {title}", "", f"*Source: {source}*", "", demote(strip_comments(body or "")), ""]

    product = [(t, b) for t, b in (("Non-Functional Requirements", s["nfr"]), ("Constraints", s["constraints"])) if b]
    if product:
        out += ["## Product Requirements and Constraints", "", f"*Source: {s['tech_spec']}*", ""]
        for title, body in product:
            out += [f"### {title}", "", demote(strip_comments(body)), ""]

    if s["adrs"]:
        out += ["## Architecture Decisions", "", "Accepted decisions; a plan that contradicts one needs a new ADR that supersedes it.", ""]
        out += [f"- ADR-{int(i):04d}: {t} (`{p}`)" if isinstance(i, int) else f"- {t} (`{p}`)" for i, t, p in s["adrs"]]
        out += [""]

    out += ["## Governance", "",
            "- This constitution supersedes other practices for specifications, plans and tasks. A plan passes the Constitution "
            "Check or records each justified violation in its Complexity Tracking table; a lasting exception becomes an ADR.",
            "- Amendments change the source, not this file: an ADR for decisions, the technical specification for requirements "
            "and constraints, `.dev-cycle.json` for the architecture style, the skills for principles. Then run "
            "`constitution.py generate` and commit the result with the change.",
            "- Gate 3 of the development cycle blocks a plan while this file is stale or edited by hand.", ""]
    return "\n".join(out)


def parse_header(text: str):
    m = HEADER_RE.match(text)
    if not m:
        return None, text
    return dict(META_RE.findall(m.group(1))), text[m.end():]


def version_info(content: str):
    m = VERSION_LINE_RE.search(content or "")
    if not m:
        return None
    return (int(m.group(1)), int(m.group(2)), int(m.group(3))), m.group(4), m.group(5)


def build(root: str, previous: str = None, today: str = None) -> tuple:
    """(text, changed). `previous` is the current file content, used for version and dates."""
    cfg = load_project_config(root)
    today = today or datetime.date.today().isoformat()
    body = render_body(cfg.project_name, collect_sources(root))
    sources_hash = sha(body)
    meta, old_content = parse_header(previous) if previous else (None, None)
    info = version_info(old_content) if meta else None
    untouched = meta and meta.get("content-sha256") == sha(old_content)
    if untouched and meta.get("sources-sha256") == sources_hash and info:
        return previous, False
    if info:
        (major, minor, _), ratified = info[0], info[1]
        version, last = f"{major}.{minor + 1}.0", today
    else:
        version, ratified, last = "1.0.0", today, today
    content = f"{body}\n**Version**: {version} | **Ratified**: {ratified} | **Last Amended**: {last}\n"
    header = ("<!-- dev-cycle-constitution\n"
              "generated-by: dev-cycle-protocol/constitution.py (do not edit; change a source and regenerate)\n"
              f"sources-sha256: {sources_hash}\n"
              f"content-sha256: {sha(content)}\n"
              "-->\n")
    return header + content, True


def kind_of(text: str) -> str:
    """generated (by this script), template (Spec Kit's untouched placeholder) or hand-written."""
    if parse_header(text)[0] is not None:
        return "generated"
    if any(marker in text for marker in TEMPLATE_MARKERS):
        return "template"
    return "hand-written"


def path_of(root: str) -> str:
    return os.path.join(root, CONSTITUTION)


def check(root: str) -> list:
    """Problems with the project's constitution; empty when it is current."""
    path = path_of(root)
    if not os.path.isdir(os.path.join(root, ".specify")):
        return ["Spec Kit is not initialized (.specify/ missing)"]
    if not os.path.isfile(path):
        return [f"{CONSTITUTION} is missing: run constitution.py generate"]
    text = read(path)
    kind = kind_of(text)
    if kind == "template":
        return [f"{CONSTITUTION} is still Spec Kit's template: run constitution.py generate"]
    if kind == "hand-written":
        return [f"{CONSTITUTION} is hand-written (Spec Kit used without the protocol): move its principles into the sources "
                "(TECH_SPEC.md, ADRs) with the user, then run constitution.py generate --replace-hand-written"]
    meta, content = parse_header(text)
    problems = []
    if meta.get("content-sha256") != sha(content):
        problems.append(f"{CONSTITUTION} was edited by hand: change the source instead and run constitution.py generate")
    current = sha(render_body(load_project_config(root).project_name, collect_sources(root)))
    if meta.get("sources-sha256") != current:
        problems.append(f"{CONSTITUTION} is stale: a source changed (principles, style, technical specification or ADRs); "
                        "run constitution.py generate")
    return problems


def main(argv: list = None) -> int:
    if sys.stdout.encoding != "utf-8":
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass
    parser = argparse.ArgumentParser(description="Generate or check Spec Kit's constitution from the project's sources")
    parser.add_argument("command", choices=["generate", "check"])
    parser.add_argument("--dir", default=None, help="Project directory (default: current; the Git root is used)")
    parser.add_argument("--stdout", action="store_true", help="generate: print instead of writing")
    parser.add_argument("--replace-hand-written", action="store_true",
                        help="generate: replace a hand-written constitution, keeping a copy, after its principles were moved")
    args = parser.parse_args(argv)
    root = load_project_config(Project(args.dir).root).root

    try:
        if args.command == "check":
            problems = check(root)
            for p in problems:
                print(f"❌ {p}")
            print(f"{'✅' if not problems else '❌'} constitution: {'current' if not problems else f'{len(problems)} problem(s)'}.")
            return 1 if problems else 0
        if not os.path.isdir(os.path.join(root, ".specify")) and not args.stdout:
            raise ConstitutionError("Spec Kit is not initialized (.specify/ missing): run speckit_setup.py init first")
        path = path_of(root)
        previous = read(path) if os.path.isfile(path) else None
        if previous is not None and kind_of(previous) == "hand-written" and not args.stdout:
            if not args.replace_hand_written:
                raise ConstitutionError(
                    f"{CONSTITUTION} is hand-written. Move its principles into the sources first (constraints and "
                    "non-functional requirements to TECH_SPEC.md, decisions to ADRs, general design rules as a suggestion "
                    "for software-design), then run generate --replace-hand-written; a copy is kept.")
            copy = os.path.join(root, HAND_WRITTEN_COPY)
            if not os.path.exists(copy):
                with open(copy, "w", encoding="utf-8", newline="\n") as f:
                    f.write(previous)
            print(f"📦 Hand-written constitution kept in {HAND_WRITTEN_COPY}")
            previous = None
        if previous is not None and parse_header(previous)[0] is not None:
            meta, content = parse_header(previous)
            if meta.get("content-sha256") != sha(content):
                print(f"⚠️ {CONSTITUTION} was edited by hand; the edits are replaced. Move them into the right source.")
        text, changed = build(root, previous)
        if args.stdout:
            print(text, end="")
            return 0
        if not changed:
            print(f"✅ {CONSTITUTION} is current.")
            return 0
        with open(path, "w", encoding="utf-8", newline="\n") as f:
            f.write(text)
        print(f"✅ {CONSTITUTION} generated ({version_info(parse_header(text)[1])[0][0]}."
              f"{version_info(parse_header(text)[1])[0][1]}.0); commit it with the change that caused it.")
        return 0
    except ConstitutionError as e:
        print(f"❌ {e}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
