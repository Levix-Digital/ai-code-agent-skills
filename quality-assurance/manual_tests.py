"""
Manual Tests - quality-assurance Skill
Validates the curated manual test suite, reports results per run, starts release
candidate runs and gates them.

Format (see MANUAL_TESTS_TEMPLATE.md):
    ## <Feature>
    ### MT-<AREA>-NNN: <title>
    - **Covers**: ...
    - **Prerequisites**: ...          (optional)
    - **Steps**: ...
    - **Expected**: ...
    - **Result** (<run>): [ ] Pass [ ] Fail [ ] Blocked
    - **Notes**: ...

Usage:
    python manual_tests.py validate|summary|failed|list|start-run|gate [options]
"""
import argparse
import os
import re
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "dev-cycle-protocol"))
from project_config import load_project_config  # noqa: E402

FEATURE_RE = re.compile(r"^##\s+(?!#)(.+?)\s*$")
TEST_RE = re.compile(r"^###\s+(?!#)(\S+?):\s*(.+?)\s*$")
TEST_ID_RE = re.compile(r"^MT-[A-Z][A-Z0-9]*-\d{3}$")
FIELD_RE = re.compile(r"^\s*-\s+\*\*([A-Za-z ]+)\*\*")
RESULT_RE = re.compile(
    r"^\s*-\s+\*\*Result\*\*(?:\s*\(([^)]*)\))?\s*:\s*"
    r"\[([ xX])\]\s*Pass\s+\[([ xX])\]\s*Fail\s+\[([ xX])\]\s*Blocked\s*$"
)
FENCE_RE = re.compile(r"^\s{0,3}(`{3,}|~{3,})")

REQUIRED_FIELDS = ["Covers", "Steps", "Expected", "Result", "Notes"]
KNOWN_FIELDS = REQUIRED_FIELDS + ["Prerequisites"]
STATUSES = ["Pass", "Fail", "Blocked"]
EMPTY_RESULT = "- **Result**{label}: [ ] Pass [ ] Fail [ ] Blocked"


class ManualTest:
    def __init__(self, test_id, title, feature, line):
        self.id, self.title, self.feature, self.line = test_id, title, feature, line
        self.fields = {}          # name -> first line number (0-based)
        self.result_line = None   # 0-based
        self.run = None
        self.status = None        # "Pass" | "Fail" | "Blocked" | None
        self.marked = 0
        self.result_malformed = False
        self.notes_lines = []     # 0-based indexes of the Notes line and its continuation
        self.end = line           # last line of the test (0-based)


class Suite:
    def __init__(self, path: str, text: str):
        self.path = path
        self.newline = "\r\n" if "\r\n" in text else "\n"
        self.lines = text.replace("\r\n", "\n").split("\n")
        self.tests, self.errors = [], []
        self._parse()

    def _parse(self):
        feature, current, fence = None, None, None
        in_notes = False
        for i, line in enumerate(self.lines):
            m_fence = FENCE_RE.match(line)
            if fence:
                if m_fence and m_fence.group(1)[0] == fence[0]:
                    fence = None
                continue
            if m_fence:
                fence = m_fence.group(1)
                in_notes = False
                continue

            m_test = TEST_RE.match(line)
            m_feature = FEATURE_RE.match(line)
            if m_feature and not line.startswith("###"):
                feature, current, in_notes = m_feature.group(1), None, False
                continue
            if line.startswith("###"):
                in_notes = False
                if not m_test:
                    self.errors.append(f"line {i + 1}: test heading must be '### MT-<AREA>-NNN: <title>'")
                    current = None
                    continue
                current = ManualTest(m_test.group(1), m_test.group(2), feature, i)
                if feature is None:
                    self.errors.append(f"line {i + 1}: test {current.id} is not under a '## <Feature>' section")
                self.tests.append(current)
                continue
            if line.startswith("#"):
                current, in_notes = None, False
                continue
            if current is None:
                continue

            m_field = FIELD_RE.match(line)
            if m_field:
                name = m_field.group(1).strip()
                in_notes = name == "Notes"
                current.fields.setdefault(name, i)
                current.end = i
                if name == "Result":
                    current.result_line = i
                    m_res = RESULT_RE.match(line)
                    if not m_res:
                        current.result_malformed = True
                    else:
                        current.run = (m_res.group(1) or "").strip() or None
                        boxes = [m_res.group(k).strip().lower() == "x" for k in (2, 3, 4)]
                        current.marked = sum(boxes)
                        if current.marked == 1:
                            current.status = STATUSES[boxes.index(True)]
                if in_notes:
                    current.notes_lines.append(i)
                continue
            if line.strip():
                current.end = i
                if in_notes:
                    current.notes_lines.append(i)
            elif in_notes:
                in_notes = False

    def notes_text(self, test: ManualTest) -> str:
        parts = []
        for k, i in enumerate(test.notes_lines):
            line = self.lines[i]
            if k == 0:
                line = re.sub(r"^\s*-\s+\*\*Notes\*\*\s*:?", "", line)
            parts.append(line.strip())
        return " ".join(p for p in parts if p)

    def validate(self) -> list:
        problems = list(self.errors)
        seen = {}
        for t in self.tests:
            where = f"line {t.line + 1}: {t.id}"
            if not TEST_ID_RE.match(t.id):
                problems.append(f"{where}: id must look like MT-<AREA>-NNN (for example MT-SYNC-001)")
            if t.id in seen:
                problems.append(f"{where}: duplicate id (also at line {seen[t.id] + 1})")
            seen.setdefault(t.id, t.line)
            for name in REQUIRED_FIELDS:
                if name not in t.fields:
                    problems.append(f"{where}: missing field '{name}'")
            for name in t.fields:
                if name not in KNOWN_FIELDS:
                    problems.append(f"{where}: unknown field '{name}'")
            if t.result_malformed:
                problems.append(f"{where}: Result line must be '- **Result** (<run>): [ ] Pass [ ] Fail [ ] Blocked'")
            elif t.marked > 1:
                problems.append(f"{where}: more than one result box is marked")
        return problems

    def select(self, run: str = None) -> list:
        return [t for t in self.tests if run is None or t.run == run]

    def start_run(self, run: str, features: list, ids: list, everything: bool) -> list:
        """Labels the selected tests with `run`, clears their results and notes; returns them."""
        wanted_features = {f.lower() for f in features}
        chosen = [t for t in self.tests
                  if everything or t.id in ids or (t.feature or "").lower() in wanted_features]
        removals = set()
        for t in chosen:
            if t.result_line is None:
                continue
            indent = re.match(r"^\s*", self.lines[t.result_line]).group(0)
            self.lines[t.result_line] = indent + EMPTY_RESULT.format(label=f" ({run})")
            if t.notes_lines:
                first = t.notes_lines[0]
                note_indent = re.match(r"^\s*", self.lines[first]).group(0)
                self.lines[first] = note_indent + "- **Notes**:"
                removals.update(t.notes_lines[1:])
        self.lines = [line for i, line in enumerate(self.lines) if i not in removals]
        return chosen

    def text(self) -> str:
        return self.newline.join(self.lines)


def load_suite(project_dir: str = None, file: str = None) -> Suite:
    cfg = load_project_config(project_dir)
    rel = file or cfg.section("tests").get("manual_tests") or "tests/MANUAL_TESTS.md"
    path = rel if os.path.isabs(rel) else os.path.join(cfg.root, rel)
    if not os.path.isfile(path):
        raise FileNotFoundError(f"manual test suite not found at {rel}")
    with open(path, "r", encoding="utf-8", newline="") as f:
        return Suite(path, f.read())


def status_label(t: ManualTest) -> str:
    return t.status or "Not run"


def cmd_summary(suite: Suite, run: str):
    tests = suite.select(run)
    title = f"run {run}" if run else "all tests"
    print(f"📋 Manual tests ({title}): {len(tests)} test(s)")
    by_feature = {}
    for t in tests:
        by_feature.setdefault(t.feature or "(no feature)", []).append(t)
    for feature, items in by_feature.items():
        counts = {s: sum(1 for t in items if status_label(t) == s) for s in STATUSES + ["Not run"]}
        print(f"   • {feature}: " + ", ".join(f"{k} {v}" for k, v in counts.items() if v))
    totals = {s: sum(1 for t in tests if status_label(t) == s) for s in STATUSES + ["Not run"]}
    print("   Total: " + ", ".join(f"{k} {v}" for k, v in totals.items()))


def cmd_failed(suite: Suite, run: str) -> int:
    bad = [t for t in suite.select(run) if t.status in ("Fail", "Blocked")]
    if not bad:
        print("✅ No Fail or Blocked tests" + (f" in run {run}." if run else "."))
        return 0
    for t in bad:
        notes = suite.notes_text(t) or "(no notes)"
        print(f"❌ {t.id} [{t.status}] {t.title}  ({t.feature})")
        print(f"   Notes: {notes}")
    return 1


def cmd_gate(suite: Suite, run: str) -> int:
    tests = suite.select(run)
    if not tests:
        print(f"❌ Run {run} has no tests. Start it with: manual_tests.py start-run --run {run} ...")
        return 1
    pending = [t for t in tests if t.status != "Pass"]
    if pending:
        for t in pending:
            print(f"❌ {t.id} {status_label(t)}: {t.title}")
        print(f"\n🚨 Acceptance gate for {run}: {len(tests) - len(pending)}/{len(tests)} Pass. Blocked.")
        return 1
    print(f"🟢 Acceptance gate for {run}: all {len(tests)} test(s) Pass.")
    return 0


def main() -> int:
    if sys.stdout.encoding != "utf-8":
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass
    parser = argparse.ArgumentParser(description="Curated manual test suite: validate, report, start and gate runs")
    parser.add_argument("command", choices=["validate", "summary", "failed", "list", "start-run", "gate"])
    parser.add_argument("--dir", default=None, help="Project directory (default: current directory)")
    parser.add_argument("--file", default=None, help="Suite path (default: tests.manual_tests or tests/MANUAL_TESTS.md)")
    parser.add_argument("--run", default=None, help="Run label, for example v1.4.0-rc.1")
    parser.add_argument("--feature", action="append", default=[], help="Feature (H2 title); repeatable")
    parser.add_argument("--id", action="append", default=[], help="Test id; repeatable")
    parser.add_argument("--all", action="store_true", help="With start-run: every test")
    args = parser.parse_args()

    try:
        suite = load_suite(args.dir, args.file)
    except FileNotFoundError as e:
        print(f"❌ {e}")
        return 1

    if args.command == "validate":
        problems = suite.validate()
        for p in problems:
            print(f"❌ {p}")
        print(f"\n{'✅' if not problems else '❌'} manual tests: {len(suite.tests)} test(s), {len(problems)} problem(s).")
        return 1 if problems else 0
    if args.command == "summary":
        cmd_summary(suite, args.run)
        return 0
    if args.command == "failed":
        return cmd_failed(suite, args.run)
    if args.command == "list":
        wanted = {f.lower() for f in args.feature}
        for t in suite.tests:
            if not wanted or (t.feature or "").lower() in wanted:
                print(f"{t.id}  {t.title}  ({t.feature}; {status_label(t)}{', run ' + t.run if t.run else ''})")
        return 0
    if args.command == "gate":
        if not args.run:
            print("❌ gate needs --run")
            return 1
        return cmd_gate(suite, args.run)
    if args.command == "start-run":
        if not args.run or not (args.feature or args.id or args.all):
            print("❌ start-run needs --run and at least one --feature, --id or --all")
            return 1
        problems = suite.validate()
        if problems:
            for p in problems:
                print(f"❌ {p}")
            print("Fix the suite before starting a run.")
            return 1
        chosen = suite.start_run(args.run, args.feature, args.id, args.all)
        missing = [i for i in args.id if i not in {t.id for t in suite.tests}]
        missing += [f for f in args.feature if f.lower() not in {(t.feature or "").lower() for t in suite.tests}]
        if missing:
            print(f"❌ Not found: {', '.join(missing)}. Nothing was written.")
            return 1
        with open(suite.path, "w", encoding="utf-8", newline="") as f:
            f.write(suite.text())
        print(f"✅ Run {args.run} started with {len(chosen)} test(s): {', '.join(t.id for t in chosen)}")
        return 0
    return 1


if __name__ == "__main__":
    sys.exit(main())
