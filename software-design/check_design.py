"""
Check Design - software-design Skill
Validates the project's declared architecture and that the core does not depend on
names it must never import (architecture.forbidden_core_imports in .dev-cycle.json).

Usage:
    python check_design.py [--dir <project>] [--strict]
Exit code 1 on errors (and on warnings with --strict).
"""
import argparse
import os
import re
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "dev-cycle-protocol"))
from project_config import ARCHITECTURE_STYLES, load_project_config  # noqa: E402

SOURCE_EXTENSIONS = {
    ".rs", ".py", ".js", ".jsx", ".ts", ".tsx", ".mjs", ".cjs", ".go",
    ".c", ".h", ".cc", ".cpp", ".hpp", ".cs", ".java", ".kt", ".kts", ".swift",
}
# Build output, dependencies and test folders (tests may import adapters on purpose).
SKIPPED_DIRS = {".git", "target", "node_modules", "dist", "build", "__pycache__", ".venv", "venv", "vendor",
                "tests", "test", "__tests__", "benches", "examples"}

# One pattern per kind of import statement; group 1 is the imported path or module.
IMPORT_PATTERNS = [
    re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?use\s+([A-Za-z_][\w:]*)"),   # Rust use
    re.compile(r"^\s*extern\s+crate\s+([A-Za-z_]\w*)"),                     # Rust extern crate
    re.compile(r"^\s*import\s+([\w.]+)"),                                   # Python, Java, Kotlin, Go single
    re.compile(r"^\s*from\s+([\w.]+)\s+import\b"),                          # Python from
    re.compile(r"""^\s*import\s+.*?\s+from\s+['"]([^'"]+)['"]"""),         # JS/TS import ... from
    re.compile(r"""^\s*import\s+['"]([^'"]+)['"]"""),                      # JS/TS side-effect import, Go single quoted
    re.compile(r"""require\(\s*['"]([^'"]+)['"]\s*\)"""),                  # CommonJS require
    re.compile(r"""^\s*["`]([^"`]+)["`]\s*$"""),                           # Go grouped import line
    re.compile(r"""^\s*#\s*include\s*[<"]([^>"]+)[>"]"""),                 # C/C++ include
    re.compile(r"^\s*using\s+(?:static\s+)?([\w.]+)\s*;"),                 # C# using
]


def imported_names(line: str) -> list:
    names = []
    for pattern in IMPORT_PATTERNS:
        m = pattern.search(line)
        if m:
            names.append(m.group(1))
    return names


def matches_forbidden(name: str, forbidden: str) -> bool:
    """True when the import path starts with the forbidden module (any separator)."""
    def parts(value: str) -> list:
        return [p for p in re.split(r"::|\.|/|\\", value.strip().replace("-", "_")) if p]
    normalized, target = parts(name), parts(forbidden)
    return bool(target) and normalized[:len(target)] == target


def scan_core(core_dir: str, forbidden: list) -> list:
    """(relative_path, line_number, import, forbidden_name) for each violation."""
    violations = []
    for root, dirs, files in os.walk(core_dir):
        dirs[:] = [d for d in dirs if d not in SKIPPED_DIRS]
        for name in files:
            if os.path.splitext(name)[1].lower() not in SOURCE_EXTENSIONS:
                continue
            path = os.path.join(root, name)
            try:
                with open(path, "r", encoding="utf-8", errors="replace") as f:
                    lines = f.readlines()
            except OSError:
                continue
            for n, line in enumerate(lines, 1):
                stripped = line.strip()
                if stripped.startswith(("//", "#!", "*", "/*")) or (stripped.startswith("#") and not stripped.startswith("#include") and not stripped.startswith("# include")):
                    continue
                rel = os.path.relpath(path, core_dir).replace(os.sep, "/")
                found = False
                for imp in imported_names(line):
                    for bad in forbidden:
                        if matches_forbidden(imp, bad):
                            violations.append((rel, n, imp, bad))
                            found = True
                # Rust also reaches other crates through fully qualified paths without `use`.
                if not found and name.endswith(".rs"):
                    code = line.split("//", 1)[0]
                    for bad in forbidden:
                        crate = bad.strip().replace("-", "_")
                        m = re.search(rf"(?<![\w:]){re.escape(crate)}::[\w:]*", code)
                        if m:
                            violations.append((rel, n, m.group(0), bad))
    return violations


def run(project_dir: str = None):
    """Returns (errors, warnings) as lists of strings."""
    errors, warnings = [], []
    cfg = load_project_config(project_dir)
    arch = cfg.section("architecture")

    style = arch.get("style")
    if not style:
        warnings.append("architecture.style is not set in .dev-cycle.json; infer it, agree it with the user and record it in an ADR")
    elif style not in ARCHITECTURE_STYLES:
        errors.append(f"architecture.style '{style}' is unknown (use one of: {', '.join(ARCHITECTURE_STYLES)})")

    core_rel = arch.get("core_path")
    forbidden = arch.get("forbidden_core_imports") or []
    if not core_rel:
        if forbidden:
            errors.append("architecture.forbidden_core_imports is set but architecture.core_path is not")
        else:
            warnings.append("architecture.core_path is not set; the core dependency check is skipped")
        return errors, warnings

    core_dir = os.path.join(cfg.root, core_rel)
    if not os.path.isdir(core_dir):
        errors.append(f"architecture.core_path '{core_rel}' does not exist")
        return errors, warnings

    if not forbidden:
        warnings.append("architecture.forbidden_core_imports is empty; list the UI, transport and adapter modules the core must not import")
        return errors, warnings

    for rel, line, imp, bad in scan_core(core_dir, forbidden):
        errors.append(f"{core_rel}/{rel}:{line}: core imports '{imp}' (forbidden: '{bad}')")
    return errors, warnings


def main() -> int:
    if sys.stdout.encoding != "utf-8":
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass
    parser = argparse.ArgumentParser(description="Check the project's architecture style and core dependencies")
    parser.add_argument("--dir", default=None, help="Project directory (default: current directory)")
    parser.add_argument("--strict", action="store_true", help="Fail on warnings too")
    args = parser.parse_args()

    errors, warnings = run(args.dir)
    for e in errors:
        print(f"❌ {e}")
    for w in warnings:
        print(f"⚠️ {w}")
    status = "✅" if not errors and not (args.strict and warnings) else "❌"
    print(f"\n{status} software-design: {len(errors)} error(s), {len(warnings)} warning(s).")
    return 1 if errors or (args.strict and warnings) else 0


if __name__ == "__main__":
    sys.exit(main())
