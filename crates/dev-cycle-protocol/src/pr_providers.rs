//! Pull request providers (`_pr_providers.py`): read and write pull requests (merge requests on
//! GitLab) through the platform CLIs, which keep the authentication: `gh` for GitHub, `glab` for
//! GitLab, both on the platform's REST API so they work the same on a local machine, a
//! self-hosted instance or a cloud agent environment. Each provider returns the model of
//! `pr_model`.

use std::io::Write;
use std::process::{Command, Stdio};

use kernel::py::json::{LoadError, RawDecoder, dumps_ascii};
use kernel::py::num::{int_value, is_space, strip};
use kernel::py::re::Regex;
use kernel::py::subprocess::spawn_error;
use kernel::py::{Crash, Dict, Value, iterate, lower, universal_newlines};

use crate::pr_model::{
    Check, Comment, PullRequest, Review, Thread, attr_error, get, or, sort_keyed, text_or_empty,
    unhashable,
};

const GITHUB_THREADS_QUERY: &str = "
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
";
const GITHUB_REPLY_MUTATION: &str = "
mutation($thread: ID!, $body: String!) {
  addPullRequestReviewThreadReply(input: {pullRequestReviewThreadId: $thread, body: $body}) { comment { id } }
}
";
const GITHUB_RESOLVE_MUTATION: &str = "
mutation($thread: ID!) { resolveReviewThread(input: {threadId: $thread}) { thread { id isResolved } } }
";
// Only the GraphQL API says whether a review thread is resolved and lets one be resolved.
// Everything else goes through the REST API, which works wherever `gh` does.
const GRAPHQL_ONLY: &str = "whether a review thread is resolved can only be read through GitHub's GraphQL API, which was refused here";

/// A failed call: `Provider` is the script's `ProviderError` (the message says what to do).
#[derive(Debug)]
pub enum Fail {
    Provider(String),
    Crash(Crash),
}

impl From<Crash> for Fail {
    fn from(c: Crash) -> Self {
        Fail::Crash(c)
    }
}

type Res<T> = Result<T, Fail>;

fn s(text: &str) -> Value {
    Value::Str(text.to_string())
}

fn head(text: &str, n: usize) -> String {
    text.chars().take(n).collect()
}

/// `decode_pages(out, what)`: one JSON value, or the pages `--paginate` prints one after the
/// other (arrays are joined); `None` for no output.
pub fn decode_pages(out: &str, what: &str) -> Res<Value> {
    let text = strip(out);
    let decoder = RawDecoder::new(text);
    let len = decoder.chars().len();
    let mut values = Vec::new();
    let mut i = 0;
    while i < len {
        match decoder.decode(i) {
            Ok((value, end)) => {
                values.push(value);
                i = end;
                while i < len && is_space(decoder.chars()[i]) {
                    i += 1;
                }
            }
            Err(LoadError::Decode(_)) => {
                return Err(Fail::Provider(format!(
                    "{what} returned no JSON: {}",
                    head(out, 200)
                )));
            }
            Err(LoadError::Crash(c)) => return Err(Fail::Crash(c)),
        }
    }
    Ok(match values.len() {
        0 => Value::Null,
        1 => values.pop().unwrap_or(Value::Null),
        _ => Value::List(
            values
                .into_iter()
                .flat_map(|page| match page {
                    Value::List(items) => items,
                    other => vec![other],
                })
                .collect(),
        ),
    })
}

/// `run(cmd, input_text)`: (exit code, stdout, stderr); 127 when the program is not there.
fn run(cmd: &[Value], input: Option<&str>) -> Result<(i32, String, String), Crash> {
    let mut args = Vec::new();
    for a in cmd {
        match a {
            Value::Str(text) => args.push(text.clone()),
            other => {
                return Err(Crash::new(format!(
                    "TypeError: expected str, bytes or os.PathLike object, not {}",
                    other.type_name()
                )));
            }
        }
    }
    let mut command = Command::new(&args[0]);
    command
        .args(&args[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::inherit()
        });
    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) => {
            let (not_found, msg) = spawn_error(&e, &args[0]);
            if not_found {
                return Ok((127, String::new(), format!("{}: not found", args[0])));
            }
            return Err(Crash::new(msg));
        }
    };
    let writer = match (input, child.stdin.take()) {
        (Some(text), Some(mut pipe)) => {
            let bytes = text.as_bytes().to_vec();
            Some(std::thread::spawn(move || {
                let _ = pipe.write_all(&bytes);
            }))
        }
        _ => None,
    };
    let out = child
        .wait_with_output()
        .map_err(|e| Crash::new(format!("OSError: {e}")))?;
    if let Some(w) = writer {
        let _ = w.join();
    }
    let code = {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            match out.status.signal() {
                Some(sig) => -sig,
                None => out.status.code().unwrap_or(-1),
            }
        }
        #[cfg(not(unix))]
        {
            out.status.code().unwrap_or(-1)
        }
    };
    let text = |b: &[u8]| universal_newlines(&String::from_utf8_lossy(b));
    Ok((code, text(&out.stdout), text(&out.stderr)))
}

/// `urllib.parse.quote(text, safe=safe)`.
pub fn quote(text: &str, safe: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || "_.-~".contains(c) || (c.is_ascii() && safe.contains(c)) {
            out.push(c);
        } else {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

/// `int(value)` in an f-string: the decimal text.
fn int(v: &Value) -> Res<String> {
    Ok(int_value(v)?)
}

/// `value[0]` on a value known to be truthy.
fn first_item(v: &Value) -> Result<Value, Crash> {
    match v {
        Value::List(items) | Value::Tuple(items) => Ok(items[0].clone()),
        Value::Str(text) => Ok(s(&text.chars().next().unwrap_or_default().to_string())),
        Value::Dict(d) => d
            .get_value(&Value::int(0))
            .cloned()
            .ok_or_else(|| Crash::new("KeyError: 0")),
        other => Err(not_subscriptable(other)),
    }
}

fn not_subscriptable(v: &Value) -> Crash {
    Crash::new(format!(
        "TypeError: '{}' object is not subscriptable",
        v.type_name()
    ))
}

/// `value[key]` for a string key.
pub(crate) fn item(v: &Value, key: &str) -> Result<Value, Crash> {
    match v {
        Value::Dict(d) => d
            .get(key)
            .cloned()
            .ok_or_else(|| Crash::new(format!("KeyError: {}", kernel::py::repr_str(key)))),
        Value::List(_) => Err(Crash::new(
            "TypeError: list indices must be integers or slices, not str",
        )),
        Value::Tuple(_) => Err(Crash::new(
            "TypeError: tuple indices must be integers or slices, not str",
        )),
        Value::Str(_) => Err(Crash::new(
            "TypeError: string indices must be integers, not 'str'",
        )),
        other => Err(not_subscriptable(other)),
    }
}

/// `value[:limit]`, then iterated.
fn first_items(v: &Value, limit: usize) -> Result<Vec<Value>, Crash> {
    match v {
        Value::List(items) | Value::Tuple(items) => Ok(items.iter().take(limit).cloned().collect()),
        Value::Str(text) => Ok(text
            .chars()
            .take(limit)
            .map(|c| s(&c.to_string()))
            .collect()),
        Value::Dict(_) => Err(Crash::new(format!("KeyError: slice(None, {limit}, None)"))),
        other => Err(not_subscriptable(other)),
    }
}

/// `user.get("login") if isinstance(user, dict) else user`.
fn login(user: Value) -> Result<Value, Crash> {
    match &user {
        Value::Dict(_) => get(&user, "login"),
        _ => Ok(user),
    }
}

/// `(value or "").lower()`.
fn lowered(v: &Value) -> Result<String, Crash> {
    Ok(lower(text_or_empty(v, "lower")?))
}

/// `(value or {}).get(key)`.
fn get_or_empty(v: &Value, key: &str) -> Result<Value, Crash> {
    if v.truthy() {
        get(v, key)
    } else {
        Ok(Value::Null)
    }
}

static REMOTE_SSH: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::fixed(r"^(?:ssh://)?git@([^:/]+)[:/](.+?)(?:\.git)?/?$", "")
});
static REMOTE_HTTP: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::fixed(r"^https?://(?:[^@/]+@)?([^/]+)/(.+?)(?:\.git)?/?$", "")
});

/// `parse_remote(url)`: (host, path) of a Git remote URL (https or ssh).
pub fn parse_remote(url: &str) -> Option<(String, String)> {
    let chars: Vec<char> = strip(url).chars().collect();
    let m = REMOTE_SSH
        .match_start(&chars)
        .or_else(|| REMOTE_HTTP.match_start(&chars))?;
    let host = lower(&m.group(&chars, 1).unwrap_or_default());
    let path = m.group(&chars, 2).unwrap_or_default();
    let host = if host.matches(':').count() == 1 && !host.starts_with('[') {
        host.split(':').next().unwrap_or_default().to_string()
    } else {
        host
    };
    Some((host, path))
}

/// `detect_provider(remote_url, configured, configured_host)`: (provider, host, path).
pub fn detect_provider(
    remote_url: &str,
    configured: &Value,
    configured_host: &Value,
) -> Result<(Value, Value, Option<String>), Crash> {
    let remote = parse_remote(remote_url);
    let host = if configured_host.truthy() {
        configured_host.clone()
    } else {
        remote.as_ref().map_or(Value::Null, |(h, _)| s(h))
    };
    let mut provider = if configured.truthy() && !configured.py_eq(&s("auto")) {
        configured.clone()
    } else {
        Value::Null
    };
    if !provider.truthy() && host.truthy() {
        let github = host.py_eq(&s("github.com")) || {
            let Value::Str(h) = &host else {
                return Err(attr_error(&host, "endswith"));
            };
            h.ends_with(".github.com") || h.contains("github")
        };
        if github {
            provider = s("github");
        } else if host.as_str().is_some_and(|h| h.contains("gitlab")) {
            provider = s("gitlab");
        }
    }
    Ok((provider, host, remote.map(|(_, p)| p)))
}

// ---------------------------------------------------------------- GitHub

fn gh_check_run(run: &Value) -> Result<Check, Crash> {
    let status = if lowered(&get(run, "status")?)? != "completed" {
        "pending"
    } else {
        match lowered(&get(run, "conclusion")?)?.as_str() {
            "success" | "neutral" => "success",
            "skipped" => "skipped",
            _ => "failure",
        }
    };
    Ok(Check {
        name: get(run, "name")?,
        status,
        url: or(get(run, "details_url")?, get(run, "html_url")?),
    })
}

fn gh_status(item: &Value) -> Result<Check, Crash> {
    let status = match lowered(&get(item, "state")?)?.as_str() {
        "success" => "success",
        "pending" => "pending",
        _ => "failure",
    };
    Ok(Check {
        name: get(item, "context")?,
        status,
        url: get(item, "target_url")?,
    })
}

/// Review threads from the REST review comments: a thread is a top comment and its replies.
/// REST does not say whether a thread is resolved: `resolved` is unknown.
fn rest_threads(review_comments: &Value) -> Result<Vec<Thread>, Crash> {
    let items = if review_comments.truthy() {
        iterate(review_comments)?
    } else {
        Vec::new()
    };
    let mut keyed = Vec::new();
    for c in items {
        let key = or(get(&c, "created_at")?, s(""));
        keyed.push((key, c));
    }
    let mut threads: Vec<Thread> = Vec::new();
    let mut by_root = Dict::new();
    for (_, c) in sort_keyed(keyed)? {
        let root = get(&c, "in_reply_to_id")?;
        let comment = Comment {
            id: s(&get(&c, "id")?.py_str()),
            author: login(get(&c, "user")?)?,
            body: get(&c, "body")?,
            created: get(&c, "created_at")?,
        };
        if root.truthy() {
            unhashable(&root)?;
            if let Some(Value::Int(i)) = by_root.get_value(&root) {
                let index: usize = i.parse().unwrap_or(0);
                threads[index].comments.push(comment);
                continue;
            }
        }
        let line = get(&c, "line")?;
        let thread = Thread {
            id: s(&get(&c, "id")?.py_str()),
            path: get(&c, "path")?,
            line: or(line.clone(), get(&c, "original_line")?),
            resolved: None,
            outdated: line.is_none(),
            comments: vec![comment],
        };
        let id = get(&c, "id")?;
        unhashable(&id)?;
        by_root.insert_value(id, Value::int(threads.len() as i64));
        threads.push(thread);
    }
    Ok(threads)
}

pub struct GitHub {
    path: String,
    host: Value,
    owner: String,
    repo: String,
}

impl GitHub {
    fn new(path: &str, host: &Value) -> Self {
        let (owner, repo) = path.split_once('/').unwrap_or((path, ""));
        GitHub {
            path: path.to_string(),
            host: or(host.clone(), s("github.com")),
            owner: owner.to_string(),
            repo: repo.to_string(),
        }
    }

    fn gh(&self, args: &[String], input: Option<&str>) -> Res<String> {
        let mut cmd: Vec<Value> = std::iter::once(s("gh"))
            .chain(args.iter().map(|a| s(a)))
            .collect();
        if !self.host.py_eq(&s("github.com")) && args.first().is_some_and(|a| a == "api") {
            cmd.push(s("--hostname"));
            cmd.push(self.host.clone());
        }
        let (code, out, err) = run(&cmd, input)?;
        if code == 127 {
            return Err(Fail::Provider(
                "the GitHub CLI is not installed: https://cli.github.com/".into(),
            ));
        }
        if code != 0 {
            let text = strip(if err.is_empty() { &out } else { &err }).to_string();
            let low = lower(&text);
            if text.contains("auth login")
                || low.contains("not logged")
                || low.contains("authentication")
            {
                return Err(Fail::Provider(format!(
                    "the GitHub CLI is not logged in: run 'gh auth login' ({})",
                    head(&text, 200)
                )));
            }
            return Err(Fail::Provider(format!(
                "gh {} failed: {}",
                args.iter().take(3).cloned().collect::<Vec<_>>().join(" "),
                head(&text, 500)
            )));
        }
        Ok(out)
    }

    fn json(&self, args: &[String], input: Option<&str>) -> Res<Value> {
        let out = self.gh(args, input)?;
        decode_pages(
            &out,
            &format!(
                "gh {}",
                args.iter().take(2).cloned().collect::<Vec<_>>().join(" ")
            ),
        )
    }

    fn rest(
        &self,
        endpoint: &str,
        method: Option<&str>,
        payload: Option<&Value>,
        paginate: bool,
    ) -> Res<Value> {
        let mut args = vec!["api".to_string(), endpoint.to_string()];
        if let Some(m) = method {
            args.push("--method".into());
            args.push(m.into());
        }
        if paginate {
            args.push("--paginate".into());
        }
        let input = match payload {
            Some(p) => {
                args.push("--input".into());
                args.push("-".into());
                Some(dumps_ascii(p, None)?)
            }
            None => None,
        };
        self.json(&args, input.as_deref())
    }

    fn graphql(&self, query: &str, variables: &[(&str, &str, bool)]) -> Res<Value> {
        let mut args = vec![
            "api".to_string(),
            "graphql".to_string(),
            "-f".to_string(),
            format!("query={query}"),
        ];
        for (key, value, is_int) in variables {
            args.push(if *is_int { "-F" } else { "-f" }.to_string());
            args.push(format!("{key}={value}"));
        }
        self.json(&args, None)
    }

    /// The pull request `number`, or the open one of `branch`; `None` when there is none.
    fn pull_request(&self, number: Option<Value>, branch: &str) -> Res<Option<PullRequest>> {
        let base = format!("repos/{}/pulls", self.path);
        let number = match number {
            Some(n) => n,
            None => {
                let headref = quote(&format!("{}:{branch}", self.owner), ":");
                let found = self.rest(
                    &format!("{base}?state=open&head={headref}&per_page=1"),
                    None,
                    None,
                    false,
                )?;
                if !found.truthy() {
                    return Ok(None);
                }
                item(&first_item(&found)?, "number")?
            }
        };
        let number = int(&number)?;
        let data = or(
            self.rest(&format!("{base}/{number}"), None, None, false)?,
            Value::Dict(Dict::new()),
        );
        let sha = get_or_empty(&get(&data, "head")?, "sha")?;
        let reviews = or(
            self.rest(
                &format!("{base}/{number}/reviews?per_page=100"),
                None,
                None,
                true,
            )?,
            Value::List(Vec::new()),
        );
        let comments = or(
            self.rest(
                &format!("repos/{}/issues/{number}/comments?per_page=100", self.path),
                None,
                None,
                true,
            )?,
            Value::List(Vec::new()),
        );
        let mut checks = Vec::new();
        if sha.truthy() {
            let sha = sha.py_str();
            let pages = or(
                self.rest(
                    &format!("repos/{}/commits/{sha}/check-runs?per_page=100", self.path),
                    None,
                    None,
                    true,
                )?,
                Value::List(Vec::new()),
            );
            let pages = match pages {
                Value::List(items) => items,
                other => vec![other],
            };
            for page in pages {
                let runs = or(get(&page, "check_runs")?, Value::List(Vec::new()));
                let mut found = Vec::new();
                for r in iterate(&runs)? {
                    found.push(gh_check_run(&r)?);
                }
                checks.extend(found);
            }
            let status = or(
                self.rest(
                    &format!("repos/{}/commits/{sha}/status", self.path),
                    None,
                    None,
                    false,
                )?,
                Value::Dict(Dict::new()),
            );
            let statuses = or(get(&status, "statuses")?, Value::List(Vec::new()));
            let mut found = Vec::new();
            for item in iterate(&statuses)? {
                found.push(gh_status(&item)?);
            }
            checks.extend(found);
        }
        let mut pr = self.model(&data, &reviews, &comments, checks)?;
        let (threads, note) = self.threads(&number)?;
        pr.threads = threads;
        pr.threads_note = Some(note);
        Ok(Some(pr))
    }

    /// (threads, note): from GraphQL with their resolution; where GraphQL is refused, from the
    /// REST review comments with the resolution unknown and a note that says so.
    fn threads(&self, number: &str) -> Res<(Vec<Thread>, Option<String>)> {
        let data = match self.graphql(
            GITHUB_THREADS_QUERY,
            &[
                ("owner", &self.owner, false),
                ("repo", &self.repo, false),
                ("number", number, true),
            ],
        ) {
            Ok(d) => d,
            Err(Fail::Provider(e)) => {
                let rest = or(
                    self.rest(
                        &format!("repos/{}/pulls/{number}/comments?per_page=100", self.path),
                        None,
                        None,
                        true,
                    )?,
                    Value::List(Vec::new()),
                );
                return Ok((
                    rest_threads(&rest)?,
                    Some(format!("{GRAPHQL_ONLY} ({})", head(&e, 160))),
                ));
            }
            Err(other) => return Err(other),
        };
        let mut v = data;
        for key in [
            "data",
            "repository",
            "pullRequest",
            "reviewThreads",
            "nodes",
        ] {
            v = get_or_empty(&v, key)?;
        }
        let nodes = or(v, Value::List(Vec::new()));
        let mut threads = Vec::new();
        for t in iterate(&nodes)? {
            let id = get(&t, "id")?;
            let path = get(&t, "path")?;
            let line = or(get(&t, "line")?, get(&t, "originalLine")?);
            let resolved = get(&t, "isResolved")?.truthy();
            let outdated = get(&t, "isOutdated")?.truthy();
            let nodes = or(
                get_or_empty(&get(&t, "comments")?, "nodes")?,
                Value::List(Vec::new()),
            );
            let mut comments = Vec::new();
            for c in iterate(&nodes)? {
                comments.push(Comment {
                    id: get(&c, "id")?,
                    author: login(get(&c, "author")?)?,
                    body: get(&c, "body")?,
                    created: get(&c, "createdAt")?,
                });
            }
            threads.push(Thread {
                id,
                path,
                line,
                resolved: Some(resolved),
                outdated,
                comments,
            });
        }
        Ok((threads, None))
    }

    fn model(
        &self,
        data: &Value,
        reviews: &Value,
        comments: &Value,
        checks: Vec<Check>,
    ) -> Result<PullRequest, Crash> {
        let state = if get(data, "merged")?.truthy() || get(data, "merged_at")?.truthy() {
            s("merged")
        } else {
            s(&lowered(&get(data, "state")?)?)
        };
        let mergeable_value = get(data, "mergeable")?;
        let mergeable = if get(data, "mergeable_state")?.py_eq(&s("dirty"))
            || matches!(mergeable_value, Value::Bool(false))
        {
            "conflict"
        } else if get(data, "mergeable")?.truthy() {
            "clean"
        } else {
            "unknown"
        };
        let number = get(data, "number")?;
        let url = get(data, "html_url")?;
        let title = get(data, "title")?;
        let draft = get(data, "draft")?.truthy();
        let author = login(get(data, "user")?)?;
        let base = get_or_empty(&get(data, "base")?, "ref")?;
        let headref = get_or_empty(&get(data, "head")?, "ref")?;
        let mut review_list = Vec::new();
        for r in iterate(reviews)? {
            let author = login(get(&r, "user")?)?;
            let state = get(&r, "state")?;
            unhashable(&state)?;
            let state = match state.as_str() {
                Some("APPROVED") => "approved",
                Some("CHANGES_REQUESTED") => "changes_requested",
                Some("DISMISSED") => "dismissed",
                _ => "commented",
            };
            review_list.push(Review { author, state });
        }
        let mut comment_list = Vec::new();
        for c in iterate(comments)? {
            comment_list.push(Comment {
                id: get(&c, "id")?,
                author: login(get(&c, "user")?)?,
                body: get(&c, "body")?,
                created: get(&c, "created_at")?,
            });
        }
        Ok(PullRequest {
            provider: "github",
            number,
            url,
            title,
            state,
            draft,
            author,
            base,
            head: headref,
            mergeable,
            checks,
            reviews: review_list,
            threads: Vec::new(),
            comments: comment_list,
            threads_note: None,
            diff_refs: None,
        })
    }

    fn open_pull_requests(&self) -> Res<Vec<PullRequest>> {
        let found = or(
            self.rest(
                &format!("repos/{}/pulls?state=open&per_page=10", self.path),
                None,
                None,
                false,
            )?,
            Value::List(Vec::new()),
        );
        let mut out = Vec::new();
        for entry in first_items(&found, 10)? {
            let number = item(&entry, "number")?;
            if let Some(pr) = self.pull_request(Some(number), "")? {
                out.push(pr);
            }
        }
        Ok(out)
    }

    /// Whether merging into `branch` needs every conversation resolved (a ruleset or the
    /// classic branch protection); `None` when the platform does not let it be read.
    fn requires_thread_resolution(&self, branch: &Value) -> Res<Option<bool>> {
        let branch = branch.py_str();
        let rules = match self.json(
            &[
                "api".to_string(),
                format!("repos/{}/rules/branches/{branch}", self.path),
            ],
            None,
        ) {
            Ok(v) => Some(or(v, Value::List(Vec::new()))),
            Err(Fail::Provider(_)) => None,
            Err(other) => return Err(other),
        };
        if let Some(rules) = &rules
            && rules.truthy()
        {
            for r in iterate(rules)? {
                if get(&r, "type")?.py_eq(&s("pull_request"))
                    && get_or_empty(&get(&r, "parameters")?, "required_review_thread_resolution")?
                        .truthy()
                {
                    return Ok(Some(true));
                }
            }
        }
        let protection = match self.json(
            &[
                "api".to_string(),
                format!("repos/{}/branches/{branch}/protection", self.path),
            ],
            None,
        ) {
            Ok(v) => or(v, Value::Dict(Dict::new())),
            Err(Fail::Provider(e)) => {
                return Ok(if rules.is_some() && e.contains("Branch not protected") {
                    Some(false)
                } else {
                    None
                });
            }
            Err(other) => return Err(other),
        };
        Ok(Some(
            get_or_empty(
                &get(&protection, "required_conversation_resolution")?,
                "enabled",
            )?
            .truthy(),
        ))
    }

    fn body(text: &str) -> Value {
        let mut d = Dict::new();
        d.insert("body", s(text));
        Value::Dict(d)
    }

    fn reply(&self, number: &Value, thread: &str, body: &str) -> Res<()> {
        if is_digits(thread) {
            let n = int(number)?;
            self.rest(
                &format!("repos/{}/pulls/{n}/comments/{thread}/replies", self.path),
                Some("POST"),
                Some(&Self::body(body)),
                false,
            )?;
            return Ok(());
        }
        self.graphql(
            GITHUB_REPLY_MUTATION,
            &[("thread", thread, false), ("body", body, false)],
        )?;
        Ok(())
    }

    fn resolve(&self, thread: &str) -> Res<()> {
        if is_digits(thread) {
            return Err(Fail::Provider("resolving a review thread needs GitHub's GraphQL API, which was refused here: resolve it on the pull request page (or with another GitHub tool you have); the merge gate keeps blocking findings open until it can read them as resolved".into()));
        }
        self.graphql(GITHUB_RESOLVE_MUTATION, &[("thread", thread, false)])?;
        Ok(())
    }

    fn comment(&self, number: &Value, body: &str) -> Res<()> {
        let n = int(number)?;
        self.rest(
            &format!("repos/{}/issues/{n}/comments", self.path),
            Some("POST"),
            Some(&Self::body(body)),
            false,
        )?;
        Ok(())
    }

    fn review(&self, number: &Value, body: &str, comments: &[LineComment]) -> Res<()> {
        let mut payload = Dict::new();
        payload.insert("event", s("COMMENT"));
        payload.insert("body", s(body));
        payload.insert(
            "comments",
            Value::List(
                comments
                    .iter()
                    .map(|c| {
                        let mut d = Dict::new();
                        d.insert("path", c.path.clone());
                        d.insert("line", Value::Int(c.line.clone()));
                        d.insert("side", s("RIGHT"));
                        d.insert("body", s(&c.body));
                        Value::Dict(d)
                    })
                    .collect(),
            ),
        );
        let n = int(number)?;
        self.rest(
            &format!("repos/{}/pulls/{n}/reviews", self.path),
            Some("POST"),
            Some(&Value::Dict(payload)),
            false,
        )?;
        Ok(())
    }

    fn request_review(&self, number: &Value, reviewers: &[String]) -> Res<()> {
        let n = int(number)?;
        let mut payload = Dict::new();
        payload.insert(
            "reviewers",
            Value::List(reviewers.iter().map(|r| s(r)).collect()),
        );
        self.rest(
            &format!("repos/{}/pulls/{n}/requested_reviewers", self.path),
            Some("POST"),
            Some(&Value::Dict(payload)),
            false,
        )?;
        Ok(())
    }
}

/// `str.isdigit()`: decimal digits and the other digits (superscripts, circled digits, ...).
fn is_digits(text: &str) -> bool {
    !text.is_empty()
        && text.chars().all(|c| {
            kernel::py::num::decimal_value(c).is_some() || kernel::py::unicode::is_other_digit(c)
        })
}

/// A line comment of the protocol's review, ready to post.
#[derive(Debug, Clone)]
pub struct LineComment {
    pub path: Value,
    /// `int(line)`, as decimal text.
    pub line: String,
    pub body: String,
}

// ---------------------------------------------------------------- GitLab

pub struct GitLab {
    host: Value,
    project: String,
}

impl GitLab {
    fn new(path: &str, host: &Value) -> Self {
        GitLab {
            host: or(host.clone(), s("gitlab.com")),
            project: format!("projects/{}", quote(path, "")),
        }
    }

    fn api(
        &self,
        endpoint: &str,
        method: Option<&str>,
        payload: Option<&Value>,
        paginate: bool,
    ) -> Res<Value> {
        let mut cmd = vec![s("glab"), s("api"), s(endpoint)];
        if let Some(m) = method {
            cmd.push(s("--method"));
            cmd.push(s(m));
        }
        if paginate {
            cmd.push(s("--paginate"));
        }
        if !self.host.py_eq(&s("gitlab.com")) {
            cmd.push(s("--hostname"));
            cmd.push(self.host.clone());
        }
        let input = match payload {
            Some(p) => {
                cmd.extend([
                    s("--input"),
                    s("-"),
                    s("-H"),
                    s("Content-Type: application/json"),
                ]);
                Some(dumps_ascii(p, None)?)
            }
            None => None,
        };
        let (code, out, err) = run(&cmd, input.as_deref())?;
        if code == 127 {
            return Err(Fail::Provider(
                "the GitLab CLI is not installed: https://gitlab.com/gitlab-org/cli".into(),
            ));
        }
        if code != 0 {
            let text = strip(if err.is_empty() { &out } else { &err }).to_string();
            if text.contains("401")
                || text.contains("auth login")
                || lower(&text).contains("unauthorized")
            {
                return Err(Fail::Provider(format!(
                    "the GitLab CLI is not logged in: run 'glab auth login --hostname {}' ({})",
                    self.host.py_str(),
                    head(&text, 200)
                )));
            }
            return Err(Fail::Provider(format!(
                "glab api {endpoint} failed: {}",
                head(&text, 500)
            )));
        }
        decode_pages(&out, &format!("glab api {endpoint}"))
    }

    fn pull_request(&self, number: Option<Value>, branch: &str) -> Res<Option<PullRequest>> {
        let number = match number {
            Some(n) => n,
            None => {
                let found = or(
                    self.api(
                        &format!(
                            "{}/merge_requests?state=opened&source_branch={}",
                            self.project,
                            quote(branch, "")
                        ),
                        None,
                        None,
                        false,
                    )?,
                    Value::List(Vec::new()),
                );
                if !found.truthy() {
                    return Ok(None);
                }
                item(&first_item(&found)?, "iid")?
            }
        };
        let base = format!("{}/merge_requests/{}", self.project, int(&number)?);
        let mr = self.api(&base, None, None, false)?;
        let discussions = or(
            self.api(
                &format!("{base}/discussions?per_page=100"),
                None,
                None,
                true,
            )?,
            Value::List(Vec::new()),
        );
        let approvals = or(
            self.api(&format!("{base}/approvals"), None, None, false)?,
            Value::Dict(Dict::new()),
        );
        Ok(Some(self.model(&mr, &discussions, &approvals)?))
    }

    fn model(
        &self,
        mr: &Value,
        discussions: &Value,
        approvals: &Value,
    ) -> Result<PullRequest, Crash> {
        let mut threads = Vec::new();
        let mut comments = Vec::new();
        for d in iterate(discussions)? {
            let all_notes = or(get(&d, "notes")?, Value::List(Vec::new()));
            let mut notes = Vec::new();
            for n in iterate(&all_notes)? {
                if !get(&n, "system")?.truthy() {
                    notes.push(n);
                }
            }
            if notes.is_empty() {
                continue;
            }
            let mut converted = Vec::new();
            for n in &notes {
                converted.push(Comment {
                    id: get(n, "id")?,
                    author: get_or_empty(&get(n, "author")?, "username")?,
                    body: get(n, "body")?,
                    created: get(n, "created_at")?,
                });
            }
            let first = &notes[0];
            if get(&d, "individual_note")?.truthy() && !get(first, "resolvable")?.truthy() {
                comments.extend(converted);
                continue;
            }
            let position = or(get(first, "position")?, Value::Dict(Dict::new()));
            let id = get(&d, "id")?;
            let path = or(get(&position, "new_path")?, get(&position, "old_path")?);
            let line = or(get(&position, "new_line")?, get(&position, "old_line")?);
            let mut resolved = true;
            for n in &notes {
                if get(n, "resolvable")?.truthy() && !get(n, "resolved")?.truthy() {
                    resolved = false;
                    break;
                }
            }
            threads.push(Thread {
                id,
                path,
                line,
                resolved: Some(resolved),
                outdated: false,
                comments: converted,
            });
        }
        let pipeline = or(get(mr, "head_pipeline")?, Value::Dict(Dict::new()));
        let mut checks = Vec::new();
        if pipeline.truthy() {
            let status = get(&pipeline, "status")?;
            unhashable(&status)?;
            let status = match status.as_str() {
                Some("success") => "success",
                Some("failed") | Some("canceled") => "failure",
                Some("skipped") => "skipped",
                _ => "pending",
            };
            checks.push(Check {
                name: s("pipeline"),
                status,
                url: get(&pipeline, "web_url")?,
            });
        }
        let raw_state = get(mr, "state")?;
        unhashable(&raw_state)?;
        let state = match raw_state.as_str() {
            Some("opened") | Some("locked") => s("open"),
            _ => raw_state,
        };
        let conflict = get(mr, "has_conflicts")?.truthy()
            || get(mr, "detailed_merge_status")?.py_eq(&s("conflict"));
        let mut reviews = Vec::new();
        for a in iterate(&or(get(approvals, "approved_by")?, Value::List(Vec::new())))? {
            reviews.push(Review {
                author: get_or_empty(&get(&a, "user")?, "username")?,
                state: "approved",
            });
        }
        Ok(PullRequest {
            provider: "gitlab",
            number: get(mr, "iid")?,
            url: get(mr, "web_url")?,
            title: get(mr, "title")?,
            state,
            draft: get(mr, "draft")?.truthy() || get(mr, "work_in_progress")?.truthy(),
            author: get_or_empty(&get(mr, "author")?, "username")?,
            base: get(mr, "target_branch")?,
            head: get(mr, "source_branch")?,
            mergeable: if conflict { "conflict" } else { "clean" },
            checks,
            reviews,
            threads,
            comments,
            threads_note: None,
            diff_refs: Some(or(get(mr, "diff_refs")?, Value::Dict(Dict::new()))),
        })
    }

    fn open_pull_requests(&self) -> Res<Vec<PullRequest>> {
        let found = or(
            self.api(
                &format!("{}/merge_requests?state=opened&per_page=10", self.project),
                None,
                None,
                false,
            )?,
            Value::List(Vec::new()),
        );
        let mut out = Vec::new();
        for entry in first_items(&found, 10)? {
            let number = item(&entry, "iid")?;
            if let Some(pr) = self.pull_request(Some(number), "")? {
                out.push(pr);
            }
        }
        Ok(out)
    }

    fn requires_thread_resolution(&self) -> Res<Option<bool>> {
        let project = match self.api(&self.project, None, None, false) {
            Ok(v) => v,
            Err(Fail::Provider(_)) => return Ok(None),
            Err(other) => return Err(other),
        };
        let value = get_or_empty(&project, "only_allow_merge_if_all_discussions_are_resolved")?;
        Ok(if value.is_none() {
            None
        } else {
            Some(value.truthy())
        })
    }

    fn mr(&self, number: &Value) -> Res<String> {
        Ok(format!("{}/merge_requests/{}", self.project, int(number)?))
    }

    fn reply(&self, number: &Value, thread: &str, body: &str) -> Res<()> {
        let endpoint = format!("{}/discussions/{thread}/notes", self.mr(number)?);
        self.api(&endpoint, Some("POST"), Some(&GitHub::body(body)), false)?;
        Ok(())
    }

    fn resolve(&self, number: &Value, thread: &str) -> Res<()> {
        let endpoint = format!("{}/discussions/{thread}", self.mr(number)?);
        let mut d = Dict::new();
        d.insert("resolved", Value::Bool(true));
        self.api(&endpoint, Some("PUT"), Some(&Value::Dict(d)), false)?;
        Ok(())
    }

    fn comment(&self, number: &Value, body: &str) -> Res<()> {
        let endpoint = format!("{}/notes", self.mr(number)?);
        self.api(&endpoint, Some("POST"), Some(&GitHub::body(body)), false)?;
        Ok(())
    }

    fn review(&self, number: &Value, body: &str, comments: &[LineComment]) -> Res<()> {
        let base = self.mr(number)?;
        let mr = or(
            self.api(&base, None, None, false)?,
            Value::Dict(Dict::new()),
        );
        let refs = or(get(&mr, "diff_refs")?, Value::Dict(Dict::new()));
        for c in comments {
            let mut position = Dict::new();
            position.insert("position_type", s("text"));
            position.insert("base_sha", get(&refs, "base_sha")?);
            position.insert("start_sha", get(&refs, "start_sha")?);
            position.insert("head_sha", get(&refs, "head_sha")?);
            position.insert("new_path", c.path.clone());
            position.insert("new_line", Value::Int(c.line.clone()));
            let mut payload = Dict::new();
            payload.insert("body", s(&c.body));
            payload.insert("position", Value::Dict(position));
            self.api(
                &format!("{base}/discussions"),
                Some("POST"),
                Some(&Value::Dict(payload)),
                false,
            )?;
        }
        self.api(
            &format!("{base}/notes"),
            Some("POST"),
            Some(&GitHub::body(body)),
            false,
        )?;
        Ok(())
    }

    fn request_review(&self, number: &Value, reviewers: &[String]) -> Res<()> {
        let base = self.mr(number)?;
        let mr = or(
            self.api(&base, None, None, false)?,
            Value::Dict(Dict::new()),
        );
        let mut ids = Vec::new();
        for r in iterate(&or(get(&mr, "reviewers")?, Value::List(Vec::new())))? {
            ids.push(get(&r, "id")?);
        }
        for name in reviewers {
            let users = or(
                self.api(
                    &format!("users?username={}", quote(name, "")),
                    None,
                    None,
                    false,
                )?,
                Value::List(Vec::new()),
            );
            if !users.truthy() {
                return Err(Fail::Provider(format!("GitLab user '{name}' not found")));
            }
            let id = item(&first_item(&users)?, "id")?;
            if !ids.iter().any(|x| x.py_eq(&id)) {
                ids.push(id);
            }
        }
        let mut payload = Dict::new();
        payload.insert("reviewer_ids", Value::List(ids));
        self.api(&base, Some("PUT"), Some(&Value::Dict(payload)), false)?;
        Ok(())
    }
}

// ---------------------------------------------------------------- factory

pub enum Provider {
    GitHub(GitHub),
    GitLab(GitLab),
}

/// `provider_for(provider, host, path)`.
pub fn provider_for(provider: &Value, host: &Value, path: &str) -> Res<Provider> {
    match provider.as_str() {
        Some("github") => Ok(Provider::GitHub(GitHub::new(path, host))),
        Some("gitlab") => Ok(Provider::GitLab(GitLab::new(path, host))),
        _ => Err(Fail::Provider(format!(
            "pull request platform '{}' is not supported yet (GitHub and GitLab are)",
            provider.py_str()
        ))),
    }
}

impl Provider {
    pub fn resolution_setting(&self) -> &'static str {
        match self {
            Provider::GitHub(_) => {
                "Settings > Branches (or Rules): Require conversation resolution before merging"
            }
            Provider::GitLab(_) => "Settings > Merge requests: All threads must be resolved",
        }
    }

    pub fn pull_request(&self, number: Option<Value>, branch: &str) -> Res<Option<PullRequest>> {
        match self {
            Provider::GitHub(p) => p.pull_request(number, branch),
            Provider::GitLab(p) => p.pull_request(number, branch),
        }
    }

    pub fn open_pull_requests(&self) -> Res<Vec<PullRequest>> {
        match self {
            Provider::GitHub(p) => p.open_pull_requests(),
            Provider::GitLab(p) => p.open_pull_requests(),
        }
    }

    pub fn requires_thread_resolution(&self, branch: &Value) -> Res<Option<bool>> {
        match self {
            Provider::GitHub(p) => p.requires_thread_resolution(branch),
            Provider::GitLab(p) => p.requires_thread_resolution(),
        }
    }

    pub fn reply(&self, number: &Value, thread: &str, body: &str) -> Res<()> {
        match self {
            Provider::GitHub(p) => p.reply(number, thread, body),
            Provider::GitLab(p) => p.reply(number, thread, body),
        }
    }

    pub fn resolve(&self, number: &Value, thread: &str) -> Res<()> {
        match self {
            Provider::GitHub(p) => p.resolve(thread),
            Provider::GitLab(p) => p.resolve(number, thread),
        }
    }

    pub fn comment(&self, number: &Value, body: &str) -> Res<()> {
        match self {
            Provider::GitHub(p) => p.comment(number, body),
            Provider::GitLab(p) => p.comment(number, body),
        }
    }

    pub fn review(&self, number: &Value, body: &str, comments: &[LineComment]) -> Res<()> {
        match self {
            Provider::GitHub(p) => p.review(number, body, comments),
            Provider::GitLab(p) => p.review(number, body, comments),
        }
    }

    pub fn request_review(&self, number: &Value, reviewers: &[String]) -> Res<()> {
        match self {
            Provider::GitHub(p) => p.request_review(number, reviewers),
            Provider::GitLab(p) => p.request_review(number, reviewers),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_remotes_and_quotes_like_python() {
        assert_eq!(
            parse_remote("git@GitHub.com:acme/widgets.git"),
            Some(("github.com".into(), "acme/widgets".into()))
        );
        assert_eq!(
            parse_remote(" https://user@gitlab.example.com:8443/group/sub/repo/ "),
            Some(("gitlab.example.com".into(), "group/sub/repo".into()))
        );
        assert_eq!(parse_remote("/srv/repo.git"), None);
        assert_eq!(quote("acme:feat/ü x", ":"), "acme:feat%2F%C3%BC%20x");
    }
}
