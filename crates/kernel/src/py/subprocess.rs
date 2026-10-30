//! `subprocess.run` and `shutil.which` as the scripts called them: the exception Python raised
//! when a program could not be started, and the output decoded as text.

use std::path::Path;
use std::process::Command;

use super::path::{abspath, join};
use super::{Crash, repr_str, universal_newlines};

/// The exception `subprocess.run` raises when the program cannot be started: whether it is
/// `FileNotFoundError`, and its text (the message alone for it, the last line otherwise).
pub fn spawn_error(e: &std::io::Error, program: &str) -> (bool, String) {
    let quoted = repr_str(program);
    match e.kind() {
        std::io::ErrorKind::NotFound if cfg!(windows) => (
            true,
            "[WinError 2] The system cannot find the file specified".to_string(),
        ),
        std::io::ErrorKind::NotFound => (
            true,
            format!("[Errno 2] No such file or directory: {quoted}"),
        ),
        std::io::ErrorKind::PermissionDenied => (
            false,
            format!("PermissionError: [Errno 13] Permission denied: {quoted}"),
        ),
        _ => (false, format!("OSError: {e}")),
    }
}

/// The exit code `subprocess` reports: negative for a signal on POSIX.
fn returncode(status: std::process::ExitStatus) -> i32 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return -signal;
        }
    }
    status.code().unwrap_or(-1)
}

/// `subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, encoding="utf-8",
/// errors="replace")`: (code, stdout, stderr), or `Err(Ok(message))` for
/// `FileNotFoundError`.
pub fn run_replace(
    cmd: &[String],
    cwd: Option<&str>,
) -> Result<(i32, String, String), Result<String, Crash>> {
    let Some((program, args)) = cmd.split_first() else {
        return Err(Err(Crash::new("IndexError: list index out of range")));
    };
    let mut command = Command::new(program);
    command.args(args);
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    let out = match command.output() {
        Ok(o) => o,
        Err(e) => {
            let (not_found, msg) = spawn_error(&e, program);
            return Err(if not_found {
                Ok(msg)
            } else {
                Err(Crash::new(msg))
            });
        }
    };
    let text = |bytes: &[u8]| universal_newlines(&String::from_utf8_lossy(bytes));
    Ok((returncode(out.status), text(&out.stdout), text(&out.stderr)))
}

fn is_program(path: &str) -> bool {
    let p = Path::new(path);
    if !p.exists() || p.is_dir() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // `os.access(path, os.X_OK)`: root may run any file with an execute bit.
        let mode = p.metadata().map(|m| m.permissions().mode()).unwrap_or(0);
        mode & 0o111 != 0
    }
    #[cfg(not(unix))]
    true
}

/// `shutil.which(cmd)` for a name without a directory part.
pub fn which(cmd: &str) -> Option<String> {
    let path = match std::env::var("PATH") {
        Ok(p) => p,
        Err(_) if cfg!(windows) => String::new(),
        Err(_) => "/bin:/usr/bin".to_string(),
    };
    let sep = if cfg!(windows) { ';' } else { ':' };
    let mut dirs: Vec<String> = Vec::new();
    if cfg!(windows) {
        // The current directory comes first on Windows (NeedCurrentDirectoryForExePathW).
        if std::env::var_os("NoDefaultCurrentDirectoryInExePath").is_none() {
            dirs.push(".".to_string());
        }
    } else if path.is_empty() {
        return None;
    }
    dirs.extend(path.split(sep).map(str::to_string));
    let files: Vec<String> = if cfg!(windows) {
        let pathext = std::env::var("PATHEXT")
            .ok()
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD;.VBS;.JS;.WS;.MSC".to_string());
        let exts: Vec<&str> = pathext.split(';').filter(|e| !e.is_empty()).collect();
        let lower = cmd.to_lowercase();
        if exts.iter().any(|e| lower.ends_with(&e.to_lowercase())) {
            vec![cmd.to_string()]
        } else {
            exts.iter().map(|e| format!("{cmd}{e}")).collect()
        }
    } else {
        vec![cmd.to_string()]
    };
    let mut seen: Vec<String> = Vec::new();
    for dir in dirs {
        let key = if cfg!(windows) {
            dir.to_lowercase()
        } else {
            dir.clone()
        };
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        for file in &files {
            let name = join(&dir, file);
            if is_program(&name) {
                return Some(name);
            }
        }
    }
    None
}

/// `tempfile.gettempdir()`: the first of the usual places where a file can be created.
fn gettempdir() -> String {
    let mut candidates: Vec<String> = ["TMPDIR", "TEMP", "TMP"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .collect();
    if cfg!(windows) {
        if let Ok(home) = std::env::var("USERPROFILE") {
            candidates.push(join(&join(&join(&home, "AppData"), "Local"), "Temp"));
        }
        if let Ok(root) = std::env::var("SYSTEMROOT") {
            candidates.push(join(&root, "Temp"));
        }
        candidates.extend(["c:\\temp", "c:\\tmp", "\\temp", "\\tmp"].map(String::from));
    } else {
        candidates.extend(["/tmp", "/var/tmp", "/usr/tmp"].map(String::from));
    }
    candidates.push(".".to_string());
    for dir in candidates {
        let dir = if dir == "." {
            dir
        } else {
            abspath(&dir).to_string_lossy().into_owned()
        };
        let probe = join(&dir, &format!("tmp{}", random_name()));
        if std::fs::write(&probe, b"blat").is_ok() {
            let _ = std::fs::remove_file(&probe);
            return dir;
        }
    }
    ".".to_string()
}

/// Eight characters of `tempfile`'s alphabet, from the process's hash seed and the clock.
fn random_name() -> String {
    use std::hash::{BuildHasher, Hasher};
    const LETTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789_";
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    h.write_u32(std::process::id());
    let mut n = h.finish();
    (0..8)
        .map(|_| {
            let c = LETTERS[(n % LETTERS.len() as u64) as usize];
            n /= LETTERS.len() as u64;
            char::from(c)
        })
        .collect()
}

/// `tempfile.mkdtemp(prefix=prefix)`: a new private directory, by absolute path.
pub fn mkdtemp(prefix: &str) -> Result<String, Crash> {
    let dir = gettempdir();
    for _ in 0..100 {
        let path = join(&dir, &format!("{prefix}{}", random_name()));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&path) {
            Ok(()) => return Ok(abspath(&path).to_string_lossy().into_owned()),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(Crash::new(format!("OSError: {e}"))),
        }
    }
    Err(Crash::new(
        "FileExistsError: [Errno 17] No usable temporary directory name found",
    ))
}
