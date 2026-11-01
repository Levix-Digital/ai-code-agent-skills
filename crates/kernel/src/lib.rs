//! What every command shares: the Python behavior the scripts relied on (`py`), output, and the
//! project configuration loader that every skill reads.

pub mod md5;
pub mod no_ai;
pub mod out;
pub mod project_config;
pub mod py;
pub mod sha256;
pub mod skills;
