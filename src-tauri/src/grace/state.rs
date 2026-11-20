//! Display-level state derived from a parsed contract and its records.
//!
//! Authorization and stage mirror the kernel (`grace5/src/contract.ts`,
//! `grace5/src/state.ts`); the explorer never re-verifies verdicts.

use std::collections::HashSet;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::grace::model::{
    GraceApproval, GraceChangeSummary, GraceContract, GraceListResult, GraceRun, GraceRunResult,
};
use crate::grace::parse::{collapse, parse_contract, parse_records, Records};

/// The kernel version this explorer was written against.
const SUPPORTED_PIN: &str = "0.9.0";

/// Kernel `state.ts` CLOSE_RECORD_MISSING.
const CLOSE_RECORD_MISSING: &str =
    "records/close.xml is missing: a change under .grace5/changes/closed/ must carry it";

/// Kernel `state.ts` duplicate-id location defect (also why the closed-side row
/// is flagged when the same id exists open-side).
const DUPLICATE_CHANGE: &str =
    "the change exists under both .grace5/changes/ and .grace5/changes/closed/";

/// Longest rejected statement quoted in an authorization note.
const NOTE_STATEMENT_MAX: usize = 200;

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

/// The approval that governs the contract's current revision: the first record
/// carrying that revision (kernel `records.approvals` `find` order).
fn governing_approval(
    contract_revision: u32,
    approvals: &[GraceApproval],
) -> Option<&GraceApproval> {
    approvals
        .iter()
        .find(|approval| approval.revision == contract_revision)
}

/// The kernel's four authorization values, in its branch order, for the
/// approval that governs the contract's current revision.
pub fn authorization(
    contract_revision: u32,
    contract_hash: &str,
    approvals: &[GraceApproval],
) -> &'static str {
    match governing_approval(contract_revision, approvals) {
        None => "unapproved",
        Some(approval) if approval.decision == "rejected" => "rejected",
        Some(approval) if approval.contract_sha256 != contract_hash => "altered after approval",
        Some(_) => "approved",
    }
}

/// The reason behind the authorization value: the kernel's obligation phrase
/// (`authorizationObligation`) first, then the record context that explains it.
/// `None` when approved — and, because the caller passes no revision for a
/// missing or unparseable contract, the structural-defect banner speaks alone.
fn authorization_note(
    contract_revision: u32,
    contract_hash: &str,
    approvals: &[GraceApproval],
) -> Option<String> {
    let governing = governing_approval(contract_revision, approvals);
    match governing {
        // Kernel branch order: a rejected decision outranks a stale hash.
        Some(approval) if approval.decision == "rejected" => {
            let statement = note_statement(&approval.statement);
            Some(if statement.is_empty() {
                format!("no approval for revision {contract_revision}")
            } else {
                format!("no approval for revision {contract_revision} — rejected: {statement}")
            })
        }
        Some(approval) if approval.contract_sha256 != contract_hash => Some(format!(
            "contract altered after approval ({})",
            approval.file
        )),
        Some(_) => None,
        None => {
            // Kernel scope (state.ts, the last approval that still governs):
            // a reverted contract never cites a later-revision record.
            let last = approvals
                .iter()
                .filter(|approval| approval.revision <= contract_revision)
                .max_by(|left, right| left.revision.cmp(&right.revision));
            Some(match last {
                None => format!("no approval for revision {contract_revision}"),
                Some(approval) => {
                    let date = approval
                        .date
                        .as_deref()
                        .map(|date| format!(" {date}"))
                        .unwrap_or_default();
                    format!(
                        "no approval for revision {contract_revision} — last: {}, {}{date}",
                        approval.file, approval.decision
                    )
                }
            })
        }
    }
}

/// Collapsed statement text, cut at [`NOTE_STATEMENT_MAX`] characters with an
/// ellipsis so a long rejection cannot dominate a table row.
fn note_statement(statement: &str) -> String {
    let statement = collapse(statement);
    if statement.chars().count() <= NOTE_STATEMENT_MAX {
        return statement;
    }
    let mut truncated: String = statement.chars().take(NOTE_STATEMENT_MAX).collect();
    truncated.push('…');
    truncated
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
    let (authorization, note) = match (contract, contract_hash) {
        (Some(contract), Some(hash)) => (
            authorization(contract.revision, hash, &records.approvals),
            authorization_note(contract.revision, hash, &records.approvals),
        ),
        _ => ("unapproved", None),
    };
    let (checks_passed, checks_required) = counts(contract, &records.runs);
    let review = records.reviews.iter().max_by_key(|review| review.n);
    GraceChangeSummary {
        id: id.to_string(),
        title: contract.map(|contract| contract.title.clone()),
        revision: contract.map(|contract| contract.revision),
        stage: stage(records, authorization).to_string(),
        authorization: authorization.to_string(),
        authorization_note: note,
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
        let canonical_grace = grace_dir.canonicalize().ok();
        let canonical_changes = changes_dir.canonicalize().ok();
        let mut open_ids: HashSet<String> = HashSet::new();

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
                let mut location_defects = Vec::new();
                if let Some(defect) =
                    location_defect(canonical_grace.as_deref(), canonical_changes.as_deref(), &dir)
                {
                    location_defects.push(defect);
                }
                open_ids.insert(id.clone());
                changes.push(summarize_change(&id, &dir, false, location_defects));
            }
        }

        let closed_dir = changes_dir.join("closed");
        let canonical_closed = closed_dir.canonicalize().ok();
        for dir in subdirectories(&closed_dir) {
            if let Some(id) = change_id(&dir) {
                let mut location_defects = Vec::new();
                if open_ids.contains(&id) {
                    location_defects.push(DUPLICATE_CHANGE.to_string());
                }
                if let Some(defect) =
                    location_defect(canonical_grace.as_deref(), canonical_closed.as_deref(), &dir)
                {
                    location_defects.push(defect);
                }
                changes.push(summarize_change(&id, &dir, true, location_defects));
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
    /// Location defects in kernel order (close record, duplicate, canonical
    /// location); the first one is the summary's `malformed` when the contract
    /// itself is fine.
    location_defects: Vec<String>,
    records: Records,
    records_defects: Vec<String>,
}

/// Summary only, for the list path (no defects vector).
pub(crate) fn summarize_change(
    id: &str,
    dir: &Path,
    in_closed_dir: bool,
    extra_location_defects: Vec<String>,
) -> GraceChangeSummary {
    summarize_parts(
        id,
        in_closed_dir,
        &read_parts(id, dir, in_closed_dir, extra_location_defects),
    )
}

/// Reads one change directory: contract bytes (hash + parse), records, and the
/// structural defects in kernel order (contract, location, records).
pub(crate) fn load_change(id: &str, dir: &Path, in_closed_dir: bool) -> ChangeLoad {
    let parts = read_parts(id, dir, in_closed_dir, Vec::new());
    let summary = summarize_parts(id, in_closed_dir, &parts);
    let mut defects = Vec::new();
    defects.extend(parts.contract_error.iter().cloned());
    defects.extend(parts.location_defects.iter().cloned());
    defects.extend(parts.records_defects.iter().cloned());
    ChangeLoad {
        summary,
        contract: parts.contract,
        records: parts.records,
        defects,
    }
}

fn read_parts(
    id: &str,
    dir: &Path,
    in_closed_dir: bool,
    extra_location_defects: Vec<String>,
) -> ChangeParts {
    let (contract, contract_hash, contract_error) = read_contract(id, dir);
    let (records, records_defects) = parse_records(dir, id);
    let mut location_defects = Vec::new();
    if in_closed_dir && !records.close_exists {
        location_defects.push(CLOSE_RECORD_MISSING.to_string());
    }
    location_defects.extend(extra_location_defects);
    ChangeParts {
        contract,
        contract_hash,
        contract_error,
        location_defects,
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
        .or_else(|| parts.location_defects.first().cloned())
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

/// The list-time counterpart of `grace_show`'s canonical containment check: a
/// change directory reached through a symlink that resolves outside `.grace5`
/// (or is not directly under its expected parent) is listed but flagged — the
/// detail command refuses it, so the row must not look healthy.
fn location_defect(
    canonical_grace: Option<&Path>,
    expected_parent: Option<&Path>,
    dir: &Path,
) -> Option<String> {
    let (Some(root), Some(expected_parent)) = (canonical_grace, expected_parent) else {
        return None;
    };
    let Ok(resolved) = dir.canonicalize() else {
        return Some(format!(
            "change directory cannot be resolved: {}",
            dir.display()
        ));
    };
    if !resolved.starts_with(root) {
        return Some(format!(
            "change directory resolves outside .grace5: {}",
            dir.display()
        ));
    }
    if resolved.parent() != Some(expected_parent) {
        return Some(format!(
            "change directory is not directly under {}: {}",
            expected_parent.display(),
            dir.display()
        ));
    }
    None
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
    use crate::grace::test_support::{
        symlink_dir, write_contract, write_fixture_change, TempDir,
    };
    use std::fs;

    const KERNEL_VECTOR: &str = "475e960d00ca71cd5b021dd98fd95438fb267b1997d5375ef1313fcbbb168000";
    /// Byte-identical to `nonascii-src.xml` used to compute the expected hash.
    const NON_ASCII_CONTRACT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Contract id="C-0042-unicode" revision="1" title="Café 日本語">
  <Problem>café 日本語</Problem>
</Contract>
"#;
    const NON_ASCII_VECTOR: &str =
        "4dc679d68fb238b7e0fa19c63dc1a750966883d3fce2b7552c799fd4cdce3d48";

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
    fn note_names_the_last_approval_when_the_contract_revision_drifted() {
        let hash = "abc";
        let mut contract = contract_with(Vec::new());
        contract.revision = 7;
        let mut records = empty_records();
        let mut approval = approval_record(6, "approved", hash);
        approval.date = Some("2026-11-20".to_string());
        records.approvals.push(approval);

        let summary = summarize(
            "C-0019-term-checks",
            false,
            Some(&contract),
            Some(hash),
            &records,
            None,
        );

        assert_eq!(summary.authorization, "unapproved");
        assert_eq!(
            summary.authorization_note.as_deref(),
            Some("no approval for revision 7 — last: approval-6.xml, approved 2026-11-20")
        );

        records.approvals[0].date = None;
        let undated = summarize(
            "C-0019-term-checks",
            false,
            Some(&contract),
            Some(hash),
            &records,
            None,
        );
        assert_eq!(
            undated.authorization_note.as_deref(),
            Some("no approval for revision 7 — last: approval-6.xml, approved")
        );
    }

    #[test]
    fn note_ignores_approvals_for_a_later_revision() {
        let hash = "abc";
        let contract = contract_with(Vec::new());
        let mut records = empty_records();
        records
            .approvals
            .push(approval_record(2, "approved", hash));

        let summary = summarize("C-1", false, Some(&contract), Some(hash), &records, None);

        assert_eq!(summary.authorization, "unapproved");
        assert_eq!(
            summary.authorization_note.as_deref(),
            Some("no approval for revision 1")
        );
    }

    #[test]
    fn note_falls_back_to_the_kernel_phrase_for_an_empty_rejection() {
        let hash = "abc";
        let contract = contract_with(Vec::new());
        let mut records = empty_records();
        records
            .approvals
            .push(approval_record(1, "rejected", hash));

        let summary = summarize("C-1", false, Some(&contract), Some(hash), &records, None);

        assert_eq!(summary.authorization, "rejected");
        assert_eq!(
            summary.authorization_note.as_deref(),
            Some("no approval for revision 1")
        );
    }

    #[test]
    fn note_names_a_rejected_statement() {
        let hash = "abc";
        let contract = contract_with(Vec::new());
        let mut records = empty_records();
        let mut approval = approval_record(1, "rejected", hash);
        approval.statement = "The Change does not cover\n  rollback.".to_string();
        records.approvals.push(approval);

        let summary = summarize("C-1", false, Some(&contract), Some(hash), &records, None);

        assert_eq!(summary.authorization, "rejected");
        assert_eq!(
            summary.authorization_note.as_deref(),
            Some("no approval for revision 1 — rejected: The Change does not cover rollback.")
        );
    }

    #[test]
    fn note_truncates_a_long_rejected_statement() {
        let hash = "abc";
        let contract = contract_with(Vec::new());
        let mut records = empty_records();
        let mut approval = approval_record(1, "rejected", hash);
        approval.statement = "a".repeat(210);
        records.approvals.push(approval);

        let summary = summarize("C-1", false, Some(&contract), Some(hash), &records, None);

        assert_eq!(
            summary.authorization_note.as_deref(),
            Some(format!("no approval for revision 1 — rejected: {}…", "a".repeat(200)).as_str())
        );
    }

    #[test]
    fn note_names_the_approval_whose_contract_hash_no_longer_matches() {
        let hash = "abc";
        let contract = contract_with(Vec::new());
        let mut records = empty_records();
        records
            .approvals
            .push(approval_record(1, "approved", "other"));

        let summary = summarize("C-1", false, Some(&contract), Some(hash), &records, None);

        assert_eq!(summary.authorization, "altered after approval");
        assert_eq!(
            summary.authorization_note.as_deref(),
            Some("contract altered after approval (approval-1.xml)")
        );
    }

    #[test]
    fn note_is_the_plain_kernel_phrase_when_no_approval_exists() {
        let contract = contract_with(Vec::new());
        let records = empty_records();

        let summary = summarize("C-1", false, Some(&contract), Some("abc"), &records, None);

        assert_eq!(summary.authorization, "unapproved");
        assert_eq!(
            summary.authorization_note.as_deref(),
            Some("no approval for revision 1")
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
        assert!(summary.authorization_note.is_none());
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
        assert!(summary.authorization_note.is_none());
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
        // Listed AND flagged: the resolved directory escapes .grace5, and the
        // detail command refuses it — the row must not look healthy.
        let entry = result
            .changes
            .iter()
            .find(|change| change.id == "C-0050-symlinked")
            .unwrap();
        assert!(
            entry
                .malformed
                .as_deref()
                .unwrap_or_default()
                .contains("outside .grace5"),
            "{:?}",
            entry.malformed
        );
    }

    #[test]
    fn flags_a_change_present_in_both_directories() {
        let dir = TempDir::new("duplicate");
        let changes = dir.path().join(".grace5").join("changes");
        let contract = br#"<Contract id="C-0070-duplicate" revision="1" title="Duplicate"><Problem>p</Problem></Contract>"#;
        write_contract(&changes.join("C-0070-duplicate"), contract);
        let closed = changes.join("closed").join("C-0070-duplicate");
        write_contract(&closed, contract);
        fs::create_dir_all(closed.join("records")).unwrap();
        fs::write(
            closed.join("records").join("close.xml"),
            br#"<Close change="C-0070-duplicate" revision="1" contract-sha256="x" date="2026-11-20" source-revision="a" candidate="b" start-revision="c" approval="approval-1.xml" review="1" kernel="grace5 0.9.0"/>"#,
        )
        .unwrap();

        let result = list(dir.path());
        let rows: Vec<_> = result
            .changes
            .iter()
            .filter(|change| change.id == "C-0070-duplicate")
            .collect();
        // Malformed never hides a row: both sides stay listed.
        assert_eq!(rows.len(), 2);
        let closed_row = rows.iter().find(|change| change.in_closed_dir).expect("closed row");
        assert_eq!(
            closed_row.malformed.as_deref(),
            Some("the change exists under both .grace5/changes/ and .grace5/changes/closed/")
        );
        let open_row = rows.iter().find(|change| !change.in_closed_dir).expect("open row");
        assert!(open_row.malformed.is_none(), "{:?}", open_row.malformed);
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
