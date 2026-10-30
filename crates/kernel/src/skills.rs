//! Where the skills are installed: the scripts read their sibling skills' files (SKILL.md,
//! styles) from the folder that holds them.

use std::path::{Path, PathBuf};

/// The launchers set it to the skills folder they belong to.
pub const ENV: &str = "DEV_CYCLE_SKILLS_ROOT";

fn holds_skills(dir: &Path) -> bool {
    dir.join("dev-cycle-protocol").join("SKILL.md").is_file()
}

/// The skills folder: `DEV_CYCLE_SKILLS_ROOT`, else the checkout the binary was built in.
pub fn root() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(ENV).filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    let exe = std::env::current_exe().ok()?;
    exe.ancestors()
        .skip(1)
        .find(|d| holds_skills(d))
        .map(Path::to_path_buf)
}
