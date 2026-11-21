//! Read-only GRACE 5 artifact parsing and Tauri commands.
//!
//! The grace5 kernel remains authoritative; this module only reads `.grace5/`
//! and never writes to it.

use std::collections::{HashMap, HashSet, VecDeque};
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

/// Finds every architecture term lexicon below `<root>/models/` in
/// breadth-first order. Entries of every directory are sorted, so the order is
/// deterministic; hidden directories and `node_modules` are skipped, symlinked
/// directories are never descended (cycle/escape safety), and a symlinked file
/// is accepted only when its canonical path stays inside the canonical project
/// root.
fn find_project_lexicons(root: &Path) -> Vec<PathBuf> {
    let models = root.join("models");
    let Ok(metadata) = std::fs::symlink_metadata(&models) else {
        return Vec::new();
    };
    // A symlinked `models/` (or a plain file) is not searched.
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Vec::new();
    }

    let mut found = Vec::new();
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    queue.push_back((models.clone(), 0));

    while let Some((dir, depth)) = queue.pop_front() {
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
                path
            } else {
                continue;
            };
            found.push(resolved);
        }

        for subdirectory in subdirectories {
            queue.push_back((subdirectory, depth + 1));
        }
    }

    found
}

/// First `# Schema: <citation>` header line of a lexicon, at column 0.
fn schema_citation(text: &str) -> Option<&str> {
    for line in text.lines() {
        let Some(rest) = line.strip_prefix('#') else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix("Schema:") else {
            continue;
        };
        if let Some(citation) = rest.split_whitespace().next() {
            return Some(citation);
        }
    }
    None
}

/// Resolves the lexicon cited by a lexicon's `# Schema:` header, ONE hop.
///
/// The citation is relative to the citing file. Leading `..` components climb
/// toward the shared parent (`citedRepoRoot`); the canonicalized import target
/// must stay inside that canonicalized root. Absolute paths, drive/UNC
/// citations, interior escapes and symlinked citation directories are
/// rejected. The citation may name the schema document beside the lexicon; the
/// lexicon is then looked up in the citation target's directory. Missing
/// files are skipped silently.
fn cited_lexicon(file: &Path, citation: &str) -> Option<PathBuf> {
    let relative = Path::new(citation);
    let components: Vec<Component> = relative.components().collect();
    if components.is_empty()
        || components
            .iter()
            .any(|component| matches!(component, Component::Prefix(_) | Component::RootDir))
    {
        return None;
    }

    let lexicon_dir = file.parent()?.canonicalize().ok()?;
    let mut cited_root_path = lexicon_dir.clone();
    let mut leading_parents = 0;
    for component in &components {
        match component {
            Component::CurDir => continue,
            Component::ParentDir => leading_parents += 1,
            _ => break,
        }
    }
    for _ in 0..leading_parents {
        cited_root_path = cited_root_path.parent()?.to_path_buf();
    }
    let cited_root = cited_root_path.canonicalize().ok()?;

    let target = lexicon_dir.join(relative);
    let names_lexicon = target
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| LEXICON_FILE_NAMES.contains(&name));
    let directory = target.parent()?;

    // A symlinked citation directory is never descended.
    let metadata = std::fs::symlink_metadata(directory).ok()?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return None;
    }

    let candidates: Vec<PathBuf> = if names_lexicon {
        vec![target]
    } else {
        LEXICON_FILE_NAMES
            .iter()
            .map(|name| directory.join(name))
            .collect()
    };

    for candidate in candidates {
        let Ok(canonical) = candidate.canonicalize() else {
            continue;
        };
        if !canonical.starts_with(&cited_root) || !canonical.is_file() {
            continue;
        }
        return Some(canonical);
    }
    None
}

/// Reads every architecture term lexicon of a project (see
/// `find_project_lexicons`), then follows each lexicon's `# Schema:` header
/// citation one hop to a sibling repository's lexicon (the cited schema
/// document sits beside its lexicon). Project files come first in BFS order,
/// imports after them in citation order, and canonical duplicates are read
/// once. Missing, non-file or unreadable files are skipped; the 1 MiB cap is
/// the notes cap.
pub fn read_project_lexicon(project: &Path) -> Option<Vec<String>> {
    let root = project.canonicalize().ok()?;
    let files = find_project_lexicons(&root);

    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut texts: Vec<String> = Vec::new();
    let mut citations: Vec<(PathBuf, String)> = Vec::new();

    for file in files {
        let key = file.canonicalize().unwrap_or_else(|_| file.clone());
        if !seen.insert(key) {
            continue;
        }
        let Some((text, _)) = fs::read_text_capped(&file) else {
            continue;
        };
        if let Some(citation) = schema_citation(&text) {
            citations.push((file, citation.to_string()));
        }
        texts.push(text);
    }

    // One hop only: an imported lexicon's own citation is never followed.
    for (file, citation) in citations {
        let Some(import) = cited_lexicon(&file, &citation) else {
            continue;
        };
        if !seen.insert(import.clone()) {
            continue;
        }
        if let Some((text, _)) = fs::read_text_capped(&import) {
            texts.push(text);
        }
    }

    if texts.is_empty() {
        None
    } else {
        Some(texts)
    }
}

/// Reads the project's lexicons (see `read_project_lexicon`).
#[command]
pub fn read_lexicon(path: String) -> Option<Vec<String>> {
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
            read_project_lexicon(dir.path()),
            Some(vec!["terms:\n  - M1.2\n".to_string()])
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
        let texts = read_project_lexicon(dir.path()).expect("capped lexicon");
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0].len(), crate::grace::fs::MAX_READ_BYTES);
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
            read_project_lexicon(dir.path()),
            Some(vec!["M1.2: gloss\n".to_string()])
        );
    }

    #[test]
    fn finds_a_yml_lexicon() {
        let dir = TempDir::new("lexicon-yml");
        write_lexicon_at(&dir, "models/architecture/term-lexicon.yml", b"- M1.2\n");
        assert_eq!(
            read_project_lexicon(dir.path()),
            Some(vec!["- M1.2\n".to_string()])
        );
    }

    #[test]
    fn returns_every_lexicon_in_breadth_first_order() {
        let dir = TempDir::new("lexicon-shallow");
        write_lexicon_at(&dir, "models/deep/nested/term-lexicon.yaml", b"deep\n");
        write_lexicon_at(&dir, "models/architecture/term-lexicon.yaml", b"shallow\n");
        assert_eq!(
            read_project_lexicon(dir.path()),
            Some(vec!["shallow\n".to_string(), "deep\n".to_string()])
        );
    }

    #[test]
    fn equal_depth_lexicons_keep_their_sorted_order() {
        let dir = TempDir::new("lexicon-tie");
        write_lexicon_at(&dir, "models/b/x/term-lexicon.yaml", b"b\n");
        write_lexicon_at(&dir, "models/a/x/term-lexicon.yaml", b"a\n");
        assert_eq!(
            read_project_lexicon(dir.path()),
            Some(vec!["a\n".to_string(), "b\n".to_string()])
        );
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
        assert_eq!(
            read_project_lexicon(dir.path()),
            Some(vec!["inside\n".to_string()])
        );
    }

    #[test]
    fn the_lexicon_search_is_depth_capped() {
        let dir = TempDir::new("lexicon-depth");
        let mut deep = String::from("models");
        for level in 0..LEXICON_MAX_DEPTH {
            deep.push_str(&format!("/d{level}"));
        }
        write_lexicon_at(&dir, &format!("{deep}/term-lexicon.yaml"), b"eight\n");
        assert_eq!(
            read_project_lexicon(dir.path()),
            Some(vec!["eight\n".to_string()])
        );

        let too_deep = format!("{deep}/d{LEXICON_MAX_DEPTH}/term-lexicon.yaml");
        let other = TempDir::new("lexicon-depth-over");
        write_lexicon_at(&other, &too_deep, b"nine\n");
        assert_eq!(read_project_lexicon(other.path()), None);
    }

    fn write_at(root: &Path, relative: &str, contents: &[u8]) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    /// mtext-style layout: `<workspace>/repos/mtext` cites a sibling checkout.
    fn cited_layout(label: &str) -> (TempDir, PathBuf, PathBuf) {
        let workspace = TempDir::new(label);
        let project = workspace.path().join("repos/mtext");
        let core = workspace
            .path()
            .join("repos/metasteme-protocols/models/architecture/core");
        fs::create_dir_all(&core).unwrap();
        fs::write(core.join("term-lexicon-schema.md"), b"# schema\n").unwrap();
        fs::write(
            core.join("term-lexicon.yaml"),
            b"# Term lexicon\n# Schema: term-lexicon-schema.md\n- term: Order\n",
        )
        .unwrap();
        (workspace, project, core)
    }

    #[test]
    fn follows_a_schema_citation_to_the_sibling_lexicon() {
        let (_workspace, project, _core) = cited_layout("lexicon-cited");
        let project_text = "# Term lexicon\n# Schema: ../../../metasteme-protocols/models/architecture/core/term-lexicon-schema.md (term-lexicon/v1).\n- term: title\n";
        write_at(&project, "models/architecture/term-lexicon.yaml", project_text.as_bytes());

        let texts = read_project_lexicon(&project).expect("project and cited lexicon");
        assert_eq!(texts.len(), 2);
        assert_eq!(texts[0], project_text);
        assert!(texts[1].contains("term: Order"), "cited lexicon is imported");
    }

    #[test]
    fn a_citation_naming_the_lexicon_file_is_used_directly() {
        let (_workspace, project, _core) = cited_layout("lexicon-cited-file");
        write_at(
            &project,
            "models/architecture/term-lexicon.yaml",
            b"# Schema: ../../../metasteme-protocols/models/architecture/core/term-lexicon.yaml\n- term: title\n",
        );
        let texts = read_project_lexicon(&project).expect("cited lexicon");
        assert_eq!(texts.len(), 2);
        assert!(texts[1].contains("term: Order"));
    }

    #[test]
    fn an_absolute_citation_is_rejected() {
        let (_workspace, project, core) = cited_layout("lexicon-cited-absolute");
        let absolute = core.join("term-lexicon.yaml");
        let header = format!("# Schema: {}\n- term: title\n", absolute.display());
        write_at(&project, "models/architecture/term-lexicon.yaml", header.as_bytes());
        assert_eq!(read_project_lexicon(&project).unwrap().len(), 1);
    }

    #[test]
    fn a_citation_escaping_the_cited_root_is_rejected() {
        let (workspace, project, _core) = cited_layout("lexicon-cited-escape");
        write_at(
            workspace.path(),
            "outside/term-lexicon.yaml",
            b"# Schema: term-lexicon-schema.md\n- term: leaked\n",
        );
        write_at(
            &project,
            "models/architecture/term-lexicon.yaml",
            b"# Schema: ../../../metasteme-protocols/models/architecture/../../../../outside/term-lexicon.yaml\n- term: title\n",
        );
        let texts = read_project_lexicon(&project).unwrap();
        assert_eq!(texts.len(), 1, "an interior .. escape is not imported");
    }

    #[test]
    fn a_symlinked_citation_escape_is_rejected() {
        use crate::grace::test_support::symlink_dir;

        let (workspace, project, _core) = cited_layout("lexicon-cited-symlink");
        let outside = TempDir::new("lexicon-cited-symlink-outside");
        fs::write(
            outside.path().join("term-lexicon.yaml"),
            b"# Schema: term-lexicon-schema.md\n- term: leaked\n",
        )
        .unwrap();
        // Replace the sibling checkout with a symlink to a tree outside the
        // cited root (the workspace).
        fs::remove_dir_all(workspace.path().join("repos/metasteme-protocols")).unwrap();
        if symlink_dir(
            outside.path(),
            &workspace.path().join("repos/metasteme-protocols"),
        )
        .is_err() {
            eprintln!("skipping cited symlink escape: cannot create a directory symlink here");
            return;
        }
        write_at(
            &project,
            "models/architecture/term-lexicon.yaml",
            b"# Schema: ../../../metasteme-protocols/term-lexicon-schema.md\n- term: title\n",
        );
        assert_eq!(read_project_lexicon(&project).unwrap().len(), 1);
    }

    #[test]
    fn a_missing_cited_lexicon_is_skipped() {
        let (_workspace, project, core) = cited_layout("lexicon-cited-missing");
        fs::remove_file(core.join("term-lexicon.yaml")).unwrap();
        write_at(
            &project,
            "models/architecture/term-lexicon.yaml",
            b"# Schema: ../../../metasteme-protocols/models/architecture/core/term-lexicon-schema.md\n- term: title\n",
        );
        let texts = read_project_lexicon(&project).unwrap();
        assert_eq!(texts.len(), 1);
    }

    #[test]
    fn an_imported_lexicons_own_citation_is_not_followed() {
        let (_workspace, project, core) = cited_layout("lexicon-one-hop");
        write_at(
            &core,
            "other/term-lexicon.yaml",
            b"# Schema: term-lexicon-schema.md\n- term: third\n",
        );
        write_at(
            &core,
            "term-lexicon.yaml",
            b"# Schema: other/term-lexicon-schema.md\n- term: Order\n",
        );
        write_at(
            &project,
            "models/architecture/term-lexicon.yaml",
            b"# Schema: ../../../metasteme-protocols/models/architecture/core/term-lexicon-schema.md\n- term: title\n",
        );
        let texts = read_project_lexicon(&project).unwrap();
        assert_eq!(texts.len(), 2, "one hop only");
        assert!(texts.iter().all(|text| !text.contains("term: third")));
    }

    #[test]
    fn duplicate_lexicon_paths_are_read_once() {
        let (_workspace, project, core) = cited_layout("lexicon-dedup");
        write_at(
            &project,
            "models/a/term-lexicon.yaml",
            b"# Schema: ../../../metasteme-protocols/models/architecture/core/term-lexicon-schema.md\n- term: a\n",
        );
        write_at(
            &project,
            "models/b/term-lexicon.yaml",
            b"# Schema: ../../../metasteme-protocols/models/architecture/core/term-lexicon-schema.md\n- term: b\n",
        );
        write_at(
            &core,
            "term-lexicon.yaml",
            b"- term: Order\n",
        );
        let texts = read_project_lexicon(&project).unwrap();
        assert_eq!(texts.len(), 3, "two project files plus one shared import");
        assert_eq!(texts.iter().filter(|text| text.contains("term: Order")).count(), 1);
    }
}
