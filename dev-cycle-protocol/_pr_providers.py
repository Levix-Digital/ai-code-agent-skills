"""
Pull request providers - dev-cycle-protocol Skill
Read and write pull requests (merge requests on GitLab) through the platform CLIs, which keep
the authentication: `gh` for GitHub, `glab` for GitLab, both on the platform's REST API so they
work the same on a local machine, a self-hosted instance or a cloud agent environment. Each provider returns the model of
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
# Only the GraphQL API says whether a review thread is resolved and lets one be resolved. Everything
# else goes through the REST API, which works wherever `gh` does (some environments refuse GraphQL).
GRAPHQL_ONLY = ("whether a review thread is resolved can only be read through GitHub's GraphQL API, "
                "which was refused here")


class ProviderError(RuntimeError):
    """A platform call that failed; the message says what to do (log in, install the CLI, ...)."""


def decode_pages(out: str, what: str):
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
        raise ProviderError(f"{what} returned no JSON: {out[:200]}")
    if not values:
        return None
    if len(values) == 1:
        return values[0]
    return [x for page in values for x in (page if isinstance(page, list) else [page])]


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

def _gh_check_run(run_: dict) -> dict:
    if (run_.get("status") or "").lower() != "completed":
        status = "pending"
    else:
        conclusion = (run_.get("conclusion") or "").lower()
        status = ("success" if conclusion in ("success", "neutral") else
                  "skipped" if conclusion == "skipped" else "failure")
    return {"name": run_.get("name"), "status": status, "url": run_.get("details_url") or run_.get("html_url")}


def _gh_status(item: dict) -> dict:
    status = {"success": "success", "pending": "pending"}.get((item.get("state") or "").lower(), "failure")
    return {"name": item.get("context"), "status": status, "url": item.get("target_url")}


def _login(user) -> str:
    return (user or {}).get("login") if isinstance(user, dict) else user


def _rest_threads(review_comments: list) -> list:
    """Review threads from the REST review comments: a thread is a top comment and its replies.
    REST does not say whether a thread is resolved: `resolved` is None (unknown)."""
    threads, by_root = [], {}
    for c in sorted(review_comments or [], key=lambda c: c.get("created_at") or ""):
        root = c.get("in_reply_to_id")
        comment = {"id": str(c.get("id")), "author": _login(c.get("user")), "body": c.get("body"),
                   "created": c.get("created_at")}
        if root and root in by_root:
            by_root[root]["comments"].append(comment)
            continue
        thread = {"id": str(c.get("id")), "path": c.get("path"), "line": c.get("line") or c.get("original_line"),
                  "resolved": None, "outdated": c.get("line") is None, "comments": [comment]}
        by_root[c.get("id")] = thread
        threads.append(thread)
    return threads


class GitHubProvider:
    """Pull requests through `gh api` (github.com or GitHub Enterprise Server): the REST API for
    everything it covers, the GraphQL API only for review thread resolution."""
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
        return decode_pages(self._gh(args, input_text), f"gh {' '.join(args[:2])}")

    def _rest(self, endpoint: str, method: str = None, payload: dict = None, paginate: bool = False):
        args = ["api", endpoint]
        if method:
            args += ["--method", method]
        if paginate:
            args.append("--paginate")
        if payload is not None:
            args += ["--input", "-"]
        return self._json(args, json.dumps(payload) if payload is not None else None)

    def _graphql(self, query: str, **variables):
        args = ["api", "graphql", "-f", f"query={query}"]
        for key, value in variables.items():
            args += ["-F" if isinstance(value, int) else "-f", f"{key}={value}"]
        return self._json(args)

    def me(self) -> str:
        return (self._gh(["api", "user", "--jq", ".login"]) or "").strip()

    def pull_request(self, number=None, branch: str = None) -> dict:
        """The pull request `number`, or the open one of `branch`; None when there is none."""
        base = f"repos/{self.path}/pulls"
        if number is None:
            head = urllib.parse.quote(f"{self.owner}:{branch or ''}", safe=":")
            found = self._rest(f"{base}?state=open&head={head}&per_page=1") or []
            if not found:
                return None
            number = found[0]["number"]
        number = int(number)
        data = self._rest(f"{base}/{number}") or {}
        sha = (data.get("head") or {}).get("sha")
        reviews = self._rest(f"{base}/{number}/reviews?per_page=100", paginate=True) or []
        comments = self._rest(f"repos/{self.path}/issues/{number}/comments?per_page=100", paginate=True) or []
        checks = []
        if sha:
            pages = self._rest(f"repos/{self.path}/commits/{sha}/check-runs?per_page=100", paginate=True) or []
            for page in pages if isinstance(pages, list) else [pages]:
                checks += [_gh_check_run(r) for r in page.get("check_runs") or []]
            status = self._rest(f"repos/{self.path}/commits/{sha}/status") or {}
            checks += [_gh_status(s) for s in status.get("statuses") or []]
        pr = self._model(data, reviews, comments, checks)
        pr["threads"], pr["threads_note"] = self._threads(number)
        return pr

    def _threads(self, number: int) -> tuple:
        """(threads, note): from GraphQL with their resolution; where GraphQL is refused, from the
        REST review comments with the resolution unknown and a note that says so."""
        try:
            data = self._graphql(GITHUB_THREADS_QUERY, owner=self.owner, repo=self.repo, number=number)
        except ProviderError as e:
            rest = self._rest(f"repos/{self.path}/pulls/{number}/comments?per_page=100", paginate=True) or []
            return _rest_threads(rest), f"{GRAPHQL_ONLY} ({str(e)[:160]})"
        nodes = (((((data or {}).get("data") or {}).get("repository") or {}).get("pullRequest") or {})
                 .get("reviewThreads") or {}).get("nodes") or []
        return [{"id": t.get("id"), "path": t.get("path"), "line": t.get("line") or t.get("originalLine"),
                 "resolved": bool(t.get("isResolved")), "outdated": bool(t.get("isOutdated")),
                 "comments": [{"id": c.get("id"), "author": _login(c.get("author")), "body": c.get("body"),
                               "created": c.get("createdAt")} for c in (t.get("comments") or {}).get("nodes") or []]}
                for t in nodes], None

    def _model(self, data: dict, reviews: list, comments: list, checks: list) -> dict:
        state = "merged" if data.get("merged") or data.get("merged_at") else (data.get("state") or "").lower()
        mergeable = ("conflict" if data.get("mergeable_state") == "dirty" or data.get("mergeable") is False else
                     "clean" if data.get("mergeable") else "unknown")
        review_state = {"APPROVED": "approved", "CHANGES_REQUESTED": "changes_requested",
                        "COMMENTED": "commented", "DISMISSED": "dismissed"}
        return {
            "provider": "github", "number": data.get("number"), "url": data.get("html_url"), "title": data.get("title"),
            "state": state, "draft": bool(data.get("draft")),
            "author": _login(data.get("user")), "base": (data.get("base") or {}).get("ref"),
            "head": (data.get("head") or {}).get("ref"), "mergeable": mergeable, "checks": checks,
            "reviews": [{"author": _login(r.get("user")), "state": review_state.get(r.get("state"), "commented")}
                        for r in reviews],
            "threads": [],
            "comments": [{"id": c.get("id"), "author": _login(c.get("user")), "body": c.get("body"),
                          "created": c.get("created_at")} for c in comments],
        }

    def open_pull_requests(self, limit: int = 10) -> list:
        found = self._rest(f"repos/{self.path}/pulls?state=open&per_page={int(limit)}") or []
        return [self.pull_request(item["number"]) for item in found[:limit]]

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
        if str(thread_id).isdigit():
            self._rest(f"repos/{self.path}/pulls/{int(number)}/comments/{thread_id}/replies", "POST", {"body": body})
            return
        self._graphql(GITHUB_REPLY_MUTATION, thread=thread_id, body=body)

    def resolve(self, number: int, thread_id: str):
        if str(thread_id).isdigit():
            raise ProviderError("resolving a review thread needs GitHub's GraphQL API, which was refused here: "
                                "resolve it on the pull request page (or with another GitHub tool you have); "
                                "the merge gate keeps blocking findings open until it can read them as resolved")
        self._graphql(GITHUB_RESOLVE_MUTATION, thread=thread_id)

    def comment(self, number: int, body: str):
        self._rest(f"repos/{self.path}/issues/{int(number)}/comments", "POST", {"body": body})

    def review(self, number: int, body: str, comments: list):
        payload = {"event": "COMMENT", "body": body,
                   "comments": [{"path": c["path"], "line": int(c["line"]), "side": "RIGHT", "body": c["body"]}
                                for c in comments]}
        self._rest(f"repos/{self.path}/pulls/{int(number)}/reviews", "POST", payload)

    def request_review(self, number: int, reviewers: list):
        self._rest(f"repos/{self.path}/pulls/{int(number)}/requested_reviewers", "POST", {"reviewers": list(reviewers)})


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
        return decode_pages(out, f"glab api {endpoint}")

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
