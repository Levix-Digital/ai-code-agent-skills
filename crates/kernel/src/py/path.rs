//! `os.path` as the scripts use it: `abspath` (lexical on POSIX, `GetFullPathNameW` on
//! Windows) and `basename`.

use std::path::{Path, PathBuf};

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
