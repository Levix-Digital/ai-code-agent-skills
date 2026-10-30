//! Pull request model (`_pr_model.py`): the platform-independent shape of a pull request (a
//! merge request on GitLab), what is pending on it, triage hints for review threads and the
//! format of what the protocol posts. The providers (`pr_providers`) build the model,
//! `pr_sync` prints and acts.
//!
//! The values the platform returned (ids, authors, bodies, dates) are kept as Python held them,
//! of any JSON type: the checks and messages treat them as the script did.

use std::sync::LazyLock;

use kernel::py::num::{is_space, strip};
use kernel::py::re::Regex;
use kernel::py::{Crash, Dict, Value, join};

pub const REVIEW_MARKER: &str = "[review]";
pub const REPLY_MARKER: &str = "[reply]";
pub const LEVELS: [&str; 3] = ["blocking", "suggestion", "nit"];

static SUGGESTION_RE: LazyLock<Regex> = LazyLock::new(|| Regex::fixed(r"```suggestion\b", "i"));
static NIT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::fixed(
        r"^\s*(\*\*\[review\]\*\*\s*`?nit`?|nit\b|nitpick\b|minor\b)",
        "i",
    )
});
static LARGE_WORDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::fixed(
        r"\b(design|architecture|architectural|refactor|rewrite|api|schema|migration|interface|breaking|approach|rethink|split (this|the) (pr|change)|whole|across)\b",
        "i",
    )
});
static CODE_BLOCK: LazyLock<Regex> = LazyLock::new(|| Regex::fixed(r"```.*?```", "s"));

// ---------------------------------------------------------------- Python semantics

pub(crate) fn attr_error(v: &Value, attr: &str) -> Crash {
    Crash::new(format!(
        "AttributeError: '{}' object has no attribute '{attr}'",
        v.type_name()
    ))
}

/// `value.get(key)`: `None` when absent; `AttributeError` when the value is not a dict.
pub(crate) fn get(v: &Value, key: &str) -> Result<Value, Crash> {
    match v {
        Value::Dict(d) => Ok(d.get(key).cloned().unwrap_or(Value::Null)),
        other => Err(attr_error(other, "get")),
    }
}

/// `a or b`.
pub(crate) fn or(a: Value, b: Value) -> Value {
    if a.truthy() { a } else { b }
}

/// `(value or "")` for a method of `str`: the text, or the `AttributeError` of a value that is
/// not one.
pub(crate) fn text_or_empty<'a>(v: &'a Value, method: &str) -> Result<&'a str, Crash> {
    match v {
        _ if !v.truthy() => Ok(""),
        Value::Str(s) => Ok(s),
        other => Err(attr_error(other, method)),
    }
}

pub(crate) fn unhashable(v: &Value) -> Result<(), Crash> {
    if v.is_hashable() {
        Ok(())
    } else {
        Err(Crash::new(format!(
            "TypeError: unhashable type: '{}'",
            v.type_name()
        )))
    }
}

/// A number as Python compares it.
enum Num {
    Int(i128),
    Float(f64),
}

fn num(v: &Value) -> Option<Num> {
    match v {
        Value::Bool(b) => Some(Num::Int(i128::from(*b))),
        Value::Int(i) => Some(match i.parse::<i128>() {
            Ok(n) => Num::Int(n),
            Err(_) => Num::Float(i.parse::<f64>().unwrap_or(0.0)),
        }),
        Value::Float(f) => Some(Num::Float(*f)),
        _ => None,
    }
}

/// `a < b` (`op` names the operator in the error).
pub(crate) fn less(a: &Value, b: &Value, op: &str) -> Result<bool, Crash> {
    if let (Some(x), Some(y)) = (num(a), num(b)) {
        return Ok(match (x, y) {
            (Num::Int(x), Num::Int(y)) => x < y,
            (Num::Int(x), Num::Float(y)) => (x as f64) < y,
            (Num::Float(x), Num::Int(y)) => x < y as f64,
            (Num::Float(x), Num::Float(y)) => x < y,
        });
    }
    match (a, b) {
        (Value::Str(x), Value::Str(y)) => Ok(x < y),
        (Value::List(x), Value::List(y)) => {
            for (p, q) in x.iter().zip(y) {
                if !p.py_eq(q) {
                    return less(p, q, op);
                }
            }
            Ok(x.len() < y.len())
        }
        _ => Err(Crash::new(format!(
            "TypeError: '{op}' not supported between instances of '{}' and '{}'",
            a.type_name(),
            b.type_name()
        ))),
    }
}

/// `sorted(items)`: CPython's sort of a short list (a natural run, then binary insertion),
/// so that values that cannot be compared fail on the same pair.
pub(crate) fn sorted(items: Vec<Value>) -> Result<Vec<Value>, Crash> {
    let pairs = items.into_iter().map(|v| (v.clone(), v)).collect();
    Ok(sort_keyed(pairs)?.into_iter().map(|(_, v)| v).collect())
}

/// `sorted(items, key=...)` with the keys already computed, in order: stable.
pub(crate) fn sort_keyed<T: Clone>(mut items: Vec<(Value, T)>) -> Result<Vec<(Value, T)>, Crash> {
    let n = items.len();
    if n < 2 {
        return Ok(items);
    }
    let lt = |a: &(Value, T), b: &(Value, T)| less(&a.0, &b.0, "<");
    // count_run
    let mut run = 1;
    while run < n && !lt(&items[run], &items[run - 1])? {
        run += 1;
    }
    if run < n {
        let mut descending = run == 1;
        if run > 1 && !lt(&items[0], &items[run - 1])? {
            items[..run].reverse();
            descending = true;
        }
        if descending {
            run += 1;
            let mut neq = 0usize;
            while run < n {
                if lt(&items[run], &items[run - 1])? {
                    if neq > 0 {
                        items[run - neq - 1..run].reverse();
                        neq = 0;
                    }
                } else if lt(&items[run - 1], &items[run])? {
                    break;
                } else {
                    neq += 1;
                }
                run += 1;
            }
            if neq > 0 {
                items[run - neq - 1..run].reverse();
            }
            items[..run].reverse();
            while run < n && !lt(&items[run], &items[run - 1])? {
                run += 1;
            }
        }
    }
    // binarysort
    for start in run..n {
        let pivot = items[start].clone();
        let (mut l, mut r) = (0, start);
        while l < r {
            let p = l + ((r - l) >> 1);
            if lt(&pivot, &items[p])? {
                r = p;
            } else {
                l = p + 1;
            }
        }
        items[l..=start].rotate_right(1);
    }
    Ok(items)
}

// ---------------------------------------------------------------- the model

#[derive(Debug, Clone)]
pub struct Comment {
    pub id: Value,
    pub author: Value,
    pub body: Value,
    pub created: Value,
}

#[derive(Debug, Clone)]
pub struct Thread {
    pub id: Value,
    pub path: Value,
    pub line: Value,
    /// `None` when the platform could not say: it counts as open.
    pub resolved: Option<bool>,
    pub outdated: bool,
    pub comments: Vec<Comment>,
}

#[derive(Debug, Clone)]
pub struct Check {
    pub name: Value,
    pub status: &'static str,
    pub url: Value,
}

#[derive(Debug, Clone)]
pub struct Review {
    pub author: Value,
    pub state: &'static str,
}

#[derive(Debug, Clone)]
pub struct PullRequest {
    pub provider: &'static str,
    pub number: Value,
    pub url: Value,
    pub title: Value,
    pub state: Value,
    pub draft: bool,
    pub author: Value,
    pub base: Value,
    pub head: Value,
    pub mergeable: &'static str,
    pub checks: Vec<Check>,
    pub reviews: Vec<Review>,
    pub threads: Vec<Thread>,
    pub comments: Vec<Comment>,
    /// GitHub: why the resolution of the threads could not be read (the key is there, `None`
    /// when it could be read).
    pub threads_note: Option<Option<String>>,
    /// GitLab only.
    pub diff_refs: Option<Value>,
}

fn s(text: &str) -> Value {
    Value::Str(text.to_string())
}

impl Comment {
    pub fn to_value(&self) -> Value {
        let mut d = Dict::new();
        d.insert("id", self.id.clone());
        d.insert("author", self.author.clone());
        d.insert("body", self.body.clone());
        d.insert("created", self.created.clone());
        Value::Dict(d)
    }
}

impl Thread {
    fn dict(&self) -> Dict {
        let mut d = Dict::new();
        d.insert("id", self.id.clone());
        d.insert("path", self.path.clone());
        d.insert("line", self.line.clone());
        d.insert("resolved", self.resolved.map_or(Value::Null, Value::Bool));
        d.insert("outdated", Value::Bool(self.outdated));
        d.insert(
            "comments",
            Value::List(self.comments.iter().map(Comment::to_value).collect()),
        );
        d
    }

    fn first_body(&self) -> Value {
        or(
            self.comments
                .first()
                .map_or(Value::Null, |c| c.body.clone()),
            s(""),
        )
    }

    fn last_body(&self) -> Value {
        or(
            self.comments.last().map_or(Value::Null, |c| c.body.clone()),
            s(""),
        )
    }
}

impl Check {
    fn to_value(&self) -> Value {
        let mut d = Dict::new();
        d.insert("name", self.name.clone());
        d.insert("status", s(self.status));
        d.insert("url", self.url.clone());
        Value::Dict(d)
    }
}

impl PullRequest {
    pub fn to_value(&self) -> Value {
        let mut d = Dict::new();
        d.insert("provider", s(self.provider));
        d.insert("number", self.number.clone());
        d.insert("url", self.url.clone());
        d.insert("title", self.title.clone());
        d.insert("state", self.state.clone());
        d.insert("draft", Value::Bool(self.draft));
        d.insert("author", self.author.clone());
        d.insert("base", self.base.clone());
        d.insert("head", self.head.clone());
        d.insert("mergeable", s(self.mergeable));
        d.insert(
            "checks",
            Value::List(self.checks.iter().map(Check::to_value).collect()),
        );
        d.insert(
            "reviews",
            Value::List(
                self.reviews
                    .iter()
                    .map(|r| {
                        let mut x = Dict::new();
                        x.insert("author", r.author.clone());
                        x.insert("state", s(r.state));
                        Value::Dict(x)
                    })
                    .collect(),
            ),
        );
        d.insert(
            "threads",
            Value::List(self.threads.iter().map(|t| Value::Dict(t.dict())).collect()),
        );
        d.insert(
            "comments",
            Value::List(self.comments.iter().map(Comment::to_value).collect()),
        );
        if let Some(note) = &self.threads_note {
            d.insert("threads_note", note.clone().map_or(Value::Null, Value::Str));
        }
        if let Some(refs) = &self.diff_refs {
            d.insert("diff_refs", refs.clone());
        }
        Value::Dict(d)
    }

    /// `pr.get("threads_note")` when it is set.
    pub fn note(&self) -> Option<&str> {
        self.threads_note.as_ref().and_then(|n| n.as_deref())
    }
}

/// What needs action on the pull request, by kind.
#[derive(Debug, Clone)]
pub struct Pending {
    /// Threads waiting, each with its triage hint.
    pub threads: Vec<(Thread, &'static str)>,
    pub comments: Vec<Comment>,
    pub changes_requested: Vec<Value>,
    pub approved: Vec<Value>,
    pub failed_checks: Vec<Check>,
    pub running_checks: Vec<Check>,
    pub conflict: bool,
    pub draft: bool,
}

impl Pending {
    pub fn to_value(&self) -> Value {
        let mut d = Dict::new();
        d.insert(
            "threads",
            Value::List(
                self.threads
                    .iter()
                    .map(|(t, hint)| {
                        let mut x = t.dict();
                        x.insert("hint", s(hint));
                        Value::Dict(x)
                    })
                    .collect(),
            ),
        );
        d.insert(
            "comments",
            Value::List(self.comments.iter().map(Comment::to_value).collect()),
        );
        d.insert(
            "changes_requested",
            Value::List(self.changes_requested.clone()),
        );
        d.insert("approved", Value::List(self.approved.clone()));
        d.insert(
            "failed_checks",
            Value::List(self.failed_checks.iter().map(Check::to_value).collect()),
        );
        d.insert(
            "running_checks",
            Value::List(self.running_checks.iter().map(Check::to_value).collect()),
        );
        d.insert("conflict", Value::Bool(self.conflict));
        d.insert("draft", Value::Bool(self.draft));
        Value::Dict(d)
    }
}

// ---------------------------------------------------------------- functions

/// `marked(body, marker)`: the text starts with the bold marker.
pub fn marked(body: &Value, marker: &Value) -> Result<bool, Crash> {
    let text = text_or_empty(body, "lstrip")?;
    Ok(text
        .trim_start_matches(is_space)
        .starts_with(&format!("**{}**", marker.py_str())))
}

/// `format_post(marker, body, level)`: the marker, the level for review findings, the text.
pub fn format_post(marker: &Value, body: &Value, level: Option<&str>) -> Result<String, Crash> {
    let Value::Str(body) = body else {
        return Err(attr_error(body, "strip"));
    };
    let body = strip(body);
    Ok(match level {
        Some(level) => format!("**{}** `{level}` {body}", marker.py_str()),
        None => format!("**{}** {body}", marker.py_str()),
    })
}

/// An open thread whose last word is not the protocol's answer.
fn thread_waiting(thread: &Thread, reply_marker: &Value) -> Result<bool, Crash> {
    if thread.resolved == Some(true) {
        return Ok(false);
    }
    match thread.comments.last() {
        None => Ok(false),
        Some(last) => Ok(!marked(&last.body, reply_marker)?),
    }
}

/// A thread opened by the protocol's review with the level `blocking`.
fn is_blocking_finding(thread: &Thread, review_marker: &Value) -> Result<bool, Crash> {
    let first = thread.first_body();
    if !marked(&first, review_marker)? {
        return Ok(false);
    }
    let text = first.as_str().unwrap_or_default();
    Ok(text.split('\n').next().unwrap_or("").contains("`blocking`"))
}

fn search_text(v: &Value) -> Result<&str, Crash> {
    match v {
        Value::Str(s) => Ok(s),
        other => Err(Crash::new(format!(
            "TypeError: expected string or bytes-like object, got '{}'",
            other.type_name()
        ))),
    }
}

/// `triage(thread)`: suggestion | nit | question | large | small.
fn triage(thread: &Thread, review_marker: &Value) -> Result<&'static str, Crash> {
    let first = thread.first_body();
    let last = thread.last_body();
    let text_value = if thread.comments.len() > 1 {
        &last
    } else {
        &first
    };
    let text = search_text(text_value)?;
    if SUGGESTION_RE.is_found(text) {
        return Ok("suggestion");
    }
    if is_blocking_finding(thread, review_marker)? {
        let first = search_text(&first)?;
        return Ok(if LARGE_WORDS.is_found(first) {
            "large"
        } else {
            "small"
        });
    }
    if NIT_RE.is_found(text) {
        return Ok("nit");
    }
    let removed = CODE_BLOCK.replace(text, "");
    let plain = strip(&removed);
    if LARGE_WORDS.is_found(plain) || plain.matches('\n').count() > 8 {
        return Ok("large");
    }
    if plain.ends_with('?') {
        return Ok("question");
    }
    Ok("small")
}

/// author -> the state of their latest review that is not a plain comment.
fn latest_reviews(pr: &PullRequest) -> Result<Dict, Crash> {
    let mut out = Dict::new();
    for review in &pr.reviews {
        if matches!(review.state, "approved" | "changes_requested" | "dismissed") {
            unhashable(&review.author)?;
            out.insert_value(review.author.clone(), s(review.state));
        }
    }
    Ok(out)
}

fn authors_in(reviews: &Dict, state: &str) -> Result<Vec<Value>, Crash> {
    sorted(
        reviews
            .iter()
            .filter(|(_, v)| v.as_str() == Some(state))
            .map(|(k, _)| k.clone())
            .collect(),
    )
}

/// `pending(pr, reply_marker, review_marker)`.
pub fn pending(
    pr: &PullRequest,
    reply_marker: &Value,
    review_marker: &Value,
) -> Result<Pending, Crash> {
    let mut waiting = Vec::new();
    for t in &pr.threads {
        if thread_waiting(t, reply_marker)? {
            waiting.push(t);
        }
    }
    let mut comments = Vec::new();
    for c in &pr.comments {
        if !marked(&c.body, reply_marker)? && !marked(&c.body, review_marker)? {
            comments.push(c.clone());
        }
    }
    let mut answered_after = Value::Null;
    for c in &pr.comments {
        if marked(&c.body, reply_marker)? {
            answered_after = c.created.clone();
        }
    }
    if answered_after.truthy() {
        let mut kept = Vec::new();
        for c in comments {
            let created = or(c.created.clone(), s(""));
            if less(&answered_after, &created, ">")
                .map_err(|_| compare_error(&created, &answered_after))?
            {
                kept.push(c);
            }
        }
        comments = kept;
    }
    let reviews = latest_reviews(pr)?;
    let mut threads = Vec::new();
    for t in waiting {
        threads.push((t.clone(), triage(t, review_marker)?));
    }
    Ok(Pending {
        threads,
        comments,
        changes_requested: authors_in(&reviews, "changes_requested")?,
        approved: authors_in(&reviews, "approved")?,
        failed_checks: pr
            .checks
            .iter()
            .filter(|c| c.status == "failure")
            .cloned()
            .collect(),
        running_checks: pr
            .checks
            .iter()
            .filter(|c| c.status == "pending")
            .cloned()
            .collect(),
        conflict: pr.mergeable == "conflict",
        draft: pr.draft,
    })
}

/// `a > b` fails naming the operands in that order.
fn compare_error(a: &Value, b: &Value) -> Crash {
    Crash::new(format!(
        "TypeError: '>' not supported between instances of '{}' and '{}'",
        a.type_name(),
        b.type_name()
    ))
}

/// Why the pull request cannot be merged yet; empty when it can.
pub fn merge_blockers(pr: &PullRequest, review_marker: &Value) -> Result<Vec<String>, Crash> {
    let mut out = Vec::new();
    if !pr.state.py_eq(&s("open")) {
        out.push(format!("the pull request is {}", pr.state.py_str()));
    }
    if pr.draft {
        out.push("the pull request is a draft".to_string());
    }
    for t in &pr.threads {
        if t.resolved == Some(true) || !is_blocking_finding(t, review_marker)? {
            continue;
        }
        let place = format!(
            "{}:{} (thread {})",
            t.path.py_str(),
            t.line.py_str(),
            t.id.py_str()
        );
        out.push(if t.resolved == Some(false) {
            format!("blocking finding still open at {place}")
        } else {
            format!(
                "blocking finding at {place}: whether it is resolved could not be read; check it on the pull request page"
            )
        });
    }
    for author in authors_in(&latest_reviews(pr)?, "changes_requested")? {
        out.push(format!("changes requested by {}", author.py_str()));
    }
    for c in &pr.checks {
        if c.status == "failure" {
            out.push(format!("check {} failed", c.name.py_str()));
        } else if c.status == "pending" {
            out.push(format!("check {} still running", c.name.py_str()));
        }
    }
    if pr.mergeable == "conflict" {
        out.push("merge conflict with the base branch".to_string());
    }
    Ok(out)
}

pub fn needs_action(p: &Pending) -> bool {
    !p.threads.is_empty()
        || !p.comments.is_empty()
        || !p.changes_requested.is_empty()
        || !p.failed_checks.is_empty()
        || p.conflict
}

fn plural(n: usize) -> &'static str {
    if n != 1 { "s" } else { "" }
}

/// One line for the diagnosis: 'PR #12: 3 threads waiting (1 for you), CI red, conflict'.
pub fn summary_line(pr: &PullRequest, p: &Pending) -> Result<String, Crash> {
    let mut parts = Vec::new();
    if !p.threads.is_empty() {
        let n = p.threads.len();
        let large = p.threads.iter().filter(|(_, h)| *h == "large").count();
        let mut part = format!("{n} thread{} waiting", plural(n));
        if large > 0 {
            part.push_str(&format!(" ({large} for you)"));
        }
        parts.push(part);
    }
    if !p.comments.is_empty() {
        let n = p.comments.len();
        parts.push(format!("{n} comment{} to answer", plural(n)));
    }
    if !p.changes_requested.is_empty() {
        parts.push(format!(
            "changes requested by {}",
            join(", ", &Value::List(p.changes_requested.clone()))?
        ));
    }
    if !p.failed_checks.is_empty() {
        parts.push("CI red".to_string());
    } else if !p.running_checks.is_empty() {
        parts.push("CI running".to_string());
    }
    if p.conflict {
        parts.push("merge conflict".to_string());
    }
    if parts.is_empty() {
        parts.push(
            if p.approved.is_empty() {
                "waiting for review"
            } else {
                "approved"
            }
            .to_string(),
        );
    }
    let kind = if pr.provider == "gitlab" {
        "MR !"
    } else {
        "PR #"
    };
    let title = or(pr.title.clone(), s("")).py_str();
    let head = format!("{kind}{} {title}", pr.number.py_str());
    Ok(format!(
        "{}: {}",
        head.trim_end_matches(is_space),
        parts.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_like_cpython() {
        let v = |x: &[&str]| {
            x.iter()
                .map(|s| Value::Str(s.to_string()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            sorted(v(&["b", "a", "c", "a"]))
                .unwrap()
                .iter()
                .map(Value::py_str)
                .collect::<Vec<_>>(),
            ["a", "a", "b", "c"]
        );
        let mixed = vec![Value::Str("a".into()), Value::Null];
        assert_eq!(
            sorted(mixed).unwrap_err().0,
            "TypeError: '<' not supported between instances of 'NoneType' and 'str'"
        );
    }
}
