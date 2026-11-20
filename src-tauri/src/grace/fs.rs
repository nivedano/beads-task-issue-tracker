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
    // Check the type before opening: opening a FIFO would block on Unix.
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "not a file",
        ));
    }
    let size = metadata.len();
    let file = fs::File::open(path)?;
    let mut buffer = Vec::with_capacity(size.min(MAX_READ_BYTES as u64) as usize);
    file.take(MAX_READ_BYTES as u64).read_to_end(&mut buffer)?;
    Ok((buffer, size > MAX_READ_BYTES as u64, size))
}

/// Lossy UTF-8 text under the same cap plus the truncation flag; `None` when
/// the file cannot be read. The flag is `true` when bytes were dropped.
pub(crate) fn read_text_capped(path: &Path) -> Option<(String, bool)> {
    capped_bytes(path)
        .ok()
        .map(|(bytes, truncated, _)| (String::from_utf8_lossy(&bytes).into_owned(), truncated))
}

/// Resolves `rel_path` inside `<project>/.grace5/` to a canonical file path,
/// refusing traversal, absolute paths, symlink escapes and non-file targets.
pub fn resolve_grace_file(project: &Path, rel_path: &str) -> Result<PathBuf, String> {
    let root = project
        .join(".grace5")
        .canonicalize()
        .map_err(|error| format!("Cannot resolve .grace5: {error}"))?;
    if Path::new(rel_path).is_absolute() {
        return Err(format!("{rel_path} is outside .grace5"));
    }
    let resolved = root
        .join(rel_path)
        .canonicalize()
        .map_err(|error| format!("Cannot resolve {rel_path}: {error}"))?;
    if !resolved.starts_with(&root) {
        return Err(format!("{rel_path} is outside .grace5"));
    }
    // Check the type before opening: opening a FIFO would block on Unix.
    let metadata =
        fs::metadata(&resolved).map_err(|error| format!("Cannot read {rel_path}: {error}"))?;
    if !metadata.is_file() {
        return Err(format!("{rel_path} is not a file"));
    }
    Ok(resolved)
}

/// Files the explorer may hand to the OS opener (defense in depth, mirroring
/// `open_image_file`'s extension allowlist).
const OPENABLE_EXTENSIONS: [&str; 6] = ["xml", "md", "markdown", "html", "log", "txt"];

/// Refuses anything outside the viewer's read-only artifact set — a `.grace5/`
/// directory must never be able to launch an executable through the explorer.
pub fn ensure_openable_extension(path: &Path) -> Result<(), String> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase());
    let allowed = extension
        .as_deref()
        .map(|extension| OPENABLE_EXTENSIONS.contains(&extension))
        .unwrap_or(false);
    if allowed {
        Ok(())
    } else {
        Err(format!(
            "Only {} files can be opened from the GRACE explorer",
            OPENABLE_EXTENSIONS
                .iter()
                .map(|extension| format!(".{extension}"))
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

/// The path argument handed to the OS opener. `canonicalize` returns the
/// Windows verbatim form (`\\?\C:\...`) that `cmd /C start` (ShellExecute)
/// cannot open, so the prefix is stripped — containment was already proven on
/// the canonical path. A no-op elsewhere.
pub fn open_argument(resolved: &Path) -> PathBuf {
    PathBuf::from(crate::strip_verbatim_prefix(&resolved.to_string_lossy()))
}

/// Quoted `cmd.exe` argument for `start`. `cmd` re-parses its command line, so
/// an unquoted `&` (a legal filename character) would split the command; the
/// quotes keep the path one argument. Windows paths cannot contain `"`.
#[cfg(target_os = "windows")]
pub(crate) fn windows_start_argument(path: &Path) -> std::ffi::OsString {
    std::ffi::OsString::from(format!("\"{}\"", path.display()))
}

/// Opens a resolved file with the OS default application, mirroring
/// `open_image_file` in `lib.rs` (the shell plugin's OpenScope only allows
/// mailto/tel/http(s), so local paths go through the native opener).
pub fn open_with_default_app(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let spawned = std::process::Command::new("open").arg(path).spawn();

    #[cfg(target_os = "windows")]
    let spawned = {
        use std::os::windows::process::CommandExt;
        let mut command = crate::new_command("cmd");
        command.args(["/C", "start", ""]);
        command.raw_arg(windows_start_argument(path));
        command.spawn()
    };

    #[cfg(target_os = "linux")]
    let spawned = std::process::Command::new("xdg-open").arg(path).spawn();

    spawned
        .map(|_| ())
        .map_err(|error| format!("Failed to open {}: {error}", path.display()))
}

/// Reads a file inside `<project>/.grace5/`, refusing anything that resolves
/// outside it (traversal, absolute paths, symlink escapes).
pub fn read_grace_file(project: &Path, rel_path: &str) -> Result<GraceFileContent, String> {
    let resolved = resolve_grace_file(project, rel_path)?;
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
    use crate::grace::test_support::{
        symlink_dir, symlink_file, write_contract, write_fixture_change as write_fixture_files,
        TempDir,
    };
    use crate::grace::GRACE_FINGERPRINT_MEMO_SLACK_MS;
    use std::fs;
    use std::path::{Path, PathBuf};

    const CHANGE_ID: &str = "C-0019-retire-briefs";

    /// Writes the fixture change under a project root and returns its directory.
    fn write_fixture_change(root: &Path) -> PathBuf {
        let change = root
            .join(".grace5")
            .join("changes")
            .join("closed")
            .join(CHANGE_ID);
        write_fixture_files(&change);
        change
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
    fn reports_text_truncation_for_notes() {
        let dir = TempDir::new("notes-large");
        let notes = dir.path().join("notes.md");
        fs::write(&notes, vec![b'a'; MAX_READ_BYTES + 10]).unwrap();

        let (text, truncated) = read_text_capped(&notes).expect("notes read");
        assert!(truncated);
        assert_eq!(text.len(), MAX_READ_BYTES);

        let small = dir.path().join("small.md");
        fs::write(&small, b"# notes").unwrap();
        let (text, truncated) = read_text_capped(&small).expect("small notes read");
        assert_eq!(text, "# notes");
        assert!(!truncated);
    }

    #[test]
    fn opens_only_explorer_file_extensions() {
        for allowed in ["notes.md", "contract.xml", "brief.html", "run.log", "a.txt", "B.MD", "n.markdown"] {
            assert!(
                ensure_openable_extension(Path::new(allowed)).is_ok(),
                "{allowed} should be openable"
            );
        }
        for refused in ["run.exe", "script.ps1", "no-extension", "a.md.exe", "x.HTML.bat"] {
            let error = ensure_openable_extension(Path::new(refused)).unwrap_err();
            assert!(
                error.contains("xml") && error.contains("log"),
                "{refused}: {error}"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn quotes_the_windows_start_argument() {
        // cmd re-parses its command line; a bare `&` would split the command.
        let argument = windows_start_argument(Path::new(r"C:\tmp\C-1&calc.txt"));
        assert_eq!(argument.to_string_lossy(), r#""C:\tmp\C-1&calc.txt""#);
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
        let expected_root = {
            let canonical = dir.path().canonicalize().expect("tempdir resolves");
            crate::strip_verbatim_prefix(&canonical.to_string_lossy()).replace('\\', "/")
        };
        assert!(
            detail.dir.starts_with(&expected_root),
            "dir must be absolute: {} (project root {})",
            detail.dir,
            expected_root
        );
        assert!(
            detail
                .dir
                .ends_with(".grace5/changes/closed/C-0019-retire-briefs"),
            "dir must end with the change path: {}",
            detail.dir
        );
        assert!(!detail.dir.contains('\\'), "dir uses forward slashes: {}", detail.dir);
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
    fn refuses_a_directory_target() {
        let dir = TempDir::new("dir-target");
        fs::create_dir_all(dir.path().join(".grace5").join("folder")).unwrap();
        assert!(read_grace_file(dir.path(), "folder").is_err());
        assert!(resolve_grace_file(dir.path(), "folder").is_err());
    }

    #[test]
    fn resolves_a_file_for_opening() {
        let dir = TempDir::new("open-resolve");
        write_fixture_change(dir.path());
        let resolved = resolve_grace_file(
            dir.path(),
            "changes/closed/C-0019-retire-briefs/notes.md",
        )
        .expect("notes.md resolves for opening");
        assert!(resolved.is_absolute(), "{resolved:?}");
        assert!(resolved.ends_with("notes.md"));
    }

    #[test]
    fn open_refuses_paths_outside_grace() {
        let dir = TempDir::new("open-containment");
        write_fixture_change(dir.path());
        let outside = dir.path().join("outside.md");
        fs::write(&outside, b"secret").unwrap();

        assert!(resolve_grace_file(dir.path(), "../outside.md").is_err());
        assert!(resolve_grace_file(dir.path(), outside.to_str().unwrap()).is_err());

        let link = dir.path().join(".grace5").join("escape.md");
        if symlink_file(&outside, &link).is_ok() {
            assert!(
                resolve_grace_file(dir.path(), "escape.md").is_err(),
                "a symlink escaping .grace5 must be refused"
            );
        } else {
            eprintln!("skipping symlink escape: cannot create a file symlink here");
        }
    }

    #[test]
    fn open_argument_is_stripped_and_non_verbatim_is_unchanged() {
        let dir = TempDir::new("open-arg");
        write_fixture_change(dir.path());
        let resolved = resolve_grace_file(
            dir.path(),
            "changes/closed/C-0019-retire-briefs/notes.md",
        )
        .expect("notes.md resolves for opening");
        let expected = PathBuf::from(crate::strip_verbatim_prefix(&resolved.to_string_lossy()));
        assert_eq!(open_argument(&resolved), expected);
        // A path without a verbatim prefix passes through untouched.
        let plain = Path::new("plain.txt");
        assert_eq!(open_argument(plain), PathBuf::from("plain.txt"));
    }

    #[test]
    #[cfg(windows)]
    fn windows_open_argument_has_no_verbatim_prefix() {
        let dir = TempDir::new("open-arg-win");
        write_fixture_change(dir.path());
        let resolved = resolve_grace_file(
            dir.path(),
            "changes/closed/C-0019-retire-briefs/notes.md",
        )
        .expect("notes.md resolves for opening");
        assert!(
            resolved.to_string_lossy().starts_with("\\\\?\\"),
            "canonicalize returns the verbatim form: {resolved:?}"
        );
        let argument = open_argument(&resolved);
        assert!(
            !argument.to_string_lossy().starts_with("\\\\?\\"),
            "cmd start / ShellExecute cannot open verbatim paths: {argument:?}"
        );
        assert_eq!(
            argument.to_string_lossy(),
            crate::strip_verbatim_prefix(&resolved.to_string_lossy())
        );
    }

    #[test]
    fn grace_open_file_refuses_before_opening() {
        let dir = TempDir::new("open-refuse");
        write_fixture_change(dir.path());
        let project = dir.path().to_string_lossy().to_string();
        assert!(crate::grace::grace_open_file(project.clone(), "../outside.md".to_string()).is_err());
        assert!(crate::grace::grace_open_file(project, "changes".to_string()).is_err());
    }

    #[test]
    #[cfg(unix)]
    fn refuses_a_fifo_without_blocking() {
        let dir = TempDir::new("fifo");
        let pipe = dir.path().join(".grace5").join("pipe");
        fs::create_dir_all(pipe.parent().unwrap()).unwrap();
        let created = std::process::Command::new("mkfifo")
            .arg(&pipe)
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if !created {
            eprintln!("skipping fifo test: mkfifo unavailable");
            return;
        }
        assert!(read_grace_file(dir.path(), "pipe").is_err());
    }

    #[test]
    fn refuses_a_changes_directory_symlinked_outside_grace() {
        let dir = TempDir::new("changes-escape");
        let project = dir.path().join("project");
        let outside = dir.path().join("outside");
        fs::create_dir_all(project.join(".grace5")).unwrap();
        write_contract(
            &outside.join("C-0001-outside"),
            br#"<Contract id="C-0001-outside" revision="1" title="Outside"><Problem>p</Problem></Contract>"#,
        );
        let changes = project.join(".grace5").join("changes");
        if symlink_dir(&outside, &changes).is_err() {
            eprintln!("skipping: this platform cannot create a directory symlink here");
            return;
        }
        let result = crate::grace::grace_show(
            project.to_string_lossy().to_string(),
            "C-0001-outside".to_string(),
        );
        assert!(
            result.is_err(),
            "a changes/ directory outside .grace5 must be refused"
        );
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
        assert!(
            detail.notes_truncated,
            "a capped notes.md must report truncation"
        );
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
        // The fingerprint walk is memoised for ~1 s, so a mutation is observed on
        // the first check after the memo expires (the real 1 s polling cadence).
        let after_memo = || {
            std::thread::sleep(
                std::time::Duration::from_millis(1000 + GRACE_FINGERPRINT_MEMO_SLACK_MS),
            );
        };

        assert!(!check_changed(&path));
        fs::create_dir_all(dir.path().join(".grace5")).unwrap();
        fs::write(dir.path().join(".grace5").join("first.log"), b"x").unwrap();
        after_memo();
        assert!(check_changed(&path));
        assert!(!check_changed(&path));

        fs::write(dir.path().join(".grace5").join("second.log"), b"y").unwrap();
        after_memo();
        assert!(check_changed(&path));
        assert!(!check_changed(&path));
    }
}
