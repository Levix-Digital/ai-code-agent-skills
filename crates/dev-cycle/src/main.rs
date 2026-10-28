//! The `dev-cycle` binary: one subcommand per translated script, `dev-cycle <skill> <script> [args]`.
//! Each script is added here when it is translated (IMP-005); until then the Python script runs.

use std::process::ExitCode;

const USAGE: &str = "usage: dev-cycle <skill> <script> [arguments]";

type Command = fn(&[String]) -> u8;

/// Translated scripts: (skill folder, script name without `.py`, entry point).
const COMMANDS: &[(&str, &str, Command)] = &[
    (
        "architecture-docs",
        "new_subsystem",
        architecture_docs::new_subsystem::main,
    ),
    (
        "dev-cycle-protocol",
        "project_config",
        kernel::project_config::main,
    ),
];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") => {
            println!("dev-cycle {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("-h" | "--help") => {
            println!("{USAGE}");
            for (skill, script, _) in COMMANDS {
                println!("  {skill} {script}");
            }
            ExitCode::SUCCESS
        }
        Some(skill) => {
            let script = args.get(1).map(String::as_str).unwrap_or("");
            match COMMANDS
                .iter()
                .find(|(k, s, _)| *k == skill && *s == script)
            {
                Some((_, _, run)) => ExitCode::from(run(&args[2..])),
                None => {
                    eprintln!("{USAGE}\ndev-cycle: no command '{skill} {script}'");
                    ExitCode::from(2)
                }
            }
        }
        None => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
