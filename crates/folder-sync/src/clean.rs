//! `clean.py`: removes a folder's `.folder_brain` index.

use std::path::Path;

use kernel::out::println;
use kernel::py::argparse::Parser;
use kernel::py::path::{basename, exists, join, realpath};
use kernel::py::{io_crash, repr_str};

pub fn main(argv: &[String]) -> u8 {
    let parser = Parser::new("clean.py", "Limpeza do Folder Brain")
        .option("--dir", "Target directory (default: current)");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return exit.report(),
    };
    clean_brain(args.value("dir").unwrap_or("."));
    0
}

fn clean_brain(target_dir: &str) {
    let root = realpath(target_dir);
    let name = basename(Path::new(&root));
    let brain = join(&root, ".folder_brain");

    if !exists(&brain) {
        println(&format!("ℹ️ No `.folder_brain` found in '{name}'."));
        return;
    }
    match rmtree(&brain) {
        Ok(()) => println(&format!("🧹 `.folder_brain` index removed from '{name}'.")),
        Err(e) => println(&format!("❌ Error removing `.folder_brain`: {e}")),
    }
}

/// `shutil.rmtree(path)`: the text of the error it raises, if any.
fn rmtree(path: &str) -> Result<(), String> {
    let oserror = |e: &std::io::Error, p: &str| {
        let text = io_crash(e, Path::new(p)).0;
        match text.split_once(": ") {
            Some((_, rest)) => rest.to_string(),
            None => text,
        }
    };
    let meta = std::fs::symlink_metadata(path).map_err(|e| oserror(&e, path))?;
    // Python takes the descriptor-based walk on Unix, which names the `PosixPath` it was
    // given, and the plain walk on Windows.
    let posix_path = format!("PosixPath({})", repr_str(path));
    if meta.file_type().is_symlink() {
        return Err(if cfg!(windows) {
            "Cannot call rmtree on a symbolic link".to_string()
        } else {
            format!("[Errno None] None: {posix_path}")
        });
    }
    if !meta.is_dir() {
        return Err(if cfg!(windows) {
            format!(
                "[WinError 267] The directory name is invalid: {}",
                repr_str(path)
            )
        } else {
            format!("[Errno 20] Not a directory: {posix_path}")
        });
    }
    std::fs::remove_dir_all(path).map_err(|e| oserror(&e, path))
}
