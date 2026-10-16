"""
Check README - user-docs Skill
Validates that the project's README is end-user documentation: standard sections,
current state only (no changelog or version sections), no implementation detail,
English only, working relative links and a length limit.

Usage:
    python check_readme.py [--dir <project>] [--file README.md] [--max-warnings N]
Exit code 1 on errors, or when warnings exceed --max-warnings.
"""
import argparse
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "dev-cycle-protocol"))
sys.path.insert(0, os.path.join(HERE, "..", "architecture-docs"))
from project_config import load_project_config  # noqa: E402
from _docs import anchors, fenced_lines, foreign_language_hits, headings, strip_code  # noqa: E402

DEFAULT_MAX_LINES = 400
EXEMPT_MARK = "readme-ok"

VERSION_RE = re.compile(r"\bv\d+\.\d+(\.\d+)?\b|\b\d+\.\d+\.\d+\b")
DATE_RE = re.compile(r"\b(19|20)\d{2}-\d{2}(-\d{2})?\b")
CHANGELOG_HEADING_RE = re.compile(
    r"\b(what'?s\s+new|change\s*log|changelog|release\s+notes|recent\s+changes|latest\s+changes|version\s+history)\b",
    re.I,
)
LINK_RE = re.compile(r"\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
ADR_RE = re.compile(r"\bADR[- ]?\d+\b")
SOURCE_EXT_RE = re.compile(r"\.(rs|py|ts|tsx|js|jsx|go|java|kt|cs|cpp|cc|c|h|hpp|swift|rb|php)\b")

# Section name -> words that identify its heading (lower case).
REQUIRED_SECTIONS = {
    "Installation": ["install", "installation", "setup", "getting started"],
    "Quick Start or Usage": ["quick start", "quickstart", "usage", "getting started", "how to use", "using"],
    "License": ["license", "licence"],
}
RECOMMENDED_SECTIONS = {
    "Features": ["features", "what it does", "highlights"],
    "Requirements": ["requirements", "prerequisites"],
    "Configuration": ["configuration", "settings", "config"],
    "Troubleshooting": ["troubleshooting", "faq", "common problems", "known issues"],
    "Documentation": ["documentation", "docs", "learn more", "further reading"],
}


class Result:
    def __init__(self, path: str):
        self.path = path
        self.errors, self.warnings = [], []

    def error(self, line, msg):
        self.errors.append(f"{os.path.basename(self.path)}:{line}: {msg}")

    def warn(self, line, msg):
        self.warnings.append(f"{os.path.basename(self.path)}:{line}: {msg}")


def has_section(titles: list, words: list) -> bool:
    return any(any(w in t.lower() for w in words) for t in titles)


def check_text(path: str, text: str, max_lines: int) -> Result:
    res = Result(path)
    lines = text.split("\n")
    raw_lines = lines

    # Title and description
    hs = headings(text)
    h1 = [h for h in hs if h[1] == 1]
    if not h1:
        res.error(1, "missing title (a single '# Name' heading)")
    else:
        if len(h1) > 1:
            res.warn(h1[1][0], "more than one H1 heading; keep a single title")
        start = h1[0][0]
        following = []
        for line in lines[start:]:
            if line.startswith("#"):
                break
            s = line.strip()
            if s and not s.startswith(("[![", "![", "<", "---", "|", "```")):
                following.append(s)
        if not following:
            res.error(start, "missing description: add one sentence on what the project is and for whom right after the title")

    # Headings: changelog style, versions, dates, language
    titles = [t for _, level, t in hs if level >= 2]
    for n, level, title in hs:
        if EXEMPT_MARK in raw_lines[n - 1]:
            continue
        if VERSION_RE.search(title) or DATE_RE.search(title):
            res.error(n, f"version or date in heading '{title}': describe the current state; history goes to the release notes")
        elif level >= 2 and CHANGELOG_HEADING_RE.search(title):
            res.error(n, f"changelog-style heading '{title}': history goes to the release notes, not the README")
        hits = foreign_language_hits(title)
        if hits:
            res.error(n, f"heading not in English: {', '.join(hits)}")

    # Sections
    for name, words in REQUIRED_SECTIONS.items():
        if not has_section(titles, words):
            res.error(1, f"missing required section: {name}")
    for name, words in RECOMMENDED_SECTIONS.items():
        if not has_section(titles, words):
            res.warn(1, f"missing recommended section: {name}")

    # Body checks (outside code blocks and inline code)
    stripped = strip_code(text).split("\n")
    fenced = fenced_lines(lines)
    for n, (line, plain, in_code) in enumerate(zip(lines, stripped, fenced), 1):
        if EXEMPT_MARK in line:
            continue
        if line.lstrip().startswith("```mermaid"):
            res.warn(n, "diagram in the README: architecture diagrams belong to the technical documentation")
        if in_code or plain.lstrip().startswith("#"):
            continue
        if VERSION_RE.search(plain):
            res.warn(n, "version reference in body text: describe the current behavior (mark required versions with <!-- readme-ok -->)")
        if ADR_RE.search(line):
            res.warn(n, "ADR reference: link to the technical documentation index instead")
        for target in LINK_RE.findall(re.sub(r"`[^`]*`", "", line)):
            if SOURCE_EXT_RE.search(target.split("#")[0]) and not re.match(r"^[a-z]+:", target):
                res.warn(n, f"link to a source file ({target}): implementation detail does not belong in the README")
        hits = foreign_language_hits(plain)
        if hits:
            res.warn(n, f"text may not be in English: {', '.join(hits)}")

    # Relative links and anchors
    own_anchors = anchors(text)
    for n, (line, in_code) in enumerate(zip(lines, fenced), 1):
        if in_code:
            continue
        for target in LINK_RE.findall(re.sub(r"`[^`]*`", "", line)):
            if re.match(r"^[a-z]+:", target) or target.startswith("mailto:"):
                continue
            file_part, _, anchor = target.partition("#")
            if not file_part:
                if anchor and anchor not in own_anchors:
                    res.error(n, f"broken anchor: #{anchor}")
                continue
            dest = os.path.normpath(os.path.join(os.path.dirname(path), file_part))
            if not os.path.exists(dest):
                res.error(n, f"broken link: {target}")

    if len(lines) > max_lines:
        res.warn(len(lines), f"{len(lines)} lines (limit {max_lines}): move long material to guides under docs/ and link them")
    return res


def run(project_dir: str = None, file: str = None) -> Result:
    cfg = load_project_config(project_dir)
    docs = cfg.section("docs")
    rel = file or docs.get("user_doc") or "README.md"
    path = rel if os.path.isabs(rel) else os.path.join(cfg.root, rel)
    max_lines = int(docs.get("readme_max_lines") or DEFAULT_MAX_LINES)
    if not os.path.isfile(path):
        res = Result(path)
        res.error(1, f"README not found at {rel}")
        return res
    with open(path, "r", encoding="utf-8") as f:
        text = f.read().replace("\r\n", "\n")
    return check_text(path, text, max_lines)


def main() -> int:
    if sys.stdout.encoding != "utf-8":
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass
    parser = argparse.ArgumentParser(description="Validate the project's README as end-user documentation")
    parser.add_argument("--dir", default=None, help="Project directory (default: current directory)")
    parser.add_argument("--file", default=None, help="README path (default: docs.user_doc or README.md)")
    parser.add_argument("--max-warnings", type=int, default=None, help="Fail when there are more warnings than this")
    args = parser.parse_args()

    res = run(args.dir, args.file)
    for e in res.errors:
        print(f"❌ {e}")
    for w in res.warnings:
        print(f"⚠️ {w}")
    too_many = args.max_warnings is not None and len(res.warnings) > args.max_warnings
    ok = not res.errors and not too_many
    print(f"\n{'✅' if ok else '❌'} user-docs: {len(res.errors)} error(s), {len(res.warnings)} warning(s).")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
