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

    resolution_setting = "Settings > Branches (or Rules): Require conversation resolution before merging"

    def requires_thread_resolution(self, branch: str):
        """Whether merging into `branch` needs every conversation resolved (a ruleset or the classic
        branch protection); None when the platform does not let it be read."""
        try:
            rules = self._json(["api", f"repos/{self.path}/rules/branches/{branch}"]) or []
        except ProviderError:
            rules = None
        if any(r.get("type") == "pull_request" and (r.get("parameters") or {}).get("required_review_thread_resolution")
               for r in rules or []):
            return True
        try:
            protection = self._json(["api", f"repos/{self.path}/branches/{branch}/protection"]) or {}
        except ProviderError as e:
            return False if rules is not None and "Branch not protected" in str(e) else None
        return bool((protection.get("required_conversation_resolution") or {}).get("enabled"))

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


# ---------------------------------------------------------------- GitLab

GITLAB_PIPELINE = {"success": "success", "failed": "failure", "canceled": "failure", "skipped": "skipped"}


class GitLabProvider:
    """Merge requests through `glab api` (gitlab.com or a self-hosted instance)."""
    name = "gitlab"
    cli = "glab"

    def __init__(self, path: str, host: str = "gitlab.com", runner=None):
        self.path, self.host, self.runner = path, host or "gitlab.com", runner or run
        self.project = f"projects/{quote_path(path or '')}"

    def _api(self, endpoint: str, method: str = None, payload: dict = None, paginate: bool = False):
        cmd = ["glab", "api", endpoint]
        if method:
            cmd += ["--method", method]
        if paginate:
            cmd.append("--paginate")
        if self.host != "gitlab.com":
            cmd += ["--hostname", self.host]
        if payload is not None:
            cmd += ["--input", "-", "-H", "Content-Type: application/json"]
        code, out, err = self.runner(cmd, json.dumps(payload) if payload is not None else None)
        if code == 127:
            raise ProviderError("the GitLab CLI is not installed: https://gitlab.com/gitlab-org/cli")
        if code != 0:
            text = (err or out).strip()
            if "401" in text or "auth login" in text or "unauthorized" in text.lower():
                raise ProviderError(f"the GitLab CLI is not logged in: run 'glab auth login --hostname {self.host}' ({text[:200]})")
            raise ProviderError(f"glab api {endpoint} failed: {text[:500]}")
        return self._decode(endpoint, out)

    @staticmethod
    def _decode(endpoint: str, out: str):
        """One JSON value, or the pages `--paginate` prints one after the other (arrays are joined)."""
        decoder, values, i, text = json.JSONDecoder(), [], 0, out.strip()
        try:
            while i < len(text):
                value, end = decoder.raw_decode(text, i)
                values.append(value)
                i = end
                while i < len(text) and text[i].isspace():
                    i += 1
        except ValueError:
            raise ProviderError(f"glab api {endpoint} returned no JSON: {out[:200]}")
        if not values:
            return None
        if len(values) == 1:
            return values[0]
        return [x for page in values for x in (page if isinstance(page, list) else [page])]

    def me(self) -> str:
        return (self._api("user") or {}).get("username", "")

    def pull_request(self, number=None, branch: str = None) -> dict:
        if number is None:
            found = self._api(f"{self.project}/merge_requests?state=opened&source_branch={urllib.parse.quote(branch or '', safe='')}") or []
            if not found:
                return None
            number = found[0]["iid"]
        base = f"{self.project}/merge_requests/{int(number)}"
        mr = self._api(base)
        discussions = self._api(f"{base}/discussions?per_page=100", paginate=True) or []
        approvals = self._api(f"{base}/approvals") or {}
        return self._model(mr, discussions, approvals)

    def _model(self, mr: dict, discussions: list, approvals: dict) -> dict:
        threads, comments = [], []
        for d in discussions:
            notes = [n for n in d.get("notes") or [] if not n.get("system")]
            if not notes:
                continue
            converted = [{"id": n.get("id"), "author": (n.get("author") or {}).get("username"), "body": n.get("body"),
                          "created": n.get("created_at")} for n in notes]
            first = notes[0]
            if d.get("individual_note") and not first.get("resolvable"):
                comments += converted
                continue
            position = first.get("position") or {}
            threads.append({"id": d.get("id"), "path": position.get("new_path") or position.get("old_path"),
                            "line": position.get("new_line") or position.get("old_line"),
                            "resolved": all(n.get("resolved") for n in notes if n.get("resolvable")),
                            "outdated": False, "comments": converted})
        pipeline = mr.get("head_pipeline") or {}
        checks = []
        if pipeline:
            checks.append({"name": "pipeline", "status": GITLAB_PIPELINE.get(pipeline.get("status"), "pending"),
                           "url": pipeline.get("web_url")})
        state = {"opened": "open", "locked": "open"}.get(mr.get("state"), mr.get("state"))
        conflict = mr.get("has_conflicts") or mr.get("detailed_merge_status") == "conflict"
        reviews = [{"author": (a.get("user") or {}).get("username"), "state": "approved"}
                   for a in approvals.get("approved_by") or []]
        return {
            "provider": "gitlab", "number": mr.get("iid"), "url": mr.get("web_url"), "title": mr.get("title"),
            "state": state, "draft": bool(mr.get("draft") or mr.get("work_in_progress")),
            "author": (mr.get("author") or {}).get("username"), "base": mr.get("target_branch"),
            "head": mr.get("source_branch"), "mergeable": "conflict" if conflict else "clean",
            "checks": checks, "reviews": reviews, "threads": threads, "comments": comments,
            "diff_refs": mr.get("diff_refs") or {},
        }

    def open_pull_requests(self, limit: int = 10) -> list:
        found = self._api(f"{self.project}/merge_requests?state=opened&per_page={int(limit)}") or []
        return [self.pull_request(item["iid"]) for item in found[:limit]]

    resolution_setting = "Settings > Merge requests: All threads must be resolved"

    def requires_thread_resolution(self, branch: str):
        """Whether merge requests need every thread resolved (a project setting on GitLab); None when
        it cannot be read."""
        try:
            value = (self._api(self.project) or {}).get("only_allow_merge_if_all_discussions_are_resolved")
        except ProviderError:
            return None
        return None if value is None else bool(value)

    # ------------------------------------------------------------ writes

    def reply(self, number: int, thread_id: str, body: str):
        self._api(f"{self.project}/merge_requests/{int(number)}/discussions/{thread_id}/notes", "POST", {"body": body})

    def resolve(self, number: int, thread_id: str):
        self._api(f"{self.project}/merge_requests/{int(number)}/discussions/{thread_id}", "PUT", {"resolved": True})

    def comment(self, number: int, body: str):
        self._api(f"{self.project}/merge_requests/{int(number)}/notes", "POST", {"body": body})

    def review(self, number: int, body: str, comments: list):
        base = f"{self.project}/merge_requests/{int(number)}"
        refs = (self._api(base) or {}).get("diff_refs") or {}
        for c in comments:
            position = {"position_type": "text", "base_sha": refs.get("base_sha"), "start_sha": refs.get("start_sha"),
                        "head_sha": refs.get("head_sha"), "new_path": c["path"], "new_line": int(c["line"])}
            self._api(f"{base}/discussions", "POST", {"body": c["body"], "position": position})
        self._api(f"{base}/notes", "POST", {"body": body})

    def request_review(self, number: int, reviewers: list):
        base = f"{self.project}/merge_requests/{int(number)}"
        ids = [r.get("id") for r in (self._api(base) or {}).get("reviewers") or []]
        for login in reviewers:
            users = self._api(f"users?username={urllib.parse.quote(login, safe='')}") or []
            if not users:
                raise ProviderError(f"GitLab user '{login}' not found")
            if users[0]["id"] not in ids:
                ids.append(users[0]["id"])
        self._api(base, "PUT", {"reviewer_ids": ids})


# ---------------------------------------------------------------- factory

def provider_for(provider: str, host: str, path: str, runner=None):
    if provider == "github":
        return GitHubProvider(path, host, runner)
    if provider == "gitlab":
        return GitLabProvider(path, host, runner)
    raise ProviderError(f"pull request platform '{provider}' is not supported yet (GitHub and GitLab are)")


def cli_available(provider: str) -> bool:
    return bool(shutil.which({"github": "gh", "gitlab": "glab"}.get(provider, "")))


def quote_path(path: str) -> str:
    return urllib.parse.quote(path, safe="")
