//! `parity run [name filter]`: compare every case (Rust against Python, or against the golden file
//! once the Python script is gone; Python against its golden file while both exist).
//! `parity record [name filter]`: write the golden files from the Python scripts.
//!
//! `PARITY_PYTHON` picks the Python executable; `PARITY_RUST` the `dev-cycle` binary
//! (default: `target/debug/dev-cycle`, built by `cargo build`).

use parity::{Case, Harness, Outcome, canonical, diff, fold_newlines};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mode, filter) = match args.as_slice() {
        [m] => (m.as_str(), None),
        [m, f] => (m.as_str(), Some(f.as_str())),
        _ => ("", None),
    };
    if mode != "run" && mode != "record" {
        eprintln!("usage: parity run|record [name filter]");
        return ExitCode::from(2);
    }
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let repo = canonical(&repo);
    let exe = if cfg!(windows) {
        "dev-cycle.exe"
    } else {
        "dev-cycle"
    };
    let harness = Harness {
        rust_bin: std::env::var_os("PARITY_RUST")
            .map(PathBuf::from)
            .unwrap_or_else(|| repo.join("target").join("debug").join(exe)),
        python: std::env::var("PARITY_PYTHON")
            .unwrap_or_else(|_| if cfg!(windows) { "python" } else { "python3" }.into()),
        work: std::env::temp_dir().join(format!("dev-cycle-parity-{}", std::process::id())),
        repo,
    };
    let cases = match harness.load_cases() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("❌ {e}");
            return ExitCode::from(2);
        }
    };
    let cases: Vec<Case> = cases
        .into_iter()
        .filter(|c| filter.is_none_or(|f| c.name.contains(f)))
        .collect();

    let mut failed = 0;
    for case in &cases {
        let result = if mode == "record" {
            record(&harness, case)
        } else {
            check(&harness, case)
        };
        match result {
            Ok(note) => println!("✅ {}: {note}", case.name),
            Err(problems) => {
                failed += 1;
                println!("❌ {}", case.name);
                for p in problems {
                    println!("   {p}");
                }
            }
        }
    }
    let _ = fs::remove_dir_all(&harness.work);
    println!("\n{} case(s), {failed} failed.", cases.len());
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn record(h: &Harness, case: &Case) -> Result<String, Vec<String>> {
    if !h.python_exists(case) {
        return Err(vec![format!(
            "{} no longer exists: its golden file is final",
            case.python
        )]);
    }
    let out = fold_newlines(&h.run_python(case).map_err(|e| vec![e])?, &case.seps);
    let path = h.golden_path(case);
    let json = serde_json::to_string_pretty(&out).map_err(|e| vec![e.to_string()])? + "\n";
    fs::create_dir_all(path.parent().unwrap_or(Path::new("."))).map_err(|e| vec![e.to_string()])?;
    fs::write(&path, json).map_err(|e| vec![e.to_string()])?;
    Ok("recorded".into())
}

fn check(h: &Harness, case: &Case) -> Result<String, Vec<String>> {
    let golden: Option<Outcome> = match fs::read_to_string(h.golden_path(case)) {
        Ok(t) => Some(serde_json::from_str(&t).map_err(|e| vec![format!("golden: {e}")])?),
        Err(_) => None,
    };
    let mut problems = Vec::new();
    let mut notes = Vec::new();
    let python = if h.python_exists(case) {
        Some(h.run_python(case).map_err(|e| vec![e])?)
    } else {
        None
    };
    if let (Some(py), Some(g)) = (&python, &golden) {
        problems.extend(
            diff(g, &fold_newlines(py, &case.seps))
                .into_iter()
                .map(|d| format!("python vs golden: {d}")),
        );
        notes.push("python = golden");
    }
    if case.rust.is_some() {
        let rs = h.run_rust(case).map_err(|e| vec![e])?;
        match (&python, &golden) {
            (Some(py), _) => {
                problems.extend(
                    diff(py, &rs)
                        .into_iter()
                        .map(|d| format!("rust vs python: {d}")),
                );
                notes.push("rust = python");
            }
            (None, Some(g)) => {
                problems.extend(
                    diff(g, &fold_newlines(&rs, &case.seps))
                        .into_iter()
                        .map(|d| format!("rust vs golden: {d}")),
                );
                notes.push("rust = golden");
            }
            (None, None) => {
                problems.push("no Python script and no golden file to compare with".into())
            }
        }
    }
    if python.is_none() && case.rust.is_none() {
        problems.push(format!(
            "{} does not exist and the case has no Rust command",
            case.python
        ));
    }
    if golden.is_none() {
        problems.push("no golden file: run `parity record`".into());
    }
    if problems.is_empty() {
        Ok(notes.join(", "))
    } else {
        Err(problems)
    }
}
