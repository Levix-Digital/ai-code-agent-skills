//! `yaml.parser`: tokens as events.

use super::scanner::{DirectiveValue, Kind, Scanner, Token};
use super::{Mark, marked};
use crate::py::{Crash, repr_str};

#[derive(Clone, Debug)]
pub(crate) enum EventKind {
    StreamStart,
    StreamEnd,
    DocumentStart,
    DocumentEnd,
    Alias(String),
    Scalar {
        anchor: Option<String>,
        tag: Option<String>,
        implicit: (bool, bool),
        value: String,
    },
    SequenceStart {
        anchor: Option<String>,
        tag: Option<String>,
    },
    SequenceEnd,
    MappingStart {
        anchor: Option<String>,
        tag: Option<String>,
    },
    MappingEnd,
}

#[derive(Clone)]
pub(crate) struct Event {
    pub kind: EventKind,
    pub start: Mark,
}

impl Event {
    pub fn anchor(&self) -> Option<&str> {
        match &self.kind {
            EventKind::Scalar { anchor, .. }
            | EventKind::SequenceStart { anchor, .. }
            | EventKind::MappingStart { anchor, .. } => anchor.as_deref(),
            EventKind::Alias(a) => Some(a),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    StreamStart,
    ImplicitDocumentStart,
    DocumentStart,
    DocumentEnd,
    DocumentContent,
    BlockNode,
    BlockSequenceFirstEntry,
    BlockSequenceEntry,
    IndentlessSequenceEntry,
    BlockMappingFirstKey,
    BlockMappingKey,
    BlockMappingValue,
    FlowSequenceFirstEntry,
    FlowSequenceEntry,
    FlowSequenceEntryMappingKey,
    FlowSequenceEntryMappingValue,
    FlowSequenceEntryMappingEnd,
    FlowMappingFirstKey,
    FlowMappingKey,
    FlowMappingValue,
    FlowMappingEmptyValue,
    End,
}

pub(crate) struct Parser {
    scanner: Scanner,
    current: Option<Event>,
    tag_handles: Vec<(String, String)>,
    states: Vec<State>,
    marks: Vec<Mark>,
    state: State,
}

fn parser_error(context: Option<&str>, cm: Option<&Mark>, problem: &str, pm: &Mark) -> Crash {
    marked("parser.ParserError", context, cm, problem, Some(pm))
}

fn default_tags() -> Vec<(String, String)> {
    vec![
        ("!".into(), "!".into()),
        ("!!".into(), "tag:yaml.org,2002:".into()),
    ]
}

/// What `check_token` asks about a token.
#[derive(Clone, Copy, PartialEq)]
enum T {
    Directive,
    DocumentStart,
    DocumentEnd,
    StreamEnd,
    BlockSequenceStart,
    BlockMappingStart,
    BlockEnd,
    FlowSequenceStart,
    FlowMappingStart,
    FlowSequenceEnd,
    FlowMappingEnd,
    Key,
    Value,
    BlockEntry,
    FlowEntry,
    Alias,
    Anchor,
    Tag,
    Scalar,
    Other,
}

fn t(kind: &Kind) -> T {
    match kind {
        Kind::Directive(..) => T::Directive,
        Kind::DocumentStart => T::DocumentStart,
        Kind::DocumentEnd => T::DocumentEnd,
        Kind::StreamEnd => T::StreamEnd,
        Kind::BlockSequenceStart => T::BlockSequenceStart,
        Kind::BlockMappingStart => T::BlockMappingStart,
        Kind::BlockEnd => T::BlockEnd,
        Kind::FlowSequenceStart => T::FlowSequenceStart,
        Kind::FlowMappingStart => T::FlowMappingStart,
        Kind::FlowSequenceEnd => T::FlowSequenceEnd,
        Kind::FlowMappingEnd => T::FlowMappingEnd,
        Kind::Key => T::Key,
        Kind::Value => T::Value,
        Kind::BlockEntry => T::BlockEntry,
        Kind::FlowEntry => T::FlowEntry,
        Kind::Alias(_) => T::Alias,
        Kind::Anchor(_) => T::Anchor,
        Kind::Tag(..) => T::Tag,
        Kind::Scalar { .. } => T::Scalar,
        Kind::StreamStart => T::Other,
    }
}

impl Parser {
    pub fn new(text: &str) -> Result<Self, Crash> {
        Ok(Parser {
            scanner: Scanner::new(text)?,
            current: None,
            tag_handles: Vec::new(),
            states: Vec::new(),
            marks: Vec::new(),
            state: State::StreamStart,
        })
    }

    // ------------------------------------------------------------- events

    fn fill(&mut self) -> Result<(), Crash> {
        if self.current.is_none() && self.state != State::End {
            self.current = Some(self.run_state()?);
        }
        Ok(())
    }

    pub fn peek_event(&mut self) -> Result<Option<&Event>, Crash> {
        self.fill()?;
        Ok(self.current.as_ref())
    }

    pub fn get_event(&mut self) -> Result<Option<Event>, Crash> {
        self.fill()?;
        Ok(self.current.take())
    }

    // ------------------------------------------------------------- tokens

    fn check(&mut self, choices: &[T]) -> Result<bool, Crash> {
        Ok(self
            .scanner
            .peek_token()?
            .is_some_and(|tok| choices.contains(&t(&tok.kind))))
    }

    fn peek_token(&mut self) -> Result<Token, Crash> {
        Ok(self
            .scanner
            .peek_token()?
            .cloned()
            .expect("the scanner ends with a stream end token"))
    }

    fn get_token(&mut self) -> Result<Token, Crash> {
        Ok(self
            .scanner
            .get_token()?
            .expect("the scanner ends with a stream end token"))
    }

    fn pop_state(&mut self) -> State {
        self.states.pop().unwrap_or(State::End)
    }

    fn run_state(&mut self) -> Result<Event, Crash> {
        match self.state {
            State::StreamStart => {
                let token = self.get_token()?;
                self.state = State::ImplicitDocumentStart;
                Ok(Event {
                    kind: EventKind::StreamStart,
                    start: token.start,
                })
            }
            State::ImplicitDocumentStart => {
                if !self.check(&[T::Directive, T::DocumentStart, T::StreamEnd])? {
                    self.tag_handles = default_tags();
                    let token = self.peek_token()?;
                    self.states.push(State::DocumentEnd);
                    self.state = State::BlockNode;
                    Ok(Event {
                        kind: EventKind::DocumentStart,
                        start: token.start,
                    })
                } else {
                    self.parse_document_start()
                }
            }
            State::DocumentStart => self.parse_document_start(),
            State::DocumentEnd => {
                let token = self.peek_token()?;
                if self.check(&[T::DocumentEnd])? {
                    self.get_token()?;
                }
                self.state = State::DocumentStart;
                Ok(Event {
                    kind: EventKind::DocumentEnd,
                    start: token.start,
                })
            }
            State::DocumentContent => {
                if self.check(&[T::Directive, T::DocumentStart, T::DocumentEnd, T::StreamEnd])? {
                    let mark = self.peek_token()?.start;
                    self.state = self.pop_state();
                    Ok(empty_scalar(mark))
                } else {
                    self.parse_node(true, false)
                }
            }
            State::BlockNode => self.parse_node(true, false),
            State::BlockSequenceFirstEntry => {
                let token = self.get_token()?;
                self.marks.push(token.start);
                self.parse_block_sequence_entry()
            }
            State::BlockSequenceEntry => self.parse_block_sequence_entry(),
            State::IndentlessSequenceEntry => {
                if self.check(&[T::BlockEntry])? {
                    let token = self.get_token()?;
                    if !self.check(&[T::BlockEntry, T::Key, T::Value, T::BlockEnd])? {
                        self.states.push(State::IndentlessSequenceEntry);
                        return self.parse_node(true, false);
                    }
                    self.state = State::IndentlessSequenceEntry;
                    return Ok(empty_scalar(token.end));
                }
                let token = self.peek_token()?;
                self.state = self.pop_state();
                Ok(Event {
                    kind: EventKind::SequenceEnd,
                    start: token.start,
                })
            }
            State::BlockMappingFirstKey => {
                let token = self.get_token()?;
                self.marks.push(token.start);
                self.parse_block_mapping_key()
            }
            State::BlockMappingKey => self.parse_block_mapping_key(),
            State::BlockMappingValue => {
                if self.check(&[T::Value])? {
                    let token = self.get_token()?;
                    if !self.check(&[T::Key, T::Value, T::BlockEnd])? {
                        self.states.push(State::BlockMappingKey);
                        return self.parse_node(true, true);
                    }
                    self.state = State::BlockMappingKey;
                    return Ok(empty_scalar(token.end));
                }
                self.state = State::BlockMappingKey;
                let token = self.peek_token()?;
                Ok(empty_scalar(token.start))
            }
            State::FlowSequenceFirstEntry => {
                let token = self.get_token()?;
                self.marks.push(token.start);
                self.parse_flow_sequence_entry(true)
            }
            State::FlowSequenceEntry => self.parse_flow_sequence_entry(false),
            State::FlowSequenceEntryMappingKey => {
                let token = self.get_token()?;
                if !self.check(&[T::Value, T::FlowEntry, T::FlowSequenceEnd])? {
                    self.states.push(State::FlowSequenceEntryMappingValue);
                    return self.parse_node(false, false);
                }
                self.state = State::FlowSequenceEntryMappingValue;
                Ok(empty_scalar(token.end))
            }
            State::FlowSequenceEntryMappingValue => {
                if self.check(&[T::Value])? {
                    let token = self.get_token()?;
                    if !self.check(&[T::FlowEntry, T::FlowSequenceEnd])? {
                        self.states.push(State::FlowSequenceEntryMappingEnd);
                        return self.parse_node(false, false);
                    }
                    self.state = State::FlowSequenceEntryMappingEnd;
                    return Ok(empty_scalar(token.end));
                }
                self.state = State::FlowSequenceEntryMappingEnd;
                let token = self.peek_token()?;
                Ok(empty_scalar(token.start))
            }
            State::FlowSequenceEntryMappingEnd => {
                self.state = State::FlowSequenceEntry;
                let token = self.peek_token()?;
                Ok(Event {
                    kind: EventKind::MappingEnd,
                    start: token.start,
                })
            }
            State::FlowMappingFirstKey => {
                let token = self.get_token()?;
                self.marks.push(token.start);
                self.parse_flow_mapping_key(true)
            }
            State::FlowMappingKey => self.parse_flow_mapping_key(false),
            State::FlowMappingValue => {
                if self.check(&[T::Value])? {
                    let token = self.get_token()?;
                    if !self.check(&[T::FlowEntry, T::FlowMappingEnd])? {
                        self.states.push(State::FlowMappingKey);
                        return self.parse_node(false, false);
                    }
                    self.state = State::FlowMappingKey;
                    return Ok(empty_scalar(token.end));
                }
                self.state = State::FlowMappingKey;
                let token = self.peek_token()?;
                Ok(empty_scalar(token.start))
            }
            State::FlowMappingEmptyValue => {
                self.state = State::FlowMappingKey;
                let token = self.peek_token()?;
                Ok(empty_scalar(token.start))
            }
            State::End => unreachable!("no event after the stream end"),
        }
    }

    fn parse_document_start(&mut self) -> Result<Event, Crash> {
        while self.check(&[T::DocumentEnd])? {
            self.get_token()?;
        }
        if !self.check(&[T::StreamEnd])? {
            let start = self.peek_token()?.start;
            self.process_directives()?;
            if !self.check(&[T::DocumentStart])? {
                let token = self.peek_token()?;
                return Err(parser_error(
                    None,
                    None,
                    &format!(
                        "expected '<document start>', but found {}",
                        repr_str(token.kind.id())
                    ),
                    &token.start,
                ));
            }
            self.get_token()?;
            self.states.push(State::DocumentEnd);
            self.state = State::DocumentContent;
            Ok(Event {
                kind: EventKind::DocumentStart,
                start,
            })
        } else {
            let token = self.get_token()?;
            self.state = State::End;
            Ok(Event {
                kind: EventKind::StreamEnd,
                start: token.start,
            })
        }
    }

    fn process_directives(&mut self) -> Result<(), Crash> {
        let mut yaml_version = false;
        self.tag_handles = Vec::new();
        while self.check(&[T::Directive])? {
            let token = self.get_token()?;
            match &token.kind {
                Kind::Directive(name, DirectiveValue::Version(major, _)) if name == "YAML" => {
                    if yaml_version {
                        return Err(parser_error(
                            None,
                            None,
                            "found duplicate YAML directive",
                            &token.start,
                        ));
                    }
                    if major != "1" {
                        return Err(parser_error(
                            None,
                            None,
                            "found incompatible YAML document (version 1.* is required)",
                            &token.start,
                        ));
                    }
                    yaml_version = true;
                }
                Kind::Directive(name, DirectiveValue::Tag(handle, prefix)) if name == "TAG" => {
                    if self.tag_handles.iter().any(|(h, _)| h == handle) {
                        return Err(parser_error(
                            None,
                            None,
                            &format!("duplicate tag handle {}", repr_str(handle)),
                            &token.start,
                        ));
                    }
                    self.tag_handles.push((handle.clone(), prefix.clone()));
                }
                _ => {}
            }
        }
        for (key, value) in default_tags() {
            if !self.tag_handles.iter().any(|(h, _)| *h == key) {
                self.tag_handles.push((key, value));
            }
        }
        Ok(())
    }

    fn parse_node(&mut self, block: bool, indentless_sequence: bool) -> Result<Event, Crash> {
        if self.check(&[T::Alias])? {
            let token = self.get_token()?;
            let Kind::Alias(name) = token.kind else {
                unreachable!()
            };
            self.state = self.pop_state();
            return Ok(Event {
                kind: EventKind::Alias(name),
                start: token.start,
            });
        }
        let mut anchor = None;
        let mut tag: Option<(Option<String>, String)> = None;
        let mut start_mark = None;
        let mut tag_mark = None;
        if self.check(&[T::Anchor])? {
            let token = self.get_token()?;
            start_mark = Some(token.start.clone());
            if let Kind::Anchor(a) = token.kind {
                anchor = Some(a);
            }
            if self.check(&[T::Tag])? {
                let token = self.get_token()?;
                tag_mark = Some(token.start.clone());
                if let Kind::Tag(h, s) = token.kind {
                    tag = Some((h, s));
                }
            }
        } else if self.check(&[T::Tag])? {
            let token = self.get_token()?;
            start_mark = Some(token.start.clone());
            tag_mark = Some(token.start.clone());
            if let Kind::Tag(h, s) = token.kind {
                tag = Some((h, s));
            }
            if self.check(&[T::Anchor])? {
                let token = self.get_token()?;
                if let Kind::Anchor(a) = token.kind {
                    anchor = Some(a);
                }
            }
        }
        let tag: Option<String> = match tag {
            None => None,
            Some((None, suffix)) => Some(suffix),
            Some((Some(handle), suffix)) => {
                match self.tag_handles.iter().find(|(h, _)| *h == handle) {
                    Some((_, prefix)) => Some(format!("{prefix}{suffix}")),
                    None => {
                        return Err(parser_error(
                            Some("while parsing a node"),
                            start_mark.as_ref(),
                            &format!("found undefined tag handle {}", repr_str(&handle)),
                            tag_mark.as_ref().expect("a tag has a mark"),
                        ));
                    }
                }
            }
        };
        let start = match start_mark {
            Some(m) => m,
            None => self.peek_token()?.start,
        };
        let implicit = tag.is_none() || tag.as_deref() == Some("!");
        if indentless_sequence && self.check(&[T::BlockEntry])? {
            self.state = State::IndentlessSequenceEntry;
            return Ok(Event {
                kind: EventKind::SequenceStart { anchor, tag },
                start,
            });
        }
        if self.check(&[T::Scalar])? {
            let token = self.get_token()?;
            let Kind::Scalar { value, plain, .. } = token.kind else {
                unreachable!()
            };
            let implicit = if (plain && tag.is_none()) || tag.as_deref() == Some("!") {
                (true, false)
            } else if tag.is_none() {
                (false, true)
            } else {
                (false, false)
            };
            self.state = self.pop_state();
            return Ok(Event {
                kind: EventKind::Scalar {
                    anchor,
                    tag,
                    implicit,
                    value,
                },
                start,
            });
        }
        let (kind, state) = if self.check(&[T::FlowSequenceStart])? {
            (
                EventKind::SequenceStart { anchor, tag },
                State::FlowSequenceFirstEntry,
            )
        } else if self.check(&[T::FlowMappingStart])? {
            (
                EventKind::MappingStart { anchor, tag },
                State::FlowMappingFirstKey,
            )
        } else if block && self.check(&[T::BlockSequenceStart])? {
            (
                EventKind::SequenceStart { anchor, tag },
                State::BlockSequenceFirstEntry,
            )
        } else if block && self.check(&[T::BlockMappingStart])? {
            (
                EventKind::MappingStart { anchor, tag },
                State::BlockMappingFirstKey,
            )
        } else if anchor.is_some() || tag.is_some() {
            self.state = self.pop_state();
            return Ok(Event {
                kind: EventKind::Scalar {
                    anchor,
                    tag,
                    implicit: (implicit, false),
                    value: String::new(),
                },
                start,
            });
        } else {
            let node = if block { "block" } else { "flow" };
            let token = self.peek_token()?;
            return Err(parser_error(
                Some(&format!("while parsing a {node} node")),
                Some(&start),
                &format!(
                    "expected the node content, but found {}",
                    repr_str(token.kind.id())
                ),
                &token.start,
            ));
        };
        self.state = state;
        Ok(Event { kind, start })
    }

    fn parse_block_sequence_entry(&mut self) -> Result<Event, Crash> {
        if self.check(&[T::BlockEntry])? {
            let token = self.get_token()?;
            if !self.check(&[T::BlockEntry, T::BlockEnd])? {
                self.states.push(State::BlockSequenceEntry);
                return self.parse_node(true, false);
            }
            self.state = State::BlockSequenceEntry;
            return Ok(empty_scalar(token.end));
        }
        if !self.check(&[T::BlockEnd])? {
            let token = self.peek_token()?;
            return Err(parser_error(
                Some("while parsing a block collection"),
                self.marks.last(),
                &format!(
                    "expected <block end>, but found {}",
                    repr_str(token.kind.id())
                ),
                &token.start,
            ));
        }
        let token = self.get_token()?;
        self.state = self.pop_state();
        self.marks.pop();
        Ok(Event {
            kind: EventKind::SequenceEnd,
            start: token.start,
        })
    }

    fn parse_block_mapping_key(&mut self) -> Result<Event, Crash> {
        if self.check(&[T::Key])? {
            let token = self.get_token()?;
            if !self.check(&[T::Key, T::Value, T::BlockEnd])? {
                self.states.push(State::BlockMappingValue);
                return self.parse_node(true, true);
            }
            self.state = State::BlockMappingValue;
            return Ok(empty_scalar(token.end));
        }
        if !self.check(&[T::BlockEnd])? {
            let token = self.peek_token()?;
            return Err(parser_error(
                Some("while parsing a block mapping"),
                self.marks.last(),
                &format!(
                    "expected <block end>, but found {}",
                    repr_str(token.kind.id())
                ),
                &token.start,
            ));
        }
        let token = self.get_token()?;
        self.state = self.pop_state();
        self.marks.pop();
        Ok(Event {
            kind: EventKind::MappingEnd,
            start: token.start,
        })
    }

    fn parse_flow_sequence_entry(&mut self, first: bool) -> Result<Event, Crash> {
        if !self.check(&[T::FlowSequenceEnd])? {
            if !first {
                if self.check(&[T::FlowEntry])? {
                    self.get_token()?;
                } else {
                    let token = self.peek_token()?;
                    return Err(parser_error(
                        Some("while parsing a flow sequence"),
                        self.marks.last(),
                        &format!("expected ',' or ']', but got {}", repr_str(token.kind.id())),
                        &token.start,
                    ));
                }
            }
            if self.check(&[T::Key])? {
                let token = self.peek_token()?;
                self.state = State::FlowSequenceEntryMappingKey;
                return Ok(Event {
                    kind: EventKind::MappingStart {
                        anchor: None,
                        tag: None,
                    },
                    start: token.start,
                });
            } else if !self.check(&[T::FlowSequenceEnd])? {
                self.states.push(State::FlowSequenceEntry);
                return self.parse_node(false, false);
            }
        }
        let token = self.get_token()?;
        self.state = self.pop_state();
        self.marks.pop();
        Ok(Event {
            kind: EventKind::SequenceEnd,
            start: token.start,
        })
    }

    fn parse_flow_mapping_key(&mut self, first: bool) -> Result<Event, Crash> {
        if !self.check(&[T::FlowMappingEnd])? {
            if !first {
                if self.check(&[T::FlowEntry])? {
                    self.get_token()?;
                } else {
                    let token = self.peek_token()?;
                    return Err(parser_error(
                        Some("while parsing a flow mapping"),
                        self.marks.last(),
                        &format!(
                            "expected ',' or '}}', but got {}",
                            repr_str(token.kind.id())
                        ),
                        &token.start,
                    ));
                }
            }
            if self.check(&[T::Key])? {
                let token = self.get_token()?;
                if !self.check(&[T::Value, T::FlowEntry, T::FlowMappingEnd])? {
                    self.states.push(State::FlowMappingValue);
                    return self.parse_node(false, false);
                }
                self.state = State::FlowMappingValue;
                return Ok(empty_scalar(token.end));
            } else if !self.check(&[T::FlowMappingEnd])? {
                self.states.push(State::FlowMappingEmptyValue);
                return self.parse_node(false, false);
            }
        }
        let token = self.get_token()?;
        self.state = self.pop_state();
        self.marks.pop();
        Ok(Event {
            kind: EventKind::MappingEnd,
            start: token.start,
        })
    }
}

fn empty_scalar(mark: Mark) -> Event {
    Event {
        kind: EventKind::Scalar {
            anchor: None,
            tag: None,
            implicit: (true, false),
            value: String::new(),
        },
        start: mark,
    }
}
