//! `fnmatch.fnmatch` and the one-directory `glob.glob(os.path.join(dir, pattern))`: shell
//! patterns translated as CPython's `fnmatch.translate` does (`*`, `?`, `[seq]`, `[!seq]`,
//! ranges with empty ones removed), compared after `os.path.normcase` (case-insensitive with
//! `\` separators on Windows).

use regex::Regex;

use super::path::join;
use super::unicode::lower;
use super::{Crash, Value};

/// `os.path.normcase(s)`.
fn normcase(s: &str) -> String {
    if cfg!(windows) {
        lower(&s.replace('/', "\\"))
    } else {
        s.to_string()
    }
}

fn hex(c: char) -> String {
    format!("\\x{{{:x}}}", c as u32)
}

/// The class `[...]` whose text sits between `[` and `]` (CPython's `stuff` handling).
fn class(stuff: &[char]) -> String {
    let negated = stuff.first() == Some(&'!');
    let body = if negated { &stuff[1..] } else { stuff };
    let mut items: Vec<String> = Vec::new();
    if body.contains(&'-') {
        // Chunks split at the hyphens that make ranges.
        let mut chunks: Vec<Vec<char>> = Vec::new();
        let all = stuff;
        let mut start = 0;
        // `k = i+2 if pat[i] == '!' else i+1`, relative to the class text.
        let mut search = if negated { 2 } else { 1 };
        while let Some(p) = (search..all.len()).find(|&p| all[p] == '-') {
            chunks.push(all[start..p].to_vec());
            start = p + 1;
            search = p + 3;
        }
        let last = all[start.min(all.len())..].to_vec();
        if last.is_empty() {
            if let Some(c) = chunks.last_mut() {
                c.push('-');
            }
        } else {
            chunks.push(last);
        }
        for k in (1..chunks.len()).rev() {
            let (prev, next) = (chunks[k - 1].last().copied(), chunks[k].first().copied());
            if let (Some(a), Some(b)) = (prev, next)
                && a > b
            {
                let mut merged = chunks[k - 1][..chunks[k - 1].len() - 1].to_vec();
                merged.extend_from_slice(&chunks[k][1..]);
                chunks[k - 1] = merged;
                chunks.remove(k);
            }
        }
        if negated && let Some(first) = chunks.first_mut() {
            first.remove(0);
        }
        // Literal characters inside each chunk; a range between consecutive chunks.
        let n = chunks.len();
        for (k, chunk) in chunks.iter().enumerate() {
            let lo = usize::from(k > 0);
            let hi = if k + 1 < n {
                chunk.len().saturating_sub(1)
            } else {
                chunk.len()
            };
            for &c in chunk.get(lo..hi.max(lo)).unwrap_or(&[]) {
                items.push(hex(c));
            }
            if k + 1 < n {
                let (Some(&a), Some(&b)) = (chunk.last(), chunks[k + 1].first()) else {
                    continue;
                };
                items.push(format!("{}-{}", hex(a), hex(b)));
            }
        }
    } else {
        items.extend(body.iter().map(|&c| hex(c)));
    }
    match (items.is_empty(), negated) {
        (true, false) => "(?:\\b\\B)".to_string(),
        (true, true) => "(?s:.)".to_string(),
        (false, false) => format!("[{}]", items.concat()),
        (false, true) => format!("[^{}]", items.concat()),
    }
}

/// `fnmatch.translate(pat)` as a Rust regex.
fn translate(pat: &str) -> String {
    let chars: Vec<char> = pat.chars().collect();
    let n = chars.len();
    let mut res = String::from("\\A(?s:");
    let mut last_star = false;
    let mut i = 0;
    while i < n {
        let c = chars[i];
        i += 1;
        match c {
            '*' => {
                if !last_star {
                    res.push_str(".*");
                }
                last_star = true;
                continue;
            }
            '?' => res.push('.'),
            '[' => {
                let mut j = i;
                if j < n && chars[j] == '!' {
                    j += 1;
                }
                if j < n && chars[j] == ']' {
                    j += 1;
                }
                while j < n && chars[j] != ']' {
                    j += 1;
                }
                if j >= n {
                    res.push_str("\\[");
                } else {
                    res.push_str(&class(&chars[i..j]));
                    i = j + 1;
                }
            }
            c => res.push_str(&regex::escape(&c.to_string())),
        }
        last_star = false;
    }
    res.push_str(")\\z");
    res
}

/// `fnmatch.fnmatchcase(name, pat)`.
pub fn fnmatchcase(name: &str, pat: &str) -> bool {
    Regex::new(&translate(pat)).is_ok_and(|re| re.is_match(name))
}

/// `fnmatch.fnmatch(name, pat)` for a pattern of any type.
pub fn fnmatch(name: &str, pat: &Value) -> Result<bool, Crash> {
    match pat {
        Value::Str(p) => Ok(fnmatchcase(&normcase(name), &normcase(p))),
        Value::Bytes(_) => Err(Crash::new(
            "TypeError: cannot use a bytes pattern on a string-like object",
        )),
        other => Err(Crash::new(format!(
            "TypeError: expected str, bytes or os.PathLike object, not {}",
            other.tp_name()
        ))),
    }
}

/// `fnmatch.fnmatch(name, pat)` for a name of any type: `normcase(name)` fails first.
pub fn fnmatch_name(name: &Value, pat: &str) -> Result<bool, Crash> {
    match name {
        Value::Str(n) => Ok(fnmatchcase(&normcase(n), &normcase(pat))),
        Value::Bytes(_) => Err(Crash::new(
            "TypeError: cannot use a str pattern on a bytes-like object",
        )),
        other => Err(Crash::new(format!(
            "TypeError: expected str, bytes or os.PathLike object, not {}",
            other.tp_name()
        ))),
    }
}

/// `glob.glob(os.path.join(dir, pattern))` for a pattern without a directory part: the
/// matching entries of `dir`, hidden ones left out unless the pattern starts with `.`.
pub fn glob_in(dir: &str, pattern: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let hidden = pattern.starts_with('.');
    let pat = normcase(pattern);
    entries
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| hidden || !name.starts_with('.'))
        .filter(|name| fnmatchcase(&normcase(name), &pat))
        .map(|name| join(dir, &name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_like_python() {
        let cases = [
            ("main", "main", true),
            ("release/0.0", "release/*", true),
            ("release/0.0", "release/?.?", true),
            ("release/0.0", "rel*", true),
            ("release/0.0", "*0", true),
            ("dev", "d[a-f]v", true),
            ("dev", "d[!e]v", false),
            ("dev", "d[!a]v", true),
            ("d-v", "d[a-]v", true),
            ("dev", "d[z-a]v", false),
            ("d!v", "d[!z-a]v", true),
            ("dzv", "d[!z-a]v", true),
            ("d[v", "d[v", true),
            ("d]v", "d[]]v", true),
            ("dav", "d[a-c-e]v", true),
            ("d-v", "d[a-c-e]v", true),
            ("ddv", "d[a-c-e]v", false),
            ("dev", "d[a-c-e]v", true),
            ("d^v", "d[^a]v", true),
            ("d.v", "d.v", true),
            ("dxv", "d.v", false),
            ("a\nb", "a?b", true),
            ("ab", "a**b", true),
        ];
        for (name, pat, want) in cases {
            assert_eq!(fnmatchcase(name, pat), want, "{name} {pat}");
        }
    }
}
