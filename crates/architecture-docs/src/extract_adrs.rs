//! `extract_adrs.py`: migration helper that copies every "ADR-NNN: Title" section of a legacy
//! single-file technical document (e.g. TECDOC.md) into docs/adr/NNNN-<slug>.md, one file per
//! ADR. Rectification notes become `amends` / `amended_by`, relative links are rewritten for the
//! new location, and `date` comes from the commit that introduced the heading. The source
//! document is not modified.

use std::collections::HashMap;
use std::process::Command;
use std::sync::LazyLock;

use kernel::out::{err, println};
use kernel::py::argparse::Parser;
use kernel::py::num::{self, strip};
use kernel::py::path::{SEP, abspath, basename, dirname, exists, isfile, join, relpath};
use kernel::py::unicode::{SPACE, digits};
use kernel::py::{Crash, Dict, Value, read_text};
use regex::{Captures, Regex};

use crate::build_index::build_index;
use crate::docs::{
    ADR_FIELD_ORDER, Doc, Project, adr_label, cmp_int, fenced_lines, github_anchor,
    write_doc_ordered,
};

fn re(pattern: &str) -> Regex {
    let pattern = pattern.replace("\\s", &format!("[{SPACE}]"));
    let pattern = pattern.replace("\\d", &format!("[{}]", digits()));
    Regex::new(&pattern).expect("valid pattern")
}

static HEADING: LazyLock<Regex> = LazyLock::new(|| re(r"^(#{2,4})\s+ADR-(\d+):\s*(.+?)\s*$"));
static ANY_HEADING: LazyLock<Regex> = LazyLock::new(|| re(r"^(#{1,6})\s+"));
static VERSION_IN_TITLE: LazyLock<Regex> =
    LazyLock::new(|| re(r"\s*\(\s*`?(v?\d+\.\d+(?:\.\d+)?)`?\s*\)\s*$"));
static NOTE_REF: LazyLock<Regex> = LazyLock::new(|| re(r"ADR-(\d+)"));
static LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"(\]\()([^){SPACE}]+)(\))")).expect("valid pattern"));
static SCHEME: LazyLock<Regex> = LazyLock::new(|| re(r"^[a-z]+:"));

struct Section {
    level: usize,
    /// The ADR number, as an integer's decimal text.
    id: String,
    title: String,
    start: usize,
    end: usize,
    heading: String,
    body: String,
    clean_title: String,
    version: Option<String>,
    path: String,
    amended_by: Vec<String>,
    amends: Vec<String>,
}

/// `line[len(hashes) + 1:]`: what follows the hashes and the (space) character after them.
fn after_hashes(line: &str, hashes: usize) -> &str {
    let mut rest = line[hashes..].chars();
    rest.next();
    rest.as_str()
}

/// `parse_sections(text)`.
fn parse_sections(text: &str) -> Result<Vec<Section>, Crash> {
    let lines: Vec<&str> = text.split('\n').collect();
    let flags = fenced_lines(&lines);
    let mut sections: Vec<Section> = Vec::new();
    let mut current: Option<Section> = None;
    for (i, (line, fenced)) in lines.iter().zip(&flags).enumerate() {
        let heading = if *fenced {
            None
        } else {
            ANY_HEADING.captures(line)
        };
        if let Some(h) = &heading
            && current.as_ref().is_some_and(|c| h[1].len() <= c.level)
            && let Some(mut c) = current.take()
        {
            c.end = i;
            sections.push(c);
        }
        let m = if *fenced {
            None
        } else {
            HEADING.captures(line)
        };
        if let Some(m) = m {
            if let Some(mut c) = current.take() {
                c.end = i;
                sections.push(c);
            }
            let level = m[1].len();
            current = Some(Section {
                level,
                id: num::int(&m[2], 10)?.to_string(),
                title: m[3].to_string(),
                start: i,
                end: 0,
                heading: strip(after_hashes(line, level)).to_string(),
                body: String::new(),
                clean_title: String::new(),
                version: None,
                path: String::new(),
                amended_by: Vec::new(),
                amends: Vec::new(),
            });
        }
    }
    if let Some(mut c) = current {
        c.end = lines.len();
        sections.push(c);
    }
    for s in &mut sections {
        s.body = lines[s.start + 1..s.end]
            .join("\n")
            .trim_matches('\n')
            .to_string();
    }
    Ok(sections)
}

/// `git_first_date(root, source, adr_number)`: the date of the commit that added the heading.
fn git_first_date(root: &str, source: &str, adr_number: &str) -> Option<String> {
    let out = Command::new("git")
        .args([
            "log",
            "-S",
            &format!("ADR-{adr_number}:"),
            "--reverse",
            "--format=%cs",
            "--",
            source,
        ])
        .current_dir(root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    text.split(num::is_space)
        .find(|w| !w.is_empty())
        .map(str::to_string)
}

/// Adds `id` to a set of ids kept sorted.
fn add_id(set: &mut Vec<String>, id: &str) {
    if !set.iter().any(|i| i == id) {
        set.push(id.to_string());
        set.sort_by(|a, b| cmp_int(a, b));
    }
}

fn int_list(ids: &[String]) -> Value {
    Value::List(ids.iter().map(|i| Value::Int(i.clone())).collect())
}

/// `main(project, source, use_git_dates, dry_run, overwrite)`.
fn extract(
    project: &Project,
    source: &str,
    use_git_dates: bool,
    dry_run: bool,
    overwrite: bool,
) -> Result<(), Crash> {
    let text = read_text(std::path::Path::new(source))?;
    let sections = parse_sections(&text)?;
    if sections.is_empty() {
        println(&format!(
            "No 'ADR-NNN: Title' headings found in {}.",
            project.rel(source)?
        ));
        return Ok(());
    }

    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut adrs: Vec<Section> = Vec::new();
    for s in sections {
        if seen.contains_key(&s.id) {
            err(&format!(
                "⚠️ duplicate {} heading (line {}); keeping the first one\n",
                adr_label(&s.id),
                s.start + 1
            ));
            continue;
        }
        seen.insert(s.id.clone(), adrs.len());
        adrs.push(s);
    }
    let mut by_heading_anchor: HashMap<String, usize> = HashMap::new();
    for (i, s) in adrs.iter().enumerate() {
        by_heading_anchor.insert(github_anchor(&s.heading), i);
    }

    for s in &mut adrs {
        let mut title = s.title.clone();
        if let Some(vm) = VERSION_IN_TITLE.captures(&title) {
            let v = &vm[1];
            s.version = Some(if v.starts_with('v') {
                v.to_string()
            } else {
                format!("v{v}")
            });
            let start = vm.get(0).map_or(0, |m| m.start());
            title = strip(&title[..start]).to_string();
        }
        s.clean_title = title.trim_end_matches([' ', '.']).to_string();
        s.path = project.adr_path(&s.id, &s.clean_title);
    }

    // Rectification notes: blockquote lines in an ADR pointing to a later ADR.
    for i in 0..adrs.len() {
        let mut kept = Vec::new();
        let body = adrs[i].body.clone();
        for line in body.split('\n') {
            let mut later = Vec::new();
            if line.trim_start_matches(num::is_space).starts_with('>') {
                for r in NOTE_REF.captures_iter(line) {
                    let r = num::int(&r[1], 10)?.to_string();
                    if cmp_int(&r, &adrs[i].id).is_gt() && seen.contains_key(&r) {
                        later.push(r);
                    }
                }
            }
            if later.is_empty() {
                kept.push(line);
                continue;
            }
            let id = adrs[i].id.clone();
            for r in later {
                add_id(&mut adrs[i].amended_by, &r);
                add_id(&mut adrs[seen[&r]].amends, &id);
            }
        }
        adrs[i].body = kept.join("\n").trim_matches('\n').to_string();
    }

    let source_dir = dirname(source);
    let fix_link = |m: &Captures| -> Result<String, Crash> {
        let target = &m[2];
        if SCHEME.is_match(target) {
            return Ok(m[0].to_string());
        }
        let (file_part, anchor) = target.split_once('#').unwrap_or((target, ""));
        if file_part.is_empty() {
            if let Some(&r) = by_heading_anchor.get(anchor) {
                return Ok(format!(
                    "{}{}{}",
                    &m[1],
                    basename(std::path::Path::new(&adrs[r].path)),
                    &m[3]
                ));
            }
            let dest = relpath(source, &project.adr_dir)?.replace(SEP, "/");
            return Ok(format!("{}{dest}#{anchor}{}", &m[1], &m[3]));
        }
        let dest = relpath(&join(&source_dir, file_part), &project.adr_dir)?.replace(SEP, "/");
        let anchor = if anchor.is_empty() {
            String::new()
        } else {
            format!("#{anchor}")
        };
        Ok(format!("{}{dest}{anchor}{}", &m[1], &m[3]))
    };

    let dry = if dry_run { "(dry-run) " } else { "" };
    let mut written = 0;
    for s in &adrs {
        if exists(&s.path) && !overwrite {
            println(&format!(
                "= {} exists, skipped (--overwrite to replace)",
                project.rel(&s.path)?
            ));
            continue;
        }
        let date = if use_git_dates {
            git_first_date(&project.root, &project.rel(source)?, &s.id)
        } else {
            None
        };
        let date = date.unwrap_or_else(|| "unknown".into());
        let mut meta = Dict::new();
        meta.insert("id", Value::Int(s.id.clone()));
        meta.insert("title", Value::str(&s.clean_title));
        meta.insert("status", Value::str("accepted"));
        meta.insert("date", Value::str(&date));
        meta.insert(
            "version",
            s.version.as_deref().map_or(Value::Null, Value::str),
        );
        meta.insert("subsystems", Value::List(Vec::new()));
        meta.insert("supersedes", Value::List(Vec::new()));
        meta.insert("superseded_by", Value::List(Vec::new()));
        meta.insert("amends", int_list(&s.amends));
        meta.insert("amended_by", int_list(&s.amended_by));

        let mut links = String::new();
        let mut last = 0;
        for m in LINK.captures_iter(&s.body) {
            let whole = m.get(0).expect("match");
            links.push_str(&s.body[last..whole.start()]);
            links.push_str(&fix_link(&m)?);
            last = whole.end();
        }
        links.push_str(&s.body[last..]);
        let body = format!("# {}: {}\n\n{links}", adr_label(&s.id), s.clean_title);
        let relation = if s.amended_by.is_empty() {
            String::new()
        } else {
            let labels: Vec<String> = s.amended_by.iter().map(|i| adr_label(i)).collect();
            format!("  amended_by {}", labels.join(", "))
        };
        println(&format!(
            "{dry}{} -> {}  [{}, {date}]{relation}",
            adr_label(&s.id),
            project.rel(&s.path)?,
            s.version.as_deref().unwrap_or("-")
        ));
        if !dry_run {
            write_doc_ordered(
                &Doc {
                    path: s.path.clone(),
                    meta: Value::Dict(meta),
                    body,
                    has_frontmatter: true,
                },
                &ADR_FIELD_ORDER,
            )?;
        }
        written += 1;
    }

    println(&format!(
        "\n{dry}{written} ADR(s) extracted from {} (source not modified).",
        project.rel(source)?
    ));
    if !dry_run && written > 0 {
        if abspath(source) != abspath(&project.index_path) {
            build_index(project, false)?;
        }
        println(
            "👉 Next: translate and restructure each ADR (Context / Decision / Alternatives Considered / Consequences),",
        );
        println(
            "   set 'subsystems', run check_docs.py, then rewrite the index and subsystem pages.",
        );
    }
    Ok(())
}

/// `python extract_adrs.py [args]`.
pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "extract_adrs.py",
        "Extract ADR sections from a legacy technical document into docs/adr",
    )
    .option(
        "--source",
        "Legacy document (default: the configured index, TECDOC.md)",
    )
    .option(
        "--dir",
        "Project directory (default: current; the Git root is used)",
    )
    .flag(
        "--no-git-dates",
        "Do not look up ADR dates in the Git history",
    )
    .flag("--overwrite", "Replace ADR files that already exist")
    .flag("--dry-run", "Only show what would be extracted");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let project = Project::new(args.value("dir"))?;
    let src = match args.value("source").filter(|s| !s.is_empty()) {
        Some(s) => abspath(s).to_string_lossy().into_owned(),
        None => project.index_path.clone(),
    };
    if !isfile(&src) {
        err(&format!("❌ Source not found: {src}\n"));
        return Ok(1);
    }
    extract(
        &project,
        &src,
        !args.flag("no_git_dates"),
        args.flag("dry_run"),
        args.flag("overwrite"),
    )?;
    Ok(0)
}
