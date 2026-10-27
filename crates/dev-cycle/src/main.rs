//! The `dev-cycle` binary: one subcommand per translated script, `dev-cycle <skill> <script> [args]`.
//! Each script is added here when it is translated (IMP-005); until then the Python script runs.

use std::process::ExitCode;

const USAGE: &str = "usage: dev-cycle <skill> <script> [arguments]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") => {
            println!("dev-cycle {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("-h" | "--help") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("{USAGE}\ndev-cycle: no command '{other}' yet");
            ExitCode::from(2)
        }
        None => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
