"""
Check Docs Utility - architecture-docs Skill
Validates the project's architecture documentation (index, subsystem pages and ADRs).

Errors (exit code 1):
  - version numbers or manual numbering in headings of the index and subsystem pages
  - ADR frontmatter: missing fields, invalid status, file name / H1 not matching the id, duplicate ids
  - ADR relations not recorded in both directions, or pointing to missing ADRs
  - a superseded ADR without superseded_by (and the reverse)
  - broken relative links and anchors
  - index outdated (run build_index.py)
  - headings not in the configured language
Warnings:
  - files over max_lines
  - body lines that look like another language
  - ADRs missing a standard section
  - source paths listed in a subsystem page's `code` field that no longer exist

Put <!-- lang-ok --> on a line to exempt it from the language check (names, quotes).
"""
import argparse
import os
import re
import sys

from _docs import (ADR_SECTIONS, ADR_STATUSES, RELATIONS, Project, adr_label, anchors, as_id_list,
                   fenced_lines, foreign_language_hits, headings, read_doc, setup_stdout, strip_code)
from build_index import build_index

VERSION_RE = re.compile(r"\bv\d+\.\d+(\.\d+)?\b|\b\d+\.\d+\.\d+\b")
NUMBERED_RE = re.compile(r"^\d+(\.\d+)*[.)]?\s")
LINK_RE = re.compile(r"\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")


class Report:
    def __init__(self, project: Project):
        self.project = project
        self.errors, self.warnings = [], []

    def error(self, path, line, msg):
        self.errors.append(f"{self.project.rel(path)}:{line}: {msg}")

    def warn(self, path, line, msg):
        self.warnings.append(f"{self.project.rel(path)}:{line}: {msg}")


def body_offset(path: str) -> int:
    """Line number where the body starts (after frontmatter), so messages point at real lines."""
    with open(path, "r", encoding="utf-8") as f:
        text = f.read()
    if text.startswith("---\n"):
        end = text.find("\n---\n", 4)
        if end != -1:
            rest = text[end + 5:]
            return text[:end + 5].count("\n") + (len(rest) - len(rest.lstrip("\n")))
    return 0


def check_headings(report: Report, path: str, body: str, offset: int, structural: bool, language: str):
    for n, level, title in headings(body):
        line = n + offset
        if structural and level > 1 and VERSION_RE.search(title):
            report.error(path, line, f"version number in heading '{title}': describe the current state; versions belong in ADRs and releases")
        if structural and level > 1 and NUMBERED_RE.match(title):
            report.error(path, line, f"manually numbered heading '{title}': remove the number")
        hits = foreign_language_hits(title, language)
        if hits and "lang-ok" not in title:
            report.error(path, line, f"heading not in '{language}': {', '.join(hits)}")


def check_language_body(report: Report, path: str, body: str, offset: int, language: str):
    raw = body.split("\n")
    for n, line in enumerate(strip_code(body).split("\n"), 1):
        if line.lstrip().startswith("#") or "lang-ok" in raw[n - 1]:
            continue
        hits = foreign_language_hits(line, language)
        if hits:
            report.warn(path, n + offset, f"text may not be in '{language}': {', '.join(hits)}")


def check_links(report: Report, path: str, body: str, offset: int, anchor_cache: dict):
    lines = body.split("\n")
    for n, (line, fenced) in enumerate(zip(lines, fenced_lines(lines)), 1):
        if fenced:
            continue
        for target in LINK_RE.findall(re.sub(r"`[^`]*`", "", line)):
            if re.match(r"^[a-z]+:", target) or target.startswith("mailto:"):
                continue
            file_part, _, anchor = target.partition("#")
            dest = os.path.normpath(os.path.join(os.path.dirname(path), file_part)) if file_part else path
            if file_part and not os.path.exists(dest):
                report.error(path, n + offset, f"broken link: {target}")
                continue
            if anchor and dest.endswith(".md") and os.path.isfile(dest):
                if dest not in anchor_cache:
                    anchor_cache[dest] = anchors(read_doc(dest).body)
                if anchor not in anchor_cache[dest]:
                    report.error(path, n + offset, f"broken anchor: {target}")


def check_size(report: Report, path: str, max_lines: int):
    with open(path, "r", encoding="utf-8") as f:
        count = sum(1 for _ in f)
    if count > max_lines:
        report.warn(path, 1, f"{count} lines (limit {max_lines}): split it into smaller subsystem pages")


def check_adrs(report: Report, project: Project):
    adrs = project.adrs()
    by_id = {}
    for doc in adrs:
        m, path = doc.meta, doc.path
        offset = body_offset(path)
        for field in ("id", "title", "status", "date"):
            if not m.get(field):
                report.error(path, 1, f"ADR frontmatter missing '{field}'")
        adr_id = m.get("id")
        if not isinstance(adr_id, int):
            report.error(path, 1, "ADR 'id' must be an integer")
            continue
        if adr_id in by_id:
            report.error(path, 1, f"duplicate ADR id {adr_id} (also {project.rel(by_id[adr_id].path)})")
        by_id[adr_id] = doc
        if not os.path.basename(path).startswith(f"{adr_id:04d}-"):
            report.error(path, 1, f"file name must start with {adr_id:04d}-")
        if m.get("status") not in ADR_STATUSES:
            report.error(path, 1, f"invalid status '{m.get('status')}' (use {', '.join(ADR_STATUSES)})")
        if doc.h1 != f"{adr_label(adr_id)}: {m.get('title')}":
            report.error(path, offset + 1, f"H1 must be '# {adr_label(adr_id)}: {m.get('title')}'")
        if VERSION_RE.search(str(m.get("title", ""))):
            report.warn(path, 1, "version number in the ADR title: use the 'version' field")
        sections = {t for _, level, t in headings(doc.body) if level == 2}
        missing = [s for s in ADR_SECTIONS if s not in sections]
        if missing:
            report.warn(path, offset + 1, f"missing standard sections: {', '.join(missing)}")

    for adr_id, doc in by_id.items():
        m = doc.meta
        for forward, back in RELATIONS:
            for field, other_field in ((forward, back), (back, forward)):
                for target in as_id_list(m.get(field)):
                    other = by_id.get(target)
                    if not other:
                        report.error(doc.path, 1, f"'{field}' references missing {adr_label(target)}")
                    elif adr_id not in as_id_list(other.meta.get(other_field)):
                        report.error(doc.path, 1, f"'{field}: {adr_label(target)}' but {adr_label(target)} has no '{other_field}: {adr_label(adr_id)}'")
        superseded_by = as_id_list(m.get("superseded_by"))
        if m.get("status") == "superseded" and not superseded_by:
            report.error(doc.path, 1, "status 'superseded' requires 'superseded_by'")
        if superseded_by and m.get("status") != "superseded":
            report.error(doc.path, 1, f"has 'superseded_by' but status is '{m.get('status')}'")


def run(project: Project, include_plans: bool = False) -> Report:
    report = Report(project)
    cfg = project.config
    language, scope = cfg["language"], set(cfg["language_scope"]) | ({"plans"} if include_plans else set())
    anchor_cache = {}

    files = []  # (path, kind)
    if os.path.isfile(project.index_path):
        files.append((project.index_path, "index"))
    else:
        report.error(project.index_path, 1, "index missing: run build_index.py")
    files += [(d.path, "architecture") for d in project.architecture_docs()]
    files += [(d.path, "adr") for d in project.adrs()]
    if "plans" in scope and os.path.isdir(project.plans_dir):
        files += [(os.path.join(project.plans_dir, n), "plans") for n in sorted(os.listdir(project.plans_dir)) if n.endswith(".md")]

    for path, kind in files:
        doc = read_doc(path)
        offset = body_offset(path)
        check_headings(report, path, doc.body, offset, structural=kind in ("index", "architecture"), language=language if kind in scope else None)
        if kind in scope:
            check_language_body(report, path, doc.body, offset, language)
        if kind != "plans":
            check_links(report, path, doc.body, offset, anchor_cache)
            check_size(report, path, cfg["max_lines"])
        if kind == "architecture":
            for code in doc.meta.get("code") or []:
                if not os.path.exists(os.path.join(project.root, code)):
                    report.warn(path, 1, f"documented source path no longer exists: {code}")

    check_adrs(report, project)
    if os.path.isfile(project.index_path) and not build_index(project, check=True):
        report.error(project.index_path, 1, "index outdated: run build_index.py")
    return report


if __name__ == "__main__":
    setup_stdout()
    parser = argparse.ArgumentParser(description="Validate the project's architecture documentation")
    parser.add_argument("--dir", type=str, default=None, help="Project directory (default: current; the Git root is used)")
    parser.add_argument("--include-plans", action="store_true", help="Also check the language of docs/plans")
    parser.add_argument("--max-warnings", type=int, default=40, help="Warnings to print (default 40; 0 = all)")
    args = parser.parse_args()

    project = Project(args.dir)
    if not project.adopted():
        print(f"ℹ️ {project.name} does not use the architecture-docs layout yet "
              f"({project.config['architecture_dir']}, {project.config['adr_dir']}). Nothing to check.")
        sys.exit(0)

    report = run(project, args.include_plans)
    for e in report.errors:
        print(f"❌ {e}")
    shown = report.warnings if args.max_warnings == 0 else report.warnings[:args.max_warnings]
    for w in shown:
        print(f"⚠️ {w}")
    if len(shown) < len(report.warnings):
        print(f"⚠️ ... {len(report.warnings) - len(shown)} more warning(s) (--max-warnings 0 to show all)")
    print(f"\n{'❌' if report.errors else '✅'} architecture-docs: {len(report.errors)} error(s), {len(report.warnings)} warning(s).")
    sys.exit(1 if report.errors else 0)
