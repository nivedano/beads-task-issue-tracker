//! Read-only GRACE 5 artifact parsing and Tauri commands.
//!
//! The grace5 kernel remains authoritative; this module only reads `.grace5/`
//! and never writes to it.

use std::collections::{HashMap, VecDeque};
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

/// Lexicon file names searched for anywhere below `<project>/models/`.
const LEXICON_FILE_NAMES: [&str; 2] = ["term-lexicon.yaml", "term-lexicon.yml"];
/// Directories searched below `models/` are capped at this depth (models/ = 0).
const LEXICON_MAX_DEPTH: usize = 8;

/// Finds the project's architecture term lexicon by breadth-first walk of
/// `<root>/models/`, shallowest file first. Entries of every directory are
/// sorted, so a tie at the same depth resolves to the lexicographically
/// smallest path relative to `models/`. Hidden directories and `node_modules`
/// are skipped, symlinked directories are never descended (cycle/escape
/// safety), and a symlinked file is accepted only when its canonical path stays
/// inside the canonical project root. Returns the canonical file path.
fn find_project_lexicon(root: &Path) -> Option<PathBuf> {
    let models = root.join("models");
    let metadata = std::fs::symlink_metadata(&models).ok()?;
    // A symlinked `models/` (or a plain file) is not searched.
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return None;
    }

    let mut best: Option<(usize, String, PathBuf)> = None;
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    queue.push_back((models.clone(), 0));

    while let Some((dir, depth)) = queue.pop_front() {
        // A shallower match cannot appear once the walk passes its depth.
        if let Some((best_depth, ..)) = &best {
            if depth > *best_depth {
                break;
            }
        }

        let Ok(read_dir) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<(std::ffi::OsString, PathBuf, std::fs::FileType)> = Vec::new();
        for entry in read_dir.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            entries.push((entry.file_name(), entry.path(), file_type));
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));

        let mut subdirectories: Vec<PathBuf> = Vec::new();
        for (name, path, file_type) in entries {
            if file_type.is_dir() {
                let name = name.to_string_lossy();
                if depth < LEXICON_MAX_DEPTH
                    && !name.starts_with('.')
                    && name != "node_modules"
                {
                    subdirectories.push(path);
                }
                continue;
            }
            if !LEXICON_FILE_NAMES
                .iter()
                .any(|candidate| name.to_string_lossy().as_ref() == *candidate)
            {
                continue;
            }

            let resolved = if file_type.is_symlink() {
                let Ok(resolved) = path.canonicalize() else {
                    continue;
                };
                if !resolved.starts_with(root) || !resolved.is_file() {
                    continue;
                }
                resolved
            } else if file_type.is_file() {
                path.clone()
            } else {
                continue;
            };

            let relative = path
                .strip_prefix(&models)
                .map(|relative| relative.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            let candidate = (depth, relative, resolved);
            let better = match &best {
                Some((best_depth, best_relative, _)) => {
                    (candidate.0, candidate.1.as_str()) < (*best_depth, best_relative.as_str())
                }
                None => true,
            };
            if better {
                best = Some(candidate);
            }
        }

        for subdirectory in subdirectories {
            queue.push_back((subdirectory, depth + 1));
        }
    }

    best.map(|(_, _, path)| path)
}

/// Reads the project's architecture term lexicon (see `find_project_lexicon`)
/// for prose highlighting. Missing, non-file or unreadable files yield `None`
/// (highlighting then degrades to the two structural classes); the same 1 MiB
/// cap as notes applies.
pub fn read_project_lexicon(project: &Path) -> Option<String> {
    let root = project.canonicalize().ok()?;
    let resolved = find_project_lexicon(&root)?;
    fs::read_text_capped(&resolved).map(|(text, _)| text)
}

/// Reads the project's architecture term lexicon (see `read_project_lexicon`).
#[command]
pub fn read_lexicon(path: String) -> Option<String> {
    read_project_lexicon(Path::new(&path))
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

    fn write_lexicon(dir: &TempDir, contents: &[u8]) {
        let path = dir
            .path()
            .join("models/architecture/term-lexicon.yaml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn reads_the_project_lexicon() {
        let dir = TempDir::new("lexicon");
        write_lexicon(&dir, b"terms:\n  - M1.2\n");
        assert_eq!(
            read_project_lexicon(dir.path()).as_deref(),
            Some("terms:\n  - M1.2\n")
        );
    }

    #[test]
    fn missing_lexicon_is_none() {
        let dir = TempDir::new("lexicon-missing");
        assert_eq!(read_project_lexicon(dir.path()), None);
    }

    #[test]
    fn lexicon_is_capped_at_one_mib() {
        let dir = TempDir::new("lexicon-cap");
        write_lexicon(&dir, &vec![b'a'; crate::grace::fs::MAX_READ_BYTES + 10]);
        let text = read_project_lexicon(dir.path()).expect("capped lexicon");
        assert_eq!(text.len(), crate::grace::fs::MAX_READ_BYTES);
    }

    #[test]
    fn a_non_file_lexicon_is_none() {
        let dir = TempDir::new("lexicon-dir");
        fs::create_dir_all(dir.path().join("models/architecture/term-lexicon.yaml")).unwrap();
        assert_eq!(read_project_lexicon(dir.path()), None);
    }

    #[test]
    fn a_lexicon_symlinked_outside_the_project_is_none() {
        use crate::grace::test_support::symlink_dir;

        let project = TempDir::new("lexicon-symlink");
        let outside = TempDir::new("lexicon-outside");
        write_lexicon(&outside, b"terms:\n  - M1.9\n");

        let models = project.path().join("models");
        if symlink_dir(&outside.path().join("models"), &models).is_err() {
            eprintln!("skipping symlink escape: cannot create a directory symlink here");
            return;
        }
        assert_eq!(read_project_lexicon(project.path()), None);
    }

    fn write_lexicon_at(dir: &TempDir, relative: &str, contents: &[u8]) {
        let path = dir.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn finds_a_lexicon_in_a_nested_subdirectory() {
        let dir = TempDir::new("lexicon-nested");
        write_lexicon_at(&dir, "models/deep/nested/term-lexicon.yaml", b"M1.2: gloss\n");
        assert_eq!(
            read_project_lexicon(dir.path()).as_deref(),
            Some("M1.2: gloss\n")
        );
    }

    #[test]
    fn finds_a_yml_lexicon() {
        let dir = TempDir::new("lexicon-yml");
        write_lexicon_at(&dir, "models/architecture/term-lexicon.yml", b"- M1.2\n");
        assert_eq!(read_project_lexicon(dir.path()).as_deref(), Some("- M1.2\n"));
    }

    #[test]
    fn the_shallowest_lexicon_wins() {
        let dir = TempDir::new("lexicon-shallow");
        write_lexicon_at(&dir, "models/deep/nested/term-lexicon.yaml", b"deep\n");
        write_lexicon_at(&dir, "models/architecture/term-lexicon.yaml", b"shallow\n");
        assert_eq!(
            read_project_lexicon(dir.path()).as_deref(),
            Some("shallow\n")
        );
    }

    #[test]
    fn equal_depth_ties_resolve_to_the_smallest_relative_path() {
        let dir = TempDir::new("lexicon-tie");
        write_lexicon_at(&dir, "models/b/x/term-lexicon.yaml", b"b\n");
        write_lexicon_at(&dir, "models/a/x/term-lexicon.yaml", b"a\n");
        assert_eq!(read_project_lexicon(dir.path()).as_deref(), Some("a\n"));
    }

    #[test]
    fn hidden_and_node_modules_directories_are_skipped() {
        let dir = TempDir::new("lexicon-skips");
        write_lexicon_at(&dir, "models/.hidden/term-lexicon.yaml", b"hidden\n");
        write_lexicon_at(&dir, "models/node_modules/pkg/term-lexicon.yaml", b"modules\n");
        assert_eq!(read_project_lexicon(dir.path()), None);
    }

    #[test]
    fn symlinked_directories_are_not_descended() {
        use crate::grace::test_support::symlink_dir;

        let project = TempDir::new("lexicon-symlink-dir");
        let outside = TempDir::new("lexicon-symlink-dir-outside");
        write_lexicon(&outside, b"outside\n");
        let models = project.path().join("models");
        fs::create_dir_all(&models).unwrap();
        if symlink_dir(&outside.path().join("models"), &models.join("linked")).is_err() {
            eprintln!("skipping symlinked directory: cannot create a directory symlink here");
            return;
        }
        assert_eq!(read_project_lexicon(project.path()), None);
    }

    #[test]
    fn a_symlinked_lexicon_file_escaping_the_project_is_rejected() {
        use crate::grace::test_support::symlink_file;

        let project = TempDir::new("lexicon-file-symlink");
        let outside = TempDir::new("lexicon-file-outside");
        write_lexicon(&outside, b"outside\n");
        let target = project.path().join("models/architecture/term-lexicon.yaml");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        if symlink_file(
            &outside.path().join("models/architecture/term-lexicon.yaml"),
            &target,
        )
        .is_err()
        {
            eprintln!("skipping symlinked file: cannot create a file symlink here");
            return;
        }
        assert_eq!(read_project_lexicon(project.path()), None);
    }

    #[test]
    fn a_symlinked_lexicon_file_inside_the_project_is_accepted() {
        use crate::grace::test_support::symlink_file;

        let dir = TempDir::new("lexicon-file-inside");
        write_lexicon_at(&dir, "models/real/lexicon.yaml", b"inside\n");
        let target = dir.path().join("models/architecture/term-lexicon.yaml");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        if symlink_file(&dir.path().join("models/real/lexicon.yaml"), &target).is_err() {
            eprintln!("skipping symlinked file: cannot create a file symlink here");
            return;
        }
        assert_eq!(read_project_lexicon(dir.path()).as_deref(), Some("inside\n"));
    }

    #[test]
    fn the_lexicon_search_is_depth_capped() {
        let dir = TempDir::new("lexicon-depth");
        let mut deep = String::from("models");
        for level in 0..LEXICON_MAX_DEPTH {
            deep.push_str(&format!("/d{level}"));
        }
        write_lexicon_at(&dir, &format!("{deep}/term-lexicon.yaml"), b"eight\n");
        assert_eq!(read_project_lexicon(dir.path()).as_deref(), Some("eight\n"));

        let too_deep = format!("{deep}/d{LEXICON_MAX_DEPTH}/term-lexicon.yaml");
        let other = TempDir::new("lexicon-depth-over");
        write_lexicon_at(&other, &too_deep, b"nine\n");
        assert_eq!(read_project_lexicon(other.path()), None);
    }
}
