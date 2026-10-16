"""
Project Configuration - dev-cycle-protocol Skill
Single loader for `.dev-cycle.json`, the project configuration shared by every skill.

The file sits at the repository root. Each skill owns one section; this module fills
defaults, maps the legacy flat keys of older manifests into their sections, and
reports problems. Skills import it from the sibling folder:

    sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "dev-cycle-protocol"))
    from project_config import load_project_config
    cfg = load_project_config(start_dir)
    cfg.section("devops")["integration_branch"]

CLI:
    python project_config.py [--dir <path>] [--json] [--validate] [--migrate [--write]]
"""
import argparse
import copy
import json
import os
import sys

MANIFEST_NAME = ".dev-cycle.json"

ARCHITECTURE_STYLES = ["hexagonal", "layered", "mvc", "pipeline", "modular-monolith", "other"]
OBSERVABILITY_PROFILES = ["cli", "service", "library", "desktop", "none"]

# Sections and their defaults. `None` means "not set": skills fall back to auto-detection.
DEFAULTS = {
    "project_name": None,
    "architecture": {
        "style": None,
        "core_path": None,
        "surfaces": [],
        "canonical_datastore": None,
    },
    "test_commands": [],
    "docs": {
        "user_doc": "README.md",
        "tec_doc": "TECDOC.md",
    },
    "tests": {
        "manual_tests": "tests/MANUAL_TESTS.md",
    },
    "devops": {
        "integration_branch": "dev",
        "production_branch": "main",
        "releases_repo": None,
        "release_candidates": True,
        "version_files": [],
        "observability": None,
    },
    "plans": {
        "dir": "docs/plans",
    },
    # Owned by other skills; passed through untouched.
    "architecture_docs": {},
    "drift_rules": [],
}

# Legacy flat keys -> (section, key). The new location wins when both are present.
LEGACY_KEYS = {
    "core_path": ("architecture", "core_path"),
    "surfaces": ("architecture", "surfaces"),
    "canonical_datastore": ("architecture", "canonical_datastore"),
    "releases_repo": ("devops", "releases_repo"),
    "plans_dir": ("plans", "dir"),
}
LEGACY_DOC_FILES = {
    "user_doc": ("docs", "user_doc"),
    "tec_doc": ("docs", "tec_doc"),
    "manual_tests": ("tests", "manual_tests"),
}


def find_manifest(start_dir: str = None):
    """Nearest `.dev-cycle.json`, walking up from `start_dir` (default: current directory)."""
    current = os.path.abspath(start_dir or os.getcwd())
    while True:
        candidate = os.path.join(current, MANIFEST_NAME)
        if os.path.isfile(candidate):
            return candidate
        parent = os.path.dirname(current)
        if parent == current:
            return None
        current = parent


class ProjectConfig:
    """Effective configuration: defaults, legacy keys mapped, then the file's own sections."""

    def __init__(self, root: str, raw: dict, manifest_path: str = None):
        self.root = root
        self.raw = raw
        self.manifest_path = manifest_path
        self.legacy_keys_used = []
        self.data = self._resolve(raw)

    def _resolve(self, raw: dict) -> dict:
        data = copy.deepcopy(DEFAULTS)
        data["project_name"] = os.path.basename(self.root)

        for key, (section, sub) in LEGACY_KEYS.items():
            if key in raw:
                data[section][sub] = raw[key]
                self.legacy_keys_used.append(key)
        for key, (section, sub) in LEGACY_DOC_FILES.items():
            if key in raw.get("doc_files", {}):
                data[section][sub] = raw["doc_files"][key]
                self.legacy_keys_used.append(f"doc_files.{key}")

        for key, value in raw.items():
            if key in LEGACY_KEYS or key == "doc_files":
                continue
            if isinstance(data.get(key), dict) and isinstance(value, dict):
                data[key] = {**data[key], **value}
            else:
                data[key] = value
        return data

    def get(self, key, default=None):
        return self.data.get(key, default)

    def section(self, name: str) -> dict:
        value = self.data.get(name)
        return value if isinstance(value, dict) else {}

    @property
    def project_name(self) -> str:
        return self.data.get("project_name") or os.path.basename(self.root)

    def path(self, section: str, key: str):
        """Absolute path of a path-valued setting, or None when unset."""
        value = self.section(section).get(key)
        return os.path.join(self.root, value) if value else None

    def validate(self) -> list:
        """Human-readable problems. An empty list means the configuration is valid."""
        problems = []
        style = self.section("architecture").get("style")
        if style is not None and style not in ARCHITECTURE_STYLES:
            problems.append(f"architecture.style '{style}' is not one of: {', '.join(ARCHITECTURE_STYLES)}")
        obs = self.section("devops").get("observability")
        if obs is not None and obs not in OBSERVABILITY_PROFILES:
            problems.append(f"devops.observability '{obs}' is not one of: {', '.join(OBSERVABILITY_PROFILES)}")
        if not isinstance(self.data.get("test_commands"), list):
            problems.append("test_commands must be a list of shell commands")
        if not isinstance(self.section("devops").get("release_candidates"), bool):
            problems.append("devops.release_candidates must be true or false")
        for key in ("surfaces",):
            if not isinstance(self.section("architecture").get(key), list):
                problems.append(f"architecture.{key} must be a list")
        if not isinstance(self.section("devops").get("version_files"), list):
            problems.append("devops.version_files must be a list of paths")
        known = set(DEFAULTS) | set(LEGACY_KEYS) | {"doc_files"}
        for key in self.raw:
            if key not in known:
                problems.append(f"unknown top-level key '{key}' (kept as is)")
        return problems

    def migrated(self) -> dict:
        """The raw file rewritten with sections instead of legacy flat keys."""
        out = {}
        for key, value in self.raw.items():
            if key in LEGACY_KEYS or key == "doc_files":
                continue
            out[key] = copy.deepcopy(value)
        for key, (section, sub) in LEGACY_KEYS.items():
            if key in self.raw:
                out.setdefault(section, {})
                out[section].setdefault(sub, self.raw[key])
        for key, (section, sub) in LEGACY_DOC_FILES.items():
            if key in self.raw.get("doc_files", {}):
                out.setdefault(section, {})
                out[section].setdefault(sub, self.raw["doc_files"][key])
        order = list(DEFAULTS)
        return {k: out[k] for k in sorted(out, key=lambda k: order.index(k) if k in order else len(order))}


def load_project_config(start_dir: str = None) -> ProjectConfig:
    """Loads the nearest `.dev-cycle.json`; returns defaults when there is none."""
    manifest = find_manifest(start_dir)
    if not manifest:
        return ProjectConfig(os.path.abspath(start_dir or os.getcwd()), {}, None)
    with open(manifest, "r", encoding="utf-8") as f:
        raw = json.load(f)
    if not isinstance(raw, dict):
        raise ValueError(f"{manifest} must contain a JSON object")
    return ProjectConfig(os.path.dirname(manifest), raw, manifest)


def main() -> int:
    if sys.stdout.encoding != "utf-8":
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass
    parser = argparse.ArgumentParser(description="Show, validate or migrate the project's .dev-cycle.json")
    parser.add_argument("--dir", default=None, help="Project directory (default: current directory)")
    parser.add_argument("--json", action="store_true", help="Print the effective configuration as JSON")
    parser.add_argument("--validate", action="store_true", help="Exit with code 1 when the configuration has problems")
    parser.add_argument("--migrate", action="store_true", help="Print the file with legacy flat keys moved into sections")
    parser.add_argument("--write", action="store_true", help="With --migrate: write the migrated file in place")
    args = parser.parse_args()

    try:
        cfg = load_project_config(args.dir)
    except (ValueError, json.JSONDecodeError) as e:
        print(f"❌ {e}")
        return 1

    if args.migrate:
        if not cfg.manifest_path:
            print(f"❌ No {MANIFEST_NAME} found.")
            return 1
        text = json.dumps(cfg.migrated(), indent=2, ensure_ascii=False) + "\n"
        if args.write:
            with open(cfg.manifest_path, "w", encoding="utf-8", newline="\n") as f:
                f.write(text)
            print(f"✅ Migrated {cfg.manifest_path}")
        else:
            print(text, end="")
        return 0

    if args.json:
        print(json.dumps(cfg.data, indent=2, ensure_ascii=False))
    else:
        source = cfg.manifest_path or f"defaults (no {MANIFEST_NAME})"
        print(f"⚙️  Project configuration: {cfg.project_name}")
        print(f"   Source: {source}")
        arch, devops = cfg.section("architecture"), cfg.section("devops")
        print(f"   Architecture style : {arch.get('style') or 'not set'}")
        print(f"   Core path          : {arch.get('core_path') or 'not set'}")
        print(f"   Test commands      : {', '.join(cfg.get('test_commands')) or 'auto-detect'}")
        print(f"   Branches           : {devops.get('integration_branch')} -> {devops.get('production_branch')}"
              f" (release candidates: {'on' if devops.get('release_candidates') else 'off'})")
        print(f"   Releases repo      : {devops.get('releases_repo') or 'not set'}")
        if cfg.legacy_keys_used:
            print(f"   ℹ️  Legacy keys in use: {', '.join(cfg.legacy_keys_used)} (run with --migrate --write)")

    problems = cfg.validate()
    for p in problems:
        print(f"⚠️  {p}")
    if args.validate and any(not p.startswith("unknown top-level key") for p in problems):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
