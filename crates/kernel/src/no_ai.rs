//! The guard of protocol principle 9 (`dev-cycle-protocol/_no_ai.py`): no artifact names an AI,
//! a model or a coding agent as author, co-author, reviewer or source. Everything the protocol
//! posts or commits is checked here first.
//!
//! It looks for attribution, not for a product's subject: "feat: add an OpenAI provider" is fine;
//! "Co-Authored-By: Claude" or "Generated with ..." are not. Code spans and file paths are
//! ignored; the text of Markdown links is checked, and links to a coding agent's own pages are
//! always flagged.

use std::io::Read;
use std::path::Path;
use std::sync::LazyLock;

use crate::out::println;
use crate::py::argparse::Parser;
use crate::py::num::strip;
use crate::py::re::Regex;
use crate::py::{Crash, Value, iterate, lower, read_text, universal_newlines};

const AGENTS: &str = r"(?:claude(?:\s+code)?|chat\s*gpt|gpt[\s-]?\d[\w.]*|openai|anthropic|copilot|gemini|codex|cursor|windsurf|devin|llm|large\s+language\s+model|an?\s+ai|the\s+ai|ai(?:\s+(?:assistant|agent|model|tool))?|(?:the|a|an)\s+(?:coding\s+)?agent|the\s+assistant|artificial\s+intelligence)";

/// Reasons whose pattern is searched in the text as written, not in the text without code.
const RAW_REASONS: [&str; 4] = [
    "session trailer",
    "session link",
    "link to a coding agent",
    "agent attribution marker",
];

static PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    let patterns: Vec<(String, &str)> = vec![
        (
            r"co-authored-by:[^\n]*(?:claude|copilot|chatgpt|gpt|openai|anthropic|gemini|codex|cursor|noreply@anthropic\.com|bot\b)".into(),
            "co-author trailer naming an AI",
        ),
        (r"\bclaude-session:".into(), "session trailer"),
        (r"claude\.ai/code/session".into(), "session link"),
        (
            r"claude\.ai/code(?!/session)\b|claude\.com/claude-code".into(),
            "link to a coding agent",
        ),
        (
            r"claude-projects-attribution".into(),
            "agent attribution marker",
        ),
        ("🤖".into(), "robot attribution emoji"),
        (
            format!(
                r"\b(?:generated|written|created|authored|produced|drafted|made|built|reviewed|suggested|assisted|powered)\s+(?:with|by|using|via)\s+(?:the\s+help\s+of\s+)?{AGENTS}\b"
            ),
            "attribution to an AI",
        ),
        (
            format!(r"\bwith\s+(?:the\s+)?help\s+(?:of|from)\s+{AGENTS}\b"),
            "attribution to an AI",
        ),
        (
            r"\bai[\s-](?:generated|assisted|written|authored|powered\s+review)\b".into(),
            "attribution to an AI",
        ),
        (
            r"\bas\s+an?\s+(?:ai|language\s+model|assistant)\b".into(),
            "speaking as an AI",
        ),
        (
            r"\b(?:claude|chat\s*gpt|copilot|gemini|codex|cursor|the\s+agent|the\s+assistant)\s+(?:found|noticed|suggested|suggests|says|said|thinks|wrote|generated|reviewed|fixed|recommended|recommends)\b".into(),
            "an AI named as the source",
        ),
    ];
    patterns
        .into_iter()
        .map(|(p, reason)| (Regex::fixed(&p, "i"), reason))
        .collect()
});

static AI_IDENTITY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::fixed(
        r"^(?:claude(?:\s+code)?|chat\s*gpt|copilot|gemini|codex|cursor|devin|openai|anthropic)$",
        "i",
    )
});
static AI_EMAIL: LazyLock<Regex> =
    LazyLock::new(|| Regex::fixed(r"@(?:anthropic|openai)\.com$", "i"));

static CODE_BLOCK: LazyLock<Regex> = LazyLock::new(|| Regex::fixed(r"```.*?```", "s"));
static CODE_SPAN: LazyLock<Regex> = LazyLock::new(|| Regex::fixed(r"`[^`\n]*`", ""));
static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::fixed(r"\[([^\]\n]*)\]\([^)\n]*\)", ""));
static EMPHASIS: LazyLock<Regex> =
    LazyLock::new(|| Regex::fixed(r"(?<![^\W_])_+|_+(?![^\W_])", ""));
static PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::fixed(r"(?<![\w/])(?:\.?[\w.-]+/)+[\w.-]+|(?<!\w)\.[\w.-]+\b", ""));

/// `strip_technical(text)`: code spans, code blocks, URLs, file paths and emphasis marks
/// removed; the text of links kept.
pub fn strip_technical(text: &str) -> String {
    let text = CODE_BLOCK.replace(text, " ");
    let text = CODE_SPAN.replace(&text, " ");
    let text = LINK.sub(&text, |m, t| m.group(t, 1).unwrap_or_default());
    // Markdown emphasis, not snake_case.
    let text = EMPHASIS.replace(&text, " ");
    PATH.replace(&text, " ")
}

/// `findings(text, allowed)`: (matched text, reason) for every attribution found; `allowed`
/// phrases (the configured `allowed_terms`, any value) are skipped.
pub fn findings(text: &str, allowed: &Value) -> Result<Vec<(String, &'static str)>, Crash> {
    let plain = strip_technical(text);
    let raw: Vec<char> = text.chars().collect();
    let plain: Vec<char> = plain.chars().collect();
    let mut out = Vec::new();
    for (regex, reason) in PATTERNS.iter() {
        let t = if RAW_REASONS.contains(reason) {
            &raw
        } else {
            &plain
        };
        for m in regex.find_iter(t) {
            let hit = strip(&m.group(t, 0).unwrap_or_default()).to_string();
            if is_allowed(&hit, allowed)? {
                continue;
            }
            out.push((hit, *reason));
        }
    }
    Ok(out)
}

/// `any(a.lower() in hit.lower() for a in allowed or [])`.
fn is_allowed(hit: &str, allowed: &Value) -> Result<bool, Crash> {
    if !allowed.truthy() {
        return Ok(false);
    }
    let hit = lower(hit);
    for a in iterate(allowed)? {
        let Value::Str(a) = &a else {
            return Err(Crash::new(format!(
                "AttributeError: '{}' object has no attribute 'lower'",
                a.type_name()
            )));
        };
        if hit.contains(&lower(a)) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `identity_findings(name, email)`: the identity when a Git author or committer is an AI tool.
pub fn identity_findings(name: &str, email: &str) -> Vec<(String, &'static str)> {
    let n: Vec<char> = strip(name).chars().collect();
    let e: Vec<char> = strip(email).chars().collect();
    if AI_IDENTITY.match_start(&n).is_some() || AI_EMAIL.search(&e).is_some() {
        vec![(format!("{name} <{email}>"), "AI identity")]
    } else {
        Vec::new()
    }
}

/// `check(text, what, allowed)`: `Ok(Err(message))` is the `ValueError` naming each finding.
pub fn check(text: &str, what: &str, allowed: &Value) -> Result<Result<(), String>, Crash> {
    let found = findings(text, allowed)?;
    if found.is_empty() {
        return Ok(Ok(()));
    }
    let listing = found
        .iter()
        .map(|(hit, reason)| format!("'{hit}' ({reason})"))
        .collect::<Vec<_>>()
        .join("; ");
    Ok(Err(format!(
        "{what} mentions an AI, which protocol principle 9 forbids: {listing}"
    )))
}

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "_no_ai.py",
        "Check a text for AI attribution (protocol principle 9) before it is posted",
    )
    .option("--file", "The text to check (stdin when left out)")
    .append_option("--allow", "A phrase that is the project's subject");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let text = match args.value("file") {
        Some(f) if !f.is_empty() => read_text(Path::new(f))?,
        _ => {
            let mut bytes = Vec::new();
            let _ = std::io::stdin().read_to_end(&mut bytes);
            universal_newlines(&String::from_utf8_lossy(&bytes))
        }
    };
    let allowed = Value::List(args.list("allow").into_iter().map(Value::Str).collect());
    let found = findings(&text, &allowed)?;
    for (hit, reason) in &found {
        println(&format!("'{hit}' ({reason})"));
    }
    if !found.is_empty() {
        println(&format!(
            "❌ {} AI attribution(s): remove them before posting (protocol principle 9).",
            found.len()
        ));
        return Ok(1);
    }
    println("✅ No AI attribution.");
    Ok(0)
}
