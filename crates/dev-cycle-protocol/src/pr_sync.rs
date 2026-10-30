//! Pull Request Sync (`pr_sync.py`): Phase 7 on the platform. Reads the pull request of the
//! current branch (a merge request on GitLab) with its review threads, comments, reviews and
//! checks, says what is pending, and posts answers, resolutions and the protocol's code review.
//! Every text it posts passes the guard of protocol principle 9 (no AI attribution) and carries
//! the protocol's marker.

use std::path::Path;

use kernel::no_ai;
use kernel::out::println;
use kernel::project_config::load_project_config;
use kernel::py::argparse::Parser;
use kernel::py::json::{self, LoadError};
use kernel::py::num::{int, int_value, strip};
use kernel::py::path::{abspath, exists, isdir};
use kernel::py::subprocess::run_replace;
use kernel::py::{Crash, Dict, Value, iterate, read_text, repr_str};

use crate::pr_model::{
    LEVELS, Pending, PullRequest, REPLY_MARKER, REVIEW_MARKER, format_post, get, merge_blockers,
    needs_action, or, pending, summary_line,
};
use crate::pr_providers::{Fail, LineComment, Provider, detect_provider, item, provider_for};

type Res<T> = Result<T, Fail>;

fn s(text: &str) -> Value {
    Value::Str(text.to_string())
}

/// `git(root, *args)`: stdout stripped, empty when Git fails. Python could not start Git in a
/// folder that is not there.
fn git(root: &str, args: &[&str]) -> Result<String, Crash> {
    if !isdir(root) {
        return Err(Crash::new(if exists(root) {
            format!(
                "NotADirectoryError: [Errno 20] Not a directory: {}",
                repr_str(root)
            )
        } else {
            format!(
                "FileNotFoundError: [Errno 2] No such file or directory: {}",
                repr_str(root)
            )
        }));
    }
    let cmd: Vec<String> = std::iter::once("git")
        .chain(args.iter().copied())
        .map(String::from)
        .collect();
    match run_replace(&cmd, Some(root)) {
        Ok((0, out, _)) => Ok(strip(&out).to_string()),
        Ok(_) => Ok(String::new()),
        Err(Ok(msg)) => Err(Crash::new(format!("FileNotFoundError: {msg}"))),
        Err(Err(c)) => Err(c),
    }
}

/// The project's platform, markers and current branch.
pub struct Context {
    pub root: String,
    pub marker: Value,
    pub reply_marker: Value,
    pub allowed: Value,
    pub provider_name: Value,
    pub host: Value,
    pub path: Option<String>,
    pub branch: String,
}

impl Context {
    pub fn new(start_dir: Option<&str>) -> Result<Context, Crash> {
        let start = match start_dir {
            Some(d) if !d.is_empty() => abspath(d),
            _ => abspath("."),
        }
        .to_string_lossy()
        .into_owned();
        let top = git(&start, &["rev-parse", "--show-toplevel"])?;
        let root = if top.is_empty() { start } else { top };
        let cfg = load_project_config(Some(&root)).map_err(|e| e.into_crash())?;
        let review = cfg.section("review");
        let field = |key: &str| review.get(key).cloned().unwrap_or(Value::Null);
        let marker = or(field("marker"), s(REVIEW_MARKER));
        let reply_marker = or(field("reply_marker"), s(REPLY_MARKER));
        let allowed = or(field("allowed_terms"), Value::List(Vec::new()));
        let remote = git(&root, &["remote", "get-url", "origin"])?;
        let (provider_name, host, path) =
            detect_provider(&remote, &field("provider"), &field("host"))?;
        let branch = git(&root, &["rev-parse", "--abbrev-ref", "HEAD"])?;
        Ok(Context {
            root,
            marker,
            reply_marker,
            allowed,
            provider_name,
            host,
            path,
            branch,
        })
    }

    pub fn provider(&self) -> Res<Provider> {
        match &self.path {
            Some(path) if self.provider_name.truthy() && !path.is_empty() => {
                provider_for(&self.provider_name, &self.host, path)
            }
            _ => Err(Fail::Provider("the pull request platform is unknown: no 'origin' remote on GitHub or GitLab (set review.provider and review.host in .dev-cycle.json)".into())),
        }
    }

    pub fn pull_request(&self, number: Option<Value>) -> Res<PullRequest> {
        match self.provider()?.pull_request(number, &self.branch)? {
            Some(pr) => Ok(pr),
            None => Err(Fail::Provider(format!(
                "no open pull request for branch '{}'",
                self.branch
            ))),
        }
    }

    /// The guard of principle 9: `Err` is the `ValueError` the text is refused with.
    fn guard(&self, text: &str, what: &str) -> Res<()> {
        match no_ai::check(text, what, &self.allowed)? {
            Ok(()) => Ok(()),
            Err(msg) => Err(Fail::Provider(msg)),
        }
    }
}

pub fn pending_of(ctx: &Context, pr: &PullRequest) -> Result<Pending, Crash> {
    pending(pr, &ctx.reply_marker, &ctx.marker)
}

/// `" ".join(text.split())[:140]`.
fn excerpt(body: &Value) -> Result<String, Crash> {
    let text = crate::pr_model::text_or_empty(body, "split")?;
    let words: Vec<&str> = text
        .split(kernel::py::num::is_space)
        .filter(|w| !w.is_empty())
        .collect();
    Ok(words.join(" ").chars().take(140).collect())
}

fn print_status(pr: &PullRequest, p: &Pending) -> Result<(), Crash> {
    println(&format!("🔀 {}", summary_line(pr, p)?));
    println(&format!(
        "   {}  ({} → {}{})",
        pr.url.py_str(),
        pr.head.py_str(),
        pr.base.py_str(),
        if p.draft { ", draft" } else { "" }
    ));
    let order = |hint: &str| match hint {
        "large" => 0,
        "question" => 1,
        "small" => 2,
        "suggestion" => 3,
        "nit" => 4,
        _ => 9,
    };
    let mut threads: Vec<_> = p.threads.iter().collect();
    threads.sort_by_key(|(_, hint)| order(hint));
    for (t, hint) in threads {
        let Some(last) = t.comments.last() else {
            return Err(Crash::new("IndexError: list index out of range"));
        };
        let excerpt = excerpt(&last.body)?;
        let place = format!(
            "{}:{}{}",
            t.path.py_str(),
            t.line.py_str(),
            if t.outdated { " (outdated)" } else { "" }
        );
        let who = if *hint == "large" {
            "for the user"
        } else {
            hint
        };
        println(&format!(
            "   💬 [{who}] {place} — {}: {excerpt}  (thread {})",
            last.author.py_str(),
            t.id.py_str()
        ));
    }
    for c in &p.comments {
        println(&format!(
            "   🗨️ {}: {}",
            c.author.py_str(),
            excerpt(&c.body)?
        ));
    }
    for c in &p.failed_checks {
        println(&format!(
            "   ❌ check {}: {}",
            c.name.py_str(),
            or(c.url.clone(), s("")).py_str()
        ));
    }
    for c in &p.running_checks {
        println(&format!("   ⏳ check {} running", c.name.py_str()));
    }
    if p.conflict {
        println("   ⚠️ merge conflict with the base branch");
    }
    if let Some(note) = pr.note().filter(|n| !n.is_empty()) {
        println(&format!("   ℹ️ {note}: threads are shown as open"));
    }
    if !needs_action(p) {
        println("   ✅ nothing to do on the pull request now");
    }
    Ok(())
}

fn read_body(body: Option<&str>, body_file: Option<&str>) -> Res<String> {
    if let Some(f) = body_file.filter(|f| !f.is_empty()) {
        return Ok(read_text(Path::new(f))?);
    }
    match body {
        Some(b) if !b.is_empty() => Ok(b.to_string()),
        _ => Err(Fail::Provider("give --body or --body-file".into())),
    }
}

fn load_review(path: &str, ctx: &Context) -> Res<(String, Vec<LineComment>)> {
    let data = match json::load_file(Path::new(path)) {
        Ok(v) => v,
        Err(LoadError::Decode(e)) => return Err(Fail::Provider(e.to_string())),
        Err(LoadError::Crash(c)) => return Err(Fail::Crash(c)),
    };
    if !data.is_dict() {
        return Err(Fail::Provider(
            "the review file must hold a JSON object".into(),
        ));
    }
    let body = format_post(
        &ctx.marker,
        &or(get(&data, "body")?, s("Review of the changes.")),
        None,
    )?;
    ctx.guard(&body, "the review summary")?;
    let mut comments = Vec::new();
    let listed = or(get(&data, "comments")?, Value::List(Vec::new()));
    for (n, c) in iterate(&listed)?.iter().enumerate() {
        let i = n + 1;
        let mut missing = Vec::new();
        for k in ["path", "line", "level", "body"] {
            if !get(c, k)?.truthy() {
                missing.push(k);
            }
        }
        if !missing.is_empty() {
            return Err(Fail::Provider(format!(
                "review comment {i} misses: {}",
                missing.join(", ")
            )));
        }
        let level = item(c, "level")?;
        let Some(level) = level.as_str().filter(|l| LEVELS.contains(l)) else {
            return Err(Fail::Provider(format!(
                "review comment {i}: level '{}' is not one of: {}",
                level.py_str(),
                LEVELS.join(", ")
            )));
        };
        let text = format_post(&ctx.marker, &item(c, "body")?, Some(level))?;
        ctx.guard(&text, &format!("review comment {i}"))?;
        let path = item(c, "path")?;
        let line = int_value(&item(c, "line")?)?;
        comments.push(LineComment {
            path,
            line,
            body: text,
        });
    }
    Ok((body, comments))
}

/// Phase 7, before the merge is asked for (or done, when the user authorized it).
fn gate_merge(ctx: &Context, provider: &Provider, pr: &PullRequest) -> Res<u8> {
    let blockers = merge_blockers(pr, &ctx.marker)?;
    println(&format!("🔀 {}", summary_line(pr, &pending_of(ctx, pr)?)?));
    for b in &blockers {
        println(&format!("   ❌ {b}"));
    }
    if let Some(note) = pr.note().filter(|n| !n.is_empty()) {
        println(&format!("   ℹ️ {note}"));
    }
    match provider.requires_thread_resolution(&pr.base)? {
        Some(false) => println(&format!(
            "   ⚠️ merging into '{}' does not require resolved conversations; turn it on in {}",
            pr.base.py_str(),
            provider.resolution_setting()
        )),
        None => println(&format!(
            "   ℹ️ could not read whether merging into '{}' requires resolved conversations",
            pr.base.py_str()
        )),
        Some(true) => {}
    }
    println(if blockers.is_empty() {
        "✅ ready to merge"
    } else {
        "❌ not ready to merge"
    });
    Ok(if blockers.is_empty() { 0 } else { 1 })
}

fn command_parser(name: &str) -> Parser {
    let p = Parser::new(&format!("pr_sync.py {name}"), "").option("--dir", "");
    let p = if name == "open" {
        p
    } else {
        p.int_option(
            "--pr",
            "Number (default: the open one of the current branch)",
        )
    };
    match name {
        "status" | "open" => p.flag("--json", ""),
        "reply" => p
            .required_option("--thread", "")
            .option("--body", "")
            .option("--body-file", ""),
        "comment" => p.option("--body", "").option("--body-file", ""),
        "resolve" => p.required_option("--thread", ""),
        "review" => p.required_option("--file", ""),
        "request-review" => p.required_append_option("--reviewer", ""),
        _ => p,
    }
}

pub fn main(argv: &[String]) -> u8 {
    match run_main(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

/// The exceptions the script catches (`ProviderError`, `ValueError`, `OSError`): their text.
fn caught(c: &Crash) -> Option<String> {
    let (kind, msg) = c.0.split_once(": ").unwrap_or((&c.0, ""));
    let caught = matches!(
        kind,
        "ValueError"
            | "UnicodeDecodeError"
            | "UnicodeEncodeError"
            | "UnicodeError"
            | "JSONDecodeError"
            | "OSError"
            | "FileNotFoundError"
            | "FileExistsError"
            | "IsADirectoryError"
            | "NotADirectoryError"
            | "PermissionError"
            | "BlockingIOError"
            | "TimeoutError"
            | "InterruptedError"
    );
    caught.then(|| msg.to_string())
}

fn run_main(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "pr_sync.py",
        "Read and answer the pull request on its platform (Phase 7)",
    )
    .commands(
        "command",
        &[
            ("status", "What is pending on the pull request"),
            ("open", "Open pull requests with what is pending"),
            (
                "gate-merge",
                "Fail while a blocking finding, a change request, CI or a conflict holds the merge",
            ),
            ("reply", "Answer a review thread"),
            ("comment", "Comment on the pull request"),
            (
                "resolve",
                "Resolve a review thread (after its fix is pushed or the reviewer agreed)",
            ),
            (
                "review",
                "Post the protocol's code review (line comments with marker and level)",
            ),
            ("request-review", "Ask people to review again"),
        ],
    );
    let args = match parser.parse_command(argv, command_parser) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let command = args.value("command").unwrap_or_default().to_string();
    let ctx = Context::new(args.value("dir"))?;
    let number = match args.value("pr") {
        Some(text) => Some(Value::Int(int(text, 10)?.to_string())),
        None => None,
    };
    match act(&ctx, &command, number, &args) {
        Ok(code) => Ok(code),
        Err(Fail::Provider(msg)) => {
            println(&format!("❌ {msg}"));
            Ok(1)
        }
        Err(Fail::Crash(c)) => match caught(&c) {
            Some(msg) => {
                println(&format!("❌ {msg}"));
                Ok(1)
            }
            None => Err(c),
        },
    }
}

fn act(
    ctx: &Context,
    command: &str,
    number: Option<Value>,
    args: &kernel::py::argparse::Parsed,
) -> Res<u8> {
    if command == "open" {
        let prs = ctx.provider()?.open_pull_requests()?;
        let mut data = Vec::new();
        for pr in prs {
            let p = pending_of(ctx, &pr)?;
            data.push((pr, p));
        }
        if args.flag("json") {
            let mut items = Vec::new();
            for (pr, p) in &data {
                let mut d = Dict::new();
                d.insert("pull_request", pr.to_value());
                d.insert("pending", p.to_value());
                d.insert("summary", s(&summary_line(pr, p)?));
                d.insert("needs_action", Value::Bool(needs_action(p)));
                items.push(Value::Dict(d));
            }
            println(&json::dumps(&Value::List(items), Some(2))?);
        } else {
            for (pr, p) in &data {
                let mark = if needs_action(p) { "🔴" } else { "🟢" };
                println(&format!("{mark} {}", summary_line(pr, p)?));
            }
            if data.is_empty() {
                println("ℹ️ No open pull request.");
            }
        }
        return Ok(0);
    }

    let pr = ctx.pull_request(number)?;
    let number = pr.number.clone();
    if command == "status" {
        let p = pending_of(ctx, &pr)?;
        if args.flag("json") {
            let mut d = Dict::new();
            d.insert("pull_request", pr.to_value());
            d.insert("pending", p.to_value());
            d.insert("summary", s(&summary_line(&pr, &p)?));
            d.insert("needs_action", Value::Bool(needs_action(&p)));
            println(&json::dumps(&Value::Dict(d), Some(2))?);
        } else {
            print_status(&pr, &p)?;
        }
        return Ok(0);
    }
    let provider = ctx.provider()?;
    match command {
        "gate-merge" => return gate_merge(ctx, &provider, &pr),
        "reply" => {
            let body = read_body(args.value("body"), args.value("body_file"))?;
            let text = format_post(&ctx.reply_marker, &s(&body), None)?;
            ctx.guard(&text, "the reply")?;
            let thread = args.value("thread").unwrap_or_default();
            provider.reply(&number, thread, &text)?;
            println(&format!("✅ Replied on thread {thread}"));
        }
        "comment" => {
            let body = read_body(args.value("body"), args.value("body_file"))?;
            let text = format_post(&ctx.reply_marker, &s(&body), None)?;
            ctx.guard(&text, "the comment")?;
            provider.comment(&number, &text)?;
            let summary = summary_line(&pr, &pending_of(ctx, &pr)?)?;
            println(&format!(
                "✅ Commented on {}",
                summary.split(':').next().unwrap_or_default()
            ));
        }
        "resolve" => {
            let thread = args.value("thread").unwrap_or_default();
            provider.resolve(&number, thread)?;
            println(&format!("✅ Resolved thread {thread}"));
        }
        "review" => {
            let (body, comments) = load_review(args.value("file").unwrap_or_default(), ctx)?;
            provider.review(&number, &body, &comments)?;
            println(&format!(
                "✅ Review posted with {} line comment(s)",
                comments.len()
            ));
        }
        "request-review" => {
            let reviewers = args.list("reviewer");
            provider.request_review(&number, &reviewers)?;
            println(&format!(
                "✅ Review requested from {}",
                reviewers.join(", ")
            ));
        }
        _ => {}
    }
    Ok(0)
}
