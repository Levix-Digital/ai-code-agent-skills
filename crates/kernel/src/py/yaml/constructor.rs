//! `yaml.composer`, `yaml.resolver` and `yaml.constructor.SafeConstructor`: events as nodes,
//! nodes as Python values.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use super::parser::{EventKind, Parser};
use super::{Mark, marked};
use crate::py::num::{self, BigInt};
use crate::py::value::{Date, DateTime, timedelta_repr};
use crate::py::{Crash, Dict, Value, repr_str};

const TAG: &str = "tag:yaml.org,2002:";

/// Python's recursion limit, roughly: deeper documents fail as they do there instead of
/// overflowing the stack.
const MAX_DEPTH: usize = 2000;

fn recursion_error() -> Crash {
    Crash::new("RecursionError: maximum recursion depth exceeded")
}

// ------------------------------------------------------------------ nodes

#[derive(Clone)]
enum NodeValue {
    Scalar(String),
    Sequence(Vec<usize>),
    Mapping(Vec<(usize, usize)>),
}

#[derive(Clone)]
struct Node {
    tag: String,
    value: NodeValue,
    start: Mark,
}

impl Node {
    fn id(&self) -> &'static str {
        match self.value {
            NodeValue::Scalar(_) => "scalar",
            NodeValue::Sequence(_) => "sequence",
            NodeValue::Mapping(_) => "mapping",
        }
    }
}

fn composer_error(context: Option<&str>, cm: Option<&Mark>, problem: &str, pm: &Mark) -> Crash {
    marked("composer.ComposerError", context, cm, problem, Some(pm))
}

fn constructor_error(context: Option<&str>, cm: Option<&Mark>, problem: &str, pm: &Mark) -> Crash {
    marked(
        "constructor.ConstructorError",
        context,
        cm,
        problem,
        Some(pm),
    )
}

// ------------------------------------------------------------------ resolver

fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("valid pattern")
}

static BOOL: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"^(?:yes|Yes|YES|no|No|NO|true|True|TRUE|false|False|FALSE|on|On|ON|off|Off|OFF)$")
});
static FLOAT: LazyLock<Regex> = LazyLock::new(|| {
    regex(concat!(
        r"^(?:[-+]?(?:[0-9][0-9_]*)\.[0-9_]*(?:[eE][-+][0-9]+)?",
        r"|\.[0-9][0-9_]*(?:[eE][-+][0-9]+)?",
        r"|[-+]?[0-9][0-9_]*(?::[0-5]?[0-9])+\.[0-9_]*",
        r"|[-+]?\.(?:inf|Inf|INF)",
        r"|\.(?:nan|NaN|NAN))$"
    ))
});
static INT: LazyLock<Regex> = LazyLock::new(|| {
    regex(concat!(
        r"^(?:[-+]?0b[0-1_]+",
        r"|[-+]?0[0-7_]+",
        r"|[-+]?(?:0|[1-9][0-9_]*)",
        r"|[-+]?0x[0-9a-fA-F_]+",
        r"|[-+]?[1-9][0-9_]*(?::[0-5]?[0-9])+)$"
    ))
});
static NULL: LazyLock<Regex> = LazyLock::new(|| regex(r"^(?:~|null|Null|NULL|)$"));
static TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| {
    regex(concat!(
        r"^(?:[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]",
        r"|[0-9][0-9][0-9][0-9]-[0-9][0-9]?-[0-9][0-9]?",
        r"(?:[Tt]|[ \t]+)[0-9][0-9]?",
        r":[0-9][0-9]:[0-9][0-9](?:\.[0-9]*)?",
        r"(?:[ \t]*(?:Z|[-+][0-9][0-9]?(?::[0-9][0-9])?))?)$"
    ))
});
/// `SafeConstructor.timestamp_regexp`; Python's `$` also matches before a final newline.
static TIMESTAMP_PARTS: LazyLock<Regex> = LazyLock::new(|| {
    regex(concat!(
        r"^(?P<year>[0-9][0-9][0-9][0-9])",
        r"-(?P<month>[0-9][0-9]?)",
        r"-(?P<day>[0-9][0-9]?)",
        r"(?:(?:[Tt]|[ \t]+)",
        r"(?P<hour>[0-9][0-9]?)",
        r":(?P<minute>[0-9][0-9])",
        r":(?P<second>[0-9][0-9])",
        r"(?:\.(?P<fraction>[0-9]*))?",
        r"(?:[ \t]*(?P<tz>Z|(?P<tz_sign>[-+])(?P<tz_hour>[0-9][0-9]?)",
        r"(?::(?P<tz_minute>[0-9][0-9]))?))?)?\n?\z"
    ))
});

/// `Resolver.resolve(ScalarNode, value, implicit)`.
pub(crate) fn resolve_scalar(value: &str, implicit: (bool, bool)) -> String {
    if implicit.0 {
        let first = value.chars().next();
        let mut candidates: Vec<(&str, &Regex)> = Vec::new();
        let c = first.unwrap_or('\0');
        if first.is_some() && "yYnNtTfFoO".contains(c) {
            candidates.push(("bool", &BOOL));
        }
        if first.is_some() && "-+0123456789.".contains(c) {
            candidates.push(("float", &FLOAT));
        }
        if first.is_some() && "-+0123456789".contains(c) {
            candidates.push(("int", &INT));
        }
        if c == '<' && value == "<<" {
            return format!("{TAG}merge");
        }
        if first.is_none() || "~nN".contains(c) {
            candidates.push(("null", &NULL));
        }
        if first.is_some() && c.is_ascii_digit() {
            candidates.push(("timestamp", &TIMESTAMP));
        }
        for (tag, re) in candidates {
            if re.is_match(value) {
                return format!("{TAG}{tag}");
            }
        }
        if c == '=' && value == "=" {
            return format!("{TAG}value");
        }
        if "!&*".contains(c) && first.is_some() && value.chars().count() == 1 {
            return format!("{TAG}yaml");
        }
    }
    format!("{TAG}str")
}

// ------------------------------------------------------------------ composer

struct Composer {
    parser: Parser,
    nodes: Vec<Node>,
    anchors: HashMap<String, usize>,
}

impl Composer {
    fn get_event(&mut self) -> Result<super::parser::Event, Crash> {
        Ok(self
            .parser
            .get_event()?
            .expect("events until the stream end"))
    }

    fn check_stream_end(&mut self) -> Result<bool, Crash> {
        Ok(matches!(
            self.parser.peek_event()?.map(|e| &e.kind),
            Some(EventKind::StreamEnd)
        ))
    }

    fn get_single_node(&mut self) -> Result<Option<usize>, Crash> {
        self.get_event()?;
        let mut document = None;
        if !self.check_stream_end()? {
            document = Some(self.compose_document()?);
        }
        if !self.check_stream_end()? {
            let event = self.get_event()?;
            let first = &self.nodes[document.expect("a document before another")].start;
            return Err(composer_error(
                Some("expected a single document in the stream"),
                Some(first),
                "but found another document",
                &event.start,
            ));
        }
        self.get_event()?;
        Ok(document)
    }

    fn compose_document(&mut self) -> Result<usize, Crash> {
        self.get_event()?;
        let node = self.compose_node(0)?;
        self.get_event()?;
        self.anchors.clear();
        Ok(node)
    }

    fn compose_node(&mut self, depth: usize) -> Result<usize, Crash> {
        if depth > MAX_DEPTH {
            return Err(recursion_error());
        }
        let event = self
            .parser
            .peek_event()?
            .cloned()
            .expect("events until the stream end");
        if let EventKind::Alias(name) = &event.kind {
            self.get_event()?;
            return match self.anchors.get(name) {
                Some(&id) => Ok(id),
                None => Err(composer_error(
                    None,
                    None,
                    &format!("found undefined alias {}", repr_str(name)),
                    &event.start,
                )),
            };
        }
        let anchor = event.anchor().map(str::to_string);
        if let Some(a) = &anchor
            && let Some(&first) = self.anchors.get(a)
        {
            return Err(composer_error(
                Some(&format!(
                    "found duplicate anchor {}; first occurrence",
                    repr_str(a)
                )),
                Some(&self.nodes[first].start),
                "second occurrence",
                &event.start,
            ));
        }
        let event = self.get_event()?;
        let start = event.start.clone();
        match event.kind {
            EventKind::Scalar {
                tag,
                implicit,
                value,
                ..
            } => {
                let tag = match tag {
                    Some(t) if t != "!" => t,
                    _ => resolve_scalar(&value, implicit),
                };
                let id = self.push(tag, NodeValue::Scalar(value), start);
                self.anchor(anchor, id);
                Ok(id)
            }
            EventKind::SequenceStart { tag, .. } => {
                let tag = match tag {
                    Some(t) if t != "!" => t,
                    _ => format!("{TAG}seq"),
                };
                let id = self.push(tag, NodeValue::Sequence(Vec::new()), start);
                self.anchor(anchor, id);
                loop {
                    if matches!(
                        self.parser.peek_event()?.map(|e| &e.kind),
                        Some(EventKind::SequenceEnd)
                    ) {
                        break;
                    }
                    let child = self.compose_node(depth + 1)?;
                    if let NodeValue::Sequence(items) = &mut self.nodes[id].value {
                        items.push(child);
                    }
                }
                self.get_event()?;
                Ok(id)
            }
            EventKind::MappingStart { tag, .. } => {
                let tag = match tag {
                    Some(t) if t != "!" => t,
                    _ => format!("{TAG}map"),
                };
                let id = self.push(tag, NodeValue::Mapping(Vec::new()), start);
                self.anchor(anchor, id);
                loop {
                    if matches!(
                        self.parser.peek_event()?.map(|e| &e.kind),
                        Some(EventKind::MappingEnd)
                    ) {
                        break;
                    }
                    let key = self.compose_node(depth + 1)?;
                    let value = self.compose_node(depth + 1)?;
                    if let NodeValue::Mapping(pairs) = &mut self.nodes[id].value {
                        pairs.push((key, value));
                    }
                }
                self.get_event()?;
                Ok(id)
            }
            _ => unreachable!("a node starts with a scalar or a collection"),
        }
    }

    fn push(&mut self, tag: String, value: NodeValue, start: Mark) -> usize {
        self.nodes.push(Node { tag, value, start });
        self.nodes.len() - 1
    }

    fn anchor(&mut self, anchor: Option<String>, id: usize) {
        if let Some(a) = anchor {
            self.anchors.insert(a, id);
        }
    }
}

// ------------------------------------------------------------------ constructor

/// A Python object under construction: collections are created empty and filled later, as
/// PyYAML's generator constructors do.
enum Obj {
    Value(Value),
    /// `SafeConstructor.nan_value`: one object, so all `.nan` keys are the same key.
    Nan,
    List(Vec<usize>),
    Tuple(Vec<usize>),
    Dict(Vec<(usize, usize)>),
    Set(Vec<usize>),
}

#[derive(Clone, Copy)]
enum Fill {
    Seq,
    Map,
    Set,
    Omap,
    Pairs,
}

struct Constructor {
    nodes: Vec<Node>,
    objs: Vec<Obj>,
    constructed: HashMap<usize, usize>,
    recursive: Vec<usize>,
    pending: Vec<(usize, usize, Fill)>,
}

impl Constructor {
    fn new_obj(&mut self, obj: Obj) -> usize {
        self.objs.push(obj);
        self.objs.len() - 1
    }

    fn construct_document(&mut self, node: usize) -> Result<Value, Crash> {
        let data = self.construct_object(node)?;
        while !self.pending.is_empty() {
            let jobs = std::mem::take(&mut self.pending);
            for (node, obj, fill) in jobs {
                self.fill(node, obj, fill)?;
            }
        }
        let mut visiting = Vec::new();
        self.materialize(data, &mut visiting)
    }

    fn construct_object(&mut self, node: usize) -> Result<usize, Crash> {
        if let Some(&obj) = self.constructed.get(&node) {
            return Ok(obj);
        }
        if self.recursive.contains(&node) {
            return Err(constructor_error(
                None,
                None,
                "found unconstructable recursive node",
                &self.nodes[node].start,
            ));
        }
        self.recursive.push(node);
        let tag = self.nodes[node].tag.clone();
        let short = tag.strip_prefix(TAG).unwrap_or("");
        let obj = match (tag.starts_with(TAG), short) {
            (true, "null") => {
                self.construct_scalar(node, 0)?;
                self.new_obj(Obj::Value(Value::Null))
            }
            (true, "bool") => {
                let v = self.construct_bool(node)?;
                self.new_obj(Obj::Value(v))
            }
            (true, "int") => {
                let v = self.construct_int(node)?;
                self.new_obj(Obj::Value(v))
            }
            (true, "float") => match self.construct_float(node)? {
                None => self.new_obj(Obj::Nan),
                Some(f) => self.new_obj(Obj::Value(Value::Float(f))),
            },
            (true, "binary") => {
                let v = self.construct_binary(node)?;
                self.new_obj(Obj::Value(v))
            }
            (true, "timestamp") => {
                let v = self.construct_timestamp(node)?;
                self.new_obj(Obj::Value(v))
            }
            (true, "str") => {
                let s = self.construct_scalar(node, 0)?;
                self.new_obj(Obj::Value(Value::Str(s)))
            }
            (true, "seq") => self.deferred(node, Obj::List(Vec::new()), Fill::Seq),
            (true, "map") => self.deferred(node, Obj::Dict(Vec::new()), Fill::Map),
            (true, "set") => self.deferred(node, Obj::Set(Vec::new()), Fill::Set),
            (true, "omap") => self.deferred(node, Obj::List(Vec::new()), Fill::Omap),
            (true, "pairs") => self.deferred(node, Obj::List(Vec::new()), Fill::Pairs),
            _ => {
                return Err(constructor_error(
                    None,
                    None,
                    &format!(
                        "could not determine a constructor for the tag {}",
                        repr_str(&tag)
                    ),
                    &self.nodes[node].start,
                ));
            }
        };
        self.constructed.insert(node, obj);
        self.recursive.retain(|&n| n != node);
        Ok(obj)
    }

    fn deferred(&mut self, node: usize, empty: Obj, fill: Fill) -> usize {
        let obj = self.new_obj(empty);
        self.pending.push((node, obj, fill));
        obj
    }

    fn fill(&mut self, node: usize, obj: usize, fill: Fill) -> Result<(), Crash> {
        match fill {
            Fill::Seq => {
                let items = self.construct_sequence(node)?;
                self.objs[obj] = Obj::List(items);
            }
            Fill::Map => {
                let pairs = self.construct_mapping(node)?;
                self.objs[obj] = Obj::Dict(pairs);
            }
            Fill::Set => {
                let pairs = self.construct_mapping(node)?;
                self.objs[obj] = Obj::Set(pairs.into_iter().map(|(k, _)| k).collect());
            }
            Fill::Omap | Fill::Pairs => {
                let context = if matches!(fill, Fill::Omap) {
                    "while constructing an ordered map"
                } else {
                    "while constructing pairs"
                };
                let n = &self.nodes[node];
                let NodeValue::Sequence(subnodes) = n.value.clone() else {
                    return Err(constructor_error(
                        Some(context),
                        Some(&n.start),
                        &format!("expected a sequence, but found {}", n.id()),
                        &n.start,
                    ));
                };
                let start = n.start.clone();
                for sub in subnodes {
                    let s = &self.nodes[sub];
                    let NodeValue::Mapping(pairs) = &s.value else {
                        return Err(constructor_error(
                            Some(context),
                            Some(&start),
                            &format!("expected a mapping of length 1, but found {}", s.id()),
                            &s.start,
                        ));
                    };
                    if pairs.len() != 1 {
                        return Err(constructor_error(
                            Some(context),
                            Some(&start),
                            &format!(
                                "expected a single mapping item, but found {} items",
                                pairs.len()
                            ),
                            &s.start,
                        ));
                    }
                    let (k, v) = pairs[0];
                    let key = self.construct_object(k)?;
                    let value = self.construct_object(v)?;
                    let pair = self.new_obj(Obj::Tuple(vec![key, value]));
                    if let Obj::List(items) = &mut self.objs[obj] {
                        items.push(pair);
                    }
                }
            }
        }
        Ok(())
    }

    /// `SafeConstructor.construct_scalar`: a mapping gives the value of its `=` key.
    fn construct_scalar(&self, node: usize, depth: usize) -> Result<String, Crash> {
        if depth > MAX_DEPTH {
            return Err(recursion_error());
        }
        let n = &self.nodes[node];
        match &n.value {
            NodeValue::Scalar(s) => Ok(s.clone()),
            NodeValue::Mapping(pairs) => {
                for &(k, v) in pairs {
                    if self.nodes[k].tag == format!("{TAG}value") {
                        return self.construct_scalar(v, depth + 1);
                    }
                }
                Err(constructor_error(
                    None,
                    None,
                    &format!("expected a scalar node, but found {}", n.id()),
                    &n.start,
                ))
            }
            NodeValue::Sequence(_) => Err(constructor_error(
                None,
                None,
                &format!("expected a scalar node, but found {}", n.id()),
                &n.start,
            )),
        }
    }

    fn construct_sequence(&mut self, node: usize) -> Result<Vec<usize>, Crash> {
        let n = &self.nodes[node];
        let NodeValue::Sequence(children) = n.value.clone() else {
            return Err(constructor_error(
                None,
                None,
                &format!("expected a sequence node, but found {}", n.id()),
                &n.start,
            ));
        };
        children
            .into_iter()
            .map(|c| self.construct_object(c))
            .collect()
    }

    fn flatten_mapping(&mut self, node: usize) -> Result<(), Crash> {
        let merge_tag = format!("{TAG}merge");
        let value_tag = format!("{TAG}value");
        let mut merge: Vec<(usize, usize)> = Vec::new();
        let mut index = 0;
        loop {
            let NodeValue::Mapping(pairs) = &self.nodes[node].value else {
                return Ok(());
            };
            if index >= pairs.len() {
                break;
            }
            let (key_node, value_node) = pairs[index];
            if self.nodes[key_node].tag == merge_tag {
                if let NodeValue::Mapping(pairs) = &mut self.nodes[node].value {
                    pairs.remove(index);
                }
                match self.nodes[value_node].value.clone() {
                    NodeValue::Mapping(_) => {
                        self.flatten_mapping(value_node)?;
                        if let NodeValue::Mapping(p) = &self.nodes[value_node].value {
                            merge.extend(p.iter().copied());
                        }
                    }
                    NodeValue::Sequence(subnodes) => {
                        let mut submerge = Vec::new();
                        for sub in subnodes {
                            if !matches!(self.nodes[sub].value, NodeValue::Mapping(_)) {
                                return Err(constructor_error(
                                    Some("while constructing a mapping"),
                                    Some(&self.nodes[node].start),
                                    &format!(
                                        "expected a mapping for merging, but found {}",
                                        self.nodes[sub].id()
                                    ),
                                    &self.nodes[sub].start,
                                ));
                            }
                            self.flatten_mapping(sub)?;
                            if let NodeValue::Mapping(p) = &self.nodes[sub].value {
                                submerge.push(p.clone());
                            }
                        }
                        submerge.reverse();
                        for value in submerge {
                            merge.extend(value);
                        }
                    }
                    NodeValue::Scalar(_) => {
                        return Err(constructor_error(
                            Some("while constructing a mapping"),
                            Some(&self.nodes[node].start),
                            &format!(
                                "expected a mapping or list of mappings for merging, but found {}",
                                self.nodes[value_node].id()
                            ),
                            &self.nodes[value_node].start,
                        ));
                    }
                }
            } else if self.nodes[key_node].tag == value_tag {
                self.nodes[key_node].tag = format!("{TAG}str");
                index += 1;
            } else {
                index += 1;
            }
        }
        if !merge.is_empty()
            && let NodeValue::Mapping(pairs) = &mut self.nodes[node].value
        {
            merge.append(pairs);
            *pairs = merge;
        }
        Ok(())
    }

    /// `construct_mapping`: the entries of a dict, keys compared as Python compares them.
    fn construct_mapping(&mut self, node: usize) -> Result<Vec<(usize, usize)>, Crash> {
        if matches!(self.nodes[node].value, NodeValue::Mapping(_)) {
            self.flatten_mapping(node)?;
        }
        let n = &self.nodes[node];
        let NodeValue::Mapping(pairs) = n.value.clone() else {
            return Err(constructor_error(
                None,
                None,
                &format!("expected a mapping node, but found {}", n.id()),
                &n.start,
            ));
        };
        let mut mapping: Vec<(usize, usize)> = Vec::new();
        for (key_node, value_node) in pairs {
            let key = self.construct_object(key_node)?;
            if !matches!(self.objs[key], Obj::Value(_) | Obj::Nan) {
                return Err(constructor_error(
                    Some("while constructing a mapping"),
                    Some(&self.nodes[node].start),
                    "found unhashable key",
                    &self.nodes[key_node].start,
                ));
            }
            let value = self.construct_object(value_node)?;
            match mapping.iter_mut().find(|(k, _)| self.same_key(*k, key)) {
                Some(entry) => entry.1 = value,
                None => mapping.push((key, value)),
            }
        }
        Ok(mapping)
    }

    fn same_key(&self, a: usize, b: usize) -> bool {
        match (&self.objs[a], &self.objs[b]) {
            (Obj::Nan, Obj::Nan) => true,
            (Obj::Value(x), Obj::Value(y)) => x.py_eq(y),
            _ => false,
        }
    }

    fn construct_bool(&self, node: usize) -> Result<Value, Crash> {
        let value = self.construct_scalar(node, 0)?;
        match value.to_lowercase().as_str() {
            "yes" | "true" | "on" => Ok(Value::Bool(true)),
            "no" | "false" | "off" => Ok(Value::Bool(false)),
            other => Err(Crash::new(format!("KeyError: {}", repr_str(other)))),
        }
    }

    fn construct_int(&self, node: usize) -> Result<Value, Crash> {
        let value = self.construct_scalar(node, 0)?.replace('_', "");
        let first = value
            .chars()
            .next()
            .ok_or_else(|| Crash::new("IndexError: string index out of range"))?;
        let negative = first == '-';
        let value = if first == '+' || first == '-' {
            &value[1..]
        } else {
            &value[..]
        };
        let sign = |n: BigInt| if negative { n.negated() } else { n };
        let n = if value == "0" {
            BigInt::zero()
        } else if let Some(rest) = value.strip_prefix("0b") {
            sign(num::int(rest, 2)?)
        } else if let Some(rest) = value.strip_prefix("0x") {
            sign(num::int(rest, 16)?)
        } else if value.starts_with('0') {
            sign(num::int(value, 8)?)
        } else if value.contains(':') {
            let mut digits = value
                .split(':')
                .map(|p| num::int(p, 10))
                .collect::<Result<Vec<_>, _>>()?;
            digits.reverse();
            let mut base = BigInt::from_i64(1);
            let sixty = BigInt::from_i64(60);
            let mut total = BigInt::zero();
            for d in digits {
                total = total.add(&d.mul(&base));
                base = base.mul(&sixty);
            }
            sign(total)
        } else {
            // `value[0]` of an empty string after the sign.
            if value.is_empty() {
                return Err(Crash::new("IndexError: string index out of range"));
            }
            sign(num::int(value, 10)?)
        };
        Ok(Value::Int(n.to_string()))
    }

    /// `None` is `nan_value`.
    fn construct_float(&self, node: usize) -> Result<Option<f64>, Crash> {
        let value = self
            .construct_scalar(node, 0)?
            .replace('_', "")
            .to_lowercase();
        let first = value
            .chars()
            .next()
            .ok_or_else(|| Crash::new("IndexError: string index out of range"))?;
        let sign = if first == '-' { -1.0 } else { 1.0 };
        let value = if first == '+' || first == '-' {
            &value[1..]
        } else {
            &value[..]
        };
        if value == ".inf" {
            return Ok(Some(sign * f64::INFINITY));
        }
        if value == ".nan" {
            return Ok(None);
        }
        if value.contains(':') {
            let mut digits = value
                .split(':')
                .map(num::float)
                .collect::<Result<Vec<_>, _>>()?;
            digits.reverse();
            let mut base = BigInt::from_i64(1);
            let sixty = BigInt::from_i64(60);
            let mut total = 0.0;
            for d in digits {
                total += d * base.to_f64()?;
                base = base.mul(&sixty);
            }
            return Ok(Some(sign * total));
        }
        Ok(Some(sign * num::float(value)?))
    }

    fn construct_binary(&self, node: usize) -> Result<Value, Crash> {
        let value = self.construct_scalar(node, 0)?;
        let start = &self.nodes[node].start;
        let chars: Vec<char> = value.chars().collect();
        if let Some(pos) = chars.iter().position(|c| !c.is_ascii()) {
            let run = chars[pos..].iter().take_while(|c| !c.is_ascii()).count();
            let what = if run > 1 {
                format!("characters in position {pos}-{}", pos + run - 1)
            } else {
                let code = crate::py::as_surrogate(chars[pos]).unwrap_or(chars[pos] as u32);
                let escaped = match code {
                    0..=0xff => format!("\\x{code:02x}"),
                    0x100..=0xffff => format!("\\u{code:04x}"),
                    _ => format!("\\U{code:08x}"),
                };
                format!("character '{escaped}' in position {pos}")
            };
            return Err(constructor_error(
                None,
                None,
                &format!(
                    "failed to convert base64 data into ascii: 'ascii' codec can't encode {what}: ordinal not in range(128)"
                ),
                start,
            ));
        }
        match decode_base64(value.as_bytes()) {
            Ok(bytes) => Ok(Value::Bytes(bytes)),
            Err(e) => Err(constructor_error(
                None,
                None,
                &format!("failed to decode base64 data: {e}"),
                start,
            )),
        }
    }

    fn construct_timestamp(&self, node: usize) -> Result<Value, Crash> {
        self.construct_scalar(node, 0)?;
        let NodeValue::Scalar(text) = &self.nodes[node].value else {
            return Err(Crash::new(
                "TypeError: expected string or bytes-like object, got 'list'",
            ));
        };
        let Some(m) = TIMESTAMP_PARTS.captures(text) else {
            return Err(Crash::new(
                "AttributeError: 'NoneType' object has no attribute 'groupdict'",
            ));
        };
        let int =
            |name: &str| -> i64 { m.name(name).map_or(0, |g| g.as_str().parse().unwrap_or(0)) };
        let (year, month, day) = (int("year"), int("month"), int("day"));
        let date = |y: i64, mo: i64, d: i64| -> Result<Date, Crash> {
            if !(1..=9999).contains(&y) {
                return Err(Crash::new(format!("ValueError: year {y} is out of range")));
            }
            if !(1..=12).contains(&mo) {
                return Err(Crash::new("ValueError: month must be in 1..12"));
            }
            if d < 1 || d > Date::days_in_month(y as i32, mo as u32) as i64 {
                return Err(Crash::new("ValueError: day is out of range for month"));
            }
            Ok(Date {
                year: y as i32,
                month: mo as u32,
                day: d as u32,
            })
        };
        if m.name("hour").is_none_or(|h| h.as_str().is_empty()) {
            return Ok(Value::Date(date(year, month, day)?));
        }
        let (hour, minute, second) = (int("hour"), int("minute"), int("second"));
        let mut fraction = 0u32;
        if let Some(f) = m.name("fraction").filter(|f| !f.as_str().is_empty()) {
            let mut digits: String = f.as_str().chars().take(6).collect();
            while digits.len() < 6 {
                digits.push('0');
            }
            fraction = digits.parse().unwrap_or(0);
        }
        let mut offset = None;
        if let Some(sign) = m.name("tz_sign") {
            let delta = int("tz_hour") * 3600 + int("tz_minute") * 60;
            let delta = if sign.as_str() == "-" { -delta } else { delta };
            if delta.abs() >= 86400 {
                return Err(Crash::new(format!(
                    "ValueError: offset must be a timedelta strictly between -timedelta(hours=24) and timedelta(hours=24), not {}.",
                    timedelta_repr(delta as i32)
                )));
            }
            offset = Some(delta as i32);
        } else if m.name("tz").is_some() {
            offset = Some(0);
        }
        let date = date(year, month, day)?;
        if !(0..=23).contains(&hour) {
            return Err(Crash::new("ValueError: hour must be in 0..23"));
        }
        if !(0..=59).contains(&minute) {
            return Err(Crash::new("ValueError: minute must be in 0..59"));
        }
        if !(0..=59).contains(&second) {
            return Err(Crash::new("ValueError: second must be in 0..59"));
        }
        Ok(Value::DateTime(DateTime {
            date,
            hour: hour as u32,
            minute: minute as u32,
            second: second as u32,
            microsecond: fraction,
            offset,
        }))
    }

    fn materialize(&self, obj: usize, visiting: &mut Vec<usize>) -> Result<Value, Crash> {
        if visiting.contains(&obj) || visiting.len() > MAX_DEPTH {
            return Err(recursion_error());
        }
        visiting.push(obj);
        let all = |items: &[usize], visiting: &mut Vec<usize>| -> Result<Vec<Value>, Crash> {
            items
                .iter()
                .map(|&i| self.materialize(i, visiting))
                .collect()
        };
        let v = match &self.objs[obj] {
            Obj::Value(v) => v.clone(),
            Obj::Nan => Value::Float(f64::NAN),
            Obj::List(items) => Value::List(all(items, visiting)?),
            Obj::Tuple(items) => Value::Tuple(all(items, visiting)?),
            Obj::Set(items) => Value::Set(all(items, visiting)?),
            Obj::Dict(pairs) => {
                let mut d = Dict::new();
                for &(k, v) in pairs {
                    let key = self.materialize(k, visiting)?;
                    let value = self.materialize(v, visiting)?;
                    d.push_new(key, value);
                }
                Value::Dict(d)
            }
        };
        visiting.pop();
        Ok(v)
    }
}

/// `base64.decodebytes`: `binascii.a2b_base64` in its default, lenient mode, as Python 3.13
/// has it: characters outside the alphabet are skipped and padding only counts at the end.
fn decode_base64(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut quad_pos = 0;
    let mut left: u32 = 0;
    let mut pads = 0;
    for &c in data {
        if c == b'=' {
            if quad_pos >= 2 {
                pads += 1;
            }
            continue;
        }
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        } as u32;
        pads = 0;
        match quad_pos {
            0 => {
                quad_pos = 1;
                left = v;
            }
            1 => {
                quad_pos = 2;
                out.push(((left << 2) | (v >> 4)) as u8);
                left = v & 0x0f;
            }
            2 => {
                quad_pos = 3;
                out.push(((left << 4) | (v >> 2)) as u8);
                left = v & 0x03;
            }
            _ => {
                quad_pos = 0;
                out.push(((left << 6) | v) as u8);
                left = 0;
            }
        }
    }
    if quad_pos == 1 {
        return Err(format!(
            "Invalid base64-encoded string: number of data characters ({}) cannot be 1 more than a multiple of 4",
            out.len() / 3 * 4 + 1
        ));
    }
    if quad_pos != 0 && quad_pos + pads < 4 {
        return Err("Incorrect padding".into());
    }
    Ok(out)
}

/// `yaml.safe_load(text)`.
pub(crate) fn load(text: &str) -> Result<Value, Crash> {
    let mut composer = Composer {
        parser: Parser::new(text)?,
        nodes: Vec::new(),
        anchors: HashMap::new(),
    };
    let Some(root) = composer.get_single_node()? else {
        return Ok(Value::Null);
    };
    let mut constructor = Constructor {
        nodes: composer.nodes,
        objs: Vec::new(),
        constructed: HashMap::new(),
        recursive: Vec::new(),
        pending: Vec::new(),
    };
    constructor.construct_document(root)
}
