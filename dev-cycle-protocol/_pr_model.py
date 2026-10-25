"""
Pull request model - dev-cycle-protocol Skill
The platform-independent shape of a pull request (a merge request on GitLab), what is pending
on it, triage hints for review threads and the format of what the protocol posts. Pure
functions: the providers (_pr_providers.py) build the model, pr_sync.py prints and acts.

A pull request:
    {provider, number, url, title, state: open|closed|merged, draft, author, base, head,
     mergeable: clean|conflict|unknown,
     checks:   [{name, status: pending|success|failure|skipped, url}],
     reviews:  [{author, state: approved|changes_requested|commented|dismissed}],
     threads:  [{id, path, line, resolved, outdated, comments: [{id, author, body, created}]}],
     threads_note: why the resolution of the threads could not be read, or None,
     comments: [{id, author, body, created}]}
A thread's `resolved` is None when the platform could not say: it counts as open, and a blocking
finding in that state holds the merge, because it cannot be shown to be resolved.
"""
import re

REVIEW_MARKER = "[review]"     # a finding of the protocol's code review
REPLY_MARKER = "[reply]"       # the protocol's answer on a thread or the pull request
LEVELS = ["blocking", "suggestion", "nit"]

SUGGESTION_RE = re.compile(r"```suggestion\b", re.I)
NIT_RE = re.compile(r"^\s*(\*\*\[review\]\*\*\s*`?nit`?|nit\b|nitpick\b|minor\b)", re.I)
LARGE_WORDS = re.compile(r"\b(design|architecture|architectural|refactor|rewrite|api|schema|migration|interface|"
                         r"breaking|approach|rethink|split (this|the) (pr|change)|whole|across)\b", re.I)


def marked(body: str, marker: str) -> bool:
    return (body or "").lstrip().startswith(f"**{marker}**")


def format_post(marker: str, body: str, level: str = None) -> str:
    """What the protocol posts: the marker, the level for review findings, then the text."""
    body = body.strip()
    if level is not None:
        if level not in LEVELS:
            raise ValueError(f"level '{level}' is not one of: {', '.join(LEVELS)}")
        return f"**{marker}** `{level}` {body}"
    return f"**{marker}** {body}"


def last_comment(thread: dict):
    comments = thread.get("comments") or []
    return comments[-1] if comments else None


def thread_waiting(thread: dict, reply_marker: str = REPLY_MARKER) -> bool:
    """An open thread whose last word is not the protocol's answer: someone waits for a fix or a reply.
    Findings of the protocol's own review count too: the author has to address them."""
    if thread.get("resolved"):
        return False
    last = last_comment(thread)
    return bool(last) and not marked(last.get("body"), reply_marker)


def is_blocking_finding(thread: dict, review_marker: str = REVIEW_MARKER) -> bool:
    """A thread opened by the protocol's review with the level `blocking`."""
    first = (thread.get("comments") or [{}])[0].get("body") or ""
    return marked(first, review_marker) and "`blocking`" in first.split("\n", 1)[0]


def triage(thread: dict, review_marker: str = REVIEW_MARKER) -> str:
    """A hint for the agent: suggestion | nit | question | large | small. The agent decides; a large
    ask always goes to the user."""
    first = (thread.get("comments") or [{}])[0].get("body") or ""
    last = (last_comment(thread) or {}).get("body") or ""
    text = last if len(thread.get("comments") or []) > 1 else first
    if SUGGESTION_RE.search(text):
        return "suggestion"
    if is_blocking_finding(thread, review_marker):
        return "large" if LARGE_WORDS.search(first) else "small"
    if NIT_RE.search(text):
        return "nit"
    plain = re.sub(r"```.*?```", "", text, flags=re.S).strip()
    if LARGE_WORDS.search(plain) or plain.count("\n") > 8:
        return "large"
    if plain.endswith("?"):
        return "question"
    return "small"


def latest_reviews(pr: dict) -> dict:
    """author -> the state of their latest review that is not a plain comment."""
    out = {}
    for review in pr.get("reviews") or []:
        if review.get("state") in ("approved", "changes_requested", "dismissed"):
            out[review.get("author")] = review.get("state")
    return out


def pending(pr: dict, reply_marker: str = REPLY_MARKER, review_marker: str = REVIEW_MARKER) -> dict:
    """What needs action on the pull request, by kind."""
    waiting = [t for t in pr.get("threads") or [] if thread_waiting(t, reply_marker)]
    comments = [c for c in pr.get("comments") or [] if not marked(c.get("body"), reply_marker)
                and not marked(c.get("body"), review_marker)]
    answered_after = None
    for c in pr.get("comments") or []:
        if marked(c.get("body"), reply_marker):
            answered_after = c.get("created")
    if answered_after:
        comments = [c for c in comments if (c.get("created") or "") > answered_after]
    reviews = latest_reviews(pr)
    return {
        "threads": [{**t, "hint": triage(t, review_marker)} for t in waiting],
        "comments": comments,
        "changes_requested": sorted(a for a, s in reviews.items() if s == "changes_requested"),
        "approved": sorted(a for a, s in reviews.items() if s == "approved"),
        "failed_checks": [c for c in pr.get("checks") or [] if c.get("status") == "failure"],
        "running_checks": [c for c in pr.get("checks") or [] if c.get("status") == "pending"],
        "conflict": pr.get("mergeable") == "conflict",
        "draft": bool(pr.get("draft")),
    }


def merge_blockers(pr: dict, review_marker: str = REVIEW_MARKER) -> list:
    """Why the pull request cannot be merged yet; empty when it can. A blocking finding of the
    protocol's review holds the merge until its thread is resolved, a reply alone does not release it."""
    out = []
    if pr.get("state") != "open":
        out.append(f"the pull request is {pr.get('state')}")
    if pr.get("draft"):
        out.append("the pull request is a draft")
    for t in pr.get("threads") or []:
        if t.get("resolved") or not is_blocking_finding(t, review_marker):
            continue
        where = f"{t.get('path')}:{t.get('line')} (thread {t.get('id')})"
        out.append(f"blocking finding still open at {where}" if t.get("resolved") is False else
                   f"blocking finding at {where}: whether it is resolved could not be read; check it on the "
                   f"pull request page")
    for author in sorted(a for a, s in latest_reviews(pr).items() if s == "changes_requested"):
        out.append(f"changes requested by {author}")
    for c in pr.get("checks") or []:
        if c.get("status") == "failure":
            out.append(f"check {c.get('name')} failed")
        elif c.get("status") == "pending":
            out.append(f"check {c.get('name')} still running")
    if pr.get("mergeable") == "conflict":
        out.append("merge conflict with the base branch")
    return out


def needs_action(p: dict) -> bool:
    return bool(p["threads"] or p["comments"] or p["changes_requested"] or p["failed_checks"] or p["conflict"])


def summary_line(pr: dict, p: dict) -> str:
    """One line for the diagnosis: 'PR #12: 3 threads waiting (1 large), CI red, conflict'."""
    parts = []
    if p["threads"]:
        large = len([t for t in p["threads"] if t["hint"] == "large"])
        parts.append(f"{len(p['threads'])} thread{'s' if len(p['threads']) != 1 else ''} waiting"
                     + (f" ({large} for you)" if large else ""))
    if p["comments"]:
        parts.append(f"{len(p['comments'])} comment{'s' if len(p['comments']) != 1 else ''} to answer")
    if p["changes_requested"]:
        parts.append(f"changes requested by {', '.join(p['changes_requested'])}")
    if p["failed_checks"]:
        parts.append("CI red")
    elif p["running_checks"]:
        parts.append("CI running")
    if p["conflict"]:
        parts.append("merge conflict")
    if not parts:
        parts.append("approved" if p["approved"] else "waiting for review")
    kind = "MR !" if pr.get("provider") == "gitlab" else "PR #"
    return f"{kind}{pr.get('number')} {pr.get('title') or ''}".rstrip() + ": " + ", ".join(parts)
