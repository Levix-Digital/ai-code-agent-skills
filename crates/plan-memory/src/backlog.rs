//! `_backlog.py` and `_backlog_store.py`, the part the roadmap reads: the project backlog
//! (default: docs/BACKLOG.md) parsed into its items, with the queries over them. The patterns
//! are matched as Python's `re` matches them (Unicode `\s` and `\d`, `$` before a final
//! newline, `re.I` with Python's case folding).

use kernel::project_config::{ConfigError, load_project_config};
use kernel::py::num::{decimal_value, is_space};
use kernel::py::path::{isfile, join};
use kernel::py::unicode::{ignorecase_eq, upper};
use kernel::py::{Crash, Value};

use crate::roadmap::{RoadmapError, field_line, read_exact, section_heading};

/// `TYPES`: type, id prefix and section heading, in order.
pub const TYPES: [(&str, &str, &str); 5] = [
    ("bug", "BUG", "Bugs"),
    ("feature", "FEAT", "Features"),
    ("improvement", "IMP", "Improvements"),
    ("adjustment", "ADJ", "Adjustments"),
    ("epic", "EPIC", "Epics"),
];
pub const CLOSED_SECTION: &str = "Closed";
pub const SECTIONS: [&str; 6] = [
    "Bugs",
    "Features",
    "Improvements",
    "Adjustments",
    "Epics",
    CLOSED_SECTION,
];
pub const SEVERITIES: [&str; 4] = ["critical", "high", "medium", "low"];
pub const PRIORITIES: [&str; 3] = ["high", "medium", "low"];
pub const NO_PLAN: &str = "-";
const PREFIXES: [&str; 5] = ["BUG", "FEAT", "IMP", "ADJ", "EPIC"];

/// `BacklogError`: what the scripts print after `❌`.
#[derive(Debug)]
pub struct BacklogError(pub String);

pub fn rstrip(s: &str) -> &str {
    s.trim_end_matches(is_space)
}

pub fn lstrip(s: &str) -> &str {
    s.trim_start_matches(is_space)
}

/// `(BUG|FEAT|IMP|ADJ|EPIC)-(\d{3,})` at the start of `s` (case-insensitive with `ci`): the
/// prefix, the digits and the rest.
fn match_id(s: &str, ci: bool) -> Option<(&'static str, &str, &str)> {
    let prefix = PREFIXES.iter().find(|p| {
        let mut chars = s.chars();
        p.chars().all(|pc| {
            chars.next().is_some_and(|c| {
                if ci {
                    ignorecase_eq(c, pc.to_ascii_lowercase())
                } else {
                    c == pc
                }
            })
        })
    })?;
    let mut rest = s;
    for _ in 0..prefix.len() {
        let c = rest.chars().next()?;
        rest = &rest[c.len_utf8()..];
    }
    let rest = rest.strip_prefix('-')?;
    let end = rest
        .char_indices()
        .find(|(_, c)| decimal_value(*c).is_none())
        .map_or(rest.len(), |(i, _)| i);
    if rest[..end].chars().count() < 3 {
        return None;
    }
    Some((prefix, &rest[..end], &rest[end..]))
}

/// `ID_RE.match(s)`: the prefix and the digits of a backlog id.
pub fn id_match(s: &str) -> Option<(&'static str, &str)> {
    match match_id(s, false)? {
        (prefix, digits, "" | "\n") => Some((prefix, digits)),
        _ => None,
    }
}

/// The value of Unicode digits, as a key that orders like the numbers (no leading zeros).
fn number_key(digits: &str) -> (usize, String) {
    let ascii: String = digits
        .chars()
        .map(|c| char::from(b'0' + decimal_value(c).unwrap_or(0) as u8))
        .collect();
    let trimmed = ascii.trim_start_matches('0').to_string();
    (trimmed.len(), trimmed)
}

/// `SUPERSEDED_RE.search(text).group(1)`.
fn superseded_ids(text: &str) -> Option<&str> {
    ids_after(text, "superseded by", true)
}

/// `DUPLICATE_RE.search(text).group(1)`.
fn duplicate_of(text: &str) -> Option<&str> {
    ids_after(text, "duplicate of", false)
}

/// `re.search(words + r"\s+(ID(?:\s*,\s*ID)*)", text, re.I).group(1)`, the list part only
/// with `many`.
fn ids_after<'a>(text: &'a str, words: &str, many: bool) -> Option<&'a str> {
    for (start, _) in text.char_indices() {
        let mut rest = &text[start..];
        let mut ok = true;
        for p in words.chars() {
            match rest.chars().next() {
                Some(c) if (p == ' ' && c == ' ') || (p != ' ' && ignorecase_eq(c, p)) => {
                    rest = &rest[c.len_utf8()..];
                }
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            continue;
        }
        let after = lstrip(rest);
        if after.len() == rest.len() {
            continue;
        }
        let Some((_, _, mut tail)) = match_id(after, true) else {
            continue;
        };
        // `(?:\s*,\s*ID)*`
        while many
            && let Some(next) = lstrip(tail).strip_prefix(',')
            && let Some((_, _, t)) = match_id(lstrip(next), true)
        {
            tail = t;
        }
        return Some(&after[..after.len() - tail.len()]);
    }
    None
}

/// `ITEM_RE.match(line)`: `### <ID>: <title>`.
fn item_heading(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("###")?;
    let after = lstrip(rest);
    if after.len() == rest.len() || after.is_empty() || after.starts_with('#') {
        return None;
    }
    for (i, c) in after.char_indices() {
        if i > 0 && c == ':' && i + 1 < after.len() {
            return Some((&after[..i], lstrip(&after[i + 1..])));
        }
        if is_space(c) {
            return None;
        }
    }
    None
}

pub struct Item {
    pub id: String,
    pub title: String,
    pub section: String,
    pub line: Option<usize>,
    /// Name and lines (first line value, then continuation lines as written), in file order.
    pub fields: Vec<(String, Vec<String>)>,
}

impl Item {
    pub fn kind(&self) -> Option<&'static str> {
        let (prefix, _) = id_match(&self.id)?;
        TYPES.iter().find(|t| t.1 == prefix).map(|t| t.0)
    }

    fn number(&self) -> (usize, String) {
        id_match(&self.id).map_or((0, String::new()), |(_, d)| number_key(d))
    }

    pub fn closed(&self) -> bool {
        self.section == CLOSED_SECTION
    }

    pub fn get(&self, name: &str) -> String {
        let Some((_, lines)) = self.fields.iter().find(|(n, _)| n == name) else {
            return String::new();
        };
        let parts: Vec<&str> = lines
            .iter()
            .map(|l| kernel::py::num::strip(l))
            .filter(|l| !l.is_empty())
            .collect();
        parts.join(" ")
    }

    pub fn status(&self) -> String {
        self.get("Status")
    }

    pub fn plan(&self) -> Option<String> {
        let value = self.get("Plan");
        (!value.is_empty() && value != NO_PLAN).then_some(value)
    }

    pub fn epic(&self) -> Option<String> {
        let value = self.get("Epic");
        (!value.is_empty() && value != NO_PLAN).then(|| upper(&value))
    }

    pub fn superseded_by(&self) -> Vec<String> {
        if self.status() != "superseded" {
            return Vec::new();
        }
        let closed = self.get("Closed");
        superseded_ids(&closed).map_or_else(Vec::new, |ids| {
            ids.split(',')
                .map(|i| upper(kernel::py::num::strip(i)))
                .collect()
        })
    }

    /// Severity for bugs, priority for everything else.
    pub fn rank(&self) -> String {
        if self.kind() == Some("bug") {
            self.get("Severity")
        } else {
            self.get("Priority")
        }
    }

    fn sort_key(&self) -> (usize, (usize, String)) {
        let scale: &[&str] = if self.kind() == Some("bug") {
            &SEVERITIES
        } else {
            &PRIORITIES
        };
        let rank = self.rank();
        let index = scale.iter().position(|s| *s == rank).unwrap_or(scale.len());
        (index, self.number())
    }
}

pub struct Backlog {
    /// The items of each section, in `SECTIONS` order.
    pub sections: Vec<Vec<Item>>,
    /// Structural problems found while parsing, with their line.
    pub errors: Vec<String>,
    /// (0-based line, name) of the `## ` headings that are not a known section.
    pub unknown_sections: Vec<(usize, String)>,
}

impl Backlog {
    pub fn parse(text: &str) -> Self {
        let mut sections: Vec<Vec<Item>> = SECTIONS.iter().map(|_| Vec::new()).collect();
        let mut errors = Vec::new();
        let mut unknown_sections = Vec::new();
        let mut section: Option<usize> = None;
        let mut item = false;
        let mut field: Option<String> = None;
        let text = text.replace("\r\n", "\n");
        for (i, line) in text.split('\n').enumerate() {
            let line = rstrip(line);
            if let Some(name) = section_heading(line) {
                item = false;
                field = None;
                section = SECTIONS.iter().position(|s| *s == name);
                if section.is_none() {
                    unknown_sections.push((i, name.to_string()));
                }
                continue;
            }
            let Some(s) = section else {
                continue;
            };
            if line.starts_with("###") {
                field = None;
                item = false;
                if let Some((id, title)) = item_heading(line) {
                    sections[s].push(Item {
                        id: id.to_string(),
                        title: title.to_string(),
                        section: SECTIONS[s].to_string(),
                        line: Some(i),
                        fields: Vec::new(),
                    });
                    item = true;
                } else {
                    errors.push(format!(
                        "line {}: item heading must be '### <ID>: <title>'",
                        i + 1
                    ));
                }
                continue;
            }
            if !item {
                continue;
            }
            let Some(it) = sections[s].last_mut() else {
                continue;
            };
            if let Some((name, value)) = field_line(line) {
                let value = vec![kernel::py::num::strip(value).to_string()];
                match it.fields.iter_mut().find(|(n, _)| *n == name) {
                    Some((_, lines)) => {
                        errors.push(format!(
                            "line {}: {} repeats the field '{name}'",
                            i + 1,
                            it.id
                        ));
                        *lines = value;
                    }
                    None => it.fields.push((name.clone(), value)),
                }
                field = Some(name);
                continue;
            }
            if let Some(name) = &field {
                if let Some((_, lines)) = it.fields.iter_mut().find(|(n, _)| n == name) {
                    lines.push(line.to_string());
                }
            } else if !kernel::py::num::strip(line).is_empty() {
                errors.push(format!(
                    "line {}: text in {} outside a '- **Field**:' line",
                    i + 1,
                    it.id
                ));
            }
        }
        Backlog {
            sections,
            errors,
            unknown_sections,
        }
    }

    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.sections.iter().flatten()
    }

    pub fn find(&self, item_id: &str) -> Result<&Item, BacklogError> {
        let want = upper(item_id);
        self.items()
            .find(|i| i.id == want)
            .ok_or_else(|| BacklogError(format!("{item_id}: no such backlog item")))
    }

    /// `open_items()`: every open item, by type, then rank, then number.
    pub fn open_items(&self) -> Vec<&Item> {
        let mut items: Vec<&Item> = self.items().filter(|i| !i.closed()).collect();
        items.sort_by_cached_key(|i| {
            let t = i
                .kind()
                .and_then(|k| TYPES.iter().position(|t| t.0 == k))
                .unwrap_or(TYPES.len());
            (t, i.sort_key())
        });
        items
    }

    pub fn epic_items(&self, epic_id: &str, include_closed: bool) -> Vec<&Item> {
        let want = upper(epic_id);
        self.items()
            .filter(|i| i.epic().as_deref() == Some(want.as_str()))
            .filter(|i| include_closed || !i.closed())
            .collect()
    }
}

const OPEN_STATUSES: [&str; 3] = ["open", "planned", "in_progress"];
const CLOSED_STATUSES: [&str; 4] = ["done", "dropped", "duplicate", "superseded"];

impl Backlog {
    /// `validate()`: the problems of the file, structural ones first.
    pub fn validate(&self) -> Vec<String> {
        let mut problems = self.errors.clone();
        for (line, name) in &self.unknown_sections {
            problems.push(format!(
                "line {}: unknown section '## {name}' (expected: {})",
                line + 1,
                SECTIONS.join(", ")
            ));
        }
        let mut seen: Vec<(&str, usize)> = Vec::new();
        let ids: Vec<&str> = self.items().map(|i| i.id.as_str()).collect();
        for item in self.items() {
            let where_ = match item.line {
                Some(line) => format!("line {}: {}", line + 1, item.id),
                None => item.id.clone(),
            };
            let Some(kind) = item.kind() else {
                problems.push(format!(
                    "{where_}: id must be BUG-NNN, FEAT-NNN, IMP-NNN, ADJ-NNN or EPIC-NNN"
                ));
                continue;
            };
            if let Some((_, first)) = seen.iter().find(|(id, _)| *id == item.id) {
                problems.push(format!(
                    "{where_}: duplicate id (first at line {})",
                    first + 1
                ));
            } else {
                seen.push((&item.id, item.line.unwrap_or(0)));
            }
            let heading = TYPES.iter().find(|t| t.0 == kind).map_or("", |t| t.2);
            if !item.closed() && item.section != heading {
                problems.push(format!("{where_}: a {kind} belongs under '## {heading}'"));
            }

            let mut required = vec![if kind == "bug" {
                "Severity"
            } else {
                "Priority"
            }];
            required.extend(["Status", "Found"]);
            if kind != "epic" {
                required.push("Plan");
            }
            required.push("Description");
            if item.closed() {
                required.push("Closed");
            }
            for name in required {
                if item.get(name).is_empty() {
                    problems.push(format!("{where_}: missing '- **{name}**:'"));
                }
            }
            let severity = item.get("Severity");
            if kind == "bug" && !severity.is_empty() && !SEVERITIES.contains(&severity.as_str()) {
                problems.push(format!(
                    "{where_}: severity must be one of: {}",
                    SEVERITIES.join(", ")
                ));
            }
            let priority = item.get("Priority");
            if kind != "bug" && !priority.is_empty() && !PRIORITIES.contains(&priority.as_str()) {
                problems.push(format!(
                    "{where_}: priority must be one of: {}",
                    PRIORITIES.join(", ")
                ));
            }

            let status = item.status();
            let allowed: &[&str] = if item.closed() {
                &CLOSED_STATUSES
            } else {
                &OPEN_STATUSES
            };
            if !status.is_empty() && !allowed.contains(&status.as_str()) {
                let place = if item.closed() {
                    "under '## Closed'"
                } else {
                    "outside '## Closed'"
                };
                problems.push(format!(
                    "{where_}: status {place} must be one of: {}",
                    allowed.join(", ")
                ));
            }
            let promoted = status == "planned" || status == "in_progress";
            if kind == "epic" {
                if promoted {
                    problems.push(format!(
                        "{where_}: an epic is never promoted; its status stays 'open' until it is closed"
                    ));
                }
                if item.plan().is_some() {
                    problems.push(format!("{where_}: an epic has no plan; promote its items"));
                }
                if item.epic().is_some() {
                    problems.push(format!("{where_}: an epic cannot belong to another epic"));
                }
            } else if let Some(epic) = item.epic() {
                if !ids.contains(&epic.as_str()) {
                    problems.push(format!("{where_}: epic {epic} is not in the backlog"));
                } else if id_match(&epic).is_none_or(|(prefix, _)| prefix != "EPIC") {
                    problems.push(format!("{where_}: {epic} is not an epic"));
                }
            }
            if promoted && item.plan().is_none() {
                problems.push(format!(
                    "{where_}: a {status} item needs its plan id in '- **Plan**:'"
                ));
            }
            if status == "duplicate" {
                let closed = item.get("Closed");
                match duplicate_of(&closed) {
                    None => problems.push(format!(
                        "{where_}: a duplicate needs 'duplicate of <ID>' in '- **Closed**:'"
                    )),
                    Some(other) if !ids.contains(&upper(other).as_str()) => {
                        problems.push(format!("{where_}: duplicate of unknown item {other}"))
                    }
                    Some(_) => {}
                }
            }
            if status == "superseded" {
                let by = item.superseded_by();
                if by.is_empty() {
                    problems.push(format!(
                        "{where_}: a superseded item needs 'superseded by <ID>[, <ID>...]' in '- **Closed**:'"
                    ));
                }
                for other in by {
                    if !ids.contains(&other.as_str()) {
                        problems.push(format!("{where_}: superseded by unknown item {other}"));
                    } else if other == item.id {
                        problems.push(format!("{where_}: an item cannot supersede itself"));
                    }
                }
            }
        }
        problems
    }
}

/// `BacklogFile(start_dir)`, to load the backlog when the project has one.
pub fn load_backlog(start_dir: Option<&str>) -> Result<Option<Backlog>, Crash> {
    let root = architecture_docs::docs::repo_root(start_dir);
    let config = load_project_config(Some(&root)).map_err(ConfigError::into_crash)?;
    let file = config.section("backlog").get_or_none("file");
    let default = kernel::py::Value::Str(join("docs", "BACKLOG.md"));
    let rel = if file.truthy() { file } else { &default };
    let path = architecture_docs::docs::join_value(&config.root.to_string_lossy(), rel)?;
    if !isfile(&path) {
        return Ok(None);
    }
    Ok(Some(Backlog::parse(&read_exact(&path)?)))
}

impl From<BacklogError> for RoadmapError {
    fn from(e: BacklogError) -> Self {
        RoadmapError::Error(e.0)
    }
}

/// `value in container`, for a value from the project configuration.
fn contains(container: &Value, item: &str) -> Result<bool, Crash> {
    match container {
        Value::Str(s) => Ok(s.contains(item)),
        Value::List(v) | Value::Tuple(v) | Value::Set(v) => {
            Ok(v.iter().any(|x| x.as_str() == Some(item)))
        }
        Value::Dict(d) => Ok(d.contains_key(item)),
        other => Err(Crash::new(format!(
            "TypeError: argument of type '{}' is not iterable",
            other.type_name()
        ))),
    }
}

impl Backlog {
    /// `open_items(item_type)`: the open items of one type, in `open_items()` order.
    pub fn open_items_of(&self, kind: &str) -> Vec<&Item> {
        self.open_items()
            .into_iter()
            .filter(|i| i.kind() == Some(kind))
            .collect()
    }

    /// `blocking_bugs(severities)`: the open bugs whose severity holds up new work.
    pub fn blocking_bugs(&self, severities: &Value) -> Result<Vec<&Item>, Crash> {
        let mut out = Vec::new();
        for bug in self.open_items_of("bug") {
            if contains(severities, &bug.get("Severity"))? {
                out.push(bug);
            }
        }
        Ok(out)
    }
}

/// `BacklogFile(start_dir)`: the backlog file of one project and its triage settings.
pub struct BacklogFile {
    root: String,
    pub path: String,
    pub blocking_severities: Value,
    pub bug_deadline: Value,
}

impl BacklogFile {
    pub fn new(start_dir: Option<&str>) -> Result<Self, Crash> {
        let root = architecture_docs::docs::repo_root(start_dir);
        let config = load_project_config(Some(&root)).map_err(ConfigError::into_crash)?;
        let root = config.root.to_string_lossy().into_owned();
        let section = config.section("backlog");
        let file = section.get_or_none("file");
        let default = Value::Str(join("docs", "BACKLOG.md"));
        let path = architecture_docs::docs::join_value(
            &root,
            if file.truthy() { file } else { &default },
        )?;
        let or = |key: &str, default: Value| {
            let v = section.get_or_none(key);
            if v.truthy() { v.clone() } else { default }
        };
        Ok(BacklogFile {
            path,
            blocking_severities: or("blocking_severities", Value::List(Vec::new())),
            bug_deadline: or("bug_deadline", Value::str("next_release")),
            root,
        })
    }

    pub fn exists(&self) -> bool {
        isfile(&self.path)
    }

    pub fn rel(&self) -> Result<String, Crash> {
        Ok(kernel::py::path::relpath(&self.path, &self.root)?.replace('\\', "/"))
    }

    pub fn load(&self) -> Result<Backlog, RoadmapError> {
        if !self.exists() {
            return Err(RoadmapError::Error(format!(
                "no backlog at {}; the first 'add' creates it",
                self.rel()?
            )));
        }
        Ok(Backlog::parse(&read_exact(&self.path)?))
    }

    /// `triage(backlog)["untriaged"]`: the open bugs not triaged yet, with the reason,
    /// reading the project roadmap when there is one.
    pub fn untriaged<'a>(
        &self,
        backlog: &'a Backlog,
    ) -> Result<Vec<(&'a Item, String)>, RoadmapError> {
        Ok(self.triage(backlog, &[])?.untriaged)
    }

    /// `triage(backlog, exclude)`.
    pub fn triage<'a>(
        &self,
        backlog: &'a Backlog,
        exclude: &[String],
    ) -> Result<Triage<'a>, RoadmapError> {
        let rf = crate::roadmap::RoadmapFile::new(Some(&self.root))?;
        let roadmap = if rf.exists() {
            Some(rf.load(false)?)
        } else {
            None
        };
        Ok(triage(
            backlog,
            roadmap.as_ref(),
            &self.blocking_severities,
            &self.bug_deadline,
            exclude,
        )?)
    }
}

/// `bug_triage(backlog, roadmap, blocking, deadline)["untriaged"]`.
pub fn bug_triage<'a>(
    backlog: &'a Backlog,
    roadmap: Option<&crate::roadmap::Roadmap>,
    blocking: &Value,
    deadline: &Value,
) -> Result<Vec<(&'a Item, String)>, Crash> {
    Ok(triage(backlog, roadmap, blocking, deadline, &[])?.untriaged)
}

/// What `bug_triage` returns.
pub struct Triage<'a> {
    pub untriaged: Vec<(&'a Item, String)>,
    pub deadline_checked: bool,
    pub next_release: Option<String>,
}

/// `bug_triage(backlog, roadmap, blocking, deadline, exclude)`: an open bug is untriaged
/// when its severity is missing or invalid, or, with a deadline, when its severity does not
/// block new work and it is not in the next release of the roadmap. Ids in `exclude` are
/// skipped.
pub fn triage<'a>(
    backlog: &'a Backlog,
    roadmap: Option<&crate::roadmap::Roadmap>,
    blocking: &Value,
    deadline: &Value,
    exclude: &[String],
) -> Result<Triage<'a>, Crash> {
    let exclude: Vec<String> = exclude.iter().map(|i| upper(i)).collect();
    let roadmap = roadmap.filter(|_| deadline.as_str() == Some("next_release"));
    let current = roadmap.and_then(|r| r.current());
    let mut out = Vec::new();
    for bug in backlog.open_items_of("bug") {
        if exclude.contains(&bug.id) {
            continue;
        }
        let severity = bug.get("Severity");
        if !SEVERITIES.contains(&severity.as_str()) {
            let why = if severity.is_empty() {
                "no severity".to_string()
            } else {
                format!("invalid severity '{severity}'")
            };
            out.push((bug, why));
            continue;
        }
        let Some(roadmap) = roadmap else {
            continue;
        };
        if contains(blocking, &severity)? {
            continue;
        }
        let release = roadmap.release_of(&bug.id).map(|i| &roadmap.releases[i]);
        match (current, release) {
            (None, _) => out.push((bug, "no release left in the roadmap to schedule it".into())),
            (Some(c), None) => out.push((
                bug,
                format!("not scheduled; it goes into {} at the latest", c.version),
            )),
            (Some(c), Some(r)) if r.version != c.version => out.push((
                bug,
                format!("scheduled for {}; the deadline is {}", r.version, c.version),
            )),
            _ => {}
        }
    }
    Ok(Triage {
        untriaged: out,
        deadline_checked: roadmap.is_some(),
        next_release: current.map(|c| c.version.clone()),
    })
}
