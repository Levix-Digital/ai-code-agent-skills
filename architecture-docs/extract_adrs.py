"""
Extract ADRs Utility - architecture-docs Skill
Migration helper: copies every "ADR-NNN: Title" section of a legacy single-file
technical document (e.g. TECDOC.md) into docs/adr/NNNN-<slug>.md, one file per ADR.

- The section text is kept as written (no rewriting, no translation).
- A version in the title, e.g. "(`v0.45.0`)", moves to the `version` field.
- Blockquote notes that point to a later ADR (rectification notes) become
  `amended_by` on the old ADR and `amends` on the later one, and are removed.
- Relative links are rewritten for the new location; links to other ADR sections
  point to the new ADR files.
- `date` is the date of the commit that introduced the ADR heading (git pickaxe).
- The source document is not modified.

After extraction, each ADR still needs the migration work: translate it into the
project language, restructure it into the standard sections and fix the warnings
reported by check_docs.py.
"""
import argparse
import os
import re
import subprocess
import sys

from _docs import ADR_FIELD_ORDER, Doc, Project, adr_label, fenced_lines, github_anchor, setup_stdout, write_doc
from build_index import build_index

HEADING_RE = re.compile(r"^(#{2,4})\s+ADR-(\d+):\s*(.+?)\s*$")
ANY_HEADING_RE = re.compile(r"^(#{1,6})\s+")
VERSION_IN_TITLE_RE = re.compile(r"\s*\(\s*`?(v?\d+\.\d+(?:\.\d+)?)`?\s*\)\s*$")
NOTE_REF_RE = re.compile(r"ADR-(\d+)")
LINK_RE = re.compile(r"(\]\()([^)\s]+)(\))")


def parse_sections(text: str) -> list:
    lines = text.split("\n")
    flags = fenced_lines(lines)
    sections, current = [], None
    for i, (line, fenced) in enumerate(zip(lines, flags)):
        heading = None if fenced else ANY_HEADING_RE.match(line)
        if heading and current and len(heading.group(1)) <= current["level"]:
            current["end"] = i
            sections.append(current)
            current = None
        m = None if fenced else HEADING_RE.match(line)
        if m:
            if current:
                current["end"] = i
                sections.append(current)
            current = {"level": len(m.group(1)), "id": int(m.group(2)), "title": m.group(3), "start": i, "heading": line[len(m.group(1)) + 1:].strip()}
    if current:
        current["end"] = len(lines)
        sections.append(current)
    for s in sections:
        s["body"] = "\n".join(lines[s["start"] + 1:s["end"]]).strip("\n")
    return sections


def git_first_date(root: str, source: str, adr_number: str):
    try:
        out = subprocess.run(["git", "log", "-S", f"ADR-{adr_number}:", "--reverse", "--format=%cs", "--", source],
                             cwd=root, capture_output=True, text=True, check=True).stdout.split()
        return out[0] if out else None
    except Exception:
        return None


def main(project: Project, source: str, use_git_dates: bool, dry_run: bool, overwrite: bool):
    with open(source, "r", encoding="utf-8") as f:
        text = f.read()
    sections = parse_sections(text)
    if not sections:
        print(f"No 'ADR-NNN: Title' headings found in {project.rel(source)}.")
        return

    seen, adrs = {}, []
    for s in sections:
        if s["id"] in seen:
            print(f"⚠️ duplicate {adr_label(s['id'])} heading (line {s['start'] + 1}); keeping the first one", file=sys.stderr)
            continue
        seen[s["id"]] = s
        adrs.append(s)
    by_heading_anchor = {github_anchor(s["heading"]): s for s in adrs}

    for s in adrs:
        title = s["title"]
        version = None
        vm = VERSION_IN_TITLE_RE.search(title)
        if vm:
            version = vm.group(1) if vm.group(1).startswith("v") else f"v{vm.group(1)}"
            title = title[:vm.start()].strip()
        s["clean_title"] = title.rstrip(" .")
        s["version"] = version
        s["path"] = project.adr_path(s["id"], s["clean_title"])
        s["amended_by"], s["amends"] = set(), set()

    # Rectification notes: blockquote lines in an ADR pointing to a later ADR
    for s in adrs:
        kept = []
        for line in s["body"].split("\n"):
            refs = [int(r) for r in NOTE_REF_RE.findall(line)] if line.lstrip().startswith(">") else []
            later = [r for r in refs if r > s["id"] and r in seen]
            if later:
                s["amended_by"].update(later)
                for r in later:
                    seen[r]["amends"].add(s["id"])
                continue
            kept.append(line)
        s["body"] = "\n".join(kept).strip("\n")

    source_dir = os.path.dirname(source)

    def fix_link(m):
        target = m.group(2)
        if re.match(r"^[a-z]+:", target):
            return m.group(0)
        file_part, _, anchor = target.partition("#")
        if not file_part:
            ref = by_heading_anchor.get(anchor)
            if ref:
                return f"{m.group(1)}{os.path.basename(ref['path'])}{m.group(3)}"
            dest = os.path.relpath(source, project.adr_dir).replace(os.sep, "/")
            return f"{m.group(1)}{dest}#{anchor}{m.group(3)}"
        dest = os.path.relpath(os.path.join(source_dir, file_part), project.adr_dir).replace(os.sep, "/")
        return f"{m.group(1)}{dest}{'#' + anchor if anchor else ''}{m.group(3)}"

    written = 0
    for s in adrs:
        if os.path.exists(s["path"]) and not overwrite:
            print(f"= {project.rel(s['path'])} exists, skipped (--overwrite to replace)")
            continue
        date = git_first_date(project.root, project.rel(source), str(s["id"])) if use_git_dates else None
        meta = {
            "id": s["id"], "title": s["clean_title"], "status": "accepted", "date": date or "unknown",
            "version": s["version"], "subsystems": [], "supersedes": [], "superseded_by": [],
            "amends": sorted(s["amends"]), "amended_by": sorted(s["amended_by"]),
        }
        body = f"# {adr_label(s['id'])}: {s['clean_title']}\n\n" + LINK_RE.sub(fix_link, s["body"])
        relation = ""
        if meta["amended_by"]:
            relation = "  amended_by " + ", ".join(adr_label(i) for i in meta["amended_by"])
        print(f"{'(dry-run) ' if dry_run else ''}{adr_label(s['id'])} -> {project.rel(s['path'])}  [{meta['version'] or '-'}, {meta['date']}]{relation}")
        if not dry_run:
            write_doc(Doc(s["path"], meta, body, True), ADR_FIELD_ORDER)
        written += 1

    print(f"\n{'(dry-run) ' if dry_run else ''}{written} ADR(s) extracted from {project.rel(source)} (source not modified).")
    if not dry_run and written:
        if os.path.abspath(source) != os.path.abspath(project.index_path):
            build_index(project)
        print("👉 Next: translate and restructure each ADR (Context / Decision / Alternatives Considered / Consequences),")
        print("   set 'subsystems', run check_docs.py, then rewrite the index and subsystem pages.")


if __name__ == "__main__":
    setup_stdout()
    parser = argparse.ArgumentParser(description="Extract ADR sections from a legacy technical document into docs/adr")
    parser.add_argument("--source", type=str, default=None, help="Legacy document (default: the configured index, TECDOC.md)")
    parser.add_argument("--dir", type=str, default=None, help="Project directory (default: current; the Git root is used)")
    parser.add_argument("--no-git-dates", action="store_true", help="Do not look up ADR dates in the Git history")
    parser.add_argument("--overwrite", action="store_true", help="Replace ADR files that already exist")
    parser.add_argument("--dry-run", action="store_true", help="Only show what would be extracted")
    args = parser.parse_args()

    project = Project(args.dir)
    src = os.path.abspath(args.source) if args.source else project.index_path
    if not os.path.isfile(src):
        print(f"❌ Source not found: {src}", file=sys.stderr)
        sys.exit(1)
    main(project, src, not args.no_git_dates, args.dry_run, args.overwrite)
