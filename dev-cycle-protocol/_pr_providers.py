"""
Pull request providers - dev-cycle-protocol Skill
Read and write pull requests (merge requests on GitLab) through the platform CLIs, which keep
the authentication: `gh` for GitHub, `glab` for GitLab. Each provider returns the model of
_pr_model.py. Commands go through an injectable runner, so the tests replay recorded responses.

    runner(cmd: list, input_text: str = None) -> (exit_code, stdout, stderr)
"""
import json
import re
import shutil
import subprocess
import urllib.parse

GITHUB_THREADS_QUERY = """
query($owner: String!, $repo: String!, $number: Int!) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $number) {
      reviewThreads(first: 100) {
        nodes {
          id isResolved isOutdated path line originalLine
          comments(first: 100) { nodes { id author { login } body createdAt } }
        }
      }
    }
  }
}
"""
GITHUB_REPLY_MUTATION = """
mutation($thread: ID!, $body: String!) {
  addPullRequestReviewThreadReply(input: {pullRequestReviewThreadId: $thread, body: $body}) { comment { id } }
}
"""
GITHUB_RESOLVE_MUTATION = """
mutation($thread: ID!) { resolveReviewThread(input: {threadId: $thread}) { thread { id isResolved } } }
"""
GITHUB_PR_FIELDS = ("number,url,title,state,isDraft,author,baseRefName,headRefName,mergeable,"
                    "reviews,comments,statusCheckRollup")


class ProviderError(RuntimeError):
    """A platform call that failed; the message says what to do (log in, install the CLI, ...)."""


def run(cmd: list, input_text: str = None) -> tuple:
    try:
        res = subprocess.run(cmd, input=input_text, capture_output=True, text=True, encoding="utf-8", errors="replace")
        return res.returncode, res.stdout, res.stderr
    except FileNotFoundError:
        return 127, "", f"{cmd[0]}: not found"


def parse_remote(url: str) -> dict:
    """{host, path} of a Git remote URL (https or ssh), or {} when it cannot be read."""
    url = (url or "").strip()
    m = re.match(r"^(?:ssh://)?git@([^:/]+)[:/](.+?)(?:\.git)?/?$", url)
    if not m:
        m = re.match(r"^https?://(?:[^@/]+@)?([^/]+)/(.+?)(?:\.git)?/?$", url)
    if not m:
        return {}
    host, path = m.group(1).lower(), m.group(2)
    return {"host": host.split(":")[0] if host.count(":") == 1 and not host.startswith("[") else host, "path": path}


def detect_provider(remote_url: str, configured: str = None, configured_host: str = None) -> tuple:
    """(provider, host, path) from the configuration or the remote; provider None when unknown."""
    remote = parse_remote(remote_url)
    host = configured_host or remote.get("host")
    provider = configured if configured and configured != "auto" else None
    if not provider and host:
        if host == "github.com" or host.endswith(".github.com") or "github" in host:
            provider = "github"
        elif "gitlab" in host:
            provider = "gitlab"
    return provider, host, remote.get("path")


# ---------------------------------------------------------------- GitHub

def _gh_check(item: dict) -> dict:
    if item.get("__typename") == "StatusContext" or "context" in item:
        state = (item.get("state") or "").upper()
        status = {"SUCCESS": "success", "PENDING": "pending", "EXPECTED": "pending"}.get(state, "failure")
        return {"name": item.get("context"), "status": status, "url": item.get("targetUrl")}
    if (item.get("status") or "").upper() != "COMPLETED":
        status = "pending"
    else:
        conclusion = (item.get("conclusion") or "").upper()
        status = ("success" if conclusion in ("SUCCESS", "NEUTRAL") else
                  "skipped" if conclusion == "SKIPPED" else "failure")
    return {"name": item.get("name"), "status": status, "url": item.get("detailsUrl")}


def _login(user) -> str:
    return (user or {}).get("login") if isinstance(user, dict) else user


class GitHubProvider:
    name = "github"
    cli = "gh"

    def __init__(self, path: str, host: str = "github.com", runner=None):
        self.path, self.host, self.runner = path, host or "github.com", runner or run
        self.owner, _, self.repo = (path or "").partition("/")

    def _gh(self, args: list, input_text: str = None) -> str:
        cmd = ["gh"] + args
        if self.host != "github.com":
            cmd += ["--hostname", self.host] if args and args[0] == "api" else []
        code, out, err = self.runner(cmd, input_text)
        if code == 127:
            raise ProviderError("the GitHub CLI is not installed: https://cli.github.com/")
        if code != 0:
            text = (err or out).strip()
            if "auth login" in text or "not logged" in text.lower() or "authentication" in text.lower():
                raise ProviderError(f"the GitHub CLI is not logged in: run 'gh auth login' ({text[:200]})")
            raise ProviderError(f"gh {' '.join(args[:3])} failed: {text[:500]}")
        return out

    def _json(self, args: list, input_text: str = None):
        out = self._gh(args, input_text)
        try:
            return json.loads(out) if out.strip() else None
        except ValueError:
            raise ProviderError(f"gh {' '.join(args[:3])} returned no JSON: {out[:200]}")

    def me(self) -> str:
        return (self._gh(["api", "user", "--jq", ".login"]) or "").strip()

    def pull_request(self, number=None, branch: str = None) -> dict:
        """The pull request `number`, or the open one of `branch`; None when there is none."""
        repo = ["--repo", f"{self.host}/{self.path}" if self.host != "github.com" else self.path]
        if number is None:
            found = self._json(["pr", "list", "--head", branch, "--state", "open", "--json", "number"] + repo) or []
            if not found:
                return None
            number = found[0]["number"]
        data = self._json(["pr", "view", str(number), "--json", GITHUB_PR_FIELDS] + repo)
        threads = self._json(["api", "graphql", "-f", f"query={GITHUB_THREADS_QUERY}", "-F", f"owner={self.owner}",
                              "-F", f"repo={self.repo}", "-F", f"number={int(number)}"])
        return self._model(data, threads)

    def _model(self, data: dict, threads: dict) -> dict:
        nodes = (((((threads or {}).get("data") or {}).get("repository") or {}).get("pullRequest") or {})
                 .get("reviewThreads") or {}).get("nodes") or []
        state = (data.get("state") or "").lower()
        mergeable = {"MERGEABLE": "clean", "CONFLICTING": "conflict"}.get(data.get("mergeable"), "unknown")
        review_state = {"APPROVED": "approved", "CHANGES_REQUESTED": "changes_requested",
                        "COMMENTED": "commented", "DISMISSED": "dismissed"}
        return {
            "provider": "github", "number": data.get("number"), "url": data.get("url"), "title": data.get("title"),
            "state": "open" if state == "open" else state, "draft": bool(data.get("isDraft")),
            "author": _login(data.get("author")), "base": data.get("baseRefName"), "head": data.get("headRefName"),
            "mergeable": mergeable,
            "checks": [_gh_check(c) for c in data.get("statusCheckRollup") or []],
            "reviews": [{"author": _login(r.get("author")), "state": review_state.get(r.get("state"), "commented")}
                        for r in data.get("reviews") or []],
            "threads": [{"id": t.get("id"), "path": t.get("path"), "line": t.get("line") or t.get("originalLine"),
                         "resolved": bool(t.get("isResolved")), "outdated": bool(t.get("isOutdated")),
                         "comments": [{"id": c.get("id"), "author": _login(c.get("author")), "body": c.get("body"),
                                       "created": c.get("createdAt")} for c in (t.get("comments") or {}).get("nodes") or []]}
                        for t in nodes],
            "comments": [{"id": c.get("id"), "author": _login(c.get("author")), "body": c.get("body"),
                          "created": c.get("createdAt")} for c in data.get("comments") or []],
        }

    def open_pull_requests(self, limit: int = 10) -> list:
        repo = ["--repo", f"{self.host}/{self.path}" if self.host != "github.com" else self.path]
        found = self._json(["pr", "list", "--state", "open", "--limit", str(limit), "--json", "number"] + repo) or []
        return [self.pull_request(item["number"]) for item in found]

    # ------------------------------------------------------------ writes

    def reply(self, number: int, thread_id: str, body: str):
        self._json(["api", "graphql", "-f", f"query={GITHUB_REPLY_MUTATION}", "-F", f"thread={thread_id}",
                    "-f", f"body={body}"])

    def resolve(self, number: int, thread_id: str):
        self._json(["api", "graphql", "-f", f"query={GITHUB_RESOLVE_MUTATION}", "-F", f"thread={thread_id}"])

    def comment(self, number: int, body: str):
        self._json(["api", f"repos/{self.path}/issues/{int(number)}/comments", "--method", "POST", "--input", "-"],
                   json.dumps({"body": body}))

    def review(self, number: int, body: str, comments: list):
        payload = {"event": "COMMENT", "body": body,
                   "comments": [{"path": c["path"], "line": int(c["line"]), "side": "RIGHT", "body": c["body"]}
                                for c in comments]}
        self._json(["api", f"repos/{self.path}/pulls/{int(number)}/reviews", "--method", "POST", "--input", "-"],
                   json.dumps(payload))

    def request_review(self, number: int, reviewers: list):
        repo = ["--repo", f"{self.host}/{self.path}" if self.host != "github.com" else self.path]
        self._gh(["pr", "edit", str(number), "--add-reviewer", ",".join(reviewers)] + repo)


# ---------------------------------------------------------------- factory

def provider_for(provider: str, host: str, path: str, runner=None):
    if provider == "github":
        return GitHubProvider(path, host, runner)
    if provider == "gitlab":
        from _pr_gitlab import GitLabProvider
        return GitLabProvider(path, host, runner)
    raise ProviderError(f"pull request platform '{provider}' is not supported yet (GitHub and GitLab are)")


def cli_available(provider: str) -> bool:
    return bool(shutil.which({"github": "gh", "gitlab": "glab"}.get(provider, "")))


def quote_path(path: str) -> str:
    return urllib.parse.quote(path, safe="")
