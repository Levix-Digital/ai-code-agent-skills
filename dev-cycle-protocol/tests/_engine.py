"""Runs the engine's commands from the tests, the way the skills run them: through the
launcher (or DEV_CYCLE_BIN), with the skills read from this checkout."""
import os
import subprocess

SKILLS_ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
ENGINE_BIN = os.path.join(SKILLS_ROOT, "dev-cycle-protocol", "bin")


def engine_command(skill: str, command: str, args: list) -> list:
    """Command line of an engine command, through its launcher."""
    command = [skill, command, *args]
    if os.environ.get("DEV_CYCLE_BIN"):
        return [os.environ["DEV_CYCLE_BIN"], *command]
    if os.name != "nt":
        return ["sh", os.path.join(ENGINE_BIN, "dev-cycle"), *command]
    # PowerShell would re-encode the engine's UTF-8 output: let the launcher fetch or build the
    # binary once, then run the binary itself.
    launcher = ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File",
                os.path.join(ENGINE_BIN, "dev-cycle.ps1")]
    subprocess.run([*launcher, "--version"], capture_output=True)
    with open(os.path.join(ENGINE_BIN, "engine-version"), encoding="utf-8") as f:
        version = f.read().strip()
    cache_root = (os.environ.get("LOCALAPPDATA") or os.environ.get("XDG_CACHE_HOME")
                  or os.path.join(os.path.expanduser("~"), ".cache"))
    for binary in (os.path.join(cache_root, "dev-cycle", f"engine-v{version}", "dev-cycle.exe"),
                   os.path.join(SKILLS_ROOT, "target", "release", "dev-cycle.exe")):
        if os.path.isfile(binary):
            return [binary, *command]
    return [*launcher, *command]


def run_engine(skill: str, command: str, args: list) -> subprocess.CompletedProcess:
    """Runs an engine command; stdout and stderr apart."""
    env = {**os.environ, "DEV_CYCLE_SKILLS_ROOT": SKILLS_ROOT}
    return subprocess.run(engine_command(skill, command, args), capture_output=True, text=True,
                          encoding="utf-8", errors="replace", env=env)


def run_checker(skill: str, script: str, args: list) -> tuple:
    """Runs an engine command named after its script: (exit code, stdout and stderr)."""
    res = run_engine(skill, script[:-len(".py")] if script.endswith(".py") else script, args)
    return res.returncode, (res.stdout + res.stderr).strip()
