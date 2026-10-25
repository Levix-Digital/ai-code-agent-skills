"""
Pull Request Sync - dev-cycle-protocol Skill
Phase 7 on the platform: reads the pull request of the current branch (a merge request on
GitLab) with its review threads, comments, reviews and checks, says what is pending, and posts
answers, resolutions and the protocol's code review. Every text it posts passes the guard of
protocol principle 9 (no AI attribution) and carries the protocol's marker.

Usage:
    python pr_sync.py status  [--pr N] [--json]
    python pr_sync.py open    [--json]                      (open pull requests with what is pending)
    python pr_sync.py gate-merge [--pr N]                   (exit 1 while something holds the merge)
    python pr_sync.py reply   --thread ID (--body TEXT | --body-file F) [--pr N]
    python pr_sync.py resolve --thread ID [--pr N]
    python pr_sync.py comment (--body TEXT | --body-file F) [--pr N]
    python pr_sync.py review  --file review.json [--pr N]
    python pr_sync.py request-review --reviewer LOGIN [--reviewer LOGIN ...] [--pr N]
All commands accept --dir <project>. review.json: {"body": "...", "comments": [{"path", "line", "level", "body"}]},
level one of blocking, suggestion, nit.
"""
import argparse
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import _no_ai  # noqa: E402
from _pr_model import (LEVELS, REPLY_MARKER, REVIEW_MARKER, format_post, merge_blockers, needs_action,  # noqa: E402
                       pending, summary_line)
from _pr_providers import ProviderError, detect_provider, provider_for  # noqa: E402
from project_config import load_project_config  # noqa: E402


def git(root: str, *args) -> str:
    res = subprocess.run(["git", *args], cwd=root, capture_output=True, text=True, encoding="utf-8", errors="replace")
    return res.stdout.strip() if res.returncode == 0 else ""


class Context:
    """The project's platform, markers and current branch."""

    def __init__(self, start_dir: str = None, runner=None):
        start = os.path.abspath(start_dir or os.getcwd())
        self.root = git(start, "rev-parse", "--show-toplevel") or start
        cfg = load_project_config(self.root)
        review = cfg.section("review")
        self.marker = review.get("marker") or REVIEW_MARKER
        self.reply_marker = review.get("reply_marker") or REPLY_MARKER
        self.allowed = review.get("allowed_terms") or []
        remote = git(self.root, "remote", "get-url", "origin")
        self.provider_name, self.host, self.path = detect_provider(remote, review.get("provider"), review.get("host"))
        self.branch = git(self.root, "rev-parse", "--abbrev-ref", "HEAD")
        self.runner = runner

    def provider(self):
        if not self.provider_name or not self.path:
            raise ProviderError("the pull request platform is unknown: no 'origin' remote on GitHub or GitLab "
                                "(set review.provider and review.host in .dev-cycle.json)")
        return provider_for(self.provider_name, self.host, self.path, self.runner)

    def pull_request(self, number=None):
        pr = self.provider().pull_request(number, self.branch)
        if pr is None:
            raise ProviderError(f"no open pull request for branch '{self.branch}'")
        return pr

    def guard(self, text: str, what: str):
        _no_ai.check(text, what, self.allowed)


def pending_of(ctx: Context, pr: dict) -> dict:
    return pending(pr, ctx.reply_marker, ctx.marker)


def print_status(pr: dict, p: dict):
    print(f"🔀 {summary_line(pr, p)}")
    print(f"   {pr.get('url')}  ({pr.get('head')} → {pr.get('base')}{', draft' if p['draft'] else ''})")
    order = {"large": 0, "question": 1, "small": 2, "suggestion": 3, "nit": 4}
    for t in sorted(p["threads"], key=lambda t: order.get(t["hint"], 9)):
        last = t["comments"][-1]
        excerpt = " ".join((last.get("body") or "").split())[:140]
        where = f"{t.get('path')}:{t.get('line')}" + (" (outdated)" if t.get("outdated") else "")
        who = "for the user" if t["hint"] == "large" else t["hint"]
        print(f"   💬 [{who}] {where} — {last.get('author')}: {excerpt}  (thread {t['id']})")
    for c in p["comments"]:
        print(f"   🗨️ {c.get('author')}: {' '.join((c.get('body') or '').split())[:140]}")
    for c in p["failed_checks"]:
        print(f"   ❌ check {c.get('name')}: {c.get('url') or ''}")
    for c in p["running_checks"]:
        print(f"   ⏳ check {c.get('name')} running")
    if p["conflict"]:
        print("   ⚠️ merge conflict with the base branch")
    if pr.get("threads_note"):
        print(f"   ℹ️ {pr['threads_note']}: threads are shown as open")
    if not needs_action(p):
        print("   ✅ nothing to do on the pull request now")


def read_body(args) -> str:
    if getattr(args, "body_file", None):
        with open(args.body_file, "r", encoding="utf-8") as f:
            return f.read()
    if not getattr(args, "body", None):
        raise ValueError("give --body or --body-file")
    return args.body


def load_review(path: str, ctx: Context) -> tuple:
    with open(path, "r", encoding="utf-8") as f:
        data = json.load(f)
    if not isinstance(data, dict):
        raise ValueError("the review file must hold a JSON object")
    body = format_post(ctx.marker, data.get("body") or "Review of the changes.")
    ctx.guard(body, "the review summary")
    comments = []
    for i, c in enumerate(data.get("comments") or [], 1):
        missing = [k for k in ("path", "line", "level", "body") if not c.get(k)]
        if missing:
            raise ValueError(f"review comment {i} misses: {', '.join(missing)}")
        if c["level"] not in LEVELS:
            raise ValueError(f"review comment {i}: level '{c['level']}' is not one of: {', '.join(LEVELS)}")
        text = format_post(ctx.marker, c["body"], c["level"])
        ctx.guard(text, f"review comment {i}")
        comments.append({"path": c["path"], "line": int(c["line"]), "body": text})
    return body, comments


def gate_merge(ctx: Context, provider, pr: dict) -> int:
    """Phase 7, before the merge is asked for (or done, when the user authorized it)."""
    blockers = merge_blockers(pr, ctx.marker)
    print(f"🔀 {summary_line(pr, pending_of(ctx, pr))}")
    for b in blockers:
        print(f"   ❌ {b}")
    if pr.get("threads_note"):
        print(f"   ℹ️ {pr['threads_note']}")
    required = provider.requires_thread_resolution(pr.get("base"))
    if required is False:
        print(f"   ⚠️ merging into '{pr.get('base')}' does not require resolved conversations; turn it on in "
              f"{provider.resolution_setting}")
    elif required is None:
        print(f"   ℹ️ could not read whether merging into '{pr.get('base')}' requires resolved conversations")
    print(f"{'✅ ready to merge' if not blockers else '❌ not ready to merge'}")
    return 1 if blockers else 0


def main(argv: list = None, runner=None) -> int:
    if sys.stdout.encoding != "utf-8":
        try:
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass
    parser = argparse.ArgumentParser(description="Read and answer the pull request on its platform (Phase 7)")
    sub = parser.add_subparsers(dest="command", required=True)

    def command(name, help_text, pr=True):
        p = sub.add_parser(name, help=help_text)
        p.add_argument("--dir", default=None)
        if pr:
            p.add_argument("--pr", type=int, default=None, help="Number (default: the open one of the current branch)")
        return p

    p = command("status", "What is pending on the pull request")
    p.add_argument("--json", action="store_true")
    p = command("open", "Open pull requests with what is pending", pr=False)
    p.add_argument("--json", action="store_true")
    command("gate-merge", "Fail while a blocking finding, a change request, CI or a conflict holds the merge")
    for name, help_text in (("reply", "Answer a review thread"), ("comment", "Comment on the pull request")):
        p = command(name, help_text)
        if name == "reply":
            p.add_argument("--thread", required=True)
        p.add_argument("--body")
        p.add_argument("--body-file")
    p = command("resolve", "Resolve a review thread (after its fix is pushed or the reviewer agreed)")
    p.add_argument("--thread", required=True)
    p = command("review", "Post the protocol's code review (line comments with marker and level)")
    p.add_argument("--file", required=True)
    p = command("request-review", "Ask people to review again")
    p.add_argument("--reviewer", action="append", required=True)
    args = parser.parse_args(argv)

    ctx = Context(args.dir, runner)
    try:
        if args.command == "open":
            prs = ctx.provider().open_pull_requests()
            data = [{"pull_request": pr, "pending": pending_of(ctx, pr)} for pr in prs]
            if args.json:
                print(json.dumps([{**d, "summary": summary_line(d["pull_request"], d["pending"]),
                                   "needs_action": needs_action(d["pending"])} for d in data], indent=2, ensure_ascii=False))
            else:
                for d in data:
                    mark = "🔴" if needs_action(d["pending"]) else "🟢"
                    print(f"{mark} {summary_line(d['pull_request'], d['pending'])}")
                if not data:
                    print("ℹ️ No open pull request.")
            return 0

        pr = ctx.pull_request(args.pr)
        number = pr["number"]
        if args.command == "status":
            p = pending_of(ctx, pr)
            if args.json:
                print(json.dumps({"pull_request": pr, "pending": p, "summary": summary_line(pr, p),
                                  "needs_action": needs_action(p)}, indent=2, ensure_ascii=False))
            else:
                print_status(pr, p)
            return 0
        provider = ctx.provider()
        if args.command == "gate-merge":
            return gate_merge(ctx, provider, pr)
        if args.command == "reply":
            text = format_post(ctx.reply_marker, read_body(args))
            ctx.guard(text, "the reply")
            provider.reply(number, args.thread, text)
            print(f"✅ Replied on thread {args.thread}")
        elif args.command == "comment":
            text = format_post(ctx.reply_marker, read_body(args))
            ctx.guard(text, "the comment")
            provider.comment(number, text)
            print(f"✅ Commented on {summary_line(pr, pending_of(ctx, pr)).split(':')[0]}")
        elif args.command == "resolve":
            provider.resolve(number, args.thread)
            print(f"✅ Resolved thread {args.thread}")
        elif args.command == "review":
            body, comments = load_review(args.file, ctx)
            provider.review(number, body, comments)
            print(f"✅ Review posted with {len(comments)} line comment(s)")
        elif args.command == "request-review":
            provider.request_review(number, args.reviewer)
            print(f"✅ Review requested from {', '.join(args.reviewer)}")
        return 0
    except (ProviderError, ValueError, OSError) as e:
        print(f"❌ {e}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
