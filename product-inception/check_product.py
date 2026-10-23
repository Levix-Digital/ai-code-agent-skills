"""
Check Product - product-inception Skill
Validates the two product documents written in Phase 0 (Product Inception): the business
document (default docs/product/VISION.md) and the technical specification (default
docs/product/TECH_SPEC.md). Checks the required sections, leftover template placeholders,
empty sections, the language (English unless configured) and a length limit. Open questions
are reported as warnings: the user decides at gate 0 whether they block the baseline.

Usage:
    python check_product.py [--dir <project>] [--init] [--json]
`--init` creates the missing documents from the templates (never overwrites).
Exit code 1 on errors.
"""
import argparse
import json
import os
import re
import shutil
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "dev-cycle-protocol"))
sys.path.insert(0, os.path.join(HERE, "..", "architecture-docs"))
from project_config import load_project_config  # noqa: E402
from _docs import fenced_lines, foreign_language_hits, headings, strip_code  # noqa: E402

DEFAULT_MAX_LINES = 400
TEMPLATES = {
    "vision": os.path.join(HERE, "templates", "VISION_TEMPLATE.md"),
    "tech_spec": os.path.join(HERE, "templates", "TECH_SPEC_TEMPLATE.md"),
}
LABELS = {"vision": "Business document", "tech_spec": "Technical specification"}
REQUIRED_SECTIONS = {
    "vision": ["Problem", "Target Users", "Value Proposition", "Business Model", "Success Metrics",
               "Scope", "Non-Goals", "Risks and Assumptions", "Open Questions"],
    "tech_spec": ["Platforms and Stack", "Architecture Overview", "Non-Functional Requirements",
                  "Integrations", "Data", "Constraints", "Open Questions"],
}
OPEN_QUESTIONS = "Open Questions"
NO_QUESTIONS_RE = re.compile(r"^(none|n/?a|no open questions)\.?$", re.I)
# A template placeholder: "[...]" that is not a link, a checkbox or a footnote.
PLACEHOLDER_RE = re.compile(r"(?<!!)\[(?![ xX]\])(?!\^)[^\]\n]{3,}\](?![(\[:])")


class Result:
    def __init__(self, kind: str, path: str, rel: str):
        self.kind, self.path, self.rel = kind, path, rel
        self.errors, self.warnings, self.open_questions = [], [], []

    def error(self, line, msg):
        self.errors.append(f"{self.rel}:{line}: {msg}")

    def warn(self, line, msg):
        self.warnings.append(f"{self.rel}:{line}: {msg}")

    def to_dict(self) -> dict:
        return {"kind": self.kind, "file": self.rel, "errors": self.errors, "warnings": self.warnings,
                "open_questions": self.open_questions}


def section_bodies(text: str) -> dict:
    """H2 title -> (heading line, body lines up to the next H2)."""
    lines = text.split("\n")
    h2 = [(n, title) for n, level, title in headings(text) if level == 2]
    out = {}
    for i, (n, title) in enumerate(h2):
        end = h2[i + 1][0] - 1 if i + 1 < len(h2) else len(lines)
        out[title.strip()] = (n, lines[n:end])
    return out


def meaningful(lines: list) -> list:
    return [l.strip() for l in lines if l.strip() and not l.strip().startswith("<!--")]


def check_text(kind: str, path: str, rel: str, text: str, max_lines: int, language: str = "en") -> Result:
    res = Result(kind, path, rel)
    lines = text.split("\n")
    hs = headings(text)
    if not [h for h in hs if h[1] == 1]:
        res.error(1, "missing title (a single '# ...' heading)")

    bodies = section_bodies(text)
    for name in REQUIRED_SECTIONS[kind]:
        if name not in bodies:
            res.error(1, f"missing section: ## {name}")
            continue
        n, body = bodies[name]
        content = meaningful(body)
        if not content:
            res.error(n, f"section '{name}' is empty")
        elif name == OPEN_QUESTIONS and not (len(content) == 1 and NO_QUESTIONS_RE.match(content[0])):
            res.open_questions = [re.sub(r"^[-*]\s+|^\d+\.\s+", "", c) for c in content]
            res.warn(n, f"{len(res.open_questions)} open question(s): answer them or let the user accept them at gate 0")

    plain = strip_code(text).split("\n")
    fenced = fenced_lines(lines)
    for n, (line, stripped, in_code) in enumerate(zip(lines, plain, fenced), 1):
        if in_code:
            continue
        if "ACTION REQUIRED" in line:
            res.error(n, "template instruction left in place")
        m = PLACEHOLDER_RE.search(re.sub(r"`[^`]*`", "", line))
        if m:
            res.error(n, f"template placeholder left in place: {m.group(0)[:60]}")
        is_heading = line.lstrip().startswith("#")
        hits = foreign_language_hits(line if is_heading else stripped, language)
        if hits:
            (res.error if is_heading else res.warn)(n, f"text may not be in English: {', '.join(hits)}")

    if len(lines) > max_lines:
        res.warn(len(lines), f"{len(lines)} lines (limit {max_lines}): keep the essentials; details go to the technical docs")
    return res


def product_paths(cfg) -> dict:
    section = cfg.section("product")
    return {kind: section.get(kind) or default for kind, default in
            (("vision", "docs/product/VISION.md"), ("tech_spec", "docs/product/TECH_SPEC.md"))}


def run(project_dir: str = None) -> list:
    cfg = load_project_config(project_dir)
    max_lines = int(cfg.section("product").get("max_lines") or DEFAULT_MAX_LINES)
    language = (cfg.section("architecture_docs") or {}).get("language", "en")
    results = []
    for kind, rel in product_paths(cfg).items():
        path = os.path.join(cfg.root, rel)
        if not os.path.isfile(path):
            res = Result(kind, path, rel)
            res.error(1, f"{LABELS[kind].lower()} not found (create it with check_product.py --init)")
            results.append(res)
            continue
        with open(path, "r", encoding="utf-8") as f:
            text = f.read().replace("\r\n", "\n")
        results.append(check_text(kind, path, rel, text, max_lines, language))
    return results


def init_docs(project_dir: str = None) -> list:
    cfg = load_project_config(project_dir)
    created = []
    for kind, rel in product_paths(cfg).items():
        path = os.path.join(cfg.root, rel)
        if os.path.exists(path):
            continue
        os.makedirs(os.path.dirname(path), exist_ok=True)
        shutil.copyfile(TEMPLATES[kind], path)
        created.append(rel)
    return created


def main(argv: list = None) -> int:
    if sys.stdout.encoding != "utf-8":
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass
    parser = argparse.ArgumentParser(description="Validate the product documents of Phase 0 (product inception)")
    parser.add_argument("--dir", default=None, help="Project directory (default: current directory)")
    parser.add_argument("--init", action="store_true", help="Create the missing documents from the templates")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)

    if args.init:
        created = init_docs(args.dir)
        for rel in created:
            print(f"✅ Created {rel} from the template")
        if not created:
            print("ℹ️ Both product documents already exist.")
        return 0

    results = run(args.dir)
    if args.json:
        print(json.dumps([r.to_dict() for r in results], indent=2, ensure_ascii=False))
    else:
        for r in results:
            for e in r.errors:
                print(f"❌ {e}")
            for w in r.warnings:
                print(f"⚠️ {w}")
            for q in r.open_questions:
                print(f"   ❓ {q}")
        errors = sum(len(r.errors) for r in results)
        warnings = sum(len(r.warnings) for r in results)
        print(f"\n{'✅' if not errors else '❌'} product-inception: {errors} error(s), {warnings} warning(s).")
    return 1 if any(r.errors for r in results) else 0


if __name__ == "__main__":
    sys.exit(main())
