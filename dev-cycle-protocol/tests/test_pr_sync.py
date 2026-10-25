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


# ---------------------------------------------------------------- GitLab

from _pr_providers import GitLabProvider  # noqa: E402

GL_MR = {"iid": 7, "web_url": "https://gitlab.acme.io/team/photos/-/merge_requests/7", "title": "Albums", "state": "opened",
         "draft": False, "author": {"username": "levi"}, "target_branch": "dev", "source_branch": "feat/007-albums",
         "has_conflicts": False, "detailed_merge_status": "mergeable", "head_pipeline": {"status": "failed", "web_url": "https://ci/p/1"},
         "diff_refs": {"base_sha": "b", "start_sha": "s", "head_sha": "h"}, "reviewers": [{"id": 3}]}


def note(nid, author, body, resolvable=False, resolved=False, system=False, position=None):
    n = {"id": nid, "author": {"username": author}, "body": body, "created_at": f"2026-10-23T1{nid % 10}:00:00Z",
         "system": system, "resolvable": resolvable, "resolved": resolved}
    if position:
        n["position"] = position
    return n


GL_DISCUSSIONS_PAGE_1 = [
    {"id": "d1", "individual_note": False, "notes": [note(1, "ana", "nit: typo", True, False, position={"new_path": "a.py", "new_line": 4})]},
    {"id": "d2", "individual_note": False, "notes": [note(2, "ana", "Why?", True, True, position={"new_path": "a.py", "new_line": 9})]},
]
GL_DISCUSSIONS_PAGE_2 = [
    {"id": "d3", "individual_note": True, "notes": [note(3, "ana", "Nice work, ship it after the fix")]},
    {"id": "d4", "individual_note": True, "notes": [note(4, "ana", "approved this merge request", system=True)]},
]


class FakeGlab:
    def __init__(self):
        self.calls = []

    def __call__(self, cmd, input_text=None):
        self.calls.append((cmd, input_text))
        endpoint = cmd[2]
        if "merge_requests?state=opened&source_branch=" in endpoint:
            return 0, json.dumps([{"iid": 7}]), ""
        if endpoint.endswith("/discussions?per_page=100"):
            return 0, json.dumps(GL_DISCUSSIONS_PAGE_1) + "\n" + json.dumps(GL_DISCUSSIONS_PAGE_2), ""
        if endpoint.endswith("/approvals"):
            return 0, json.dumps({"approved_by": [{"user": {"username": "bob"}}]}), ""
        if endpoint.startswith("users?username="):
            return 0, json.dumps([{"id": 9, "username": "ana"}]), ""
        if endpoint == "user":
            return 0, json.dumps({"username": "levi"}), ""
        if endpoint.endswith("merge_requests/7") and "--method" not in cmd:
            return 0, json.dumps(GL_MR), ""
        return 0, "{}", ""


def test_gitlab_model_and_pending():
    fake = FakeGlab()
    gl = GitLabProvider("team/photos", "gitlab.acme.io", fake)
    pr = gl.pull_request(None, "feat/007-albums")
    assert fake.calls[0][0][2] == "projects/team%2Fphotos/merge_requests?state=opened&source_branch=feat%2F007-albums"
    assert "--hostname" in fake.calls[0][0] and "gitlab.acme.io" in fake.calls[0][0]
    assert pr["provider"] == "gitlab" and pr["number"] == 7 and pr["state"] == "open" and pr["mergeable"] == "clean"
    assert pr["checks"] == [{"name": "pipeline", "status": "failure", "url": "https://ci/p/1"}]
    assert [t["id"] for t in pr["threads"]] == ["d1", "d2"] and pr["threads"][1]["resolved"]
    assert [c["id"] for c in pr["comments"]] == [3]                    # the system note is left out
    p = pending(pr)
    assert [t["id"] for t in p["threads"]] == ["d1"] and p["approved"] == ["bob"] and p["failed_checks"]
    assert summary_line(pr, p).startswith("MR !7 Albums: 1 thread waiting")
    assert gl.me() == "levi"


def test_gitlab_writes():
    fake = FakeGlab()
    gl = GitLabProvider("team/photos", runner=fake)
    gl.reply(7, "d1", "**[reply]** Fixed.")
    gl.resolve(7, "d1")
    gl.comment(7, "**[reply]** Thanks.")
    gl.review(7, "**[review]** Summary", [{"path": "a.py", "line": 4, "body": "**[review]** `nit` x"}])
    gl.request_review(7, ["ana"])
    calls = [(c[0][2], c[0][c[0].index("--method") + 1] if "--method" in c[0] else "GET", c[1]) for c in fake.calls]
    assert calls[0][:2] == ("projects/team%2Fphotos/merge_requests/7/discussions/d1/notes", "POST")
    assert calls[1][:2] == ("projects/team%2Fphotos/merge_requests/7/discussions/d1", "PUT") and json.loads(calls[1][2]) == {"resolved": True}
    assert calls[2][:2] == ("projects/team%2Fphotos/merge_requests/7/notes", "POST")
    position = json.loads(calls[4][2])["position"]
    assert position == {"position_type": "text", "base_sha": "b", "start_sha": "s", "head_sha": "h", "new_path": "a.py", "new_line": 4}
    assert calls[5][:2] == ("projects/team%2Fphotos/merge_requests/7/notes", "POST")
    assert json.loads(calls[-1][2]) == {"reviewer_ids": [3, 9]}


def test_gitlab_errors():
    with pytest.raises(ProviderError, match="not installed"):
        GitLabProvider("a/b", runner=lambda c, i=None: (127, "", "")).me()
    with pytest.raises(ProviderError, match="glab auth login --hostname gitlab.com"):
        GitLabProvider("a/b", runner=lambda c, i=None: (1, "", "401 Unauthorized")).me()


def test_cli_on_gitlab(tmp_path, capsys):
    subprocess.run(["git", "init", "-q", "-b", "feat/007-albums"], cwd=tmp_path, check=True)
    subprocess.run(["git", "remote", "add", "origin", "git@gitlab.acme.io:team/photos.git"], cwd=tmp_path, check=True)
    assert pr_sync.main(["status", "--dir", str(tmp_path)], runner=FakeGlab()) == 0
    assert "MR !7 Albums" in capsys.readouterr().out


# ---------------------------------------------------------------- merge gate

from _pr_model import merge_blockers  # noqa: E402


def test_merge_blockers():
    blockers = merge_blockers(model())
    assert "blocking finding still open at app/db.py:7 (thread T6)" in blockers
    assert "changes requested by ana" in blockers
    assert "check tests failed" in blockers and "check lint still running" in blockers
    assert "merge conflict with the base branch" in blockers
    assert not any("app/share.py" in b for b in blockers)              # nits and suggestions never hold the merge


def test_a_reply_does_not_release_a_blocking_finding():
    pr = {"state": "open", "threads": [
        {"id": "T6", "path": "a.py", "line": 1, "resolved": False,
         "comments": [{"body": "**[review]** `blocking` No test."}, {"body": "**[reply]** Added in abc."}]},
        {"id": "T7", "path": "a.py", "line": 2, "resolved": True, "comments": [{"body": "**[review]** `blocking` x"}]}]}
    assert merge_blockers(pr) == ["blocking finding still open at a.py:1 (thread T6)"]
    assert merge_blockers({**pr, "threads": pr["threads"][1:]}) == []
    assert merge_blockers({**pr, "draft": True, "threads": []}) == ["the pull request is a draft"]


CLEAN_VIEW = {**GH_VIEW, "mergeable": "MERGEABLE", "reviews": [], "statusCheckRollup": [
    {"__typename": "CheckRun", "name": "tests", "status": "COMPLETED", "conclusion": "SUCCESS", "detailsUrl": "https://ci/1"}]}
CLEAN_THREADS = {"data": {"repository": {"pullRequest": {"reviewThreads": {"nodes": [
    thread("T6", "app/db.py", 7, [("levi", "**[review]** `blocking` Missing regression test.")], resolved=True)]}}}}}


class ProtectedGh(FakeGh):
    def __init__(self, rules="[]", protection=(0, "{}", ""), **kw):
        super().__init__(**kw)
        self.rules, self.protection = rules, protection

    def __call__(self, cmd, input_text=None):
        if cmd[:2] == ["gh", "api"] and cmd[2].startswith("repos/acme/photos/rules/branches/"):
            return (0, self.rules, "") if self.rules is not None else (1, "", "gh: Not Found (HTTP 404)")
        if cmd[:2] == ["gh", "api"] and cmd[2].endswith("/protection"):
            return self.protection
        return super().__call__(cmd, input_text)


def test_cli_gate_merge(repo, capsys):
    assert pr_sync.main(["gate-merge", "--dir", str(repo)], runner=ProtectedGh()) == 1
    out = capsys.readouterr().out
    assert "❌ blocking finding still open at app/db.py:7" in out and "not ready to merge" in out
    on = (0, json.dumps({"required_conversation_resolution": {"enabled": True}}), "")
    fake = ProtectedGh(protection=on, view=CLEAN_VIEW, threads=CLEAN_THREADS)
    assert pr_sync.main(["gate-merge", "--dir", str(repo)], runner=fake) == 0
    out = capsys.readouterr().out
    assert "✅ ready to merge" in out and "⚠️" not in out and "ℹ️" not in out


def test_gate_merge_reports_the_conversation_resolution_setting(repo, capsys):
    clean = {"view": CLEAN_VIEW, "threads": CLEAN_THREADS}
    unprotected = (1, "", "gh: Branch not protected (HTTP 404)")
    assert pr_sync.main(["gate-merge", "--dir", str(repo)], runner=ProtectedGh(protection=unprotected, **clean)) == 0
    assert "does not require resolved conversations" in capsys.readouterr().out
    ruleset = json.dumps([{"type": "pull_request", "parameters": {"required_review_thread_resolution": True}}])
    assert pr_sync.main(["gate-merge", "--dir", str(repo)], runner=ProtectedGh(rules=ruleset, protection=unprotected, **clean)) == 0
    assert "⚠️" not in capsys.readouterr().out
    forbidden = (1, "", "gh: Resource not accessible by integration (HTTP 403)")
    assert pr_sync.main(["gate-merge", "--dir", str(repo)], runner=ProtectedGh(rules=None, protection=forbidden, **clean)) == 0
    assert "could not read whether" in capsys.readouterr().out


def test_gitlab_thread_resolution_setting():
    def runner(project):
        def run(cmd, input_text=None):
            return (0, json.dumps(project), "") if cmd[2] == "projects/team%2Fphotos" else (0, "{}", "")
        return run
    assert GitLabProvider("team/photos", runner=runner({"only_allow_merge_if_all_discussions_are_resolved": True})).requires_thread_resolution("dev")
    assert GitLabProvider("team/photos", runner=runner({"only_allow_merge_if_all_discussions_are_resolved": False})).requires_thread_resolution("dev") is False
    assert GitLabProvider("team/photos", runner=lambda c, i=None: (1, "", "403 Forbidden")).requires_thread_resolution("dev") is None
