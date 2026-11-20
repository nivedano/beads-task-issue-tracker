//! Display-level state derived from a parsed contract and its records.
//!
//! Authorization and stage mirror the kernel (`grace5/src/contract.ts`,
//! `grace5/src/state.ts`); the explorer never re-verifies verdicts.

use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::grace::model::{
    GraceApproval, GraceChangeSummary, GraceContract, GraceListResult, GraceRun, GraceRunResult,
};
use crate::grace::parse::{parse_contract, parse_records, Records};

/// The kernel version this explorer was written against.
const SUPPORTED_PIN: &str = "0.9.0";

/// Kernel `state.ts` CLOSE_RECORD_MISSING.
const CLOSE_RECORD_MISSING: &str =
    "records/close.xml is missing: a change under .grace5/changes/closed/ must carry it";

/// SHA-256 of the raw contract bytes with only `\r\n` pairs read as `\n`
/// (`authorization.md`; never decodes the bytes to a string first).
pub fn contract_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
            hasher.update(b"\n");
            index += 2;
        } else {
            hasher.update([bytes[index]]);
            index += 1;
        }
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// The kernel's four authorization values, in its branch order, for the
/// approval that governs the contract's current revision.
pub fn authorization(
    contract_revision: u32,
    contract_hash: &str,
    approvals: &[GraceApproval],
) -> &'static str {
    match approvals
        .iter()
        .find(|approval| approval.revision == contract_revision)
    {
        None => "unapproved",
        Some(approval) if approval.decision == "rejected" => "rejected",
        Some(approval) if approval.contract_sha256 != contract_hash => "altered after approval",
        Some(_) => "approved",
    }
}

/// Display-level summary of one change directory.
pub fn summarize(
    id: &str,
    in_closed_dir: bool,
    contract: Option<&GraceContract>,
    contract_hash: Option<&str>,
    records: &Records,
    malformed: Option<String>,
) -> GraceChangeSummary {
    let authorization = match (contract, contract_hash) {
        (Some(contract), Some(hash)) => authorization(contract.revision, hash, &records.approvals),
        _ => "unapproved",
    };
    let (checks_passed, checks_required) = counts(contract, &records.runs);
    let review = records.reviews.iter().max_by_key(|review| review.n);
    GraceChangeSummary {
        id: id.to_string(),
        title: contract.map(|contract| contract.title.clone()),
        revision: contract.map(|contract| contract.revision),
        stage: stage(records, authorization).to_string(),
        authorization: authorization.to_string(),
        malformed,
        in_closed_dir,
        closed: records.close_exists,
        checks_passed,
        checks_required,
        review_status: review.map(|review| review.status.clone()),
        review_open_findings: review
            .map(|review| {
                review
                    .findings
                    .iter()
                    .filter(|finding| finding.status == "open")
                    .count() as u32
            })
            .unwrap_or(0),
        problem: contract.map(|contract| contract.problem.clone()),
    }
}

/// Lists every change under `.grace5/changes/` and `.grace5/changes/closed/`.
/// A malformed change is flagged and never hides a valid one.
pub fn list(path: &Path) -> GraceListResult {
    let grace_dir = path.join(".grace5");
    let has_grace = grace_dir.is_dir();
    let pin = fs::read_to_string(grace_dir.join("kernel-version"))
        .ok()
        .map(|text| text.trim().to_string())
        .filter(|pin| !pin.is_empty());
    let pin_supported = pin.as_deref() == Some(SUPPORTED_PIN);

    let mut changes = Vec::new();
    if has_grace {
        let changes_dir = grace_dir.join("changes");
        for dir in subdirectories(&changes_dir) {
            let is_closed_dir = dir
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name == "closed")
                .unwrap_or(false);
            if is_closed_dir {
                continue;
            }
            if let Some(id) = change_id(&dir) {
                changes.push(summarize_change(&id, &dir, false));
            }
        }
        for dir in subdirectories(&changes_dir.join("closed")) {
            if let Some(id) = change_id(&dir) {
                changes.push(summarize_change(&id, &dir, true));
            }
        }
    }
    changes.sort_by(|left, right| left.id.cmp(&right.id));

    // Scope semantics: a change on the closed side is closed for filtering even
    // when its close record is missing (that case is flagged malformed).
    let closed_count = changes
        .iter()
        .filter(|change| change.closed || change.in_closed_dir)
        .count() as u32;
    let open_count = changes.len() as u32 - closed_count;
    GraceListResult {
        has_grace,
        pin,
        pin_supported,
        open_count,
        closed_count,
        changes,
    }
}

/// Stage is lifecycle progress, not authority: the first rung that holds wins.
fn stage(records: &Records, authorization: &str) -> &'static str {
    if records.close_exists {
        "closed"
    } else if records
        .reviews
        .iter()
        .any(|review| review.status == "completed")
    {
        "reviewed"
    } else if !records.runs.is_empty() {
        "executing"
    } else if authorization == "approved" {
        "approved"
    } else {
        "prepared"
    }
}

/// Kernel `checkState`: the highest-numbered run containing the check decides it.
pub(crate) fn latest_result<'a>(
    check: &str,
    runs: &'a [GraceRun],
) -> Option<&'a GraceRunResult> {
    runs.iter()
        .filter_map(|run| {
            run.results
                .iter()
                .find(|result| result.check == check)
                .map(|result| (run.n, result))
        })
        .max_by_key(|(n, _)| *n)
        .map(|(_, result)| result)
}

fn counts(contract: Option<&GraceContract>, runs: &[GraceRun]) -> (u32, u32) {
    let Some(contract) = contract else {
        return (0, 0);
    };
    let required: Vec<_> = contract
        .checks
        .iter()
        .filter(|check| check.required)
        .collect();
    let mut passed = 0;
    for check in &required {
        if latest_result(&check.id, runs)
            .map(|result| result.verdict == "passed")
            .unwrap_or(false)
        {
            passed += 1;
        }
    }
    (passed, required.len() as u32)
}

/// Contract, records, structural defects and summary for one change directory.
pub(crate) struct ChangeLoad {
    pub summary: GraceChangeSummary,
    pub contract: Option<GraceContract>,
    pub records: Records,
    pub defects: Vec<String>,
}

struct ChangeParts {
    contract: Option<GraceContract>,
    contract_hash: Option<String>,
    contract_error: Option<String>,
    location_error: Option<String>,
    records: Records,
    records_defects: Vec<String>,
}

/// Summary only, for the list path (no defects vector).
pub(crate) fn summarize_change(id: &str, dir: &Path, in_closed_dir: bool) -> GraceChangeSummary {
    summarize_parts(id, in_closed_dir, &read_parts(id, dir, in_closed_dir))
}

/// Reads one change directory: contract bytes (hash + parse), records, and the
/// structural defects in kernel order (contract, location, records).
pub(crate) fn load_change(id: &str, dir: &Path, in_closed_dir: bool) -> ChangeLoad {
    let parts = read_parts(id, dir, in_closed_dir);
    let summary = summarize_parts(id, in_closed_dir, &parts);
    let mut defects = Vec::new();
    defects.extend(parts.contract_error.iter().cloned());
    defects.extend(parts.location_error.iter().cloned());
    defects.extend(parts.records_defects.iter().cloned());
    ChangeLoad {
        summary,
        contract: parts.contract,
        records: parts.records,
        defects,
    }
}

fn read_parts(id: &str, dir: &Path, in_closed_dir: bool) -> ChangeParts {
    let (contract, contract_hash, contract_error) = read_contract(id, dir);
    let (records, records_defects) = parse_records(dir, id);
    let location_error = if in_closed_dir && !records.close_exists {
        Some(CLOSE_RECORD_MISSING.to_string())
    } else {
        None
    };
    ChangeParts {
        contract,
        contract_hash,
        contract_error,
        location_error,
        records,
        records_defects,
    }
}

fn summarize_parts(
    id: &str,
    in_closed_dir: bool,
    parts: &ChangeParts,
) -> GraceChangeSummary {
    let malformed = parts
        .contract_error
        .clone()
        .or_else(|| parts.location_error.clone())
        .or_else(|| parts.records_defects.first().cloned());
    summarize(
        id,
        in_closed_dir,
        parts.contract.as_ref(),
        parts.contract_hash.as_deref(),
        &parts.records,
        malformed,
    )
}

fn read_contract(
    id: &str,
    dir: &Path,
) -> (Option<GraceContract>, Option<String>, Option<String>) {
    match fs::read(dir.join("contract.xml")) {
        Ok(bytes) => {
            let hash = contract_sha256(&bytes);
            match parse_contract(&bytes, id) {
                Ok(contract) => (Some(contract), Some(hash), None),
                Err(error) => (None, Some(hash), Some(error)),
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (
            None,
            None,
            Some("contract.xml is missing".to_string()),
        ),
        Err(error) => (
            None,
            None,
            Some(format!("contract.xml cannot be read: {error}")),
        ),
    }
}

fn change_id(dir: &Path) -> Option<String> {
    dir.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
}

fn subdirectories(dir: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            // Follow symlinks, as the kernel's statSync does.
            if fs::metadata(&path)
                .map(|metadata| metadata.is_dir())
                .unwrap_or(false)
            {
                paths.push(path);
            }
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grace::model::{
        GraceAffected, GraceCheck, GraceFinding, GraceReview, GraceRunResult,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    const KERNEL_VECTOR: &str = "475e960d00ca71cd5b021dd98fd95438fb267b1997d5375ef1313fcbbb168000";
    /// Byte-identical to `nonascii-src.xml` used to compute the expected hash.
    const NON_ASCII_CONTRACT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Contract id="C-0042-unicode" revision="1" title="Café 日本語">
  <Problem>café 日本語</Problem>
</Contract>
"#;
    const NON_ASCII_VECTOR: &str =
        "4dc679d68fb238b7e0fa19c63dc1a750966883d3fce2b7552c799fd4cdce3d48";

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
                "grace-state-{label}-{}-{nanos}-{}",
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

    fn empty_records() -> Records {
        Records {
            approvals: Vec::new(),
            runs: Vec::new(),
            reviews: Vec::new(),
            close: None,
            close_exists: false,
            briefs: Vec::new(),
            log_dirs: Vec::new(),
        }
    }

    fn approval_record(revision: u32, decision: &str, contract_sha256: &str) -> GraceApproval {
        GraceApproval {
            file: format!("approval-{revision}.xml"),
            revision,
            contract_sha256: contract_sha256.to_string(),
            brief_sha256: None,
            date: None,
            decision: decision.to_string(),
            statement: String::new(),
            bound: Vec::new(),
        }
    }

    fn check(id: &str, required: bool) -> GraceCheck {
        GraceCheck {
            id: id.to_string(),
            kind: "automated".to_string(),
            required,
            environment: None,
            cwd: None,
            budget: None,
            command: None,
            procedure: None,
            expects: Vec::new(),
            forbids: Vec::new(),
            origin: None,
            log: None,
        }
    }

    fn contract_with(checks: Vec<GraceCheck>) -> GraceContract {
        GraceContract {
            id: "C-0001-test".to_string(),
            revision: 1,
            title: "Test change".to_string(),
            problem: "The problem".to_string(),
            change: String::new(),
            constraints: Vec::new(),
            non_goals: Vec::new(),
            acceptance: Vec::new(),
            affected: GraceAffected {
                paths: Vec::new(),
                names: Vec::new(),
                protected: Vec::new(),
            },
            environments: Vec::new(),
            checks,
            decisions: Vec::new(),
        }
    }

    fn result(check: &str, verdict: &str) -> GraceRunResult {
        GraceRunResult {
            check: check.to_string(),
            kind: "automated".to_string(),
            required: true,
            definition_sha256: String::new(),
            definition: String::new(),
            verdict: verdict.to_string(),
            claim: None,
            cwd: None,
            exit: None,
            elapsed_ms: None,
            log: None,
            log_sha256: None,
            environment: None,
            trace: None,
            observation: None,
            reason: None,
        }
    }

    fn run(n: u32, results: Vec<GraceRunResult>) -> GraceRun {
        GraceRun {
            file: format!("run-{n}.xml"),
            n,
            run_id: String::new(),
            started: String::new(),
            ended: String::new(),
            status: "completed".to_string(),
            start: false,
            source_revision: String::new(),
            dirty: false,
            candidate: String::new(),
            kernel: String::new(),
            revision: 1,
            contract_sha256: String::new(),
            preexisting: Vec::new(),
            results,
        }
    }

    fn review_record(n: u32, status: &str, findings: Vec<GraceFinding>) -> GraceReview {
        GraceReview {
            file: format!("review-{n}.xml"),
            n,
            revision: 1,
            contract_sha256: String::new(),
            status: status.to_string(),
            independence: String::new(),
            start_revision: String::new(),
            end_revision: String::new(),
            candidate: String::new(),
            findings,
            adequacy: Vec::new(),
            rules: Vec::new(),
        }
    }

    fn write_contract(dir: &Path, src: &[u8]) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("contract.xml"), src).unwrap();
    }

    fn write_fixture_change(dir: &Path) {
        fs::create_dir_all(dir.join("records")).unwrap();
        fs::write(
            dir.join("contract.xml"),
            include_bytes!("fixtures/C-0019-retire-briefs/contract.xml"),
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
            fs::write(dir.join("records").join(name), bytes).unwrap();
        }
    }

    #[test]
    fn hashes_the_fixture_like_the_kernel() {
        let bytes = include_bytes!("fixtures/C-0019-retire-briefs/contract.xml");
        assert_eq!(contract_sha256(bytes), KERNEL_VECTOR);
    }

    #[test]
    fn hashes_crlf_bytes_like_lf_bytes() {
        let lf = include_bytes!("fixtures/C-0019-retire-briefs/contract.xml");
        let mut crlf = Vec::with_capacity(lf.len() + lf.len() / 8);
        for byte in lf.iter() {
            if *byte == b'\n' {
                crlf.extend_from_slice(b"\r\n");
            } else {
                crlf.push(*byte);
            }
        }
        assert!(crlf.len() > lf.len());
        assert_eq!(contract_sha256(&crlf), contract_sha256(lf));
    }

    #[test]
    fn hashes_non_ascii_contract_text() {
        assert_eq!(contract_sha256(NON_ASCII_CONTRACT.as_bytes()), NON_ASCII_VECTOR);
    }

    #[test]
    fn authorization_follows_the_kernel_branch_order() {
        let hash = "abc";
        assert_eq!(authorization(1, hash, &[]), "unapproved");
        assert_eq!(
            authorization(1, hash, &[approval_record(2, "approved", hash)]),
            "unapproved"
        );
        assert_eq!(
            authorization(1, hash, &[approval_record(1, "rejected", hash)]),
            "rejected"
        );
        assert_eq!(
            authorization(1, hash, &[approval_record(1, "rejected", "other")]),
            "rejected"
        );
        assert_eq!(
            authorization(1, hash, &[approval_record(1, "approved", "other")]),
            "altered after approval"
        );
        assert_eq!(
            authorization(1, hash, &[approval_record(1, "approved", hash)]),
            "approved"
        );
    }

    #[test]
    fn stage_follows_the_ladder() {
        let hash = "abc";
        let contract = contract_with(Vec::new());
        let mut records = empty_records();
        assert_eq!(
            summarize("C-1", false, Some(&contract), Some(hash), &records, None).stage,
            "prepared"
        );

        records
            .approvals
            .push(approval_record(1, "rejected", hash));
        let rejected = summarize("C-1", false, Some(&contract), Some(hash), &records, None);
        assert_eq!(rejected.authorization, "rejected");
        assert_eq!(rejected.stage, "prepared");

        records.approvals.clear();
        records.approvals.push(approval_record(1, "approved", hash));
        let approved = summarize("C-1", false, Some(&contract), Some(hash), &records, None);
        assert_eq!(approved.authorization, "approved");
        assert_eq!(approved.stage, "approved");

        records.runs.push(run(1, Vec::new()));
        assert_eq!(
            summarize("C-1", false, Some(&contract), Some(hash), &records, None).stage,
            "executing"
        );

        records.reviews.push(review_record(1, "completed", Vec::new()));
        assert_eq!(
            summarize("C-1", false, Some(&contract), Some(hash), &records, None).stage,
            "reviewed"
        );

        records.close_exists = true;
        assert_eq!(
            summarize("C-1", true, Some(&contract), Some(hash), &records, None).stage,
            "closed"
        );
    }

    #[test]
    fn counts_only_passed_required_checks_from_the_latest_run_containing_each() {
        let contract = contract_with(vec![
            check("CK1", true),
            check("CK6", true),
            check("CK9", false),
        ]);
        let mut records = empty_records();
        records.runs.push(run(
            1,
            vec![result("CK1", "passed"), result("CK6", "failed")],
        ));
        records.runs.push(run(
            3,
            vec![result("CK6", "passed"), result("CK9", "passed")],
        ));
        let summary = summarize("C-1", false, Some(&contract), Some("h"), &records, None);
        assert_eq!(summary.checks_passed, 2);
        assert_eq!(summary.checks_required, 2);

        records.runs.push(run(5, vec![result("CK1", "failed")]));
        let summary = summarize("C-1", false, Some(&contract), Some("h"), &records, None);
        assert_eq!(summary.checks_passed, 1);
        assert_eq!(summary.checks_required, 2);
    }

    #[test]
    fn summarizes_the_fixture_change() {
        let dir = TempDir::new("summary");
        let change = dir.path().join("C-0019-retire-briefs");
        write_fixture_change(&change);
        let bytes = fs::read(change.join("contract.xml")).unwrap();
        let hash = contract_sha256(&bytes);
        let contract = parse_contract(&bytes, "C-0019-retire-briefs").unwrap();
        let (records, defects) = parse_records(&change, "C-0019-retire-briefs");
        assert!(defects.is_empty(), "{defects:?}");

        let summary = summarize(
            "C-0019-retire-briefs",
            true,
            Some(&contract),
            Some(&hash),
            &records,
            None,
        );
        assert_eq!(summary.stage, "closed");
        assert_eq!(summary.authorization, "approved");
        assert!(summary.closed);
        assert!(summary.in_closed_dir);
        assert_eq!(summary.revision, Some(1));
        assert_eq!(summary.checks_passed, 8);
        assert_eq!(summary.checks_required, 8);
        assert_eq!(summary.review_status.as_deref(), Some("completed"));
        assert_eq!(summary.review_open_findings, 2);
        assert!(summary.malformed.is_none());
        assert!(summary
            .problem
            .as_deref()
            .unwrap_or_default()
            .starts_with("The HTML brief was introduced"));
    }

    #[test]
    fn lists_valid_malformed_and_closed_changes() {
        let dir = TempDir::new("list");
        let root = dir.path();
        let changes = root.join(".grace5").join("changes");
        let valid = changes.join("C-0019-retire-briefs");
        write_contract(
            &valid,
            include_bytes!("fixtures/C-0019-retire-briefs/contract.xml"),
        );
        let broken = changes.join("C-0042-broken");
        write_contract(
            &broken,
            br#"<Contract id="C-0042-broken" revision="1"><Problem>p</Problem></Contract>"#,
        );
        let closed = changes.join("closed").join("C-0043-closed");
        write_contract(
            &closed,
            br#"<Contract id="C-0043-closed" revision="1" title="Closed change"><Problem>p</Problem></Contract>"#,
        );
        fs::create_dir_all(closed.join("records")).unwrap();
        fs::write(
            closed.join("records").join("close.xml"),
            br#"<Close change="C-0043-closed" revision="1" contract-sha256="x" date="2026-11-20" source-revision="a" candidate="b" start-revision="c" approval="approval-1.xml" review="1" kernel="grace5 0.9.0"/>"#,
        )
        .unwrap();

        let result = list(root);
        assert!(result.has_grace);
        assert_eq!(result.changes.len(), 3);
        assert_eq!(result.open_count, 2);
        assert_eq!(result.closed_count, 1);

        let find = |id: &str| result.changes.iter().find(|change| change.id == id).unwrap();
        let valid = find("C-0019-retire-briefs");
        assert!(valid.malformed.is_none(), "{:?}", valid.malformed);
        assert_eq!(
            valid.title.as_deref(),
            Some("The contract is the approval surface; the separate brief is retired")
        );
        assert_eq!(valid.stage, "prepared");
        assert!(!valid.closed);
        assert!(!valid.in_closed_dir);

        let broken = find("C-0042-broken");
        assert!(broken
            .malformed
            .as_deref()
            .unwrap_or_default()
            .contains("title"));
        assert!(broken.title.is_none());
        assert_eq!(broken.stage, "prepared");

        let closed = find("C-0043-closed");
        assert!(closed.closed);
        assert!(closed.in_closed_dir);
        assert_eq!(closed.stage, "closed");
        assert!(closed.malformed.is_none(), "{:?}", closed.malformed);
    }

    #[test]
    fn flags_a_change_under_closed_without_close_xml() {
        let dir = TempDir::new("no-close");
        let root = dir.path();
        let change = root
            .join(".grace5")
            .join("changes")
            .join("closed")
            .join("C-0044-no-close");
        write_contract(
            &change,
            br#"<Contract id="C-0044-no-close" revision="1" title="No close"><Problem>p</Problem></Contract>"#,
        );

        let result = list(root);
        let summary = result.changes.first().expect("change is listed");
        // The change is not closed (no close record), but it lives on the closed side.
        assert!(!summary.closed);
        assert!(summary.in_closed_dir);
        assert_eq!(summary.stage, "prepared");
        assert!(summary
            .malformed
            .as_deref()
            .unwrap_or_default()
            .contains("close.xml"));
        assert_eq!(result.open_count, 0);
        assert_eq!(result.closed_count, 1);
    }

    #[test]
    fn flags_a_change_without_a_contract() {
        let dir = TempDir::new("no-contract");
        let root = dir.path();
        let change = root
            .join(".grace5")
            .join("changes")
            .join("C-0045-no-contract");
        fs::create_dir_all(change.join("records")).unwrap();
        fs::write(
            change.join("records").join("run-1.xml"),
            br#"<Run change="C-0045-no-contract" revision="1" contract-sha256="x" n="1" run-id="r" started="" ended="" status="completed" start="true" source-revision="" dirty="false" candidate="" kernel="grace5 0.9.0"/>"#,
        )
        .unwrap();

        let result = list(root);
        let summary = result.changes.first().expect("change is listed");
        assert!(summary.title.is_none());
        assert!(summary.revision.is_none());
        assert_eq!(summary.authorization, "unapproved");
        assert_eq!(summary.stage, "executing");
        assert_eq!(
            summary.malformed.as_deref(),
            Some("contract.xml is missing")
        );
    }

    #[test]
    fn flags_a_contract_that_cannot_be_read() {
        let dir = TempDir::new("unreadable-contract");
        let root = dir.path();
        let change = root
            .join(".grace5")
            .join("changes")
            .join("C-0046-unreadable-contract");
        fs::create_dir_all(change.join("contract.xml")).unwrap();

        let result = list(root);
        let summary = result.changes.first().expect("change is listed");
        let malformed = summary.malformed.as_deref().unwrap_or_default();
        assert!(
            malformed.starts_with("contract.xml cannot be read"),
            "{malformed}"
        );
    }

    #[cfg(unix)]
    fn symlink_dir(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn symlink_dir(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(target, link)
    }

    #[test]
    fn lists_a_change_directory_reached_through_a_symlink() {
        let dir = TempDir::new("symlinked-change");
        let root = dir.path();
        let changes = root.join(".grace5").join("changes");
        fs::create_dir_all(&changes).unwrap();
        let target = root.join("C-0050-symlinked");
        write_contract(
            &target,
            br#"<Contract id="C-0050-symlinked" revision="1" title="Linked"><Problem>p</Problem></Contract>"#,
        );
        let link = changes.join("C-0050-symlinked");
        if symlink_dir(&target, &link).is_err() {
            eprintln!("skipping: this platform cannot create a directory symlink here");
            return;
        }

        let result = list(root);
        assert_eq!(
            result
                .changes
                .iter()
                .filter(|change| change.id == "C-0050-symlinked")
                .count(),
            1
        );
    }

    #[test]
    fn reports_the_kernel_pin() {
        let dir = TempDir::new("pin");
        let root = dir.path();
        fs::create_dir_all(root.join(".grace5")).unwrap();

        let missing = list(root);
        assert_eq!(missing.pin, None);
        assert!(!missing.pin_supported);

        fs::write(root.join(".grace5").join("kernel-version"), " 0.9.0\n").unwrap();
        let supported = list(root);
        assert_eq!(supported.pin.as_deref(), Some("0.9.0"));
        assert!(supported.pin_supported);

        fs::write(root.join(".grace5").join("kernel-version"), "0.8.0").unwrap();
        let unsupported = list(root);
        assert_eq!(unsupported.pin.as_deref(), Some("0.8.0"));
        assert!(!unsupported.pin_supported);
    }

    #[test]
    fn lists_nothing_without_grace() {
        let dir = TempDir::new("empty");
        let result = list(dir.path());
        assert!(!result.has_grace);
        assert!(result.changes.is_empty());
        assert_eq!(result.open_count, 0);
        assert_eq!(result.closed_count, 0);
        assert_eq!(result.pin, None);
        assert!(!result.pin_supported);
    }
}
