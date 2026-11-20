//! Read-only GRACE 5 artifact parsing and Tauri commands.
//!
//! The grace5 kernel remains authoritative; this module only reads `.grace5/`
//! and never writes to it.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
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
    let (dir, in_closed_dir) = change_dir(&project, &id)?;
    let relative = if in_closed_dir {
        format!(".grace5/changes/closed/{id}")
    } else {
        format!(".grace5/changes/{id}")
    };
    let load = state::load_change(&id, &dir, in_closed_dir);
    let notes_md = fs::read_text_capped(&dir.join("notes.md"));
    let latest_results = latest_results(load.contract.as_ref(), &load.records.runs);
    // The change directory as an absolute path (the frontend resolves logs and
    // artifacts against it); forward slashes match the relative path fields.
    // `strip_verbatim_prefix` drops the Windows `\\?\` form `canonicalize` adds.
    let absolute_dir = project
        .canonicalize()
        .map_err(|error| format!("Cannot resolve {path}: {error}"))?
        .join(&relative);
    let absolute_dir = crate::strip_verbatim_prefix(&absolute_dir.to_string_lossy());
    let absolute_dir = absolute_dir.replace('\\', "/");
    Ok(GraceChangeDetail {
        summary: load.summary,
        dir: absolute_dir,
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

/// Opens one file inside the project's `.grace5/` with the OS default app.
#[command]
pub fn grace_open_file(path: String, rel_path: String) -> Result<(), String> {
    let resolved = fs::resolve_grace_file(Path::new(&path), &rel_path)?;
    fs::open_with_default_app(&resolved)
}

/// Whether the project's `.grace5` tree changed since the previous call.
#[command]
pub async fn grace_check_changed(path: String) -> Result<bool, String> {
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
        .filter_map(|check| state::latest_result(&check.id, runs).cloned())
        .collect()
}

/// Resolves a change id to its directory, refusing anything that is not one
/// normal path component inside `changes/` or `changes/closed/`.
fn change_dir(project: &Path, id: &str) -> Result<(PathBuf, bool), String> {
    validate_change_id(id)?;
    let grace_dir = project.join(".grace5");
    let canonical_grace = grace_dir
        .canonicalize()
        .map_err(|error| format!("Cannot resolve .grace5: {error}"))?;
    let changes = grace_dir.join("changes");
    let canonical_changes = changes
        .canonicalize()
        .map_err(|error| format!("Cannot resolve .grace5/changes: {error}"))?;
    let closed = changes.join("closed");
    let canonical_closed = closed.canonicalize().ok();
    for (in_closed_dir, candidate, expected_parent) in [
        (false, changes.join(id), Some(canonical_changes)),
        (true, closed.join(id), canonical_closed),
    ] {
        let Some(expected_parent) = expected_parent else {
            continue;
        };
        if !candidate.is_dir() {
            continue;
        }
        let Ok(resolved) = candidate.canonicalize() else {
            continue;
        };
        // The change must live inside this project's .grace5 (a symlinked
        // changes/ pointing elsewhere is refused) and directly in its parent.
        if !resolved.starts_with(&canonical_grace)
            || resolved.parent() != Some(expected_parent.as_path())
        {
            continue;
        }
        return Ok((resolved, in_closed_dir));
    }
    Err(format!("Unknown GRACE change: {id}"))
}

fn validate_change_id(id: &str) -> Result<(), String> {
    let invalid = || format!("Invalid GRACE change id: {id}");
    if id.is_empty() || id.contains('/') || id.contains('\\') || id.contains('\0') {
        return Err(invalid());
    }
    let mut components = Path::new(id).components();
    let single_normal = match components.next() {
        Some(Component::Normal(name)) => {
            name == std::ffi::OsStr::new(id) && components.next().is_none()
        }
        _ => false,
    };
    if !single_normal {
        return Err(invalid());
    }
    Ok(())
}
