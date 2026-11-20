//! Read-only GRACE 5 artifact parsing.
//!
//! The grace5 kernel remains authoritative; this module only reads `.grace5/`
//! and never writes to it.

use std::path::Path;

use tauri::command;

use crate::grace::model::GraceListResult;

pub mod model;
pub mod parse;
pub mod state;

/// Lists the `.grace5` changes of a project (registered in Task 4).
#[command]
pub fn grace_list(path: String) -> Result<GraceListResult, String> {
    Ok(state::list(Path::new(&path)))
}
