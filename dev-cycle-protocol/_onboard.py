"""
Onboarding rules - dev-cycle-protocol Skill
Pure functions behind `/dev-cycle`: from the state of a repository (collected by onboard.py)
to the options the user is offered, in the recommended order, and the summary sized to what
was found. No I/O here, so every rule is unit-tested.

State (dict, every key optional):
    repo:     {git, commits, branch, dirty, has_code}
    config:   {exists, problems}
    branches: {integration, integration_exists, production, production_exists}
    baseline: {vision, tech_spec, style, tec_doc, epics, roadmap}
    plans:    [{id, title, status, done, total}]          (active and in progress)
    rc:       {run, version, not_run, failed, released}    (latest release candidate run)
    bugs:     ["BUG-001 (high) Crash", ...]                 (open blocking bugs)
    roadmap:  {current, open_items: [...]}                  (current release)
    speckit:  {state, problems, uv, installed}              (state: undecided | declined | enabled | unmanaged)
    agent:    {name, attribution_hidden}                    (name given by the command, e.g. claude-code)
    findings: ["<file>: could not be read (...)", ...]
"""

# Option kinds, in the recommended order (D4). The first three compete with a request.
ORDER = ["plan", "rc", "bugs", "setup", "baseline", "release", "request"]
COMPETING = {"plan", "rc", "bugs"}


def is_new(state: dict) -> bool:
    """A new or empty repository: no product baseline, no plans and no meaningful code."""
    baseline = state.get("baseline") or {}
    has_baseline = any(baseline.get(k) for k in ("vision", "tech_spec", "epics", "roadmap"))
    return not has_baseline and not state.get("plans") and not (state.get("repo") or {}).get("has_code")


def setup_items(state: dict, new: bool) -> list:
    """Missing pieces of the initial setup, in the recommended order (D5, D6)."""
    items = []
    repo, config = state.get("repo") or {}, state.get("config") or {}
    branches, speckit = state.get("branches") or {}, state.get("speckit") or {}
    if not repo.get("git"):
        items.append(("git", "Initialize the Git repository", "there is no Git repository"))
    if not config.get("exists"):
        items.append(("config", "Create .dev-cycle.json (proposed from what was detected, written after you confirm)",
                      "the project has no configuration"))
    elif config.get("problems"):
        items.append(("config", "Fix .dev-cycle.json", "; ".join(config["problems"])))
    if repo.get("commits") and branches.get("integration") and not branches.get("integration_exists"):
        items.append(("branch", f"Create the integration branch '{branches['integration']}' "
                                f"from '{branches.get('production') or 'main'}'",
                      "feature work goes through the integration branch"))
    agent = state.get("agent") or {}
    if agent.get("name") == "claude-code" and not agent.get("attribution_hidden"):
        items.append(("attribution", "Turn off the agent's automatic attribution in commits and pull requests "
                                     "(.claude/settings.json)", "authorship belongs to the repository's Git identity"))
    state_name = speckit.get("state")
    if state_name == "unmanaged":
        items.append(("speckit-adopt", "Adopt the Spec Kit already in the repository (pin its version, preset, "
                                       "feat/NNN-slug branches; the hand-written constitution is kept and moved into the sources)",
                      "Spec Kit is used here without the protocol"))
    elif state_name == "enabled" and speckit.get("problems"):
        items.append(("speckit-fix", "Fix the Spec Kit setup", "; ".join(speckit["problems"])))
    elif state_name == "undecided" and not new:
        if speckit.get("uv"):
            items.append(("speckit-ask", "Decide whether to use Spec Kit for the detail of features (asked once)",
                          "no decision recorded"))
        else:
            items.append(("speckit-ask", "Decide whether to use Spec Kit (needs uv: https://docs.astral.sh/uv/)",
                          "no decision recorded; uv is not installed"))
    return items


def options(state: dict, request: str = None) -> list:
    """Every option that applies, in the recommended order. Each: {kind, key, title, reason}."""
    new = is_new(state)
    out = []

    def add(kind, key, title, reason=""):
        out.append({"kind": kind, "key": key, "title": title, "reason": reason})

    if new:
        for key, title, reason in setup_items(state, new):
            if key in ("git", "config", "branch"):
                continue   # Phase 0 step 1 sets up the repository and the configuration
            add("setup", key, title, reason)
        idea = f": {request}" if request else " (tell the agent your idea)"
        add("baseline", "inception", f"Start the product from the idea{idea}", "new repository: Phase 0, product inception")
        return out

    for plan in state.get("plans") or []:
        left = (plan.get("total") or 0) - (plan.get("done") or 0)
        progress = f"{plan.get('done', 0)}/{plan.get('total', 0)} milestones" if plan.get("total") else "no milestones"
        if plan.get("total") and left <= 0:
            continue
        add("plan", f"plan:{plan['id']}", f"Resume the plan '{plan.get('title') or plan['id']}' ({progress})",
            f"plan {plan.get('status')}")
    rc = state.get("rc")
    if rc and not rc.get("released"):
        if rc.get("not_run") or rc.get("failed"):
            detail = ", ".join(p for p in (f"{rc.get('not_run')} not run" if rc.get("not_run") else "",
                                           f"{rc.get('failed')} failed" if rc.get("failed") else "") if p)
            add("rc", f"rc:{rc['run']}", f"Finish the acceptance of {rc['run']} ({detail})", "release candidate pending")
        else:
            add("rc", f"rc:{rc['run']}", f"{rc['run']} is accepted: open the release pull request (gate 8)",
                "release gate pending")
    bugs = state.get("bugs") or []
    if bugs:
        add("bugs", "bugs", f"Plan the open blocking bugs: {', '.join(b.split(' ')[0] for b in bugs)}", "bugs come first")
    for key, title, reason in setup_items(state, new):
        add("setup", key, title, reason)
    baseline = state.get("baseline") or {}
    if not (baseline.get("vision") and baseline.get("tech_spec")):
        add("baseline", "adoption", "Adopt a product baseline (Phase 0, adoption mode): vision, technical specification, "
                                    "backlog with epics, roadmap", "the project has no product baseline")
    roadmap = state.get("roadmap") or {}
    if roadmap.get("current") and roadmap.get("open_items"):
        add("release", "release", f"Next items of {roadmap['current']}: {', '.join(i.split(' ')[0] for i in roadmap['open_items'][:5])}",
            "current release of the roadmap")
    if request:
        add("request", "request", f"Your request: {request}", "Phase 1, discovery")
    return out


def recommendation(state: dict, request: str = None) -> dict:
    """What the agent does: {mode, ask, recommended, options, summary}.
    `ask` is False only when nothing competes with the request and nothing is missing (D2)."""
    new = is_new(state)
    opts = options(state, request)
    kinds = {o["kind"] for o in opts}
    if new:
        ask = any(o["kind"] == "setup" for o in opts) or not request
        return {"mode": "new", "ask": ask, "recommended": opts[-1]["key"] if not ask else opts[0]["key"],
                "options": opts, "summary": summary(state, opts, request, new)}
    ask = bool(kinds & COMPETING) or "setup" in kinds or "baseline" in kinds or not request
    return {"mode": "existing", "ask": ask, "recommended": opts[0]["key"] if opts else None,
            "options": opts, "summary": summary(state, opts, request, new)}


def summary(state: dict, opts: list, request: str, new: bool) -> str:
    """One line when there is nothing to decide; otherwise a short heading for the options."""
    if new:
        return "New repository: the cycle starts with Phase 0 (product inception)."
    roadmap = state.get("roadmap") or {}
    release = f"current release {roadmap['current']}" if roadmap.get("current") else "no roadmap"
    bugs = len(state.get("bugs") or [])
    plans = len([o for o in opts if o["kind"] == "plan"])
    line = (f"{release}, {bugs or 'no'} blocking bug{'s' if bugs != 1 else ''}, "
            f"{plans or 'no'} plan{'s' if plans != 1 else ''} in progress")
    kinds = {o["kind"] for o in opts}
    if request and not (kinds & COMPETING) and "setup" not in kinds and "baseline" not in kinds:
        return f"Project up to date ({line}). Going on with: {request}"
    if "setup" in kinds and not (kinds & COMPETING) and not state.get("plans"):
        return f"This repository is not set up for the cycle yet ({line}). Initial setup, in the recommended order:"
    return f"Project state: {line}. Where to start:"
