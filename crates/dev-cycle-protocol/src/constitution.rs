//! `constitution.py`: generates Spec Kit's constitution (.specify/memory/constitution.md) from
//! the project's sources of truth (the principles of software-design and of the cycle, the
//! product's non-functional requirements and constraints, the accepted ADRs), and checks that
//! the file is current: its header records a hash of the generated content and of the sources.

use std::path::{Path, PathBuf};

use architecture_docs::docs::{Project, get, join_value, zero_pad4};
use kernel::out::{err, out, println};
use kernel::project_config::{ConfigError, load_project_config};
use kernel::py::argparse::Parser;
use kernel::py::num::{self, decimal_value, is_space, strip};
use kernel::py::path::{exists, isdir, isfile, join};
use kernel::py::unicode::{is_word, lower};
use kernel::py::{Crash, Value, encode_check, read_text, today, write_text};

const TEMPLATE_MARKERS: [&str; 2] = ["[PROJECT_NAME]", "[PRINCIPLE_1_NAME]"];
const HEADER_START: &str = "<!-- dev-cycle-constitution\n";
const STYLE_SECTIONS: [&str; 4] = [
    "Structure",
    "Dependency rule",
    "Where things go",
    "Common mistakes",
];
const ROMAN: [&str; 8] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII"];

fn constitution() -> String {
    join(&join(".specify", "memory"), "constitution.md")
}

fn hand_written_copy() -> String {
    join(&join(".specify", "memory"), "constitution.hand-written.md")
}

/// `ConstitutionError` (printed after `❌`), or an exception the script does not catch.
pub enum Error {
    Constitution(String),
    Crash(Crash),
}

impl From<Crash> for Error {
    fn from(c: Crash) -> Self {
        Error::Crash(c)
    }
}

impl From<ConfigError> for Error {
    fn from(e: ConfigError) -> Self {
        Error::Crash(e.into_crash())
    }
}

/// `hashlib.sha256(text.encode("utf-8")).hexdigest()`.
fn sha(text: &str) -> Result<String, Crash> {
    encode_check(text)?;
    Ok(kernel::sha256::hex(text.as_bytes()))
}

fn read(path: &str) -> Result<String, Crash> {
    read_text(Path::new(path))
}

fn rstrip(s: &str) -> &str {
    s.trim_end_matches(is_space)
}

/// `re.match(r"^## (?!#)(.+?)\s*$", line).group(1)`.
fn h2_title(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("## ")?;
    if rest.is_empty() || rest.starts_with('#') {
        return None;
    }
    let trimmed = rstrip(rest);
    if trimmed.is_empty() {
        let c = rest.chars().next()?;
        return Some(&rest[..c.len_utf8()]);
    }
    Some(trimmed)
}

/// H2 title (without emoji or decoration) -> body up to the next H2 or a '---' rule, in the
/// order of a Python dict (a repeated title keeps its first place and its last body).
fn h2_sections(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut title: Option<String> = None;
    let mut body: Vec<&str> = Vec::new();
    let close = |out: &mut Vec<(String, String)>, title: &Option<String>, body: &[&str]| {
        if let Some(t) = title {
            let text = strip(&body.join("\n")).to_string();
            match out.iter_mut().find(|(k, _)| k == t) {
                Some(entry) => entry.1 = text,
                None => out.push((t.clone(), text)),
            }
        }
    };
    for line in text.split('\n') {
        let m = h2_title(line);
        if m.is_some() || strip(line) == "---" {
            close(&mut out, &title, &body);
            title = m.map(|g| {
                let kept = g.trim_start_matches(|c: char| !(is_word(c) || c == '('));
                strip(kept).to_string()
            });
            body.clear();
            continue;
        }
        if title.is_some() {
            body.push(line);
        }
    }
    close(&mut out, &title, &body);
    out
}

fn find_section(sections: &[(String, String)], prefix: &str) -> Option<String> {
    let prefix = lower(prefix);
    sections
        .iter()
        .find(|(t, _)| lower(t).starts_with(&prefix))
        .map(|(_, b)| b.clone())
}

fn section<'a>(sections: &'a [(String, String)], name: &str) -> Option<&'a str> {
    sections
        .iter()
        .find(|(t, _)| t == name)
        .map(|(_, b)| b.as_str())
}

/// Headings `##` to `#####` one level down.
fn demote(text: &str) -> String {
    text.split('\n')
        .map(|line| {
            let n = line.len() - line.trim_start_matches('#').len();
            if (2..=5).contains(&n) && line[n..].starts_with(' ') {
                format!("#{line}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `re.sub(r"<!--.*?-->", "", text, flags=re.S).strip()`.
fn strip_comments(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("<!--") {
        let Some(len) = rest[start + 4..].find("-->") else {
            break;
        };
        out.push_str(&rest[..start]);
        rest = &rest[start + 4 + len + 3..];
    }
    out.push_str(rest);
    strip(&out).to_string()
}

struct Sources {
    principles: String,
    simplicity: Option<String>,
    cycle: String,
    style: Option<String>,
    style_name: Value,
    nfr: Option<String>,
    constraints: Option<String>,
    tech_spec: Option<Value>,
    /// (id, title, path) of each accepted ADR.
    adrs: Vec<(Value, String, String)>,
}

/// The text of each source the constitution is built from.
fn collect_sources(root: &str, skills: &Path) -> Result<Sources, Error> {
    let cfg = load_project_config(Some(root))?;
    let skill = |parts: &[&str]| {
        let mut p = skills.to_path_buf();
        for part in parts {
            p = p.join(part);
        }
        p.to_string_lossy().into_owned()
    };
    let design = h2_sections(&read(&skill(&["software-design", "SKILL.md"]))?);
    let protocol = h2_sections(&read(&skill(&["dev-cycle-protocol", "SKILL.md"]))?);
    let principles = find_section(&design, "Universal Principles").filter(|s| !s.is_empty());
    let simplicity = find_section(&design, "Simplicity First");
    let cycle = find_section(&protocol, "Principles").filter(|s| !s.is_empty());
    let (Some(principles), Some(cycle)) = (principles, cycle) else {
        return Err(Error::Constitution(
            "software-design or dev-cycle-protocol SKILL.md no longer has the expected principles sections".into(),
        ));
    };
    let style_name = cfg.section("architecture").get_or_none("style").clone();
    let mut style = None;
    if style_name.truthy() {
        let path = skill(&[
            "software-design",
            "styles",
            &format!("{}.md", style_name.py_str()),
        ]);
        if isfile(&path) {
            let sections = h2_sections(&read(&path)?);
            let parts: Vec<String> = STYLE_SECTIONS
                .iter()
                .filter_map(|name| {
                    section(&sections, name)
                        .filter(|b| !b.is_empty())
                        .map(|b| format!("#### {name}\n\n{b}"))
                })
                .collect();
            style = Some(parts.join("\n\n"));
        }
    }
    let config_root = cfg.root.to_string_lossy().into_owned();
    let value = cfg.section("product").get_or_none("tech_spec");
    let default = Value::str("docs/product/TECH_SPEC.md");
    let tech_rel = if value.truthy() { value } else { &default };
    let tech_path = join_value(&config_root, tech_rel)?;
    let (mut tech_spec, mut nfr, mut constraints) = (None, None, None);
    if isfile(&tech_path) {
        let sections = h2_sections(&read(&tech_path)?);
        tech_spec = Some(tech_rel.clone());
        nfr = find_section(&sections, "Non-Functional Requirements");
        constraints = find_section(&sections, "Constraints");
    }
    let project = Project::new(Some(&config_root))?;
    let mut adrs = Vec::new();
    for adr in project.adrs()? {
        if !get(&adr.meta, "status")?.py_eq(&Value::str("accepted")) {
            continue;
        }
        let id = get(&adr.meta, "id")?.clone();
        let title = get(&adr.meta, "title")?;
        let title = if title.truthy() {
            title.py_str()
        } else {
            adr.h1().unwrap_or_default()
        };
        adrs.push((id, title, project.rel(&adr.path)?));
    }
    Ok(Sources {
        principles,
        simplicity,
        cycle,
        style: style.filter(|s| !s.is_empty()),
        style_name,
        nfr,
        constraints,
        tech_spec,
        adrs,
    })
}

/// Everything except the header and the version line; its hash is the sources hash.
fn render_body(name: &str, s: &Sources) -> String {
    let mut principles: Vec<(String, String, String)> = vec![
        (
            "Design".into(),
            "software-design".into(),
            s.principles.clone(),
        ),
        (
            "Simplicity First".into(),
            "software-design".into(),
            s.simplicity.clone().unwrap_or_default(),
        ),
    ];
    let style_name = s.style_name.py_str();
    if let Some(style) = &s.style {
        principles.push((
            format!("Architecture Style: {style_name}"),
            format!("software-design/styles/{style_name}.md"),
            style.clone(),
        ));
    } else if s.style_name.truthy() {
        principles.push((
            format!("Architecture Style: {style_name}"),
            ".dev-cycle.json".into(),
            format!("The project follows the `{style_name}` style recorded in `.dev-cycle.json`."),
        ));
    }
    principles.push((
        "Development Cycle".into(),
        "dev-cycle-protocol".into(),
        s.cycle.clone(),
    ));

    let mut out: Vec<String> = vec![
        format!("# {name} Constitution"),
        String::new(),
        "The non-negotiable rules every specification, plan and task of this project follows. It is generated from the \
         project's sources of truth by `dev-cycle-protocol/constitution.py`; change a source and regenerate it, never edit it."
            .into(),
        String::new(),
        "## Core Principles".into(),
        String::new(),
    ];
    for (i, (title, source, body)) in principles.iter().enumerate() {
        out.extend([
            format!("### {}. {title}", ROMAN[i]),
            String::new(),
            format!("*Source: {source}*"),
            String::new(),
            demote(&strip_comments(body)),
            String::new(),
        ]);
    }

    let product: Vec<(&str, &String)> = [
        ("Non-Functional Requirements", &s.nfr),
        ("Constraints", &s.constraints),
    ]
    .into_iter()
    .filter_map(|(t, b)| b.as_ref().filter(|b| !b.is_empty()).map(|b| (t, b)))
    .collect();
    if !product.is_empty() {
        let tech = s.tech_spec.as_ref().map_or("None".into(), Value::py_str);
        out.extend([
            "## Product Requirements and Constraints".into(),
            String::new(),
            format!("*Source: {tech}*"),
            String::new(),
        ]);
        for (title, body) in product {
            out.extend([
                format!("### {title}"),
                String::new(),
                demote(&strip_comments(body)),
                String::new(),
            ]);
        }
    }

    if !s.adrs.is_empty() {
        out.extend([
            "## Architecture Decisions".into(),
            String::new(),
            "Accepted decisions; a plan that contradicts one needs a new ADR that supersedes it."
                .into(),
            String::new(),
        ]);
        for (id, title, path) in &s.adrs {
            let number = match id {
                Value::Int(text) => Some(zero_pad4(text)),
                Value::Bool(b) => Some(zero_pad4(if *b { "1" } else { "0" })),
                _ => None,
            };
            out.push(match number {
                Some(n) => format!("- ADR-{n}: {title} (`{path}`)"),
                None => format!("- {title} (`{path}`)"),
            });
        }
        out.push(String::new());
    }

    out.extend([
        "## Governance".into(),
        String::new(),
        "- This constitution supersedes other practices for specifications, plans and tasks. A plan passes the Constitution \
         Check or records each justified violation in its Complexity Tracking table; a lasting exception becomes an ADR."
            .into(),
        "- Amendments change the source, not this file: an ADR for decisions, the technical specification for requirements \
         and constraints, `.dev-cycle.json` for the architecture style, the skills for principles. Then run \
         `constitution.py generate` and commit the result with the change."
            .into(),
        "- Gate 3 of the development cycle blocks a plan while this file is stale or edited by hand.".into(),
        String::new(),
    ]);
    out.join("\n")
}

/// `dict(META_RE.findall(block))`: `^([a-z0-9-]+):\s*(.+)$` per line, `\s*` may cross lines.
fn meta_lines(block: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut pos = 0;
    while pos <= block.len() {
        let at_line_start = pos == 0 || block[..pos].ends_with('\n');
        let mut matched = None;
        if at_line_start {
            let rest = &block[pos..];
            let key_len = rest
                .find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
                .unwrap_or(rest.len());
            if key_len > 0 && rest[key_len..].starts_with(':') {
                let after = &rest[key_len + 1..];
                let spaces: Vec<usize> = std::iter::once(0)
                    .chain(
                        after
                            .char_indices()
                            .take_while(|(_, c)| is_space(*c))
                            .map(|(i, c)| i + c.len_utf8()),
                    )
                    .collect();
                for &k in spaces.iter().rev() {
                    let value = &after[k..];
                    let line = &value[..value.find('\n').unwrap_or(value.len())];
                    if !line.is_empty() {
                        let end = pos + key_len + 1 + k + line.len();
                        matched = Some((rest[..key_len].to_string(), line.to_string(), end));
                        break;
                    }
                }
            }
        }
        match matched {
            Some((key, value, end)) => {
                match out.iter_mut().find(|(k, _)| *k == key) {
                    Some(entry) => entry.1 = value,
                    None => out.push((key, value)),
                }
                pos = end;
            }
            None => match block[pos..].find('\n') {
                Some(i) => pos += i + 1,
                None => break,
            },
        }
    }
    out
}

type Meta = Vec<(String, String)>;

fn meta_get<'a>(meta: &'a Meta, key: &str) -> Option<&'a str> {
    meta.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// `parse_header(text)`: the header's fields (None without a header) and the content after it.
fn parse_header(text: &str) -> (Option<Meta>, &str) {
    if !text.starts_with(HEADER_START) {
        return (None, text);
    }
    let start = HEADER_START.len();
    let Some(i) = text[start..].find("\n-->\n") else {
        return (None, text);
    };
    let block = &text[start..start + i];
    (
        Some(meta_lines(block)),
        &text[start + i + "\n-->\n".len()..],
    )
}

/// `version_info(content)`: (major, minor, patch digits), ratified, last amended.
fn version_info(content: &str) -> Option<([String; 3], String, String)> {
    let digits = |s: &str| -> Option<(String, usize)> {
        let n = s
            .char_indices()
            .find(|(_, c)| decimal_value(*c).is_none())
            .map_or(s.len(), |(i, _)| i);
        (n > 0).then(|| (s[..n].to_string(), n))
    };
    let nonspace = |s: &str| -> Option<(String, usize)> {
        let n = s.find(is_space).unwrap_or(s.len());
        (n > 0).then(|| (s[..n].to_string(), n))
    };
    for line in content.split('\n') {
        let Some(rest) = line.strip_prefix("**Version**: ") else {
            continue;
        };
        let parse = || -> Option<([String; 3], String, String)> {
            let (major, n) = digits(rest)?;
            let rest = rest[n..].strip_prefix('.')?;
            let (minor, n) = digits(rest)?;
            let rest = rest[n..].strip_prefix('.')?;
            let (patch, n) = digits(rest)?;
            let rest = rest[n..].strip_prefix(" | **Ratified**: ")?;
            let (ratified, n) = nonspace(rest)?;
            let rest = rest[n..].strip_prefix(" | **Last Amended**: ")?;
            let (last, n) = nonspace(rest)?;
            rest[n..]
                .is_empty()
                .then_some(([major, minor, patch], ratified, last))
        };
        if let Some(info) = parse() {
            return Some(info);
        }
    }
    None
}

/// `int(digits)` as text.
fn int_text(digits: &str) -> Result<num::BigInt, Crash> {
    num::int(digits, 10)
}

fn body_of(root: &str, skills: &Path) -> Result<String, Error> {
    let cfg = load_project_config(Some(root))?;
    let sources = collect_sources(root, skills)?;
    Ok(render_body(&cfg.project_name(), &sources))
}

/// `build(root, previous)`: (text, changed).
fn build(root: &str, skills: &Path, previous: Option<&str>) -> Result<(String, bool), Error> {
    let today = today();
    let body = body_of(root, skills)?;
    let sources_hash = sha(&body)?;
    let (meta, old_content) = match previous.filter(|p| !p.is_empty()) {
        Some(p) => parse_header(p),
        None => (None, ""),
    };
    let meta = meta.filter(|m| !m.is_empty());
    let info = if meta.is_some() {
        version_info(old_content)
    } else {
        None
    };
    if let Some(meta) = &meta {
        let untouched = meta_get(meta, "content-sha256") == Some(sha(old_content)?.as_str());
        if untouched
            && meta_get(meta, "sources-sha256") == Some(sources_hash.as_str())
            && info.is_some()
        {
            return Ok((previous.unwrap_or_default().to_string(), false));
        }
    }
    let (version, ratified, last) = match info {
        Some(([major, minor, _], ratified, _)) => {
            let major = int_text(&major)?;
            let minor = int_text(&minor)?.add(&num::BigInt::from_i64(1));
            (format!("{major}.{minor}.0"), ratified, today)
        }
        None => ("1.0.0".to_string(), today.clone(), today),
    };
    let content = format!(
        "{body}\n**Version**: {version} | **Ratified**: {ratified} | **Last Amended**: {last}\n"
    );
    let header = format!(
        "<!-- dev-cycle-constitution\n\
         generated-by: dev-cycle-protocol/constitution.py (do not edit; change a source and regenerate)\n\
         sources-sha256: {sources_hash}\n\
         content-sha256: {}\n\
         -->\n",
        sha(&content)?
    );
    Ok((header + &content, true))
}

/// generated (by this script), template (Spec Kit's untouched placeholder) or hand-written.
fn kind_of(text: &str) -> &'static str {
    if parse_header(text).0.is_some() {
        return "generated";
    }
    if TEMPLATE_MARKERS.iter().any(|m| text.contains(m)) {
        return "template";
    }
    "hand-written"
}

fn path_of(root: &str) -> String {
    join(root, &constitution())
}

/// Problems with the project's constitution; empty when it is current.
pub fn check(root: &str, skills: &Path) -> Result<Vec<String>, Error> {
    let c = constitution();
    if !isdir(&join(root, ".specify")) {
        return Ok(vec![
            "Spec Kit is not initialized (.specify/ missing)".into(),
        ]);
    }
    let path = path_of(root);
    if !isfile(&path) {
        return Ok(vec![format!(
            "{c} is missing: run constitution.py generate"
        )]);
    }
    let text = read(&path)?;
    match kind_of(&text) {
        "template" => {
            return Ok(vec![format!(
                "{c} is still Spec Kit's template: run constitution.py generate"
            )]);
        }
        "hand-written" => {
            return Ok(vec![format!(
                "{c} is hand-written (Spec Kit used without the protocol): move its principles into the sources \
                 (TECH_SPEC.md, ADRs) with the user, then run constitution.py generate --replace-hand-written"
            )]);
        }
        _ => {}
    }
    let (meta, content) = parse_header(&text);
    let meta = meta.unwrap_or_default();
    let mut problems = Vec::new();
    if meta_get(&meta, "content-sha256") != Some(sha(content)?.as_str()) {
        problems.push(format!(
            "{c} was edited by hand: change the source instead and run constitution.py generate"
        ));
    }
    let current = sha(&body_of(root, skills)?)?;
    if meta_get(&meta, "sources-sha256") != Some(current.as_str()) {
        problems.push(format!(
            "{c} is stale: a source changed (principles, style, technical specification or ADRs); \
             run constitution.py generate"
        ));
    }
    Ok(problems)
}

fn generate(
    root: &str,
    skills: &Path,
    stdout: bool,
    replace_hand_written: bool,
) -> Result<u8, Error> {
    let c = constitution();
    if !isdir(&join(root, ".specify")) && !stdout {
        return Err(Error::Constitution(
            "Spec Kit is not initialized (.specify/ missing): run speckit_setup.py init first"
                .into(),
        ));
    }
    let path = path_of(root);
    let mut previous = if isfile(&path) {
        Some(read(&path)?)
    } else {
        None
    };
    if let Some(prev) = &previous
        && kind_of(prev) == "hand-written"
        && !stdout
    {
        if !replace_hand_written {
            return Err(Error::Constitution(format!(
                "{c} is hand-written. Move its principles into the sources first (constraints and \
                 non-functional requirements to TECH_SPEC.md, decisions to ADRs, general design rules as a suggestion \
                 for software-design), then run generate --replace-hand-written; a copy is kept."
            )));
        }
        let copy = join(root, &hand_written_copy());
        if !exists(&copy) {
            write_text(Path::new(&copy), prev)?;
        }
        println(&format!(
            "📦 Hand-written constitution kept in {}",
            hand_written_copy()
        ));
        previous = None;
    }
    if let Some(prev) = &previous
        && let (Some(meta), content) = parse_header(prev)
        && meta_get(&meta, "content-sha256") != Some(sha(content)?.as_str())
    {
        println(&format!(
            "⚠️ {c} was edited by hand; the edits are replaced. Move them into the right source."
        ));
    }
    let (text, changed) = build(root, skills, previous.as_deref())?;
    if stdout {
        out(&text);
        return Ok(0);
    }
    if !changed {
        println(&format!("✅ {c} is current."));
        return Ok(0);
    }
    write_text(Path::new(&path), &text)?;
    let ([major, minor, _], _, _) = version_info(parse_header(&text).1).unwrap_or_default();
    println(&format!(
        "✅ {c} generated ({}.{}.0); commit it with the change that caused it.",
        int_text(&major)?,
        int_text(&minor)?
    ));
    Ok(0)
}

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "constitution.py",
        "Generate or check Spec Kit's constitution from the project's sources",
    )
    .positional_choice("command", &["generate", "check"], "")
    .option(
        "--dir",
        "Project directory (default: current; the Git root is used)",
    )
    .flag("--stdout", "generate: print instead of writing")
    .flag(
        "--replace-hand-written",
        "generate: replace a hand-written constitution, keeping a copy, after its principles were moved",
    );
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let project_root = Project::new(args.value("dir"))?.root;
    let root = load_project_config(Some(&project_root))
        .map_err(ConfigError::into_crash)?
        .root
        .to_string_lossy()
        .into_owned();
    let Some(skills): Option<PathBuf> = kernel::skills::root() else {
        err(&format!(
            "dev-cycle: cannot find the skills folder; set {} to it\n",
            kernel::skills::ENV
        ));
        return Ok(2);
    };
    let result = if args.value("command") == Some("check") {
        check(&root, &skills).map(|problems| {
            for p in &problems {
                println(&format!("❌ {p}"));
            }
            let summary = if problems.is_empty() {
                "✅ constitution: current.".to_string()
            } else {
                format!("❌ constitution: {} problem(s).", problems.len())
            };
            println(&summary);
            u8::from(!problems.is_empty())
        })
    } else {
        generate(
            &root,
            &skills,
            args.flag("stdout"),
            args.flag("replace_hand_written"),
        )
    };
    match result {
        Ok(code) => Ok(code),
        Err(Error::Constitution(e)) => {
            println(&format!("❌ {e}"));
            Ok(1)
        }
        Err(Error::Crash(c)) => Err(c),
    }
}
