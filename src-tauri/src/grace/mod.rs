//! Read-only GRACE 5 artifact parsing and Tauri commands.
//!
//! The grace5 kernel remains authoritative; this module only reads `.grace5/`
//! and never writes to it.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime};

use tauri::command;

use crate::grace::model::{
    GraceChangeDetail, GraceContract, GraceFileContent, GraceListResult, GraceRun, GraceRunResult,
};

pub mod fs;
pub mod model;
pub mod parse;
pub mod state;

/// Fingerprint of the last `.grace5` scan, per project (the `br_check_changed` pattern).
static GRACE_LAST_FINGERPRINT: LazyLock<Mutex<HashMap<String, (SystemTime, u64)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The fast check runs every second; the tree walk is not free on large
/// projects, so the walk result is memoised per project for this long. A change
/// inside the TTL is therefore observed one tick late at worst — the watcher
/// still covers the normal path.
const GRACE_FINGERPRINT_MEMO_TTL: Duration = Duration::from_millis(1000);
/// Extra wait for tests that must outlive the memo.
#[cfg(test)]
pub(crate) const GRACE_FINGERPRINT_MEMO_SLACK_MS: u64 = 150;

/// `Instant` and the fingerprint computed at that instant, per project path.
static GRACE_FINGERPRINT_MEMO: LazyLock<Mutex<HashMap<String, (Instant, Option<(SystemTime, u64)>)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// `fs::grace_fingerprint` behind a ~1 s per-project memo (semantics unchanged:
/// it still returns the newest mtime + file count, just possibly one tick old).
fn memoized_grace_fingerprint(path: &Path, key: &str) -> Option<(SystemTime, u64)> {
    let now = Instant::now();
    if let Ok(memo) = GRACE_FINGERPRINT_MEMO.lock() {
        if let Some((computed_at, fingerprint)) = memo.get(key) {
            if now.duration_since(*computed_at) < GRACE_FINGERPRINT_MEMO_TTL {
                return *fingerprint;
            }
        }
    }

    let current = fs::grace_fingerprint(path);
    if let Ok(mut memo) = GRACE_FINGERPRINT_MEMO.lock() {
        memo.insert(key.to_string(), (now, current));
    }
    current
}

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
    let (notes_md, notes_truncated) = match fs::read_text_capped(&dir.join("notes.md")) {
        Some((text, truncated)) => (Some(text), truncated),
        None => (None, false),
    };
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
        notes_truncated,
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
    fs::ensure_openable_extension(&resolved)?;
    fs::open_with_default_app(&fs::open_argument(&resolved))
}

/// Whether the project's `.grace5` tree changed since the previous call.
#[command]
pub async fn grace_check_changed(path: String) -> Result<bool, String> {
    let current = memoized_grace_fingerprint(Path::new(&path), &path);
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

/// Shared tempdir and fixture assembly for the GRACE unit tests (one copy for
/// `fs`, `parse` and `state` instead of three near-identical ones).
#[cfg(test)]
pub(crate) mod test_support {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub(crate) const CHANGE_ID: &str = "C-0019-retire-briefs";

    /// The checked-in fixture change, relative to its change directory.
    const FIXTURE_FILES: &[(&str, &[u8])] = &[
        (
            "contract.xml",
            include_bytes!("fixtures/C-0019-retire-briefs/contract.xml"),
        ),
        (
            "notes.md",
            include_bytes!("fixtures/C-0019-retire-briefs/notes.md"),
        ),
        (
            "records/approval-1.xml",
            include_bytes!("fixtures/C-0019-retire-briefs/records/approval-1.xml"),
        ),
        (
            "records/run-1.xml",
            include_bytes!("fixtures/C-0019-retire-briefs/records/run-1.xml"),
        ),
        (
            "records/run-3.xml",
            include_bytes!("fixtures/C-0019-retire-briefs/records/run-3.xml"),
        ),
        (
            "records/review-1.xml",
            include_bytes!("fixtures/C-0019-retire-briefs/records/review-1.xml"),
        ),
        (
            "records/close.xml",
            include_bytes!("fixtures/C-0019-retire-briefs/records/close.xml"),
        ),
        (
            "records/brief-1.html",
            include_bytes!("fixtures/C-0019-retire-briefs/records/brief-1.html"),
        ),
    ];

    /// A unique directory under the OS temp dir, removed on drop.
    pub(crate) struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        pub(crate) fn new(label: &str) -> TempDir {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "grace-{label}-{}-{nanos}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            TempDir { path }
        }

        pub(crate) fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    /// Writes the fixture contract, notes and records into `change_dir`.
    pub(crate) fn write_fixture_change(change_dir: &Path) {
        for (relative, bytes) in FIXTURE_FILES {
            let path = change_dir.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, bytes).unwrap();
        }
    }

    pub(crate) fn write_contract(dir: &Path, src: &[u8]) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("contract.xml"), src).unwrap();
    }

    #[cfg(unix)]
    pub(crate) fn symlink_file(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    pub(crate) fn symlink_file(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_file(target, link)
    }

    #[cfg(unix)]
    pub(crate) fn symlink_dir(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    pub(crate) fn symlink_dir(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(target, link)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grace::test_support::TempDir;
    use std::fs;

    #[test]
    fn memoizes_the_fingerprint_for_about_one_second() {
        let dir = TempDir::new("memo");
        let grace = dir.path().join(".grace5");
        fs::create_dir_all(&grace).unwrap();
        fs::write(grace.join("kernel-version"), "0.9.0").unwrap();
        let key = dir.path().to_string_lossy().to_string();

        let first = memoized_grace_fingerprint(dir.path(), &key).expect("fingerprint");
        fs::write(grace.join("extra.log"), b"x").unwrap();
        let second = memoized_grace_fingerprint(dir.path(), &key).expect("memoized fingerprint");
        assert_eq!(first, second, "within the TTL the tree walk is not repeated");

        std::thread::sleep(
            GRACE_FINGERPRINT_MEMO_TTL
                + std::time::Duration::from_millis(GRACE_FINGERPRINT_MEMO_SLACK_MS),
        );
        let third = memoized_grace_fingerprint(dir.path(), &key).expect("refreshed fingerprint");
        assert_ne!(first, third, "after the TTL the walk observes the new file");
    }
}
