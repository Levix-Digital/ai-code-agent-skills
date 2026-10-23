"""
Verify Gate Utility - dev-cycle-protocol Skill
Automated gate checks of the role-based development cycle:
  gate 0  - before asking for the product baseline approval (Phase 0, product inception)
  gate 3  - before asking for plan approval (bugs first: open blocking bugs hold back other plans)
  gate rc - release candidate acceptance (all manual tests of the run Pass)
  gate 8  - release readiness, before the release pull request (runs every role's checker)
Project-agnostic: adapts to the project's .dev-cycle.json and detected stack.
"""
import argparse
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from project_config import load_project_config  # noqa: E402

SKILLS_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def run_cmd(cmd: list, cwd: str) -> tuple:
    try:
        res = subprocess.run(cmd, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                             text=True, encoding="utf-8", errors="replace")
        return res.returncode, res.stdout.strip()
    except Exception as e:
        return -1, str(e)


def check_secret_hygiene(cwd: str) -> list:
    violations = []
    dangerous_patterns = [".env", ".env.local", "id_rsa", "credentials.json", "*secret*", "*private_key*"]

    # 1. Dangerous files tracked in Git
    for pat in dangerous_patterns:
        code, out = run_cmd(["git", "ls-files", pat], cwd)
        if code == 0 and out.strip():
            for f in out.splitlines():
                if f.strip():
                    violations.append(f"Tracked credential file committed to Git: {f.strip()}")

    # 2. Dangerous files staged for commit
    code, status_out = run_cmd(["git", "status", "--porcelain"], cwd)
    if code == 0 and status_out.strip():
        for line in status_out.splitlines():
            status_tag = line[:2]
            filename = line[3:].strip()
            if status_tag[0] in ["A", "M", "R"]:
                for pat in [".env", "id_rsa", "credentials.json"]:
                    if pat in filename.lower():
                        if not any(safe in filename.lower() for safe in [".example", ".sample", "template"]):
                            violations.append(f"Staged credential file ready to be committed: {filename}")
    return violations


def run_checker(skill: str, script: str, args: list) -> tuple:
    """Runs a sibling skill's checker; returns (exit_code, output), or (None, reason) when missing."""
    path = os.path.join(SKILLS_ROOT, skill, script)
    if not os.path.isfile(path):
        return None, f"{skill}/{script} not installed"
    res = subprocess.run([sys.executable, path, *args], capture_output=True, text=True,
                         encoding="utf-8", errors="replace")
    return res.returncode, (res.stdout + res.stderr).strip()


def report_checker(label: str, code, output: str, blocking: bool = True, max_lines: int = 20) -> bool:
    """Prints a checker's outcome; returns False when it blocks the gate."""
    lines = output.splitlines() if output else []
    if code is None:
        print(f"⚠️ {label}: skipped ({output}).")
        return True
    if code == 0:
        print(f"✅ {label} passed. {lines[-1] if lines else ''}")
        return True
    print(f"{'❌' if blocking else '⚠️'} {label} failed:")
    for line in lines[:max_lines]:
        print(f"   {line}")
    if len(lines) > max_lines:
        print(f"   ... ({len(lines) - max_lines} more lines)")
    return not blocking


def load_backlog(cwd: str):
    """(BacklogFile, Backlog) of the project, or (BacklogFile, None) when there is no backlog file."""
    sys.path.insert(0, os.path.join(SKILLS_ROOT, "plan-memory"))
    from _backlog_store import BacklogFile
    bf = BacklogFile(cwd)
    return bf, (bf.load() if bf.exists else None)


def check_bugs_first(cwd: str, plan, store, accept_reason: str = None) -> bool:
    """Open bugs with a blocking severity come before any plan that is not a bug plan."""
    bf, backlog = load_backlog(cwd)
    if backlog is None:
        return True
    problems = backlog.validate()
    if problems:
        print(f"⚠️ Backlog {bf.rel} has {len(problems)} problem(s); run plan-memory/backlog.py validate.")
    blocking = [b for b in backlog.blocking_bugs(bf.blocking_severities) if b.id not in plan.backlog_items]
    others = [b for b in backlog.open_items("bug") if b not in blocking and b.id not in plan.backlog_items]
    if others:
        print(f"ℹ️ Other open bugs in {bf.rel}: {', '.join(b.id for b in others)}")
    if not blocking:
        print(f"✅ Bugs first: no open {'/'.join(bf.blocking_severities) or 'blocking'} bug outside this plan.")
        return True
    listing = ", ".join(f"{b.id} ({b.rank})" for b in blocking)
    if plan.kind == "bug":
        print(f"ℹ️ Bug plan; other blocking bugs still open: {listing}")
        return True
    if accept_reason:
        plan.add_log(store.author(), f"Approved with open blocking bugs {', '.join(b.id for b in blocking)}: {accept_reason}")
        store.save(plan)
        print(f"⚠️ Bugs first overridden by the user: {listing}. Recorded in the plan's status log; commit the plan file.")
        return True
    print(f"❌ Bugs first: open {', '.join(bf.blocking_severities)} bugs come before this {plan.kind} plan: {listing}")
    for b in blocking:
        print(f"   {b.id}: {b.title}")
    print("   Plan them first (plan-memory/backlog.py promote), or, only when the user explicitly accepts the risk,")
    print('   run this gate again with --accept-open-bugs "<the user\'s reason>".')
    return False


def load_roadmap(cwd: str):
    """(RoadmapFile, Roadmap) of the project, or (RoadmapFile, None) when there is no roadmap file."""
    sys.path.insert(0, os.path.join(SKILLS_ROOT, "plan-memory"))
    from _roadmap_store import RoadmapFile
    rf = RoadmapFile(cwd)
    return rf, (rf.load() if rf.exists else None)


NEEDS_CLARIFICATION = "[NEEDS CLARIFICATION"
SPEC_REQUIRED = ["spec.md", "plan.md", "tasks.md"]


def check_spec_dir(cwd: str, plan) -> bool:
    """Spec Kit artifacts of a feature plan: complete, without open clarifications, on a protocol branch."""
    cfg = load_project_config(cwd).section("speckit")
    if not cfg.get("enabled") or plan.kind == "bug":
        return True
    if not plan.spec_dir:
        print("⚠️ Spec Kit is enabled but this feature plan has no spec_dir (save_plan.py --spec-dir specs/NNN-slug); "
              "the user decides whether this change goes without Spec Kit.")
        return True
    folder = os.path.join(cwd, plan.spec_dir)
    missing = [n for n in SPEC_REQUIRED if not os.path.isfile(os.path.join(folder, n))]
    if missing:
        print(f"❌ Spec Kit artifacts missing in {plan.spec_dir}: {', '.join(missing)} "
              "(/speckit-specify, /speckit-plan, /speckit-tasks).")
        return False
    passed = True
    for name in ("spec.md", "plan.md"):
        with open(os.path.join(folder, name), "r", encoding="utf-8") as f:
            count = f.read().count(NEEDS_CLARIFICATION)
        if count:
            print(f"❌ {plan.spec_dir}/{name} has {count} open [NEEDS CLARIFICATION] marker(s): run /speckit-clarify.")
            passed = False
    with open(os.path.join(folder, "plan.md"), "r", encoding="utf-8") as f:
        if "## Dev Cycle Protocol" not in f.read():
            print(f"⚠️ {plan.spec_dir}/plan.md has no 'Dev Cycle Protocol' sections (is the dev-cycle preset installed?).")
    prefix = cfg.get("branch_prefix") or "feat"
    code, branch = run_cmd(["git", "rev-parse", "--abbrev-ref", "HEAD"], cwd)
    if code == 0 and not re.match(rf"^{re.escape(prefix)}/\d{{3,}}-", branch):
        print(f"⚠️ Current branch '{branch}' is not a Spec Kit feature branch ({prefix}/NNN-slug).")
    if passed:
        print(f"✅ Spec Kit artifacts complete in {plan.spec_dir} (spec, plan, tasks; no open clarification).")
    return passed


def check_speckit(cwd: str) -> bool:
    """When Spec Kit is enabled: the setup matches the pin and the constitution is current."""
    cfg = load_project_config(cwd)
    if not cfg.section("speckit").get("enabled"):
        return True
    passed = report_checker("Spec Kit setup (speckit_setup.py status)",
                            *run_checker("dev-cycle-protocol", "speckit_setup.py", ["status", "--dir", cwd]))
    passed &= report_checker("Constitution (constitution.py check)",
                             *run_checker("dev-cycle-protocol", "constitution.py", ["check", "--dir", cwd]))
    return passed


def verify_gate_0(cwd: str, project_name: str) -> bool:
    """Product baseline gate (Phase 0): vision, technical specification, architecture, backlog and roadmap."""
    print("=" * 80)
    print(f"🌱 VERIFYING GATE 0 (PRODUCT BASELINE): {project_name}")
    print("=" * 80)

    passed = True
    cfg = load_project_config(cwd)

    # 1. Business document and technical specification (product-inception)
    code, out = run_checker("product-inception", "check_product.py", ["--dir", cwd])
    passed &= report_checker("Product documents (product-inception)", code, out, max_lines=40)
    questions = [l.strip() for l in (out or "").splitlines() if l.strip().startswith("❓")]
    if code == 0 and questions:
        print(f"⚠️ {len(questions)} open question(s) remain; the user decides whether the baseline is approved with them:")
        for q in questions:
            print(f"   {q}")

    # 2. Architecture style (software-design) and technical documentation (architecture-docs)
    style = cfg.section("architecture").get("style")
    if style:
        print(f"✅ Architecture style set: {style}")
    else:
        print("❌ architecture.style is not set in .dev-cycle.json (software-design).")
        passed = False
    index = (cfg.section("architecture_docs") or {}).get("index") or cfg.section("docs").get("tec_doc")
    if index and os.path.isfile(os.path.join(cwd, index)):
        code, out = run_checker("architecture-docs", "check_docs.py", ["--dir", cwd])
        if code is not None and "does not use the architecture-docs layout" in out:
            print("⚠️ TecDoc not migrated to the architecture-docs layout (docs/architecture + docs/adr) yet.")
        else:
            passed &= report_checker("Architecture docs check (architecture-docs)", code, out)
    else:
        print(f"❌ Technical documentation index missing ({index or 'not configured'}): create it with architecture-docs.")
        passed = False

    # 3. Product backlog with epics (plan-memory)
    try:
        bf, backlog = load_backlog(cwd)
        if backlog is None:
            print(f"❌ No backlog at {bf.rel}: capture the product's epics and items (plan-memory/backlog.py).")
            passed = False
        else:
            problems = backlog.validate()
            epics = backlog.open_items("epic")
            if problems:
                print(f"❌ Backlog {bf.rel} has {len(problems)} problem(s); run plan-memory/backlog.py validate.")
                passed = False
            elif not epics:
                print(f"❌ Backlog {bf.rel} has no epic: the complete product is planned as epics and their items.")
                passed = False
            else:
                loose = [i.id for i in backlog.open_items() if i.type not in ("epic", "bug") and not i.epic]
                print(f"✅ Backlog: {len(epics)} epic(s), {len(backlog.open_items()) - len(epics)} open item(s).")
                if loose:
                    print(f"ℹ️ Items without an epic: {', '.join(loose)}")
    except Exception as e:
        print(f"❌ Could not read the backlog: {e}")
        backlog = None
        passed = False

    # 4. Roadmap (plan-memory)
    try:
        rf, roadmap = load_roadmap(cwd)
        if roadmap is None:
            print(f"❌ No roadmap at {rf.rel}: plan the releases (plan-memory/roadmap.py add-release, assign).")
            passed = False
        else:
            problems = roadmap.validate(backlog)
            current = roadmap.current()
            if problems:
                print(f"❌ Roadmap {rf.rel} has {len(problems)} problem(s); run plan-memory/roadmap.py validate.")
                passed = False
            elif current is None or not current.items:
                print(f"❌ Roadmap {rf.rel} has no release with items to start from.")
                passed = False
            else:
                print(f"✅ Roadmap: {len(roadmap.releases)} release(s); first: {current.version} {current.name} "
                      f"({len(current.items)} item(s)).")
    except Exception as e:
        print(f"❌ Could not read the roadmap: {e}")
        passed = False

    # 5. Spec Kit and the generated constitution, when enabled
    passed &= check_speckit(cwd)

    # 6. Secret hygiene
    secrets = check_secret_hygiene(cwd)
    if secrets:
        for s in secrets:
            print(f"🚨 SECRET HYGIENE ALERT: {s}")
        passed = False
    else:
        print("✅ Secret hygiene check passed.")

    print("-" * 80)
    print("🟢 GATE 0 STATUS: READY FOR THE USER'S APPROVAL OF THE PRODUCT BASELINE" if passed
          else "🚨 GATE 0 STATUS: PRODUCT BASELINE INCOMPLETE")
    print("=" * 80 + "\n")
    return passed


def verify_gate_3(cwd: str, project_name: str, accept_open_bugs: str = None) -> bool:
    """Plan approval gate."""
    print("=" * 80)
    print(f"🔒 VERIFYING GATE 3 (PLAN APPROVAL): {project_name}")
    print("=" * 80)

    passed = True
    code, status_out = run_cmd(["git", "status", "--porcelain"], cwd)
    dirty_files = [l for l in status_out.splitlines() if l.strip()]
    if dirty_files:
        print(f"⚠️ Working tree has {len(dirty_files)} uncommitted/untracked changes.")
    else:
        print("✅ Git working tree is clean.")

    try:
        sys.path.insert(0, os.path.join(SKILLS_ROOT, "plan-memory"))
        from _store import PlanStore
        store = PlanStore(cwd)
        in_force = [p for p in store.load_all() if p.meta.get("status") in ("active", "in_progress")]
        if in_force:
            latest = store.latest(in_force)
            print(f"✅ Active plan found in {store.plans_dir}: '{latest.meta['plan_id']}' ({len(in_force)} active/in progress)")
            _, tracked = run_cmd(["git", "ls-files", "--", latest.path], cwd)
            if not tracked:
                print("⚠️ The active plan file is not committed yet. Commit it after approval.")
            passed &= check_bugs_first(cwd, latest, store, accept_open_bugs)
            passed &= check_spec_dir(cwd, latest)
        else:
            print(f"⚠️ No active plan in {store.plans_dir}. Save the plan with plan-memory in Phase 2 (full path).")
    except Exception as e:
        print(f"ℹ️ Could not check plan-memory plans: {e}")

    passed &= check_speckit(cwd)

    secrets = check_secret_hygiene(cwd)
    if secrets:
        for s in secrets:
            print(f"🚨 SECRET HYGIENE ALERT: {s}")
        passed = False
    else:
        print("✅ Secret hygiene check passed (no unencrypted credential files detected).")

    print("-" * 80)
    print("🟢 GATE 3 STATUS: READY FOR EXPLICIT USER APPROVAL" if passed else "🚨 GATE 3 STATUS: BLOCKED BY COMPLIANCE CHECKS")
    print("=" * 80 + "\n")
    return passed


def verify_gate_rc(cwd: str, project_name: str, run: str) -> bool:
    """Release candidate acceptance gate: every manual test of the run is Pass."""
    print("=" * 80)
    print(f"✅ VERIFYING RELEASE CANDIDATE ACCEPTANCE: {project_name} ({run or 'no run'})")
    print("=" * 80)
    if not run:
        print("❌ --run <rc tag> is required (the run started with quality-assurance/manual_tests.py start-run).")
        print("=" * 80 + "\n")
        return False
    code, out = run_checker("quality-assurance", "manual_tests.py", ["gate", "--run", run, "--dir", cwd])
    passed = code is not None and report_checker(f"Manual test run {run}", code, out)
    if code is None:
        print(f"❌ {out}")
    print("-" * 80)
    print(f"🟢 RC GATE STATUS: CANDIDATE ACCEPTED" if passed else "🚨 RC GATE STATUS: NOT ACCEPTED")
    print("=" * 80 + "\n")
    return passed


def verify_gate_8(cwd: str, project_name: str, run: str = None) -> bool:
    """Release readiness gate, before opening the pull request into the production branch."""
    print("=" * 80)
    print(f"🚀 VERIFYING GATE 8 (RELEASE READINESS): {project_name}")
    print("=" * 80)

    passed = True
    cfg = load_project_config(cwd)

    # 1. Documentation and manual tests at the configured paths
    for label, rel in (("UserDoc", cfg.section("docs").get("user_doc")),
                       ("TecDoc", cfg.section("docs").get("tec_doc")),
                       ("Manual test suite", cfg.section("tests").get("manual_tests"))):
        if rel and os.path.exists(os.path.join(cwd, rel)):
            print(f"✅ {label} verified ({rel} exists).")
        else:
            print(f"❌ {label} missing ({rel or 'path not configured'} required).")
            passed = False

    # 2. Technical documentation (architecture-docs)
    code, out = run_checker("architecture-docs", "check_docs.py", ["--dir", cwd, "--max-warnings", "10"])
    if code is not None and "does not use the architecture-docs layout" in out:
        print("⚠️ TecDoc not migrated to the architecture-docs layout (docs/architecture + docs/adr) yet.")
    else:
        passed &= report_checker("Architecture docs check (architecture-docs)", code, out)

    # 3. README (user-docs)
    passed &= report_checker("README check (user-docs)",
                             *run_checker("user-docs", "check_readme.py", ["--dir", cwd]))

    # 4. Manual test suite format (quality-assurance)
    passed &= report_checker("Manual test suite (quality-assurance)",
                             *run_checker("quality-assurance", "manual_tests.py", ["validate", "--dir", cwd]))

    # 5. Design and core dependencies (software-design)
    passed &= report_checker("Design check (software-design)",
                             *run_checker("software-design", "check_design.py", ["--dir", cwd]))

    # 6. CI policy (devops-release), when the project uses GitHub Actions
    if os.path.isdir(os.path.join(cwd, ".github", "workflows")):
        passed &= report_checker("CI policy (devops-release)",
                                 *run_checker("devops-release", "check_ci_policy.py", ["--dir", cwd]))
    else:
        print("⚠️ No .github/workflows: CI policy check skipped.")

    # 7. Version files agree (devops-release)
    if cfg.section("devops").get("version_files"):
        passed &= report_checker("Version files (devops-release)",
                                 *run_checker("devops-release", "release_status.py", ["version", "--dir", cwd]))
    else:
        print("⚠️ devops.version_files is not set: version consistency not checked.")

    # 8. Release candidate acceptance (quality-assurance)
    if cfg.section("devops").get("release_candidates", True):
        if run:
            code, out = run_checker("quality-assurance", "manual_tests.py", ["gate", "--run", run, "--dir", cwd])
            ok = code is not None and report_checker(f"Release candidate run {run} (quality-assurance)", code, out)
            if code is None:
                print(f"❌ {out}")
            passed &= ok
        else:
            print("⚠️ Release candidates are on but no --run was given: acceptance of the candidate not checked.")

    # 9. Backlog format (plan-memory) and open blocking bugs for the release notes
    code, out = run_checker("plan-memory", "backlog.py", ["validate", "--dir", cwd])
    passed &= report_checker("Backlog (plan-memory)", code, out)
    if code is not None:
        try:
            bf, backlog = load_backlog(cwd)
            if backlog is not None:
                open_bugs = backlog.blocking_bugs(bf.blocking_severities)
                if open_bugs:
                    print(f"⚠️ Open {'/'.join(bf.blocking_severities)} bugs; list them as known limitations in the release notes: "
                          + ", ".join(f"{b.id} {b.title}" for b in open_bugs))
        except Exception as e:
            print(f"ℹ️ Could not read the backlog: {e}")

    # 10. Roadmap (plan-memory): items of the release being shipped that are still open
    code, out = run_checker("plan-memory", "roadmap.py", ["validate", "--dir", cwd])
    passed &= report_checker("Roadmap (plan-memory)", code, out)
    if code is not None:
        try:
            rf, roadmap = load_roadmap(cwd)
            if roadmap is not None:
                version = re.sub(r"-rc\.\d+$", "", run) if run else None
                release = next((r for r in roadmap.releases if r.version == version), None) if version else roadmap.current()
                if release is None and version:
                    print(f"⚠️ {version} is not in the roadmap {rf.rel}; add it or check the version.")
                elif release is not None:
                    _, backlog = load_backlog(cwd)
                    still_open = []
                    for item_id in release.items:
                        try:
                            item = backlog.find(item_id) if backlog else None
                        except ValueError:
                            item = None
                        if item is not None and not item.closed:
                            still_open.append(f"{item.id} [{item.status}] {item.title}")
                    if still_open:
                        print(f"⚠️ Items of {release.version} still open; the user decides whether they move to the next release "
                              f"(roadmap.py assign --move): " + "; ".join(still_open))
                    else:
                        print(f"✅ Every item of {release.version} is closed.")
        except Exception as e:
            print(f"ℹ️ Could not read the roadmap: {e}")

    # 11. Secret hygiene
    secrets = check_secret_hygiene(cwd)
    if secrets:
        for s in secrets:
            print(f"🚨 SECRET HYGIENE ALERT: {s}")
        passed = False
    else:
        print("✅ Secret hygiene check passed.")

    # 12. Git status
    code, status_out = run_cmd(["git", "status", "--porcelain"], cwd)
    dirty = [l for l in status_out.splitlines() if l.strip()]
    if dirty:
        print(f"⚠️ Uncommitted changes remaining before release: {len(dirty)} files.")
    else:
        print("✅ Working directory clean for release commit.")

    print("-" * 80)
    print("🟢 GATE 8 STATUS: READY FOR THE RELEASE PULL REQUEST (the user merges)" if passed
          else "🚨 GATE 8 STATUS: BLOCKED BY COMPLIANCE CHECKS")
    print("=" * 80 + "\n")
    return passed


def main() -> int:
    if sys.stdout.encoding != "utf-8":
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass
    parser = argparse.ArgumentParser(description="Verify gate compliance for dev-cycle-protocol")
    parser.add_argument("--gate", type=str, required=True, choices=["0", "3", "8", "rc", "gate0", "gate3", "gate8", "gaterc"],
                        help="Gate to verify (0=product baseline, 3=plan approval, rc=candidate acceptance, 8=release readiness)")
    parser.add_argument("--dir", type=str, default=None, help="Workspace directory")
    parser.add_argument("--project", type=str, default=None, help="Project name override")
    parser.add_argument("--run", type=str, default=None, help="Release candidate run label, for example v1.4.0-rc.1")
    parser.add_argument("--accept-open-bugs", type=str, default=None, metavar="REASON",
                        help="Gate 3: the user explicitly accepts approving a non-bug plan with open blocking bugs")
    args = parser.parse_args()

    cwd = os.path.abspath(args.dir or os.getcwd())
    p_name = args.project or load_project_config(cwd).project_name

    gate = args.gate.replace("gate", "")
    if gate == "0":
        success = verify_gate_0(cwd, p_name)
    elif gate == "3":
        success = verify_gate_3(cwd, p_name, args.accept_open_bugs)
    elif gate == "rc":
        success = verify_gate_rc(cwd, p_name, args.run)
    else:
        success = verify_gate_8(cwd, p_name, args.run)
    return 0 if success else 1


if __name__ == "__main__":
    sys.exit(main())
