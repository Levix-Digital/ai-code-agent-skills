"""
Bug triage - plan-memory Skill
Whether every open bug of the backlog is triaged: it has a valid severity, and a bug whose
severity does not block new work (bugs first) is scheduled in the roadmap no later than the
next release (`backlog.bug_deadline: "next_release"`). Pure function over a loaded backlog and roadmap.
"""
from _backlog import SEVERITIES


def bug_triage(backlog, roadmap, blocking: list, deadline: str = "next_release", exclude=()) -> dict:
    """{"untriaged": [(item, reason)], "deadline_checked": bool, "next_release": version or None}.

    An open bug is untriaged when its severity is missing or invalid, or, with a deadline, when its
    severity is not in `blocking` and it is not in the next release of the roadmap (the first one not
    released). Without a roadmap file the deadline cannot be checked. Ids in `exclude` are skipped.
    """
    exclude = {i.upper() for i in exclude}
    check_deadline = deadline == "next_release" and roadmap is not None
    current = roadmap.current() if check_deadline else None
    out = []
    for bug in backlog.open_items("bug"):
        if bug.id in exclude:
            continue
        severity = bug.get("Severity")
        if severity not in SEVERITIES:
            out.append((bug, "no severity" if not severity else f"invalid severity '{severity}'"))
            continue
        if not check_deadline or severity in blocking:
            continue
        release = roadmap.release_of(bug.id)
        if current is None:
            out.append((bug, "no release left in the roadmap to schedule it"))
        elif release is None:
            out.append((bug, f"not scheduled; it goes into {current.version} at the latest"))
        elif release.version != current.version:
            out.append((bug, f"scheduled for {release.version}; the deadline is {current.version}"))
    return {"untriaged": out, "deadline_checked": check_deadline,
            "next_release": current.version if current else None}


def triage_lines(result: dict) -> list:
    return [f"{bug.id} ({bug.rank or '?'}) {bug.title}: {reason}" for bug, reason in result["untriaged"]]
