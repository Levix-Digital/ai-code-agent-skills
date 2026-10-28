//! Python's `argparse` for the parsers the scripts build: the same option matching (unique
//! prefixes, `--opt=value`), the same help layout and the same error messages and exit codes.
//! Grows with the scripts translated; today it covers optional arguments that store a value or
//! `True`, required or not.

use super::{repr_str, textwrap};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Help,
    /// `action="store_true"`.
    StoreTrue,
    /// `action="store"` with one value.
    Store,
}

#[derive(Debug, Clone)]
pub struct Arg {
    pub flags: Vec<String>,
    pub dest: String,
    pub kind: Kind,
    pub help: String,
    pub required: bool,
    /// `choices=[...]`: empty when any value is accepted.
    pub choices: Vec<String>,
    /// `type=int`: the value must be what `int()` reads.
    pub int: bool,
}

impl Arg {
    fn metavar(&self) -> String {
        if self.choices.is_empty() {
            self.dest.to_uppercase()
        } else {
            format!("{{{}}}", self.choices.join(","))
        }
    }

    /// `-h/--help`, as error messages name the argument.
    fn name(&self) -> String {
        self.flags.join("/")
    }

    fn takes_value(&self) -> bool {
        self.kind == Kind::Store
    }

    fn usage(&self) -> String {
        let first = &self.flags[0];
        let part = if self.takes_value() {
            format!("{first} {}", self.metavar())
        } else {
            first.clone()
        };
        if self.required {
            part
        } else {
            format!("[{part}]")
        }
    }

    fn invocation(&self) -> String {
        if self.takes_value() {
            let mv = self.metavar();
            self.flags
                .iter()
                .map(|f| format!("{f} {mv}"))
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            self.flags.join(", ")
        }
    }
}

#[derive(Debug, Default)]
pub struct Parsed {
    values: BTreeMap<String, String>,
    flags: BTreeMap<String, bool>,
}

impl Parsed {
    /// A `store` argument: `None` when not given.
    pub fn value(&self, dest: &str) -> Option<&str> {
        self.values.get(dest).map(String::as_str)
    }

    pub fn flag(&self, dest: &str) -> bool {
        self.flags.get(dest).copied().unwrap_or(false)
    }
}

/// What argparse prints before it exits: help on stdout (0), usage errors on stderr (2).
#[derive(Debug, PartialEq)]
pub enum Exit {
    Help(String),
    Error(String),
}

impl Exit {
    /// Prints the text and returns the exit code.
    pub fn report(&self) -> u8 {
        match self {
            Exit::Help(t) => {
                crate::out::out(t);
                0
            }
            Exit::Error(t) => {
                crate::out::err(t);
                2
            }
        }
    }
}

pub struct Parser {
    pub prog: String,
    pub description: Option<String>,
    pub args: Vec<Arg>,
}

/// One command-line string, classified like `_parse_optional`.
enum Token {
    Positional,
    Optional {
        arg: Option<usize>,
        option: String,
        explicit: Option<String>,
        /// `-xVALUE`: the value follows a short option directly (no `=`).
        short_glued: bool,
    },
}

impl Parser {
    pub fn new(prog: &str, description: &str) -> Self {
        Parser {
            prog: prog.to_string(),
            description: Some(description.to_string()),
            args: vec![Arg {
                flags: vec!["-h".into(), "--help".into()],
                dest: "help".into(),
                kind: Kind::Help,
                help: "show this help message and exit".into(),
                required: false,
                choices: Vec::new(),
                int: false,
            }],
        }
    }

    fn add(mut self, flag: &str, kind: Kind, help: &str, required: bool) -> Self {
        let dest = flag.trim_start_matches('-').replace('-', "_");
        self.args.push(Arg {
            flags: vec![flag.to_string()],
            dest,
            kind,
            help: help.to_string(),
            required,
            choices: Vec::new(),
            int: false,
        });
        self
    }

    pub fn flag(self, flag: &str, help: &str) -> Self {
        self.add(flag, Kind::StoreTrue, help, false)
    }

    pub fn option(self, flag: &str, help: &str) -> Self {
        self.add(flag, Kind::Store, help, false)
    }

    /// `add_argument(flag, required=True)`.
    pub fn required_option(self, flag: &str, help: &str) -> Self {
        self.add(flag, Kind::Store, help, true)
    }

    /// `add_argument(flag, choices=choices)`.
    pub fn choice_option(mut self, flag: &str, choices: &[&str], help: &str) -> Self {
        self = self.add(flag, Kind::Store, help, false);
        if let Some(arg) = self.args.last_mut() {
            arg.choices = choices.iter().map(|c| c.to_string()).collect();
        }
        self
    }

    /// `add_argument(flag, type=int)`: the value is kept as given; `int()` reads it.
    pub fn int_option(mut self, flag: &str, help: &str) -> Self {
        self = self.add(flag, Kind::Store, help, false);
        if let Some(arg) = self.args.last_mut() {
            arg.int = true;
        }
        self
    }

    fn option_strings(&self) -> Vec<(&str, usize)> {
        let mut out = Vec::new();
        for (i, a) in self.args.iter().enumerate() {
            for f in &a.flags {
                out.push((f.as_str(), i));
            }
        }
        out
    }

    fn find(&self, option: &str) -> Option<usize> {
        self.option_strings()
            .into_iter()
            .find(|(f, _)| *f == option)
            .map(|(_, i)| i)
    }

    fn error(&self, msg: &str) -> Exit {
        Exit::Error(format!(
            "{}{}: error: {msg}\n",
            self.format_usage(),
            self.prog
        ))
    }

    fn get_option_tuples(&self, s: &str) -> Vec<Token> {
        let chars: Vec<char> = s.chars().collect();
        let mut result = Vec::new();
        if chars.len() >= 2 && chars[0] == '-' && chars[1] == '-' {
            let (prefix, explicit) = match s.split_once('=') {
                Some((p, e)) => (p, Some(e.to_string())),
                None => (s, None),
            };
            for (f, i) in self.option_strings() {
                if f.starts_with(prefix) {
                    result.push(Token::Optional {
                        arg: Some(i),
                        option: f.to_string(),
                        explicit: explicit.clone(),
                        short_glued: false,
                    });
                }
            }
        } else if chars.len() >= 2 && chars[0] == '-' {
            let (prefix, explicit) = match s.split_once('=') {
                Some((p, e)) => (p, Some(e.to_string())),
                None => (s, None),
            };
            let short: String = chars[..2].iter().collect();
            let short_explicit: String = chars[2..].iter().collect();
            for (f, i) in self.option_strings() {
                if f == short {
                    result.push(Token::Optional {
                        arg: Some(i),
                        option: f.to_string(),
                        explicit: Some(short_explicit.clone()),
                        short_glued: true,
                    });
                } else if f.starts_with(prefix) {
                    result.push(Token::Optional {
                        arg: Some(i),
                        option: f.to_string(),
                        explicit: explicit.clone(),
                        short_glued: false,
                    });
                }
            }
        }
        result
    }

    fn classify(&self, s: &str) -> Result<Token, Exit> {
        if s.is_empty() || !s.starts_with('-') {
            return Ok(Token::Positional);
        }
        if let Some(i) = self.find(s) {
            return Ok(Token::Optional {
                arg: Some(i),
                option: s.to_string(),
                explicit: None,
                short_glued: false,
            });
        }
        if s.chars().count() == 1 {
            return Ok(Token::Positional);
        }
        if let Some((opt, explicit)) = s.split_once('=')
            && let Some(i) = self.find(opt)
        {
            return Ok(Token::Optional {
                arg: Some(i),
                option: opt.to_string(),
                explicit: Some(explicit.to_string()),
                short_glued: false,
            });
        }
        let mut tuples = self.get_option_tuples(s);
        if tuples.len() > 1 {
            let names: Vec<String> = tuples
                .iter()
                .map(|t| match t {
                    Token::Optional { option, .. } => option.clone(),
                    Token::Positional => String::new(),
                })
                .collect();
            return Err(self.error(&format!(
                "ambiguous option: {s} could match {}",
                names.join(", ")
            )));
        }
        if let Some(t) = tuples.pop() {
            return Ok(t);
        }
        if is_negative_number(s) || s.contains(' ') {
            return Ok(Token::Positional);
        }
        Ok(Token::Optional {
            arg: None,
            option: s.to_string(),
            explicit: None,
            short_glued: false,
        })
    }

    /// `parser.parse_args(argv)`.
    pub fn parse(&self, argv: &[String]) -> Result<Parsed, Exit> {
        // Classify every string first: '--' makes everything after it positional.
        let mut tokens = Vec::new();
        let mut after_dashes = false;
        for s in argv {
            if after_dashes {
                tokens.push(Token::Positional);
            } else if s == "--" {
                after_dashes = true;
                tokens.push(Token::Positional);
            } else {
                tokens.push(self.classify(s)?);
            }
        }
        let mut parsed = Parsed::default();
        let mut extras: Vec<String> = Vec::new();
        let mut i = 0;
        while i < argv.len() {
            match &tokens[i] {
                Token::Positional => {
                    extras.push(argv[i].clone());
                    i += 1;
                }
                Token::Optional { arg: None, .. } => {
                    extras.push(argv[i].clone());
                    i += 1;
                }
                Token::Optional {
                    arg: Some(a),
                    explicit,
                    short_glued,
                    ..
                } => {
                    let arg = &self.args[*a];
                    let mut actions: Vec<(usize, Option<String>)> = Vec::new();
                    let mut next = i + 1;
                    match explicit {
                        Some(value) if arg.takes_value() => {
                            actions.push((*a, Some(value.clone())));
                        }
                        Some(value) => {
                            // A flag glued to more short flags (`-hv`) or given a value.
                            if *short_glued && !value.is_empty() {
                                actions.push((*a, None));
                                let rest = format!("-{value}");
                                if let Some(b) = self.find(&rest) {
                                    actions.push((b, None));
                                } else {
                                    extras.push(rest);
                                }
                            } else {
                                return Err(self.error(&format!(
                                    "argument {}: ignored explicit argument {}",
                                    arg.name(),
                                    repr_str(value)
                                )));
                            }
                        }
                        None if arg.takes_value() => match tokens.get(i + 1) {
                            Some(Token::Positional) if argv[i + 1] != "--" => {
                                actions.push((*a, Some(argv[i + 1].clone())));
                                next = i + 2;
                            }
                            _ => {
                                return Err(self.error(&format!(
                                    "argument {}: expected one argument",
                                    arg.name()
                                )));
                            }
                        },
                        None => actions.push((*a, None)),
                    }
                    for (a, value) in actions {
                        let arg = &self.args[a];
                        match arg.kind {
                            Kind::Help => return Err(Exit::Help(self.format_help())),
                            Kind::StoreTrue => {
                                parsed.flags.insert(arg.dest.clone(), true);
                            }
                            Kind::Store => {
                                let value = value.unwrap_or_default();
                                if arg.int && super::num::int(&value, 10).is_err() {
                                    return Err(self.error(&format!(
                                        "argument {}: invalid int value: {}",
                                        arg.name(),
                                        repr_str(&value)
                                    )));
                                }
                                if !arg.choices.is_empty() && !arg.choices.contains(&value) {
                                    let choices: Vec<String> =
                                        arg.choices.iter().map(|c| repr_str(c)).collect();
                                    return Err(self.error(&format!(
                                        "argument {}: invalid choice: {} (choose from {})",
                                        arg.name(),
                                        repr_str(&value),
                                        choices.join(", ")
                                    )));
                                }
                                parsed.values.insert(arg.dest.clone(), value);
                            }
                        }
                    }
                    i = next;
                }
            }
        }
        let missing: Vec<String> = self
            .args
            .iter()
            .filter(|a| a.required && !parsed.values.contains_key(&a.dest))
            .map(Arg::name)
            .collect();
        if !missing.is_empty() {
            return Err(self.error(&format!(
                "the following arguments are required: {}",
                missing.join(", ")
            )));
        }
        if !extras.is_empty() {
            return Err(self.error(&format!("unrecognized arguments: {}", extras.join(" "))));
        }
        Ok(parsed)
    }

    /// `HelpFormatter` width: the terminal's columns (or `$COLUMNS`, or 80) minus 2.
    fn width() -> usize {
        let columns = std::env::var("COLUMNS")
            .ok()
            .and_then(|c| c.trim().parse::<i64>().ok())
            .filter(|c| *c > 0)
            .map(|c| c as usize)
            .or_else(|| {
                terminal_size::terminal_size_of(std::io::stdout())
                    .map(|(w, _)| w.0 as usize)
                    .filter(|w| *w > 0)
            })
            .unwrap_or(80);
        columns.saturating_sub(2)
    }

    fn usage_body(&self, width: usize) -> String {
        let prefix = "usage: ";
        let parts: Vec<String> = self.args.iter().map(Arg::usage).collect();
        let usage = format!("{} {}", self.prog, parts.join(" "));
        let text_width = width;
        if prefix.len() + clen(&usage) <= text_width {
            return usage;
        }
        let get_lines = |parts: &[String], indent: &str, prefix: Option<&str>| -> Vec<String> {
            let mut lines = Vec::new();
            let mut line: Vec<String> = Vec::new();
            let indent_len = clen(indent);
            let mut line_len = match prefix {
                Some(p) => clen(p) as isize - 1,
                None => indent_len as isize - 1,
            };
            for part in parts {
                if line_len + 1 + clen(part) as isize > text_width as isize && !line.is_empty() {
                    lines.push(format!("{indent}{}", line.join(" ")));
                    line.clear();
                    line_len = indent_len as isize - 1;
                }
                line.push(part.clone());
                line_len += clen(part) as isize + 1;
            }
            if !line.is_empty() {
                lines.push(format!("{indent}{}", line.join(" ")));
            }
            if prefix.is_some() && !lines.is_empty() {
                lines[0] = lines[0][indent.len()..].to_string();
            }
            lines
        };
        let prog_len = clen(&self.prog);
        let lines = if (prefix.len() + prog_len) as f64 <= 0.75 * text_width as f64 {
            let indent = " ".repeat(prefix.len() + prog_len + 1);
            let mut all = vec![self.prog.clone()];
            all.extend(parts);
            get_lines(&all, &indent, Some(prefix))
        } else {
            let indent = " ".repeat(prefix.len());
            let lines = get_lines(&parts, &indent, None);
            let mut out = vec![self.prog.clone()];
            out.extend(lines);
            out
        };
        lines.join("\n")
    }

    /// `parser.format_usage()`.
    pub fn format_usage(&self) -> String {
        format!("usage: {}\n", self.usage_body(Self::width()))
    }

    /// `parser.format_help()`.
    pub fn format_help(&self) -> String {
        let width = Self::width();
        let mut help = format!("usage: {}\n\n", self.usage_body(width));
        if let Some(desc) = &self.description {
            let text = collapse_ws(desc);
            let text_width = width.max(11);
            help.push_str(&textwrap::fill(&text, text_width, ""));
            help.push_str("\n\n");
        }
        let indent = 2usize;
        let max_help_position = 24usize.min(width.saturating_sub(20).max(4));
        let action_max = self
            .args
            .iter()
            .map(|a| clen(&a.invocation()) + indent)
            .max()
            .unwrap_or(0);
        let help_position = (action_max + 2).min(max_help_position);
        let help_width = width.saturating_sub(help_position).max(11);
        let action_width = help_position.saturating_sub(indent + 2);
        help.push_str("options:\n");
        for a in &self.args {
            let inv = a.invocation();
            let lines = textwrap::wrap(&collapse_ws(&a.help), help_width);
            if a.help.is_empty() {
                help.push_str(&format!("{}{inv}\n", " ".repeat(indent)));
            } else if clen(&inv) <= action_width {
                let pad = action_width - clen(&inv);
                help.push_str(&format!("{}{inv}{}  ", " ".repeat(indent), " ".repeat(pad)));
                if let Some(first) = lines.first() {
                    help.push_str(first);
                }
                help.push('\n');
            } else {
                help.push_str(&format!("{}{inv}\n", " ".repeat(indent)));
                if let Some(first) = lines.first() {
                    help.push_str(&format!("{}{first}\n", " ".repeat(help_position)));
                }
            }
            for line in lines.iter().skip(1) {
                help.push_str(&format!("{}{line}\n", " ".repeat(help_position)));
            }
        }
        let help = help.trim_matches('\n');
        format!("{help}\n")
    }
}

fn clen(s: &str) -> usize {
    s.chars().count()
}

/// `re.sub(r'\s+', ' ', text, flags=re.ASCII).strip()`.
fn collapse_ws(text: &str) -> String {
    let mut out = String::new();
    let mut in_ws = false;
    for c in text.chars() {
        if matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{b}' | '\u{c}') {
            in_ws = true;
        } else {
            if in_ws && !out.is_empty() {
                out.push(' ');
            }
            in_ws = false;
            out.push(c);
        }
    }
    out.trim().to_string()
}

/// `^-\d+$|^-\d*\.\d+$`.
fn is_negative_number(s: &str) -> bool {
    let Some(rest) = s.strip_prefix('-') else {
        return false;
    };
    let digits = |t: &str| !t.is_empty() && t.chars().all(|c| c.is_ascii_digit());
    let rest = rest.strip_suffix('\n').unwrap_or(rest);
    digits(rest)
        || rest
            .split_once('.')
            .is_some_and(|(a, b)| (a.is_empty() || digits(a)) && digits(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parser() -> Parser {
        Parser::new("prog.py", "Does things")
            .option("--dir", "Directory")
            .flag("--json", "JSON")
            .flag("--jsonl", "Lines")
    }

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn prefixes_values_and_errors() {
        let p = parser();
        let r = p.parse(&args(&["--di=x", "--jsonl"])).unwrap();
        assert_eq!(r.value("dir"), Some("x"));
        assert!(r.flag("jsonl") && !r.flag("json"));
        assert_eq!(
            p.parse(&args(&["--dir", "-1"])).unwrap().value("dir"),
            Some("-1")
        );
        let Err(Exit::Error(e)) = p.parse(&args(&["--js"])) else {
            panic!()
        };
        assert!(e.ends_with("error: ambiguous option: --js could match --json, --jsonl\n"));
        let Err(Exit::Error(e)) = p.parse(&args(&["--dir"])) else {
            panic!()
        };
        assert!(e.ends_with("error: argument --dir: expected one argument\n"));
        let Err(Exit::Error(e)) = p.parse(&args(&["x", "--json=1"])) else {
            panic!()
        };
        assert!(e.ends_with("error: argument --json: ignored explicit argument '1'\n"));
    }
}
