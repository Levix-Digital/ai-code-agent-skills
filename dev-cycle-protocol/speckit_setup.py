"""
Spec Kit Setup - dev-cycle-protocol Skill
Installs, pins, checks and upgrades GitHub Spec Kit in a project, the optional executor of
Phases 1, 2 and 4 of the full path. The CLI is always called at the exact version pinned in
`.dev-cycle.json` (`speckit.version`) through uvx, never through a global install.

Commands:
    python speckit_setup.py status  [--dir <project>]
    python speckit_setup.py latest
    python speckit_setup.py check   [--version vX.Y.Z] [--script sh|ps|py] [--record] [--keep] [--json]
    python speckit_setup.py init    [--version vX.Y.Z] [--integration claude] [--script sh|ps|py] [--dir <project>] [--dry-run]
    python speckit_setup.py upgrade --to vX.Y.Z [--dir <project>] [--dry-run]
    python speckit_setup.py adopt   [--dir <project>] [--dry-run]     (Spec Kit already used without the protocol)

`check` runs the compatibility check: Spec Kit is initialized in a temporary repository and
every behavior the protocol depends on is verified. `init` without --version checks the latest
release: when it passes, the project pins it (and the verified version moves forward); when it
fails, the project pins the last verified version and an adaptation document is drafted.
"""
import argparse
import datetime
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from project_config import MANIFEST_NAME, find_manifest, load_project_config  # noqa: E402

SKILLS_ROOT = os.path.dirname(HERE)
SPECKIT_REPO = "https://github.com/github/spec-kit.git"
PRESET_DIR = os.path.join(HERE, "speckit-preset")
PRESET_ID = "dev-cycle"
PRESET_MARKER = "## Dev Cycle Protocol"
COMPAT_FILE = os.path.join(PRESET_DIR, "compat.json")
ADAPTATION_DIR = os.path.join(SKILLS_ROOT, "docs", "protocol-updates")
SCRIPT_TYPES = ["sh", "ps", "py"]
VERSION_RE = re.compile(r"^v(\d+)\.(\d+)\.(\d+)$")
# Spec Kit commands the protocol calls in Phases 1, 2 and 4 (and Phase 0 for the constitution).
REQUIRED_COMMANDS = ["specify", "clarify", "plan", "tasks", "analyze", "implement", "constitution"]
REQUIRED_TEMPLATES = ["spec-template.md", "plan-template.md", "tasks-template.md", "constitution-template.md"]


class SetupError(RuntimeError):
    """A setup step that cannot continue; the message says what to do."""


# ---------------------------------------------------------------- versions

def parse_version(tag: str) -> tuple:
    m = VERSION_RE.match(tag or "")
    if not m:
        raise SetupError(f"'{tag}' is not a release tag like v1.2.3 (branches, hashes and 'latest' are not accepted)")
    return tuple(int(g) for g in m.groups())


def newest(tags: list):
    stable = [t for t in tags if VERSION_RE.match(t)]
    return max(stable, key=parse_version) if stable else None


def latest_release(runner=None) -> str:
    runner = runner or run
    code, out = runner(["git", "ls-remote", "--tags", "--refs", SPECKIT_REPO])
    if code != 0:
        raise SetupError(f"could not list Spec Kit releases: {out}")
    tags = [line.rsplit("refs/tags/", 1)[-1].strip() for line in out.splitlines() if "refs/tags/" in line]
    tag = newest(tags)
    if not tag:
        raise SetupError("no Spec Kit release tag found")
    return tag


def read_compat() -> dict:
    try:
        with open(COMPAT_FILE, "r", encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def verified_version() -> str:
    return read_compat().get("verified")


def record_verified(version: str, integration: str, script: str) -> bool:
    """Moves the verified version forward (never back). Returns True when it changed."""
    current = verified_version()
    if current and parse_version(current) >= parse_version(version):
        return False
    data = {"verified": version, "verified_at": datetime.date.today().isoformat(),
            "checked_with": {"integration": integration, "script": script}}
    with open(COMPAT_FILE, "w", encoding="utf-8", newline="\n") as f:
        f.write(json.dumps(data, indent=2) + "\n")
    return True


# ---------------------------------------------------------------- commands

def default_script() -> str:
    return "ps" if os.name == "nt" else "sh"


def specify_cmd(version: str) -> list:
    parse_version(version)
    return ["uvx", "--from", f"git+{SPECKIT_REPO}@{version}", "specify"]


def init_cmd(version: str, integration: str, script: str) -> list:
    return specify_cmd(version) + ["init", "--here", "--force", "--integration", integration, "--script", script,
                                   "--extension", "git", "--non-interactive", "--ignore-agent-tools"]


def preset_add_cmd(version: str) -> list:
    return specify_cmd(version) + ["preset", "add", "--dev", PRESET_DIR]


def preset_update_cmd(version: str) -> list:
    return specify_cmd(version) + ["preset", "update", PRESET_ID, "--dev", PRESET_DIR]


def upgrade_cmds(version: str, integration: str, script: str) -> list:
    return [specify_cmd(version) + ["integration", "upgrade", integration, "--script", script],
            specify_cmd(version) + ["extension", "update"],
            preset_update_cmd(version)]


def branch_script_cmd(script: str, short_name: str, description: str) -> list:
    base = os.path.join(".specify", "extensions", "git", "scripts")
    if script == "sh":
        return ["bash", os.path.join(base, "bash", "create-new-feature-branch.sh"), "--json",
                "--short-name", short_name, description]
    if script == "ps":
        return ["pwsh", "-NoProfile", "-File", os.path.join(base, "powershell", "create-new-feature-branch.ps1"),
                "-Json", "-ShortName", short_name, description]
    return [sys.executable, os.path.join(base, "python", "create_new_feature_branch.py"), "--json",
            "--short-name", short_name, description]


def setup_plan_cmd(script: str) -> list:
    base = os.path.join(".specify", "scripts")
    if script == "sh":
        return ["bash", os.path.join(base, "bash", "setup-plan.sh"), "--json"]
    if script == "ps":
        return ["pwsh", "-NoProfile", "-File", os.path.join(base, "powershell", "setup-plan.ps1"), "-Json"]
    return [sys.executable, os.path.join(base, "python", "setup_plan.py"), "--json"]


def run(cmd: list, cwd: str = None) -> tuple:
    try:
        res = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, encoding="utf-8", errors="replace")
        return res.returncode, (res.stdout + res.stderr).strip()
    except FileNotFoundError as e:
        return 127, f"{cmd[0]}: not found ({e})"


def require_uv():
    if not shutil.which("uvx"):
        raise SetupError("uv is required for Spec Kit (https://docs.astral.sh/uv/): "
                         "'pip install uv' or 'winget install astral-sh.uv' / 'curl -LsSf https://astral.sh/uv/install.sh | sh'")


# ---------------------------------------------------------------- project files

def read_json(path: str) -> dict:
    try:
        with open(path, "r", encoding="utf-8") as f:
            data = json.load(f)
        return data if isinstance(data, dict) else {}
    except (OSError, ValueError):
        return {}


def installed_version(root: str):
    """The Spec Kit version that initialized or last upgraded the project, as a tag (vX.Y.Z)."""
    for name, key in (("integration.json", "version"), ("init-options.json", "speckit_version")):
        value = read_json(os.path.join(root, ".specify", name)).get(key)
        if value:
            return value if str(value).startswith("v") else f"v{value}"
    return None


def git_config_path(root: str) -> str:
    return os.path.join(root, ".specify", "extensions", "git", "git-config.yml")


def read_branch_prefix(root: str):
    try:
        with open(git_config_path(root), "r", encoding="utf-8") as f:
            for line in f:
                m = re.match(r'^branch_prefix:\s*"?([^"#\n]*)"?', line)
                if m:
                    return m.group(1).strip()
    except OSError:
        return None
    return None


def set_branch_prefix(root: str, prefix: str):
    path = git_config_path(root)
    with open(path, "r", encoding="utf-8") as f:
        text = f.read()
    if re.search(r"^branch_prefix:", text, re.M):
        text = re.sub(r"^branch_prefix:.*$", f'branch_prefix: "{prefix}"', text, count=1, flags=re.M)
    else:
        text = text.rstrip("\n") + f'\n\nbranch_prefix: "{prefix}"\n'
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)


def write_speckit_config(root: str, values: dict) -> str:
    """Updates the `speckit` section of the project's .dev-cycle.json (created when missing)."""
    path = find_manifest(root) or os.path.join(root, MANIFEST_NAME)
    raw = read_json(path) if os.path.isfile(path) else {}
    raw["speckit"] = {**(raw.get("speckit") or {}), **values}
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(json.dumps(raw, indent=2, ensure_ascii=False) + "\n")
    return path


def preset_installed(root: str) -> bool:
    return os.path.isfile(os.path.join(root, ".specify", "presets", PRESET_ID, "preset.yml"))


# ---------------------------------------------------------------- compatibility check

def find_command_files(root: str) -> dict:
    """Spec Kit command name -> agent file installed for it (skills or commands, any integration)."""
    found = {}
    pattern = re.compile(r"speckit[.-](" + "|".join(REQUIRED_COMMANDS) + r")(?:[./]|SKILL|$)")
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in (".git", ".specify")]
        for name in filenames + dirnames:
            rel = os.path.relpath(os.path.join(dirpath, name), root).replace("\\", "/")
            m = pattern.search(rel)
            if m and m.group(1) not in found:
                found[m.group(1)] = rel
    return found


def compatibility_check(version: str, integration: str = "claude", script: str = None, keep: bool = False,
                        runner=None, workdir: str = None) -> dict:
    """Initializes Spec Kit `version` in a temporary repository and verifies every behavior the
    protocol depends on. Returns {"version", "passed", "checks": [{"name", "ok", "detail"}], "dir"}."""
    runner = runner or run
    script = script or default_script()
    parse_version(version)
    root = workdir or tempfile.mkdtemp(prefix="speckit-check-")
    checks = []

    def check(name: str, ok: bool, detail: str = "") -> bool:
        checks.append({"name": name, "ok": bool(ok), "detail": detail})
        return bool(ok)

    def finish():
        result = {"version": version, "integration": integration, "script": script,
                  "passed": all(c["ok"] for c in checks), "checks": checks, "dir": root if keep else None}
        if not keep and not workdir:
            shutil.rmtree(root, ignore_errors=True)
        return result

    git = ["git", "-c", "user.name=dev-cycle", "-c", "user.email=dev-cycle@example.invalid"]
    code, out = runner(["git", "init", "-q", "-b", "dev"], root)
    if code == 0:
        code, out = runner(git + ["commit", "-q", "--allow-empty", "-m", "init"], root)
    if not check("Temporary Git repository", code == 0, out if code else ""):
        return finish()

    code, out = runner(init_cmd(version, integration, script), root)
    if not check(f"specify init ({integration}, {script})", code == 0, out[-600:] if code else ""):
        return finish()

    got = installed_version(root)
    check("Version recorded in .specify", got == version, f"found {got}")

    missing = [t for t in REQUIRED_TEMPLATES if not os.path.isfile(os.path.join(root, ".specify", "templates", t))]
    check("Core templates", not missing, f"missing: {', '.join(missing)}" if missing else "")

    commands = find_command_files(root)
    missing = [c for c in REQUIRED_COMMANDS if c not in commands]
    check("Agent commands", not missing, f"missing: {', '.join(missing)}" if missing else "")

    if check("Git extension branch_prefix setting", read_branch_prefix(root) is not None,
             f"{git_config_path(root)} has no branch_prefix"):
        set_branch_prefix(root, "feat")
        runner(git + ["add", "-A"], root)
        runner(git + ["commit", "-q", "-m", "speckit"], root)
        code, out = runner(branch_script_cmd(script, "compat-check", "Compatibility check"), root)
        branch = None
        m = re.search(r'"BRANCH_NAME"\s*:\s*"([^"]+)"', out)
        if m:
            branch = m.group(1)
        check("Branch created as feat/NNN-slug", code == 0 and branch and re.match(r"^feat/\d{3}-compat-check$", branch),
              f"got {branch!r}" + (f": {out[-300:]}" if code else ""))

    code, out = runner(preset_add_cmd(version), root)
    if check("Dev Cycle preset installed", code == 0 and preset_installed(root), out[-400:] if code else ""):
        feature_dir = os.path.join("specs", "001-compat-check")
        os.makedirs(os.path.join(root, feature_dir), exist_ok=True)
        with open(os.path.join(root, feature_dir, "spec.md"), "w", encoding="utf-8") as f:
            f.write("# Compatibility check\n")
        with open(os.path.join(root, ".specify", "feature.json"), "w", encoding="utf-8") as f:
            json.dump({"feature_directory": feature_dir.replace("\\", "/")}, f)
        code, out = runner(setup_plan_cmd(script), root)
        plan_path = os.path.join(root, feature_dir, "plan.md")
        plan = ""
        if os.path.isfile(plan_path):
            with open(plan_path, "r", encoding="utf-8") as f:
                plan = f.read()
        check("Feature resolved from .specify/feature.json", code == 0 and bool(plan), out[-300:] if code else "")
        check("Plan template has the Constitution Check", "Constitution Check" in plan)
        check("Plan template has the protocol sections (preset)", PRESET_MARKER in plan)
    return finish()


def print_check(result: dict):
    print(f"🔎 Spec Kit {result['version']} compatibility check ({result['integration']}, {result['script']})")
    for c in result["checks"]:
        print(f"   {'✅' if c['ok'] else '❌'} {c['name']}" + (f" — {c['detail']}" if c["detail"] and not c["ok"] else ""))
    print(f"{'🟢 COMPATIBLE' if result['passed'] else '🔴 NOT COMPATIBLE'}")
    if result.get("dir"):
        print(f"   Kept at {result['dir']}")


def write_adaptation_draft(result: dict, fallback: str) -> str:
    """Drafts docs/protocol-updates/speckit-<version>.md in the skills repository. Returns the path,
    or None when the skills repository cannot be written (the text is printed instead)."""
    failed = [c for c in result["checks"] if not c["ok"]]
    lines = [f"# Protocol update for Spec Kit {result['version']}", "",
             f"Found on {datetime.date.today().isoformat()} by the compatibility check "
             f"(`dev-cycle-protocol/speckit_setup.py check --version {result['version']}`, "
             f"{result['integration']}, {result['script']}). Projects keep using {fallback} until the protocol is adapted.", "",
             "## What diverged", ""]
    lines += [f"- **{c['name']}**: {c['detail'] or 'failed'}" for c in failed]
    lines += ["", "## What the protocol must change", "",
              "[Read the Spec Kit release notes and the failed checks; list the changes to the preset, "
              "speckit_setup.py, gates and SKILL files]", "",
              "## Done when", "", f"`speckit_setup.py check --version {result['version']} --record` passes.", ""]
    text = "\n".join(lines)
    path = os.path.join(ADAPTATION_DIR, f"speckit-{result['version']}.md")
    try:
        os.makedirs(ADAPTATION_DIR, exist_ok=True)
        if not os.path.exists(path):
            with open(path, "w", encoding="utf-8", newline="\n") as f:
                f.write(text)
        return path
    except OSError:
        print("⚠️ The skills repository is not writable; give this adaptation document to the user:\n")
        print(text)
        return None


# ---------------------------------------------------------------- project commands

def project_root(start_dir: str = None) -> str:
    start = os.path.abspath(start_dir or os.getcwd())
    code, out = run(["git", "rev-parse", "--show-toplevel"], start)
    return os.path.abspath(out) if code == 0 and out else start


def choose_version(requested: str, integration: str, script: str, runner=None) -> str:
    """The version a new project pins: the requested one, else the latest when it passes the check,
    else the last verified one (with an adaptation document drafted)."""
    if requested:
        parse_version(requested)
        return requested
    latest = latest_release(runner)
    verified = verified_version()
    if verified and parse_version(latest) <= parse_version(verified):
        print(f"ℹ️ Latest Spec Kit {latest} is already verified ({verified}).")
        return verified
    print(f"🔎 Spec Kit {latest} is newer than the verified {verified or 'none'}; running the compatibility check...")
    result = compatibility_check(latest, integration, script, runner=runner)
    print_check(result)
    if result["passed"]:
        if record_verified(latest, integration, script):
            print(f"✅ Verified version moved to {latest} ({os.path.relpath(COMPAT_FILE, SKILLS_ROOT)}); commit it in the skills repository.")
        return latest
    if not verified:
        raise SetupError("the latest Spec Kit release is not compatible and no verified version exists")
    path = write_adaptation_draft(result, verified)
    if path:
        print(f"📝 Adaptation document drafted: {path}")
        print(f"   Capture it in the skills repository's backlog: python \"{os.path.join(SKILLS_ROOT, 'plan-memory', 'backlog.py')}\" "
              f"add --type improvement --title \"Adapt the protocol to Spec Kit {latest}\" "
              f"--description \"See docs/protocol-updates/speckit-{latest}.md\" --dir \"{SKILLS_ROOT}\"")
    print(f"↩️ Using the last verified version {verified}.")
    return verified


def execute(cmds: list, root: str, dry_run: bool):
    for cmd in cmds:
        print(f"$ {' '.join(cmd)}")
        if dry_run:
            continue
        code, out = run(cmd, root)
        if code != 0:
            raise SetupError(f"command failed ({code}):\n{out[-1500:]}")


def cmd_init(args) -> int:
    root = project_root(args.dir)
    cfg = load_project_config(root).section("speckit")
    if os.path.isdir(os.path.join(root, ".specify")):
        raise SetupError(f"Spec Kit is already initialized in {root}; use 'upgrade --to <version>' to change versions")
    integration = args.integration or cfg.get("integration") or "claude"
    script = args.script or cfg.get("script") or default_script()
    prefix = cfg.get("branch_prefix") or "feat"
    if not args.dry_run:
        require_uv()
    version = choose_version(args.version or cfg.get("version"), integration, script)
    execute([init_cmd(version, integration, script)], root, args.dry_run)
    if not args.dry_run:
        set_branch_prefix(root, prefix)
    execute([preset_add_cmd(version)], root, args.dry_run)
    if args.dry_run:
        print(f"(dry run) would pin speckit.version = {version} in {MANIFEST_NAME}")
        return 0
    path = write_speckit_config(root, {"enabled": True, "version": version, "integration": integration,
                                       "script": script, "branch_prefix": prefix})
    print(f"✅ Spec Kit {version} initialized ({integration}, {script}), branches '{prefix}/NNN-slug', preset '{PRESET_ID}'.")
    print(f"   Pinned in {path}. Next: generate the constitution (dev-cycle-protocol/constitution.py generate),")
    print("   then commit .specify/, the agent command files and .dev-cycle.json.")
    return 0


def installed_settings(root: str) -> tuple:
    """(integration, script) recorded by Spec Kit in the project, or (None, None)."""
    integration_json = read_json(os.path.join(root, ".specify", "integration.json"))
    options = read_json(os.path.join(root, ".specify", "init-options.json"))
    integration = integration_json.get("integration") or options.get("integration")
    settings = (integration_json.get("integration_settings") or {}).get(integration or "", {})
    return integration, settings.get("script") or options.get("script")


def extension_add_cmd(version: str, extension: str) -> list:
    return specify_cmd(version) + ["extension", "add", extension]


def cmd_adopt(args) -> int:
    """Brings a project that used Spec Kit without the protocol under it: pins the installed version
    (when it passes the compatibility check), the git extension with feat/NNN-slug branches, the preset."""
    root = project_root(args.dir)
    cfg = load_project_config(root)
    if not os.path.isdir(os.path.join(root, ".specify")):
        raise SetupError(f"Spec Kit is not initialized in {root}; use 'init'")
    if cfg.section("speckit").get("enabled") and cfg.section("speckit").get("version"):
        raise SetupError("Spec Kit is already managed by the protocol here (speckit.enabled); use 'status' or 'upgrade'")
    version = installed_version(root)
    verified = verified_version()
    if not version:
        raise SetupError("the installed Spec Kit version is unknown (no .specify/integration.json or init-options.json); "
                         f"run 'upgrade --to {verified}' after enabling, or re-initialize")
    integration, script = installed_settings(root)
    integration = integration or cfg.section("speckit").get("integration") or "claude"
    script = script or cfg.section("speckit").get("script") or default_script()
    prefix = cfg.section("speckit").get("branch_prefix") or "feat"
    if not args.dry_run:
        require_uv()
        if verified != version:
            result = compatibility_check(version, integration, script)
            print_check(result)
            if not result["passed"]:
                raise SetupError(f"the installed Spec Kit {version} is not compatible with the protocol; upgrade it to the "
                                 f"verified {verified} first (speckit_setup.py upgrade --to {verified}), then adopt again")
    cmds = []
    if not os.path.isfile(git_config_path(root)):
        cmds.append(extension_add_cmd(version, "git"))
    cmds.append(preset_update_cmd(version) if preset_installed(root) else preset_add_cmd(version))
    execute(cmds, root, args.dry_run)
    if args.dry_run:
        print(f"(dry run) would pin speckit.version = {version} and set branches {prefix}/NNN-slug")
        return 0
    set_branch_prefix(root, prefix)
    write_speckit_config(root, {"enabled": True, "version": version, "integration": integration, "script": script,
                                "branch_prefix": prefix})
    print(f"✅ Spec Kit {version} adopted ({integration}, {script}), branches '{prefix}/NNN-slug', preset '{PRESET_ID}'. "
          "Existing specs/ are kept.")
    print("   Next: the constitution. If it was written by hand, move its principles into the sources (TECH_SPEC.md, ADRs) "
          "with the user, then run constitution.py generate --replace-hand-written (a copy is kept).")
    return 0


def cmd_upgrade(args) -> int:
    root = project_root(args.dir)
    cfg = load_project_config(root).section("speckit")
    if not os.path.isdir(os.path.join(root, ".specify")):
        raise SetupError(f"Spec Kit is not initialized in {root}; use 'init'")
    current = cfg.get("version") or installed_version(root)
    target = args.to
    if current and parse_version(target) <= parse_version(current):
        raise SetupError(f"{target} is not newer than the pinned {current}")
    integration = cfg.get("integration") or "claude"
    script = cfg.get("script") or default_script()
    if not args.dry_run:
        require_uv()
        verified = verified_version()
        if not verified or parse_version(target) > parse_version(verified):
            result = compatibility_check(target, integration, script)
            print_check(result)
            if not result["passed"]:
                path = write_adaptation_draft(result, current or verified)
                raise SetupError(f"Spec Kit {target} is not compatible with the protocol; the project stays on {current}."
                                 + (f" Adaptation document: {path}" if path else ""))
            record_verified(target, integration, script)
    execute(upgrade_cmds(target, integration, script), root, args.dry_run)
    if args.dry_run:
        return 0
    prefix = cfg.get("branch_prefix") or "feat"
    if read_branch_prefix(root) != prefix:
        set_branch_prefix(root, prefix)
    write_speckit_config(root, {"version": target})
    print(f"✅ Spec Kit upgraded {current} -> {target}. Review the diff (locally edited files are kept), "
          "regenerate the constitution if templates changed, and commit.")
    return 0


def status_data(root: str) -> dict:
    cfg = load_project_config(root).section("speckit")
    initialized = os.path.isdir(os.path.join(root, ".specify"))
    pinned, installed = cfg.get("version"), installed_version(root) if initialized else None
    problems = []
    if cfg.get("enabled"):
        if not initialized:
            problems.append("speckit.enabled is true but Spec Kit is not initialized (run speckit_setup.py init)")
        if not pinned:
            problems.append("speckit.version is not set")
        elif installed and installed != pinned:
            problems.append(f"pinned {pinned} but the project was set up with {installed}")
        if initialized and not preset_installed(root):
            problems.append(f"preset '{PRESET_ID}' is not installed")
        prefix = cfg.get("branch_prefix") or "feat"
        if initialized and read_branch_prefix(root) != prefix:
            problems.append(f"git extension branch_prefix is {read_branch_prefix(root)!r}, expected '{prefix}'")
    return {"enabled": bool(cfg.get("enabled")), "pinned": pinned, "installed": installed,
            "integration": cfg.get("integration"), "script": cfg.get("script"),
            "initialized": initialized, "preset": initialized and preset_installed(root),
            "branch_prefix": read_branch_prefix(root) if initialized else None,
            "verified": verified_version(), "uv": bool(shutil.which("uvx")), "problems": problems}


def cmd_status(args) -> int:
    data = status_data(project_root(args.dir))
    if args.json:
        print(json.dumps(data, indent=2))
        return 1 if data["problems"] else 0
    print(f"🧩 Spec Kit: {'enabled' if data['enabled'] else 'disabled'}")
    print(f"   Pinned / installed : {data['pinned'] or '-'} / {data['installed'] or '-'} (verified by the protocol: {data['verified'] or '-'})")
    print(f"   Integration, script: {data['integration'] or '-'}, {data['script'] or '-'}")
    print(f"   Preset, branches   : {'installed' if data['preset'] else 'missing'}, {data['branch_prefix'] or '-'}/NNN-slug")
    print(f"   uv                 : {'available' if data['uv'] else 'missing'}")
    for p in data["problems"]:
        print(f"⚠️  {p}")
    return 1 if data["problems"] else 0


def main(argv: list = None) -> int:
    if sys.stdout.encoding != "utf-8":
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass
    parser = argparse.ArgumentParser(description="Install, pin, check and upgrade Spec Kit for dev-cycle-protocol")
    sub = parser.add_subparsers(dest="command", required=True)
    p = sub.add_parser("status", help="Spec Kit state of the project")
    p.add_argument("--dir", default=None)
    p.add_argument("--json", action="store_true")
    sub.add_parser("latest", help="Latest Spec Kit release and the verified version")
    p = sub.add_parser("check", help="Compatibility check of a Spec Kit version in a temporary repository")
    p.add_argument("--version", default=None, help="Release tag (default: the latest release)")
    p.add_argument("--integration", default="claude")
    p.add_argument("--script", choices=SCRIPT_TYPES, default=None)
    p.add_argument("--record", action="store_true", help="When it passes, move the verified version forward")
    p.add_argument("--keep", action="store_true", help="Keep the temporary repository")
    p.add_argument("--json", action="store_true")
    p = sub.add_parser("init", help="Initialize Spec Kit in the project at a pinned version")
    p.add_argument("--dir", default=None)
    p.add_argument("--version", default=None, help="Release tag (default: latest if compatible, else the verified one)")
    p.add_argument("--integration", default=None, help="Agent integration (default: speckit.integration or claude)")
    p.add_argument("--script", choices=SCRIPT_TYPES, default=None)
    p.add_argument("--dry-run", action="store_true")
    p = sub.add_parser("adopt", help="Bring a project that used Spec Kit without the protocol under it")
    p.add_argument("--dir", default=None)
    p.add_argument("--dry-run", action="store_true")
    p = sub.add_parser("upgrade", help="Upgrade the project's Spec Kit after a compatibility check")
    p.add_argument("--dir", default=None)
    p.add_argument("--to", required=True, help="Release tag")
    p.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)

    try:
        if args.command == "status":
            return cmd_status(args)
        if args.command == "latest":
            print(f"Latest Spec Kit release: {latest_release()} (verified by the protocol: {verified_version() or '-'})")
            return 0
        if args.command == "check":
            require_uv()
            version = args.version or latest_release()
            script = args.script or default_script()
            result = compatibility_check(version, args.integration, script, keep=args.keep)
            if args.json:
                print(json.dumps(result, indent=2))
            else:
                print_check(result)
            if result["passed"] and args.record and record_verified(version, args.integration, script):
                print(f"✅ Verified version moved to {version}; commit {os.path.relpath(COMPAT_FILE, SKILLS_ROOT)}.")
            return 0 if result["passed"] else 1
        if args.command == "init":
            return cmd_init(args)
        if args.command == "adopt":
            return cmd_adopt(args)
        return cmd_upgrade(args)
    except SetupError as e:
        print(f"❌ {e}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
