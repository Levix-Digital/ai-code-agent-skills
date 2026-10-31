//! The `dev-cycle` binary: one subcommand per translated script, `dev-cycle <skill> <script> [args]`.
//! Each script is added here when it is translated (IMP-005); until then the Python script runs.

use std::process::ExitCode;

const USAGE: &str = "usage: dev-cycle <skill> <script> [arguments]";

type Command = fn(&[String]) -> u8;

/// Translated scripts: (skill folder, script name without `.py`, entry point).
const COMMANDS: &[(&str, &str, Command)] = &[
    (
        "architecture-docs",
        "build_index",
        architecture_docs::build_index::main,
    ),
    (
        "architecture-docs",
        "check_docs",
        architecture_docs::check_docs::main,
    ),
    (
        "architecture-docs",
        "extract_adrs",
        architecture_docs::extract_adrs::main,
    ),
    (
        "architecture-docs",
        "new_adr",
        architecture_docs::new_adr::main,
    ),
    (
        "architecture-docs",
        "new_subsystem",
        architecture_docs::new_subsystem::main,
    ),
    ("dev-cycle-protocol", "_no_ai", kernel::no_ai::main),
    (
        "dev-cycle-protocol",
        "constitution",
        dev_cycle_protocol::constitution::main,
    ),
    (
        "dev-cycle-protocol",
        "inspect_project",
        dev_cycle_protocol::inspect_project::main,
    ),
    (
        "dev-cycle-protocol",
        "onboard",
        dev_cycle_protocol::onboard::main,
    ),
    (
        "dev-cycle-protocol",
        "pr_sync",
        dev_cycle_protocol::pr_sync::main,
    ),
    (
        "dev-cycle-protocol",
        "project_config",
        kernel::project_config::main,
    ),
    (
        "dev-cycle-protocol",
        "speckit_setup",
        dev_cycle_protocol::speckit_setup::main,
    ),
    (
        "dev-cycle-protocol",
        "verify_gate",
        dev_cycle_protocol::verify_gate::main,
    ),
    (
        "gedo-feedback",
        "gedo_feedback",
        gedo_feedback::gedo_feedback::main,
    ),
    (
        "product-inception",
        "check_product",
        product_inception::check_product::main,
    ),
    (
        "software-design",
        "check_design",
        software_design::check_design::main,
    ),
    ("user-docs", "check_readme", user_docs::check_readme::main),
    (
        "quality-assurance",
        "manual_tests",
        quality_assurance::manual_tests::main,
    ),
    (
        "devops-release",
        "check_ci_policy",
        devops_release::check_ci_policy::main,
    ),
    (
        "devops-release",
        "release_status",
        devops_release::release_status::main,
    ),
    (
        "devops-release",
        "branching",
        devops_release::branching::main,
    ),
    ("plan-memory", "export_plan", plan_memory::export_plan::main),
    ("plan-memory", "get_plan", plan_memory::get_plan::main),
    ("plan-memory", "list_plans", plan_memory::list_plans::main),
    (
        "plan-memory",
        "rebuild_index",
        plan_memory::rebuild_index::main,
    ),
    ("plan-memory", "roadmap", plan_memory::roadmap::main),
    ("plan-memory", "save_plan", plan_memory::save_plan::main),
    (
        "plan-memory",
        "update_plan_status",
        plan_memory::update_plan_status::main,
    ),
    (
        "plan-memory",
        "search_plans",
        plan_memory::search_plans::main,
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
