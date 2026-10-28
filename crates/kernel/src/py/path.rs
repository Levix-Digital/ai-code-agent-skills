//! `os.path` as the scripts use it: `abspath` (lexical on POSIX, `GetFullPathNameW` on
//! Windows), `basename`, `dirname`, `join`, `relpath`, and `os.makedirs`. Paths are the
//! strings Python builds, with its separators: `/` on POSIX, `\` on Windows.

use std::path::{Path, PathBuf};

use super::{Crash, io_crash, repr_str};

#[cfg(windows)]
pub const SEP: char = '\\';
#[cfg(not(windows))]
pub const SEP: char = '/';

fn is_sep(c: char) -> bool {
    c == '/' || (cfg!(windows) && c == '\\')
}

/// `ntpath.splitdrive`: `C:` or the `\\server\share` of a UNC path.
fn splitdrive(p: &str) -> (&str, &str) {
    if !cfg!(windows) {
        return ("", p);
    }
    let b: Vec<char> = p.chars().collect();
    if b.len() >= 2 && is_sep(b[0]) && is_sep(b[1]) {
        // \\server\share\rest
        let rest = &p[2..];
        let Some(i) = rest.find(is_sep) else {
            return (p, "");
        };
        let after = &rest[i + 1..];
        let j = after.find(is_sep).map_or(after.len(), |j| j);
        let end = 2 + i + 1 + j;
        return (&p[..end], &p[end..]);
    }
    if b.len() >= 2 && b[1] == ':' {
        let end = b[0].len_utf8() + 1;
        return (&p[..end], &p[end..]);
    }
    ("", p)
}

/// `os.path.join(a, b)`.
pub fn join(a: &str, b: &str) -> String {
    if !cfg!(windows) {
        if b.starts_with('/') {
            return b.to_string();
        }
        if a.is_empty() || a.ends_with('/') {
            return format!("{a}{b}");
        }
        return format!("{a}/{b}");
    }
    let (mut drive, mut path) = {
        let (d, p) = splitdrive(a);
        (d.to_string(), p.to_string())
    };
    let (p_drive, p_path) = splitdrive(b);
    if p_path.starts_with(is_sep) {
        if !p_drive.is_empty() || drive.is_empty() {
            drive = p_drive.to_string();
        }
        path = p_path.to_string();
    } else if !p_drive.is_empty() && p_drive != drive {
        if p_drive.to_lowercase() != drive.to_lowercase() {
            return b.to_string();
        }
        drive = p_drive.to_string();
        if !path.is_empty() && !path.ends_with(is_sep) {
            path.push(SEP);
        }
        path.push_str(p_path);
    } else {
        if !path.is_empty() && !path.ends_with(is_sep) {
            path.push(SEP);
        }
        path.push_str(p_path);
    }
    if !path.is_empty() && !path.starts_with(is_sep) && !drive.is_empty() && !drive.ends_with(':') {
        return format!("{drive}{SEP}{path}");
    }
    format!("{drive}{path}")
}

/// `os.path.dirname(p)`.
pub fn dirname(p: &str) -> String {
    let (drive, rest) = splitdrive(p);
    let i = rest.rfind(is_sep).map_or(0, |i| i + 1);
    let head = &rest[..i];
    let stripped = head.trim_end_matches(is_sep);
    let head = if stripped.is_empty() { head } else { stripped };
    format!("{drive}{head}")
}

/// `os.path.relpath(path, start)`.
pub fn relpath(path: &str, start: &str) -> Result<String, Crash> {
    if path.is_empty() {
        return Err(Crash::new("ValueError: no path specified"));
    }
    let start_abs = abspath(start).to_string_lossy().into_owned();
    let path_abs = abspath(path).to_string_lossy().into_owned();
    let (start_drive, start_rest) = splitdrive(&start_abs);
    let (path_drive, path_rest) = splitdrive(&path_abs);
    let fold = |s: &str| {
        if cfg!(windows) {
            s.to_lowercase().replace('/', "\\")
        } else {
            s.to_string()
        }
    };
    if fold(start_drive) != fold(path_drive) {
        return Err(Crash::new(format!(
            "ValueError: path is on mount {}, start on mount {}",
            repr_str(path_drive),
            repr_str(start_drive)
        )));
    }
    let start_list: Vec<&str> = start_rest.split(is_sep).filter(|x| !x.is_empty()).collect();
    let path_list: Vec<&str> = path_rest.split(is_sep).filter(|x| !x.is_empty()).collect();
    let common = start_list
        .iter()
        .zip(&path_list)
        .take_while(|(a, b)| fold(a) == fold(b))
        .count();
    let mut rel: Vec<&str> = vec![".."; start_list.len() - common];
    rel.extend(&path_list[common..]);
    if rel.is_empty() {
        return Ok(".".into());
    }
    Ok(rel
        .iter()
        .skip(1)
        .fold(rel[0].to_string(), |acc, part| join(&acc, part)))
}

pub fn isfile(p: &str) -> bool {
    std::fs::metadata(p).is_ok_and(|m| m.is_file())
}

pub fn isdir(p: &str) -> bool {
    std::fs::metadata(p).is_ok_and(|m| m.is_dir())
}

pub fn exists(p: &str) -> bool {
    std::fs::metadata(p).is_ok()
}

/// `os.path.split(p)`.
fn split(p: &str) -> (String, String) {
    let (drive, rest) = splitdrive(p);
    let i = rest.rfind(is_sep).map_or(0, |i| i + 1);
    let (head, tail) = rest.split_at(i);
    let stripped = head.trim_end_matches(is_sep);
    let head = if stripped.is_empty() { head } else { stripped };
    (format!("{drive}{head}"), tail.to_string())
}

/// `os.makedirs(name, exist_ok=True)`: the error names the directory that could not be made.
pub fn makedirs(name: &str) -> Result<(), Crash> {
    let (mut head, mut tail) = split(name);
    if tail.is_empty() {
        (head, tail) = split(&head);
    }
    if !head.is_empty() && !tail.is_empty() && !exists(&head) {
        match makedirs(&head) {
            Err(c) if !c.0.starts_with("FileExistsError") => return Err(c),
            _ => {}
        }
        if tail == "." {
            return Ok(());
        }
    }
    match std::fs::create_dir(name) {
        Ok(()) => Ok(()),
        Err(_) if isdir(name) => Ok(()),
        Err(e) => Err(io_crash(&e, Path::new(name))),
    }
}

/// `os.path.abspath(p)`; an empty `p` is the current directory.
pub fn abspath(p: &str) -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_default();
    if cfg!(windows) {
        let target = if p.is_empty() { cwd } else { PathBuf::from(p) };
        std::path::absolute(&target).unwrap_or(target)
    } else {
        let joined = if p.starts_with('/') {
            p.to_string()
        } else if p.is_empty() {
            cwd.to_string_lossy().into_owned()
        } else {
            format!("{}/{p}", cwd.to_string_lossy())
        };
        PathBuf::from(normpath_posix(&joined))
    }
}

/// `posixpath.normpath`.
pub fn normpath_posix(p: &str) -> String {
    if p.is_empty() {
        return ".".into();
    }
    let initial = if p.starts_with("//") && !p.starts_with("///") {
        2
    } else if p.starts_with('/') {
        1
    } else {
        0
    };
    let mut parts: Vec<&str> = Vec::new();
    for comp in p.split('/') {
        if comp.is_empty() || comp == "." {
            continue;
        }
        if comp != ".." || (initial == 0 && parts.is_empty()) || parts.last() == Some(&"..") {
            parts.push(comp);
        } else if !parts.is_empty() {
            parts.pop();
        }
    }
    let out = format!("{}{}", "/".repeat(initial), parts.join("/"));
    if out.is_empty() { ".".into() } else { out }
}

/// `os.path.basename(p)`.
pub fn basename(p: &Path) -> String {
    let s = p.to_string_lossy();
    let cut = if cfg!(windows) {
        s.rfind(['/', '\\', ':']).map(|i| i + 1)
    } else {
        s.rfind('/').map(|i| i + 1)
    };
    s[cut.unwrap_or(0)..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normpath_matches_python() {
        assert_eq!(normpath_posix("/a/./b/../c/"), "/a/c");
        assert_eq!(normpath_posix("//a//b"), "//a/b");
        assert_eq!(normpath_posix("///a"), "/a");
        assert_eq!(normpath_posix("/.."), "/");
        assert_eq!(normpath_posix("../x"), "../x");
    }

    #[test]
    fn basename_of_root_is_empty() {
        assert_eq!(basename(Path::new("/")), "");
        assert_eq!(basename(Path::new("/a/proj")), "proj");
    }
}
