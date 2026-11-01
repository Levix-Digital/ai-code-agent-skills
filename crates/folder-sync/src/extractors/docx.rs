//! The text of a `.docx` file as python-docx reads it: the body's paragraphs, with the
//! heading each falls under, then its tables.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;

use kernel::py::num::strip;
use roxmltree::{Document, Node};

use super::{Section, section};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const RELS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const CONTENT_TYPES: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const RT_OFFICE_DOCUMENT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const RT_STYLES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
const CT_DOCUMENT: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const CT_STYLES: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml";

/// Any reason python-docx raises; its text is never shown.
pub struct Error;

type Result<T> = std::result::Result<T, Error>;

fn fail<T>() -> Result<T> {
    Err(Error)
}

fn is_w(n: &Node, name: &str) -> bool {
    n.is_element() && n.tag_name().name() == name && n.tag_name().namespace() == Some(W)
}

fn children<'a, 'i>(n: Node<'a, 'i>, name: &'a str) -> impl Iterator<Item = Node<'a, 'i>> + 'a {
    n.children().filter(move |c| is_w(c, name))
}

fn child<'a, 'i>(n: Node<'a, 'i>, name: &'a str) -> Option<Node<'a, 'i>> {
    children(n, name).next()
}

fn w_attr<'a>(n: Node<'a, '_>, name: &str) -> Option<&'a str> {
    n.attribute((W, name))
}

/// `int(value)`.
fn int(value: &str) -> Result<i64> {
    let n = kernel::py::num::int(value, 10).map_err(|_| Error)?;
    let f = n.to_f64().map_err(|_| Error)?;
    if f.abs() > 9.0e15 {
        return fail();
    }
    Ok(f as i64)
}

/// The parts of the package, by part name (`/word/document.xml`), with their content types.
struct Package {
    blobs: HashMap<String, Vec<u8>>,
    content_types: HashMap<String, String>,
    rels: HashMap<String, Vec<(String, String)>>,
}

/// `posixpath.abspath(posixpath.join(base, target))`.
fn resolve(base: &str, target: &str) -> String {
    let joined = if target.starts_with('/') {
        target.to_string()
    } else if base.ends_with('/') {
        format!("{base}{target}")
    } else {
        format!("{base}/{target}")
    };
    let mut parts: Vec<&str> = Vec::new();
    for p in joined.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    format!("/{}", parts.join("/"))
}

fn base_uri(partname: &str) -> &str {
    match partname.rfind('/') {
        Some(0) => "/",
        Some(i) => &partname[..i],
        None => "",
    }
}

fn rels_name(partname: &str) -> String {
    if partname == "/" {
        return "/_rels/.rels".to_string();
    }
    let base = base_uri(partname);
    let file = &partname[partname.rfind('/').map_or(0, |i| i + 1)..];
    if base == "/" {
        format!("/_rels/{file}.rels")
    } else {
        format!("{base}/_rels/{file}.rels")
    }
}

fn ext(partname: &str) -> &str {
    let file = &partname[partname.rfind('/').map_or(0, |i| i + 1)..];
    let stem_start = file.len() - file.trim_start_matches('.').len();
    match file[stem_start..].rfind('.') {
        Some(i) => &file[stem_start + i + 1..],
        None => "",
    }
}

impl Package {
    fn open(path: &Path) -> Result<Package> {
        let file = std::fs::File::open(path).map_err(|_| Error)?;
        let mut zip = zip::ZipArchive::new(file).map_err(|_| Error)?;
        let mut members: HashMap<String, Vec<u8>> = HashMap::new();
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|_| Error)?;
            if entry.is_dir() {
                continue;
            }
            let name = entry.name().map_err(|_| Error)?.into_owned();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).map_err(|_| Error)?;
            members.insert(name, data);
        }
        let Some(ct_xml) = members.get("[Content_Types].xml") else {
            return fail();
        };
        let ct_text = std::str::from_utf8(ct_xml).map_err(|_| Error)?;
        let ct_doc = Document::parse(ct_text).map_err(|_| Error)?;
        let mut overrides = HashMap::new();
        let mut defaults = HashMap::new();
        for n in ct_doc.root_element().children().filter(|n| n.is_element()) {
            if n.tag_name().namespace() != Some(CONTENT_TYPES) {
                continue;
            }
            let ct = n.attribute("ContentType").unwrap_or_default().to_string();
            match n.tag_name().name() {
                "Override" => {
                    let name = n.attribute("PartName").ok_or(Error)?;
                    if !name.starts_with('/') {
                        return fail();
                    }
                    overrides.insert(name.to_lowercase(), ct);
                }
                "Default" => {
                    let e = n.attribute("Extension").ok_or(Error)?;
                    defaults.insert(e.to_lowercase(), ct);
                }
                _ => {}
            }
        }

        // Every part the relationships reach, from the package's own.
        let mut pkg = Package {
            blobs: HashMap::new(),
            content_types: HashMap::new(),
            rels: HashMap::new(),
        };
        let mut seen: HashSet<String> = HashSet::new();
        let mut todo = vec!["/".to_string()];
        while let Some(source) = todo.pop() {
            let rels = match members.get(&rels_name(&source)[1..]) {
                Some(xml) => parse_rels(xml, base_uri_of_source(&source))?,
                None => Vec::new(),
            };
            let mut next = Vec::new();
            for (_, target) in &rels {
                if target.is_empty() || !seen.insert(target.clone()) {
                    continue;
                }
                let blob = members.get(&target[1..]).ok_or(Error)?.clone();
                let ct = overrides
                    .get(&target.to_lowercase())
                    .or_else(|| defaults.get(&ext(target).to_lowercase()))
                    .ok_or(Error)?
                    .clone();
                pkg.blobs.insert(target.clone(), blob);
                pkg.content_types.insert(target.clone(), ct);
                next.push(target.clone());
            }
            pkg.rels.insert(source, rels);
            for t in next.into_iter().rev() {
                todo.push(t);
            }
        }
        Ok(pkg)
    }

    /// `part_related_by(reltype)`: `None` when there is no such relationship.
    fn related(&self, source: &str, reltype: &str) -> Result<Option<&str>> {
        let found: Vec<&str> = self
            .rels
            .get(source)
            .into_iter()
            .flatten()
            .filter(|(t, target)| t == reltype && !target.is_empty())
            .map(|(_, target)| target.as_str())
            .collect();
        match found.as_slice() {
            [] => Ok(None),
            [one] => Ok(Some(one)),
            _ => fail(),
        }
    }

    fn xml(&self, partname: &str) -> Result<String> {
        let blob = self.blobs.get(partname).ok_or(Error)?;
        let text = std::str::from_utf8(blob).map_err(|_| Error)?;
        Ok(text.strip_prefix('\u{feff}').unwrap_or(text).to_string())
    }
}

fn base_uri_of_source(source: &str) -> &str {
    if source == "/" { "/" } else { base_uri(source) }
}

/// The relationships of a part: `(type, target part name)`, the target empty when external.
fn parse_rels(xml: &[u8], base: &str) -> Result<Vec<(String, String)>> {
    let text = std::str::from_utf8(xml).map_err(|_| Error)?;
    let doc = Document::parse(text.strip_prefix('\u{feff}').unwrap_or(text)).map_err(|_| Error)?;
    let mut out = Vec::new();
    for n in doc.root_element().children().filter(|n| n.is_element()) {
        if n.tag_name().namespace() != Some(RELS) || n.tag_name().name() != "Relationship" {
            continue;
        }
        let kind = n.attribute("Type").ok_or(Error)?.to_string();
        let target = n.attribute("Target").ok_or(Error)?;
        let external = n.attribute("TargetMode") == Some("External");
        out.push((
            kind,
            if external {
                String::new()
            } else {
                resolve(base, target)
            },
        ));
    }
    Ok(out)
}

/// A run's text: breaks, tabs and non-breaking hyphens as characters.
fn run_text(r: Node) -> String {
    let mut out = String::new();
    for c in r.children().filter(|c| c.is_element()) {
        if c.tag_name().namespace() != Some(W) {
            continue;
        }
        match c.tag_name().name() {
            "br" => {
                if w_attr(c, "type").is_none_or(|t| t == "textWrapping") {
                    out.push('\n');
                }
            }
            "cr" => out.push('\n'),
            "noBreakHyphen" => out.push('-'),
            "ptab" | "tab" => out.push('\t'),
            "t" => {
                if let Some(t) = c.first_child().and_then(|f| f.text()) {
                    out.push_str(t);
                }
            }
            _ => {}
        }
    }
    out
}

/// `Paragraph.text`: its runs, including those of hyperlinks.
fn paragraph_text(p: Node) -> String {
    let mut out = String::new();
    for c in p.children() {
        if is_w(&c, "r") {
            out.push_str(&run_text(c));
        } else if is_w(&c, "hyperlink") {
            for r in children(c, "r") {
                out.push_str(&run_text(r));
            }
        }
    }
    out
}

/// A `w:style`, as read: its attributes, and its name or the error reading it raises.
struct Style {
    id: Option<String>,
    kind: Option<String>,
    default: Option<String>,
    name: Result<Option<String>>,
}

/// The paragraph styles of the document, read on first use.
struct Styles {
    styles: Vec<Style>,
}

const STYLE_TYPES: &[&str] = &["character", "numbering", "paragraph", "table"];

fn on_off(v: &str) -> Result<bool> {
    match v {
        "1" | "true" | "on" => Ok(true),
        "0" | "false" | "off" => Ok(false),
        _ => fail(),
    }
}

impl Styles {
    fn load(pkg: &Package, document: &str) -> Result<Styles> {
        let Some(part) = pkg.related(document, RT_STYLES)? else {
            // python-docx's default styles: one paragraph style, "Normal".
            return Ok(Styles {
                styles: vec![Style {
                    id: Some("Normal".into()),
                    kind: Some("paragraph".into()),
                    default: Some("1".into()),
                    name: Ok(Some("Normal".into())),
                }],
            });
        };
        if pkg.content_types.get(part).map(String::as_str) != Some(CT_STYLES) {
            return fail();
        }
        let text = pkg.xml(part)?;
        let doc = Document::parse(&text).map_err(|_| Error)?;
        let root = doc.root_element();
        let styles = children(root, "style")
            .map(|s| {
                let name = match child(s, "name") {
                    None => Ok(None),
                    Some(n) => w_attr(n, "val").map(|v| Some(v.to_string())).ok_or(Error),
                };
                Style {
                    id: w_attr(s, "styleId").map(str::to_string),
                    kind: w_attr(s, "type").map(str::to_string),
                    default: w_attr(s, "default").map(str::to_string),
                    name,
                }
            })
            .collect();
        Ok(Styles { styles })
    }

    fn type_of(t: &Option<String>) -> Result<Option<&str>> {
        match t {
            None => Ok(None),
            Some(t) if STYLE_TYPES.contains(&t.as_str()) => Ok(Some(t.as_str())),
            Some(_) => fail(),
        }
    }

    /// The UI name of the paragraph style with `style_id`, or of the default one.
    fn paragraph_style_name(&self, style_id: Option<&str>) -> Result<String> {
        let mut found = None;
        if let Some(id) = style_id.filter(|i| !i.is_empty()) {
            if id.contains('"') {
                return fail();
            }
            if let Some(s) = self.styles.iter().find(|s| s.id.as_deref() == Some(id))
                && Self::type_of(&s.kind)? == Some("paragraph")
            {
                found = Some(s);
            }
        }
        if found.is_none() {
            for s in &self.styles {
                if Self::type_of(&s.kind)? == Some("paragraph")
                    && s.default
                        .as_deref()
                        .map(on_off)
                        .transpose()?
                        .unwrap_or(false)
                {
                    found = Some(s);
                }
            }
        }
        let Some(style) = found else {
            return fail();
        };
        match &style.name {
            Ok(Some(name)) => Ok(babel_fish(name)),
            _ => fail(),
        }
    }
}

/// `BabelFish.internal2ui`.
fn babel_fish(name: &str) -> String {
    match name {
        "caption" => "Caption".into(),
        "footer" => "Footer".into(),
        "header" => "Header".into(),
        n if n.len() == 9
            && n.starts_with("heading ")
            && n.as_bytes()[8].is_ascii_digit()
            && n.as_bytes()[8] != b'0' =>
        {
            format!("Heading {}", &n[8..])
        }
        n => n.to_string(),
    }
}

/// `CT_P.style`: the paragraph's `w:pStyle` value.
fn paragraph_style_id<'a>(p: Node<'a, '_>) -> Result<Option<&'a str>> {
    let Some(ppr) = child(p, "pPr") else {
        return Ok(None);
    };
    let Some(ps) = child(ppr, "pStyle") else {
        return Ok(None);
    };
    w_attr(ps, "val").map(Some).ok_or(Error)
}

fn grid_span(tc: Node) -> Result<i64> {
    match child(tc, "tcPr").and_then(|p| child(p, "gridSpan")) {
        None => Ok(1),
        Some(g) => int(w_attr(g, "val").ok_or(Error)?),
    }
}

fn grid_before(tr: Node) -> Result<i64> {
    match child(tr, "trPr").and_then(|p| child(p, "gridBefore")) {
        None => Ok(0),
        Some(g) => int(w_attr(g, "val").ok_or(Error)?),
    }
}

fn v_merge<'a>(tc: Node<'a, '_>) -> Option<&'a str> {
    let m = child(tc, "tcPr").and_then(|p| child(p, "vMerge"))?;
    Some(w_attr(m, "val").unwrap_or("continue"))
}

/// The `w:tc` of the row above `tc` at its grid offset.
fn tc_above<'a, 'i>(tr: Node<'a, 'i>, tc: Node<'a, 'i>) -> Result<(Node<'a, 'i>, Node<'a, 'i>)> {
    let above = tr
        .prev_siblings()
        .skip(1)
        .find(|n| is_w(n, "tr"))
        .ok_or(Error)?;
    let mut offset = grid_before(tr)?;
    for prev in tc.prev_siblings().skip(1).filter(|n| is_w(n, "tc")) {
        offset += grid_span(prev)?;
    }
    let mut remaining = offset - grid_before(above)?;
    for c in children(above, "tc") {
        if remaining < 0 {
            break;
        }
        if remaining == 0 {
            return Ok((above, c));
        }
        remaining -= grid_span(c)?;
    }
    fail()
}

fn cell_text(tc: Node) -> String {
    children(tc, "p")
        .map(paragraph_text)
        .collect::<Vec<_>>()
        .join("\n")
}

/// `row.cells`: one text per layout-grid cell, spans repeated, merged cells read from the
/// cell they continue.
fn row_cells(tr: Node) -> Result<Vec<String>> {
    fn tc_cells(tr: Node, tc: Node, out: &mut Vec<String>) -> Result<()> {
        if v_merge(tc) == Some("continue") {
            let (above_tr, above_tc) = tc_above(tr, tc)?;
            return tc_cells(above_tr, above_tc, out);
        }
        let text = cell_text(tc);
        for _ in 0..grid_span(tc)?.max(0) {
            out.push(text.clone());
        }
        Ok(())
    }
    let mut out = Vec::new();
    for tc in children(tr, "tc") {
        tc_cells(tr, tc, &mut out)?;
    }
    Ok(out)
}

/// The full text and the sections of a `.docx` file.
pub fn extract(path: &Path) -> Result<(String, Vec<Section>)> {
    let pkg = Package::open(path)?;
    let document = pkg
        .related("/", RT_OFFICE_DOCUMENT)?
        .ok_or(Error)?
        .to_string();
    if pkg.content_types.get(&document).map(String::as_str) != Some(CT_DOCUMENT) {
        return fail();
    }
    let text = pkg.xml(&document)?;
    let doc = Document::parse(&text).map_err(|_| Error)?;
    let root = doc.root_element();
    if !is_w(&root, "document") {
        return fail();
    }
    let body = child(root, "body").ok_or(Error)?;

    let mut styles: Option<Styles> = None;
    let mut full_text = Vec::new();
    let mut sections = Vec::new();
    let mut heading = "Geral".to_string();
    for p in children(body, "p") {
        let raw = paragraph_text(p);
        let text = strip(&raw);
        if text.is_empty() {
            continue;
        }
        let style_id = paragraph_style_id(p)?;
        if styles.is_none() {
            styles = Some(Styles::load(&pkg, &document)?);
        }
        let name = styles
            .as_ref()
            .expect("styles")
            .paragraph_style_name(style_id)?;
        if name.starts_with("Heading") || name == "Title" || name == "Subtitle" {
            heading = text.to_string();
        }
        full_text.push(text.to_string());
        sections.push(section(&heading, text.to_string()));
    }

    for tbl in children(body, "tbl") {
        let mut rows = Vec::new();
        for tr in children(tbl, "tr") {
            let cells = row_cells(tr)?;
            let row = cells
                .iter()
                .map(|c| strip(c))
                .filter(|c| !c.is_empty())
                .collect::<Vec<_>>()
                .join(" | ");
            if !row.is_empty() {
                rows.push(row);
            }
        }
        if !rows.is_empty() {
            let t = format!("\n[Tabela]:\n{}", rows.join("\n"));
            full_text.push(t.clone());
            sections.push(section("Tabela", t));
        }
    }
    Ok((full_text.join("\n\n"), sections))
}
