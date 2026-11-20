//! Read-only GRACE 5 artifact parsing and Tauri commands.
//!
//! The grace5 kernel remains authoritative; this module only reads `.grace5/`
//! and never writes to it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

use tauri::command;

use crate::grace::model::{
    GraceChangeDetail, GraceContract, GraceFileContent, GraceListResult, GraceRun, GraceRunResult,
};

pub mod fs;
pub mod model;
pub mod parse;
pub mod state;

/// Fingerprint of the last `.grace5` scan, per project (the `bd_check_changed` pattern).
static GRACE_LAST_FINGERPRINT: LazyLock<Mutex<HashMap<String, (SystemTime, u64)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Lists the `.grace5` changes of a project.
#[command]
pub fn grace_list(path: String) -> Result<GraceListResult, String> {
    Ok(state::list(Path::new(&path)))
}

/// Full detail for one change, looked up in `changes/` then `changes/closed/`.
#[command]
pub fn grace_show(path: String, id: String) -> Result<GraceChangeDetail, String> {
    let project = PathBuf::from(&path);
    let changes = project.join(".grace5").join("changes");
    let open = changes.join(&id);
    let closed = changes.join("closed").join(&id);
    let (dir, in_closed_dir) = if open.is_dir() {
        (open, false)
    } else if closed.is_dir() {
        (closed, true)
    } else {
        return Err(format!("Unknown GRACE change: {id}"));
    };
    let relative = if in_closed_dir {
        format!(".grace5/changes/closed/{id}")
    } else {
        format!(".grace5/changes/{id}")
    };
    let load = state::load_change(&id, &dir, in_closed_dir);
    let notes_md = std::fs::read(dir.join("notes.md"))
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
    let latest_results = latest_results(load.contract.as_ref(), &load.records.runs);
    Ok(GraceChangeDetail {
        summary: load.summary,
        dir: relative,
        contract: load.contract,
        approvals: load.records.approvals,
        runs: load.records.runs,
        reviews: load.records.reviews,
        close: load.records.close,
        briefs: load.records.briefs,
        notes_md,
        log_dirs: load.records.log_dirs,
        defects: load.defects,
        latest_results,
    })
}

/// Reads one file inside the project's `.grace5/`.
#[command]
pub fn grace_read_file(path: String, rel_path: String) -> Result<GraceFileContent, String> {
    fs::read_grace_file(Path::new(&path), &rel_path)
}

/// Whether the project's `.grace5` tree changed since the previous call.
#[command]
pub fn grace_check_changed(path: String) -> Result<bool, String> {
    let current = fs::grace_fingerprint(Path::new(&path));
    let mut fingerprints = GRACE_LAST_FINGERPRINT
        .lock()
        .map_err(|error| format!("Lock error: {error}"))?;
    let previous = fingerprints.get(&path).copied();
    match (current, previous) {
        (Some(current), Some(previous)) => {
            let changed = current != previous;
            fingerprints.insert(path, current);
            Ok(changed)
        }
        (Some(current), None) => {
            fingerprints.insert(path, current);
            Ok(true)
        }
        (None, Some(_)) => {
            fingerprints.remove(&path);
            Ok(true)
        }
        (None, None) => Ok(false),
    }
}

/// Kernel `checkState`: the highest-numbered run containing a check decides it.
fn latest_results(contract: Option<&GraceContract>, runs: &[GraceRun]) -> Vec<GraceRunResult> {
    let Some(contract) = contract else {
        return Vec::new();
    };
    contract
        .checks
        .iter()
        .filter_map(|check| {
            runs.iter()
                .filter_map(|run| {
                    run.results
                        .iter()
                        .find(|result| result.check == check.id)
                        .map(|result| (run.n, result))
                })
                .max_by_key(|(n, _)| *n)
                .map(|(_, result)| result.clone())
        })
        .collect()
}
