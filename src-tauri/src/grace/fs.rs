//! Safe reads, fingerprints and watch targets for a project's `.grace5/`.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::grace::model::GraceFileContent;

/// `read_grace_file` never returns more than one MiB of decoded text.
pub const MAX_READ_BYTES: usize = 1024 * 1024;

/// Reads at most `MAX_READ_BYTES` from a file without loading the rest of it.
fn capped_bytes(path: &Path) -> std::io::Result<(Vec<u8>, bool, u64)> {
    let file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "not a file",
        ));
    }
    let size = metadata.len();
    let mut buffer = Vec::with_capacity(size.min(MAX_READ_BYTES as u64) as usize);
    file.take(MAX_READ_BYTES as u64).read_to_end(&mut buffer)?;
    Ok((buffer, size > MAX_READ_BYTES as u64, size))
}

/// Lossy UTF-8 text under the same cap; `None` when the file cannot be read.
pub(crate) fn read_text_capped(path: &Path) -> Option<String> {
    capped_bytes(path)
        .ok()
        .map(|(bytes, _, _)| String::from_utf8_lossy(&bytes).into_owned())
}

/// Reads a file inside `<project>/.grace5/`, refusing anything that resolves
/// outside it (traversal, absolute paths, symlink escapes).
pub fn read_grace_file(project: &Path, rel_path: &str) -> Result<GraceFileContent, String> {
    let root = project
        .join(".grace5")
        .canonicalize()
        .map_err(|error| format!("Cannot resolve .grace5: {error}"))?;
    let resolved = root
        .join(rel_path)
        .canonicalize()
        .map_err(|error| format!("Cannot resolve {rel_path}: {error}"))?;
    if !resolved.starts_with(&root) {
        return Err(format!("{rel_path} is outside .grace5"));
    }
    let (bytes, truncated, size_bytes) =
        capped_bytes(&resolved).map_err(|error| format!("Cannot read {rel_path}: {error}"))?;
    Ok(GraceFileContent {
        content: String::from_utf8_lossy(&bytes).into_owned(),
        truncated,
        size_bytes,
    })
}

/// Newest mtime (files and directories) and file count over `.grace5`;
/// `None` without `.grace5`. Symlinked directories are not followed (loop
/// safety), so a symlink counts as a file.
pub fn grace_fingerprint(project: &Path) -> Option<(SystemTime, u64)> {
    let root = project.join(".grace5");
    if !root.is_dir() {
        return None;
    }
    let mut newest = modified_time(&root);
    let mut count = 0u64;
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };
            newest = newest_of(newest, metadata.modified().ok());
            if metadata.is_dir() {
                stack.push(path);
            } else {
                count += 1;
            }
        }
    }
    newest.map(|time| (time, count))
}

fn modified_time(path: &Path) -> Option<SystemTime> {
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
}

fn newest_of(current: Option<SystemTime>, candidate: Option<SystemTime>) -> Option<SystemTime> {
    match (current, candidate) {
        (Some(current), Some(candidate)) => Some(if candidate > current {
            candidate
        } else {
            current
        }),
        (current, None) => current,
        (None, candidate) => candidate,
    }
}

/// The registries present at a project root, in a stable order.
pub fn watch_targets(project: &Path) -> Vec<PathBuf> {
    let mut targets = Vec::new();
    for name in [".beads", ".grace5"] {
        let path = project.join(name);
        if path.is_dir() {
            targets.push(path);
        }
    }
    targets
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    const CHANGE_ID: &str = "C-0019-retire-briefs";

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(label: &str) -> TempDir {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "grace-fs-{label}-{}-{nanos}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            TempDir { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn write_fixture_change(root: &Path) -> PathBuf {
        let change = root
            .join(".grace5")
            .join("changes")
            .join("closed")
            .join(CHANGE_ID);
        fs::create_dir_all(change.join("records")).unwrap();
        fs::write(
            change.join("contract.xml"),
            include_bytes!("fixtures/C-0019-retire-briefs/contract.xml"),
        )
        .unwrap();
        fs::write(
            change.join("notes.md"),
            include_bytes!("fixtures/C-0019-retire-briefs/notes.md"),
        )
        .unwrap();
        for (name, bytes) in [
            (
                "approval-1.xml",
                &include_bytes!("fixtures/C-0019-retire-briefs/records/approval-1.xml")[..],
            ),
            (
                "run-1.xml",
                &include_bytes!("fixtures/C-0019-retire-briefs/records/run-1.xml")[..],
            ),
            (
                "run-3.xml",
                &include_bytes!("fixtures/C-0019-retire-briefs/records/run-3.xml")[..],
            ),
            (
                "review-1.xml",
                &include_bytes!("fixtures/C-0019-retire-briefs/records/review-1.xml")[..],
            ),
            (
                "close.xml",
                &include_bytes!("fixtures/C-0019-retire-briefs/records/close.xml")[..],
            ),
            (
                "brief-1.html",
                &include_bytes!("fixtures/C-0019-retire-briefs/records/brief-1.html")[..],
            ),
        ] {
            fs::write(change.join("records").join(name), bytes).unwrap();
        }
        change
    }

    #[cfg(unix)]
    fn symlink_file(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn symlink_file(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_file(target, link)
    }

    fn write_contract(dir: &Path, src: &[u8]) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("contract.xml"), src).unwrap();
    }

    #[test]
    fn reads_a_grace_file() {
        let dir = TempDir::new("read");
        write_fixture_change(dir.path());
        let file = read_grace_file(
            dir.path(),
            "changes/closed/C-0019-retire-briefs/notes.md",
        )
        .expect("notes.md reads");
        assert!(file.content.starts_with("# Implementation notes"));
        assert!(!file.truncated);
        assert_eq!(file.size_bytes, 8097);
    }

    #[test]
    fn refuses_paths_outside_grace() {
        let dir = TempDir::new("containment");
        write_fixture_change(dir.path());
        let outside = dir.path().join("outside.md");
        fs::write(&outside, b"secret").unwrap();

        assert!(read_grace_file(dir.path(), "../outside.md").is_err());
        assert!(read_grace_file(dir.path(), outside.to_str().unwrap()).is_err());

        let link = dir.path().join(".grace5").join("escape.md");
        if symlink_file(&outside, &link).is_ok() {
            assert!(
                read_grace_file(dir.path(), "escape.md").is_err(),
                "a symlink escaping .grace5 must be refused"
            );
        } else {
            eprintln!("skipping symlink escape: cannot create a file symlink here");
        }

        assert!(read_grace_file(
            dir.path(),
            "changes/closed/C-0019-retire-briefs/notes.md"
        )
        .is_ok());
    }

    #[test]
    fn truncates_large_files_at_one_mib() {
        let dir = TempDir::new("large");
        let big = dir.path().join(".grace5").join("big.log");
        fs::create_dir_all(big.parent().unwrap()).unwrap();
        fs::write(&big, vec![b'a'; 1_500_000]).unwrap();

        let file = read_grace_file(dir.path(), "big.log").expect("large file reads");
        assert!(file.truncated);
        assert_eq!(file.size_bytes, 1_500_000);
        assert_eq!(file.content.len(), 1024 * 1024);
    }

    #[test]
    fn refuses_a_missing_file() {
        let dir = TempDir::new("missing");
        fs::create_dir_all(dir.path().join(".grace5")).unwrap();
        assert!(read_grace_file(dir.path(), "nope.txt").is_err());
    }

    #[test]
    fn fingerprints_the_grace_tree() {
        let dir = TempDir::new("fingerprint");
        assert_eq!(grace_fingerprint(dir.path()), None);

        let grace = dir.path().join(".grace5");
        fs::create_dir_all(&grace).unwrap();
        let first = grace_fingerprint(dir.path()).expect("fingerprint after creating .grace5");

        fs::write(grace.join("new.log"), b"data").unwrap();
        let second = grace_fingerprint(dir.path()).expect("fingerprint after adding a file");
        assert_ne!(first, second);
        assert_eq!(second.1, 1);

        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(grace.join("new.log"), b"more data").unwrap();
        let third = grace_fingerprint(dir.path()).expect("fingerprint after touching a file");
        assert_ne!(second, third);
        assert_eq!(third.1, 1);
    }

    #[test]
    fn watches_existing_registries() {
        let dir = TempDir::new("targets");
        assert!(watch_targets(dir.path()).is_empty());

        fs::create_dir_all(dir.path().join(".beads")).unwrap();
        assert_eq!(
            watch_targets(dir.path()),
            vec![dir.path().join(".beads")]
        );

        fs::remove_dir_all(dir.path().join(".beads")).unwrap();
        fs::create_dir_all(dir.path().join(".grace5")).unwrap();
        assert_eq!(
            watch_targets(dir.path()),
            vec![dir.path().join(".grace5")]
        );

        fs::create_dir_all(dir.path().join(".beads")).unwrap();
        assert_eq!(
            watch_targets(dir.path()),
            vec![
                dir.path().join(".beads"),
                dir.path().join(".grace5")
            ]
        );
    }

    #[test]
    fn shows_a_change_with_latest_results() {
        let dir = TempDir::new("show");
        write_fixture_change(dir.path());
        let detail = crate::grace::grace_show(
            dir.path().to_string_lossy().to_string(),
            CHANGE_ID.to_string(),
        )
        .expect("fixture change shows");

        assert!(detail.summary.closed);
        assert!(
            detail.summary.malformed.is_none(),
            "{:?}",
            detail.summary.malformed
        );
        assert!(detail.contract.is_some());
        assert!(detail
            .notes_md
            .as_deref()
            .unwrap_or_default()
            .contains("# Implementation notes"));
        assert_eq!(detail.briefs.len(), 1);
        assert_eq!(detail.dir, ".grace5/changes/closed/C-0019-retire-briefs");
        assert!(detail.defects.is_empty(), "{:?}", detail.defects);
        assert_eq!(detail.latest_results.len(), 8);
        let ck6 = detail
            .latest_results
            .iter()
            .find(|result| result.check == "CK6")
            .expect("CK6 has a latest result");
        assert_eq!(ck6.verdict, "passed");
        let ck1 = detail
            .latest_results
            .iter()
            .find(|result| result.check == "CK1")
            .expect("CK1 has a latest result");
        assert_eq!(ck1.elapsed_ms, Some(5206), "run-3 supersedes run-1");
    }

    #[test]
    fn refuses_an_unknown_change() {
        let dir = TempDir::new("unknown");
        write_fixture_change(dir.path());
        let result = crate::grace::grace_show(
            dir.path().to_string_lossy().to_string(),
            "C-9999-unknown".to_string(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn refuses_unsafe_change_ids() {
        let dir = TempDir::new("ids");
        write_contract(
            &dir.path().join(".grace5").join("changes").join("C-0001-safe"),
            br#"<Contract id="C-0001-safe" revision="1" title="Safe"><Problem>p</Problem></Contract>"#,
        );
        let project = dir.path().to_string_lossy().to_string();
        let absolute = project.clone();
        for id in [
            "",
            ".",
            "..",
            "../..",
            "../../..",
            "../C-0001-safe",
            "closed/C-0001-safe",
            "..\\..",
            absolute.as_str(),
        ] {
            assert!(
                crate::grace::grace_show(project.clone(), id.to_string()).is_err(),
                "id {id:?} must be refused"
            );
        }
        assert!(crate::grace::grace_show(project, "C-0001-safe".to_string()).is_ok());
    }

    #[test]
    fn caps_the_notes_markdown() {
        let dir = TempDir::new("notes-cap");
        write_fixture_change(dir.path());
        let notes = dir
            .path()
            .join(".grace5/changes/closed/C-0019-retire-briefs/notes.md");
        fs::write(&notes, vec![b'n'; 1_500_000]).unwrap();

        let detail = crate::grace::grace_show(
            dir.path().to_string_lossy().to_string(),
            CHANGE_ID.to_string(),
        )
        .expect("fixture change shows");
        let notes_md = detail.notes_md.expect("notes read");
        assert_eq!(notes_md.len(), 1024 * 1024);
    }

    #[test]
    fn fingerprints_directory_renames() {
        let dir = TempDir::new("fingerprint-rename");
        let grace = dir.path().join(".grace5");
        fs::create_dir_all(grace.join("sub")).unwrap();
        fs::write(grace.join("sub").join("a.log"), b"data").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let before = grace_fingerprint(dir.path()).expect("fingerprint before the rename");

        fs::rename(
            grace.join("sub").join("a.log"),
            grace.join("sub").join("b.log"),
        )
        .unwrap();
        let after = grace_fingerprint(dir.path()).expect("fingerprint after the rename");
        assert_ne!(before, after, "a pure rename must change the fingerprint");
    }

    fn check_changed(path: &str) -> bool {
        tauri::async_runtime::block_on(crate::grace::grace_check_changed(path.to_string()))
            .expect("check_changed resolves")
    }

    #[test]
    fn reports_grace_changes_once() {
        let dir = TempDir::new("changed");
        let path = dir.path().to_string_lossy().to_string();

        assert!(!check_changed(&path));
        fs::create_dir_all(dir.path().join(".grace5")).unwrap();
        fs::write(dir.path().join(".grace5").join("first.log"), b"x").unwrap();
        assert!(check_changed(&path));
        assert!(!check_changed(&path));

        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(dir.path().join(".grace5").join("second.log"), b"y").unwrap();
        assert!(check_changed(&path));
        assert!(!check_changed(&path));
    }
}
