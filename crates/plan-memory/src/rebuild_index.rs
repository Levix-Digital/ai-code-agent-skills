//! `rebuild_index.py`: drops and rebuilds the local search index of the project's plans
//! from docs/plans. The index is disposable: the plan files are the source of truth.

use kernel::out::println;
use kernel::py::Crash;
use kernel::py::argparse::Parser;

use crate::index::sync_index;
use crate::store::PlanStore;

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "rebuild_index.py",
        "Rebuild the local plan search index from docs/plans",
    )
    .option("--project", "Project name (default: from the repository)")
    .option("--dir", "Project directory (default: current)");
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let store = PlanStore::new(args.value("dir"), args.value("project"))?;
    sync_index(&store, None, true, false)?;
    println(&format!("✅ Index rebuilt for '{}'.", store.project));
    Ok(0)
}
