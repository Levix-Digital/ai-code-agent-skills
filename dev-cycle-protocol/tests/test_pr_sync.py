import json
import os
import subprocess
import sys

import pytest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))

import _no_ai  # noqa: E402
import pr_sync  # noqa: E402
from _pr_model import format_post, needs_action, pending, summary_line, triage  # noqa: E402
from _pr_providers import GitHubProvider, ProviderError, detect_provider, parse_remote  # noqa: E402

GH_VIEW = {
    "number": 12, "url": "https://github.com/acme/photos/pull/12", "title": "Album sharing", "state": "OPEN",
    "isDraft": False, "author": {"login": "levi"}, "baseRefName": "dev", "headRefName": "feat/012-sharing",
    "mergeable": "CONFLICTING",
    "reviews": [{"author": {"login": "ana"}, "state": "COMMENTED"}, {"author": {"login": "ana"}, "state": "CHANGES_REQUESTED"},
                {"author": {"login": "bob"}, "state": "APPROVED"}],
    "comments": [{"id": "IC_1", "author": {"login": "ana"}, "body": "Can we ship this before Friday?", "createdAt": "2026-10-23T10:00:00Z"}],
    "statusCheckRollup": [
        {"__typename": "CheckRun", "name": "tests", "status": "COMPLETED", "conclusion": "FAILURE", "detailsUrl": "https://ci/1"},
        {"__typename": "CheckRun", "name": "lint", "status": "IN_PROGRESS", "conclusion": None, "detailsUrl": "https://ci/2"},
        {"__typename": "StatusContext", "context": "deploy/preview", "state": "SUCCESS", "targetUrl": "https://ci/3"}],
}


def thread(tid, path, line, comments, resolved=False, outdated=False):
    return {"id": tid, "isResolved": resolved, "isOutdated": outdated, "path": path, "line": line, "originalLine": line,
            "comments": {"nodes": [{"id": f"{tid}-{i}", "author": {"login": a}, "body": b, "createdAt": f"2026-10-23T1{i}:00:00Z"}
                                   for i, (a, b) in enumerate(comments)]}}


GH_THREADS = {"data": {"repository": {"pullRequest": {"reviewThreads": {"nodes": [
    thread("T1", "app/share.py", 10, [("ana", "nit: rename `x` to `album_id`")]),
    thread("T2", "app/share.py", 30, [("ana", "```suggestion\nreturn None\n```")]),
    thread("T3", "app/api.py", 5, [("ana", "I think the sharing API design should change: links instead of invites?")]),
    thread("T4", "app/api.py", 9, [("ana", "Why is this async?"), ("levi", "**[reply]** It calls the network.")]),
    thread("T5", "app/db.py", 2, [("ana", "Typo")], resolved=True),
    thread("T6", "app/db.py", 7, [("levi", "**[review]** `blocking` Missing regression test for the empty album.")]),
]}}}}}


class FakeGh:
    def __init__(self, view=GH_VIEW, threads=GH_THREADS, fail=None):
        self.calls, self.view, self.threads, self.fail = [], view, threads, fail

    def __call__(self, cmd, input_text=None):
        self.calls.append((cmd, input_text))
        if self.fail:
            return self.fail
        joined = " ".join(cmd)
        if cmd[:3] == ["gh", "pr", "list"]:
            return 0, json.dumps([{"number": 12}]), ""
        if cmd[:3] == ["gh", "pr", "view"]:
            return 0, json.dumps(self.view), ""
        if "graphql" in joined and "reviewThreads" in joined:
            return 0, json.dumps(self.threads), ""
        if cmd[:3] == ["gh", "api", "user"]:
            return 0, "levi\n", ""
        return 0, "{}", ""


def model():
    return GitHubProvider("acme/photos", runner=FakeGh()).pull_request(12)


# ---------------------------------------------------------------- detection

@pytest.mark.parametrize("url,expected", [
    ("https://github.com/acme/photos.git", ("github", "github.com", "acme/photos")),
    ("git@github.com:acme/photos.git", ("github", "github.com", "acme/photos")),
    ("https://gitlab.com/group/sub/photos.git", ("gitlab", "gitlab.com", "group/sub/photos")),
    ("ssh://git@gitlab.acme.io/team/photos.git", ("gitlab", "gitlab.acme.io", "team/photos")),
    ("https://bitbucket.org/acme/photos.git", (None, "bitbucket.org", "acme/photos")),
])
def test_detect_provider(url, expected):
    assert detect_provider(url) == expected


def test_configuration_wins_over_detection():
    assert detect_provider("https://git.acme.io/team/photos.git", "gitlab", "git.acme.io") == ("gitlab", "git.acme.io", "team/photos")
    assert parse_remote("not a url") == {}


# ---------------------------------------------------------------- model

def test_github_model():
    pr = model()
    assert pr["provider"] == "github" and pr["number"] == 12 and pr["state"] == "open" and pr["mergeable"] == "conflict"
    assert [c["status"] for c in pr["checks"]] == ["failure", "pending", "success"]
    assert pr["threads"][0]["comments"][0]["author"] == "ana" and pr["threads"][4]["resolved"]


def test_pending_and_triage():
    p = pending(model())
    hints = {t["id"]: t["hint"] for t in p["threads"]}
    assert hints == {"T1": "nit", "T2": "suggestion", "T3": "large", "T6": "small"}   # T4 answered, T5 resolved
    assert p["changes_requested"] == ["ana"] and p["approved"] == ["bob"]
    assert [c["name"] for c in p["failed_checks"]] == ["tests"] and p["conflict"]
    assert [c["id"] for c in p["comments"]] == ["IC_1"]
    assert needs_action(p)
    line = summary_line(model(), p)
    assert line.startswith("PR #12 Album sharing: 4 threads waiting (1 for you), 1 comment to answer")
    assert "changes requested by ana" in line and "CI red" in line and "merge conflict" in line


def test_triage_question_and_review_findings():
    assert triage({"comments": [{"body": "Why is this async?"}]}) == "question"
    assert triage({"comments": [{"body": "**[review]** `blocking` The schema migration is not backward compatible"}]}) == "large"
    assert triage({"comments": [{"body": "**[review]** `nit` Name"}]}) == "nit"


def test_comment_answered_by_the_protocol_is_not_pending():
    pr = model()
    pr["comments"].append({"id": "IC_2", "author": "levi", "body": "**[reply]** Yes, today.", "created": "2026-10-23T11:00:00Z"})
    assert pending(pr)["comments"] == []


def test_format_post():
    assert format_post("[review]", " Missing test ", "blocking") == "**[review]** `blocking` Missing test"
    assert format_post("[reply]", "Fixed in abc123.") == "**[reply]** Fixed in abc123."
    with pytest.raises(ValueError):
        format_post("[review]", "x", "critical")


# ---------------------------------------------------------------- provider commands

def test_github_reads_use_the_cli():
    fake = FakeGh()
    pr = GitHubProvider("acme/photos", runner=fake).pull_request(None, "feat/012-sharing")
    assert pr["number"] == 12
    assert fake.calls[0][0][:6] == ["gh", "pr", "list", "--head", "feat/012-sharing", "--state"]
    assert any("reviewThreads" in " ".join(c[0]) and "number=12" in c[0] for c in fake.calls)


def test_github_writes():
    fake = FakeGh()
    gh = GitHubProvider("acme/photos", runner=fake)
    gh.reply(12, "T1", "**[reply]** Fixed in abc.")
    gh.resolve(12, "T1")
    gh.comment(12, "**[reply]** Done.")
    gh.review(12, "**[review]** Summary", [{"path": "a.py", "line": 3, "body": "**[review]** `nit` x"}])
    gh.request_review(12, ["ana"])
    cmds = [" ".join(c[0]) for c in fake.calls]
    assert "addPullRequestReviewThreadReply" in cmds[0] and "thread=T1" in cmds[0]
    assert "resolveReviewThread" in cmds[1]
    assert "repos/acme/photos/issues/12/comments" in cmds[2] and json.loads(fake.calls[2][1]) == {"body": "**[reply]** Done."}
    review = json.loads(fake.calls[3][1])
    assert review["event"] == "COMMENT" and review["comments"][0] == {"path": "a.py", "line": 3, "side": "RIGHT", "body": "**[review]** `nit` x"}
    assert cmds[4].startswith("gh pr edit 12 --add-reviewer ana")


def test_cli_errors_are_explained():
    with pytest.raises(ProviderError, match="not installed"):
        GitHubProvider("a/b", runner=lambda c, i=None: (127, "", "")).me()
    with pytest.raises(ProviderError, match="gh auth login"):
        GitHubProvider("a/b", runner=lambda c, i=None: (4, "", "To get started with GitHub CLI, please run:  gh auth login")).me()


# ---------------------------------------------------------------- the guard

@pytest.mark.parametrize("text", [
    "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>", "Generated with Claude Code", "This was AI-generated",
    "Copilot suggested this change", "fix: the agent found a bug", "Claude-Session: https://claude.ai/code/session_x",
    "🤖 done", "Written with the help of ChatGPT", "As an AI, I think",
])
def test_guard_finds_attribution(text):
    assert _no_ai.findings(text)
    with pytest.raises(ValueError, match="principle 9"):
        _no_ai.check(text)


@pytest.mark.parametrize("text", [
    "feat: add an OpenAI provider", "chore: add .claude/settings.json", "docs: the coding agent setup",
    "- Claude Code marketplace (levix-agent-skills) with the dev-skills plugin", "use the `claude` integration",
    "feat: AI summary of photos", "Fixed in abc123.", "The agent loop calls the model",
])
def test_guard_allows_technical_references(text):
    assert _no_ai.findings(text) == []


def test_guard_allowed_terms():
    assert _no_ai.findings("Generated with Gemini", allowed=["Gemini"]) == []


# ---------------------------------------------------------------- CLI

@pytest.fixture
def repo(tmp_path):
    subprocess.run(["git", "init", "-q", "-b", "feat/012-sharing"], cwd=tmp_path, check=True)
    subprocess.run(["git", "remote", "add", "origin", "https://github.com/acme/photos.git"], cwd=tmp_path, check=True)
    subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "i"],
                   cwd=tmp_path, check=True)
    return tmp_path


def test_cli_status(repo, capsys):
    assert pr_sync.main(["status", "--dir", str(repo)], runner=FakeGh()) == 0
    out = capsys.readouterr().out
    assert "PR #12 Album sharing" in out and "[for the user] app/api.py:5" in out and "(thread T3)" in out
    assert "❌ check tests" in out and "merge conflict" in out
    assert pr_sync.main(["status", "--dir", str(repo), "--json"], runner=FakeGh()) == 0
    data = json.loads(capsys.readouterr().out)
    assert data["needs_action"] and len(data["pending"]["threads"]) == 4


def test_cli_reply_adds_the_marker_and_refuses_attribution(repo, capsys):
    fake = FakeGh()
    assert pr_sync.main(["reply", "--dir", str(repo), "--thread", "T1", "--body", "Fixed in abc123."], runner=fake) == 0
    sent = [c for c in fake.calls if "addPullRequestReviewThreadReply" in " ".join(c[0])][0][0]
    assert "body=**[reply]** Fixed in abc123." in sent
    fake = FakeGh()
    assert pr_sync.main(["reply", "--dir", str(repo), "--thread", "T1", "--body", "Fixed, as Claude suggested."], runner=fake) == 1
    assert "principle 9" in capsys.readouterr().out
    assert not any("addPullRequestReviewThreadReply" in " ".join(c[0]) for c in fake.calls)


def test_cli_review_file(repo, tmp_path, capsys):
    review = tmp_path / "review.json"
    review.write_text(json.dumps({"body": "Looks good apart from two points.", "comments": [
        {"path": "app/share.py", "line": 10, "level": "blocking", "body": "No test for an empty album."},
        {"path": "app/share.py", "line": 12, "level": "nit", "body": "Name it album_id."}]}), encoding="utf-8")
    fake = FakeGh()
    assert pr_sync.main(["review", "--dir", str(repo), "--file", str(review)], runner=fake) == 0
    payload = json.loads([c for c in fake.calls if "pulls/12/reviews" in " ".join(c[0])][0][1])
    assert payload["body"] == "**[review]** Looks good apart from two points."
    assert [c["body"] for c in payload["comments"]] == ["**[review]** `blocking` No test for an empty album.",
                                                       "**[review]** `nit` Name it album_id."]
    review.write_text(json.dumps({"comments": [{"path": "a", "line": 1, "level": "critical", "body": "x"}]}), encoding="utf-8")
    assert pr_sync.main(["review", "--dir", str(repo), "--file", str(review)], runner=FakeGh()) == 1
    assert "level 'critical'" in capsys.readouterr().out


def test_cli_without_a_known_platform(tmp_path, capsys):
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    assert pr_sync.main(["status", "--dir", str(tmp_path)], runner=FakeGh()) == 1
    assert "platform is unknown" in capsys.readouterr().out


def test_cli_no_open_pull_request(repo, capsys):
    class Empty(FakeGh):
        def __call__(self, cmd, input_text=None):
            if cmd[:3] == ["gh", "pr", "list"]:
                return 0, "[]", ""
            return super().__call__(cmd, input_text)
    assert pr_sync.main(["status", "--dir", str(repo)], runner=Empty()) == 1
    assert "no open pull request for branch 'feat/012-sharing'" in capsys.readouterr().out
