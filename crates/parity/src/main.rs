//! `parity run [name filter]`: compare every case's command against its golden file, which was
//! recorded from the Python script it was translated from.
//! `parity record [name filter]`: write the golden files from the engine, for a case added or
//! changed on purpose (a behavior change the golden file has to follow).
//!
//! `PARITY_RUST` picks the `dev-cycle` binary (default: `target/debug/dev-cycle`, built by
//! `cargo build`).

use parity::{Case, Harness, Outcome, canonical, diff, fold_newlines};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Copied as `gh`, `glab` or `gedo` into a case's PATH: the fake CLI.
    let name = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()));
    if let Some(name) = name.filter(|n| ["gh", "glab", "gedo"].contains(&n.as_str())) {
        let code = parity::fake_cli(&name, &args);
        return ExitCode::from(u8::try_from(code).unwrap_or(1));
    }
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
    let out = fold_newlines(&h.run_rust(case).map_err(|e| vec![e])?, &case.seps);
    let path = h.golden_path(case);
    let json = serde_json::to_string_pretty(&out).map_err(|e| vec![e.to_string()])? + "\n";
    fs::create_dir_all(path.parent().unwrap_or(Path::new("."))).map_err(|e| vec![e.to_string()])?;
    fs::write(&path, json).map_err(|e| vec![e.to_string()])?;
    Ok("recorded".into())
}

fn check(h: &Harness, case: &Case) -> Result<String, Vec<String>> {
    let golden: Outcome = match fs::read_to_string(h.golden_path(case)) {
        Ok(t) => serde_json::from_str(&t).map_err(|e| vec![format!("golden: {e}")])?,
        Err(_) => return Err(vec!["no golden file: run `parity record`".into()]),
    };
    let rs = h.run_rust(case).map_err(|e| vec![e])?;
    let problems: Vec<String> = diff(&golden, &fold_newlines(&rs, &case.seps))
        .into_iter()
        .map(|d| format!("rust vs golden: {d}"))
        .collect();
    if problems.is_empty() {
        Ok("rust = golden".into())
    } else {
        Err(problems)
    }
}
