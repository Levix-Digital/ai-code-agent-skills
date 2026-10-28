//! `yaml.representer.SafeRepresenter` and `yaml.serializer`: Python values as events.

use super::constructor::resolve_scalar;
use super::emitter::{Emitter, Event};
use crate::py::{Crash, Value, float_repr};

const TAG: &str = "tag:yaml.org,2002:";

/// The `safe_dump` options the scripts pass (always with `sort_keys=False`).
#[derive(Clone, Debug)]
pub struct DumpOptions {
    /// `default_flow_style`: `None` writes a collection of plain scalars in flow style.
    pub default_flow_style: Option<bool>,
    pub allow_unicode: bool,
    pub width: Option<usize>,
}

enum Node {
    Scalar {
        tag: String,
        value: String,
        style: Option<char>,
    },
    Sequence {
        tag: String,
        items: Vec<Node>,
        flow: bool,
    },
    Mapping {
        tag: String,
        pairs: Vec<(Node, Node)>,
        flow: bool,
    },
}

impl Node {
    fn is_plain_scalar(&self) -> bool {
        matches!(self, Node::Scalar { style: None, .. })
    }
}

fn scalar(tag: &str, value: impl Into<String>) -> Node {
    Node::Scalar {
        tag: format!("{TAG}{tag}"),
        value: value.into(),
        style: None,
    }
}

/// `base64.encodebytes`: lines of 76 characters, each ending with a newline.
fn encodebytes(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for line in data.chunks(57) {
        for chunk in line.chunks(3) {
            let b = [
                chunk[0],
                *chunk.get(1).unwrap_or(&0),
                *chunk.get(2).unwrap_or(&0),
            ];
            let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
            for i in 0..4 {
                if i <= chunk.len() {
                    out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
                } else {
                    out.push('=');
                }
            }
        }
        out.push('\n');
    }
    out
}

fn represent_float(f: f64) -> String {
    if f.is_nan() {
        return ".nan".into();
    }
    if f == f64::INFINITY {
        return ".inf".into();
    }
    if f == f64::NEG_INFINITY {
        return "-.inf".into();
    }
    let value = float_repr(f).to_lowercase();
    if !value.contains('.') && value.contains('e') {
        value.replacen('e', ".0e", 1)
    } else {
        value
    }
}

fn represent(v: &Value, flow_style: Option<bool>) -> Node {
    let collection = |items: Vec<Node>| -> (Vec<Node>, bool) {
        let best = items.iter().all(Node::is_plain_scalar);
        (items, flow_style.unwrap_or(best))
    };
    match v {
        Value::Null => scalar("null", "null"),
        Value::Str(s) => scalar("str", s.clone()),
        Value::Bytes(b) => Node::Scalar {
            tag: format!("{TAG}binary"),
            value: encodebytes(b),
            style: Some('|'),
        },
        Value::Bool(b) => scalar("bool", if *b { "true" } else { "false" }),
        Value::Int(i) => scalar("int", i.clone()),
        Value::Float(f) => scalar("float", represent_float(*f)),
        Value::List(items) | Value::Tuple(items) => {
            let (items, flow) =
                collection(items.iter().map(|i| represent(i, flow_style)).collect());
            Node::Sequence {
                tag: format!("{TAG}seq"),
                items,
                flow,
            }
        }
        Value::Dict(d) => mapping(
            "map",
            d.iter()
                .map(|(k, v)| (represent(k, flow_style), represent(v, flow_style)))
                .collect(),
            flow_style,
        ),
        Value::Set(items) => mapping(
            "set",
            items
                .iter()
                .map(|k| (represent(k, flow_style), scalar("null", "null")))
                .collect(),
            flow_style,
        ),
        Value::Date(d) => scalar("timestamp", d.iso()),
        Value::DateTime(d) => scalar("timestamp", d.iso(' ')),
    }
}

fn mapping(tag: &str, pairs: Vec<(Node, Node)>, flow_style: Option<bool>) -> Node {
    let best = pairs
        .iter()
        .all(|(k, v)| k.is_plain_scalar() && v.is_plain_scalar());
    Node::Mapping {
        tag: format!("{TAG}{tag}"),
        pairs,
        flow: flow_style.unwrap_or(best),
    }
}

fn serialize(node: &Node, events: &mut Vec<Event>) {
    match node {
        Node::Scalar { tag, value, style } => {
            let detected = resolve_scalar(value, (true, false));
            let default = resolve_scalar(value, (false, true));
            events.push(Event::Scalar {
                tag: Some(tag.clone()),
                implicit: (*tag == detected, *tag == default),
                value: value.clone(),
                style: *style,
            });
        }
        Node::Sequence { tag, items, flow } => {
            events.push(Event::SequenceStart {
                tag: Some(tag.clone()),
                implicit: *tag == format!("{TAG}seq"),
                flow: *flow,
            });
            for item in items {
                serialize(item, events);
            }
            events.push(Event::SequenceEnd);
        }
        Node::Mapping { tag, pairs, flow } => {
            events.push(Event::MappingStart {
                tag: Some(tag.clone()),
                implicit: *tag == format!("{TAG}map"),
                flow: *flow,
            });
            for (k, v) in pairs {
                serialize(k, events);
                serialize(v, events);
            }
            events.push(Event::MappingEnd);
        }
    }
}

/// `yaml.safe_dump(value, sort_keys=False, ...)`.
pub(crate) fn dump(value: &Value, options: &DumpOptions) -> Result<String, Crash> {
    let node = represent(value, options.default_flow_style);
    let mut events = vec![Event::StreamStart, Event::DocumentStart];
    serialize(&node, &mut events);
    events.push(Event::DocumentEnd);
    events.push(Event::StreamEnd);
    Ok(Emitter::new(options.allow_unicode, options.width).emit_all(events))
}
