//! Structural parsing of `.grace5/changes/<dir>/contract.xml`.
//!
//! Validation is structural only: XML parse failure, a root element that is not
//! `Contract`, a missing or empty `id`/`revision`/`title`, and an `id` that
//! disagrees with the change directory. Every section is read leniently, and
//! `grace5 check` remains authoritative for semantic defects.

use std::fs;
use std::path::Path;

use roxmltree::{Document, Node};

use super::model::{
    GraceAdequacy, GraceAffected, GraceApproval, GraceBound, GraceBrief, GraceCheck, GraceClose,
    GraceConstraint, GraceContract, GraceCriterion, GraceEnvironment, GraceExpect, GraceFinding,
    GraceItem, GraceNotRequired, GraceObservation, GraceReview, GraceRuleVerdict, GraceRun,
    GraceRunResult, GraceSatisfied, GraceTrace,
};

/// The XSD `collapse` whitespace facet: trimmed, inner runs reduced to one space.
pub fn collapse(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut pending_space = false;
    for ch in value.chars() {
        if matches!(ch, ' ' | '\t' | '\n' | '\r') {
            pending_space = !out.is_empty();
        } else {
            if pending_space {
                out.push(' ');
                pending_space = false;
            }
            out.push(ch);
        }
    }
    out
}

/// Every descendant text node in document order, concatenated, then collapsed.
pub fn element_text(el: &Node) -> String {
    let raw: String = el
        .descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect();
    collapse(&raw)
}

/// Parses the bytes of one change's `contract.xml`.
pub fn parse_contract(bytes: &[u8], dir_name: &str) -> Result<GraceContract, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| format!("contract.xml is not valid UTF-8: {error}"))?;
    let document = Document::parse(text)
        .map_err(|error| format!("contract.xml is not well-formed XML: {error}"))?;
    let root = document.root_element();
    if root.tag_name().name() != "Contract" {
        return Err(format!(
            "contract.xml root element is <{}>, expected <Contract>",
            root.tag_name().name()
        ));
    }

    let id = attribute(root, "id").unwrap_or_default();
    if id.is_empty() {
        return Err("contract.xml is missing the id attribute".to_string());
    }
    let revision_text = attribute(root, "revision").unwrap_or_default();
    if revision_text.is_empty() {
        return Err("contract.xml is missing the revision attribute".to_string());
    }
    let revision = revision_text.parse::<u32>().map_err(|_| {
        format!("contract.xml revision \"{revision_text}\" is not an unsigned number")
    })?;
    let title = attribute(root, "title").unwrap_or_default();
    if title.is_empty() {
        return Err("contract.xml is missing the title attribute".to_string());
    }
    if id != dir_name {
        return Err(format!(
            "contract id \"{id}\" does not match change directory \"{dir_name}\""
        ));
    }

    let checks_root = child_named(root, "Checks");
    let environments = children_named(checks_root.unwrap_or(root), "Environment")
        .into_iter()
        .map(|node| GraceEnvironment {
            id: attribute(node, "id").unwrap_or_default(),
            device: attribute(node, "device"),
            build: attribute(node, "build"),
            text: element_text(&node),
        })
        .collect();
    let checks = checks_root
        .map(|checks| {
            checks
                .children()
                .filter(|child| {
                    child.is_element()
                        && matches!(child.tag_name().name(), "Automated" | "Trace" | "Manual")
                })
                .map(build_check)
                .collect()
        })
        .unwrap_or_default();

    Ok(GraceContract {
        id,
        revision,
        title,
        problem: text_of(root, "Problem"),
        change: text_of(root, "Change"),
        constraints: section(root, "Constraints", "Constraint")
            .into_iter()
            .map(|node| GraceConstraint {
                id: attribute(node, "id").unwrap_or_default(),
                text: element_text(&node),
                rule: attribute(node, "rule"),
            })
            .collect(),
        non_goals: section(root, "NonGoals", "NonGoal")
            .into_iter()
            .map(item)
            .collect(),
        acceptance: section(root, "Acceptance", "Criterion")
            .into_iter()
            .map(|node| GraceCriterion {
                id: attribute(node, "id").unwrap_or_default(),
                text: element_text(&node),
                checks: attribute(node, "checks")
                    .unwrap_or_default()
                    .split(' ')
                    .filter(|token| !token.is_empty())
                    .map(str::to_string)
                    .collect(),
            })
            .collect(),
        affected: GraceAffected {
            paths: texts(root, "Affected", "Path"),
            names: texts(root, "Affected", "Name"),
            protected: texts(root, "Affected", "Protected"),
        },
        environments,
        checks,
        decisions: section(root, "Decisions", "Decision")
            .into_iter()
            .map(item)
            .collect(),
    })
}

fn build_check(node: Node) -> GraceCheck {
    let kind = node.tag_name().name().to_ascii_lowercase();
    let manual = kind == "manual";
    let trace = kind == "trace";
    GraceCheck {
        id: attribute(node, "id").unwrap_or_default(),
        environment: attribute(node, "environment"),
        cwd: if manual { None } else { attribute(node, "cwd") },
        budget: number(node.attribute("budget")),
        command: child_named(node, "Command").map(|child| element_text(&child)),
        procedure: child_named(node, "Procedure").map(|child| element_text(&child)),
        expects: children_named(node, "Expect")
            .into_iter()
            .map(|child| GraceExpect {
                text: element_text(&child),
                min: number(child.attribute("min")).unwrap_or(1),
                max: number(child.attribute("max")),
            })
            .collect(),
        forbids: children_named(node, "Forbid")
            .into_iter()
            .map(|child| element_text(&child))
            .collect(),
        origin: if trace { attribute(node, "origin") } else { None },
        log: if trace { attribute(node, "log") } else { None },
        required: xs_boolean(node.attribute("required"), true),
        kind,
    }
}

fn item(node: Node) -> GraceItem {
    GraceItem {
        id: attribute(node, "id").unwrap_or_default(),
        text: element_text(&node),
    }
}

fn child_named<'a, 'input>(parent: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    parent
        .children()
        .find(|child| child.is_element() && child.tag_name().name() == name)
}

fn children_named<'a, 'input>(parent: Node<'a, 'input>, name: &str) -> Vec<Node<'a, 'input>> {
    parent
        .children()
        .filter(|child| child.is_element() && child.tag_name().name() == name)
        .collect()
}

fn section<'a, 'input>(
    root: Node<'a, 'input>,
    section: &str,
    child: &str,
) -> Vec<Node<'a, 'input>> {
    match child_named(root, section) {
        Some(parent) => children_named(parent, child),
        None => Vec::new(),
    }
}

fn texts(root: Node, section_name: &str, child: &str) -> Vec<String> {
    section(root, section_name, child)
        .into_iter()
        .map(|node| element_text(&node))
        .collect()
}

fn text_of(root: Node, name: &str) -> String {
    child_named(root, name)
        .map(|node| element_text(&node))
        .unwrap_or_default()
}

fn attribute(node: Node, name: &str) -> Option<String> {
    node.attribute(name).map(collapse)
}

fn number(value: Option<&str>) -> Option<u64> {
    value.and_then(|raw| collapse(raw).parse().ok())
}

/// The `xs:boolean` lexical space; anything but `true`/`1` is false.
fn xs_boolean(value: Option<&str>, fallback: bool) -> bool {
    match value {
        Some(raw) => matches!(raw.trim(), "true" | "1"),
        None => fallback,
    }
}

/// Everything found under one change's `records/` directory.
pub struct Records {
    pub approvals: Vec<GraceApproval>,
    pub runs: Vec<GraceRun>,
    pub reviews: Vec<GraceReview>,
    pub close: Option<GraceClose>,
    pub close_exists: bool,
    pub briefs: Vec<GraceBrief>,
    pub log_dirs: Vec<u32>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RecordSchema {
    Approval,
    Run,
    Review,
    Close,
}

struct PendingRecord {
    file: String,
    schema: RecordSchema,
    number: Option<u32>,
}

/// Reads `records/` of one change. A record that fails the structural checks is
/// reported in the second value and left out of the model.
pub fn parse_records(change_dir: &Path, change_id: &str) -> (Records, Vec<String>) {
    let records_dir = change_dir.join("records");
    let mut defects = Vec::new();
    let mut pending = Vec::new();
    let mut briefs = Vec::new();
    let mut close_exists = false;

    let mut names = file_names(&records_dir);
    names.sort();
    for file in names {
        if let Some(revision) = brief_revision(&file) {
            briefs.push(GraceBrief { revision, file });
            continue;
        }
        if !file.ends_with(".xml") {
            continue;
        }
        if file == "close.xml" {
            close_exists = true;
            pending.push(PendingRecord {
                file,
                schema: RecordSchema::Close,
                number: None,
            });
            continue;
        }
        match record_name(&file) {
            Some((schema, number)) => pending.push(PendingRecord {
                file,
                schema,
                number: Some(number),
            }),
            None => defects.push(format!("{file}: not a record name the method defines")),
        }
    }

    let mut approvals = Vec::new();
    let mut runs = Vec::new();
    let mut reviews = Vec::new();
    let mut close = None;
    for record in pending {
        let file = record.file.as_str();
        let text = match fs::read_to_string(records_dir.join(file)) {
            Ok(text) => text,
            Err(error) => {
                defects.push(format!("{file}: {error}"));
                continue;
            }
        };
        let document = match Document::parse(&text) {
            Ok(document) => document,
            Err(error) => {
                defects.push(format!("{file}: {error}"));
                continue;
            }
        };
        let root = document.root_element();
        let named = attribute(root, "change").unwrap_or_default();
        if named != change_id {
            defects.push(format!("{file}: names change {named}, not {change_id}"));
            continue;
        }
        let own = match record.schema {
            RecordSchema::Approval => attribute(root, "revision"),
            RecordSchema::Close => None,
            RecordSchema::Run | RecordSchema::Review => attribute(root, "n"),
        };
        if let Some(own) = own {
            let expected = record.number.unwrap_or(0);
            if own.parse::<u32>().unwrap_or(0) != expected {
                defects.push(format!(
                    "{file}: carries number {own}, its name says {expected}"
                ));
                continue;
            }
        }
        match record.schema {
            RecordSchema::Approval => approvals.push(read_approval(root, file)),
            RecordSchema::Run => runs.push(read_run(root, file)),
            RecordSchema::Review => reviews.push(read_review(root, file)),
            RecordSchema::Close => close = Some(read_close(root, file)),
        }
    }

    approvals.sort_by_key(|approval| approval.revision);
    runs.sort_by_key(|run| run.n);
    reviews.sort_by_key(|review| review.n);
    briefs.sort_by_key(|brief| brief.revision);

    let mut log_dirs = Vec::new();
    if let Ok(entries) = fs::read_dir(records_dir.join("logs")) {
        for entry in entries.filter_map(Result::ok) {
            if let Ok(name) = entry.file_name().into_string() {
                if let Some(number) = name.strip_prefix("run-").and_then(record_number) {
                    log_dirs.push(number);
                }
            }
        }
    }
    log_dirs.sort_unstable();

    (
        Records {
            approvals,
            runs,
            reviews,
            close,
            close_exists,
            briefs,
            log_dirs,
        },
        defects,
    )
}

fn read_approval(root: Node, file: &str) -> GraceApproval {
    GraceApproval {
        file: file.to_string(),
        revision: u32_attribute(root, "revision"),
        contract_sha256: attribute(root, "contract-sha256").unwrap_or_default(),
        brief_sha256: attribute(root, "brief-sha256"),
        date: attribute(root, "date"),
        decision: if attribute(root, "decision").as_deref() == Some("approved") {
            "approved".to_string()
        } else {
            "rejected".to_string()
        },
        statement: child_named(root, "Statement")
            .map(|node| element_text(&node))
            .unwrap_or_default(),
        bound: children_named(root, "Bound")
            .into_iter()
            .map(|node| GraceBound {
                check: attribute(node, "check").unwrap_or_default(),
                definition_sha256: attribute(node, "definition-sha256").unwrap_or_default(),
            })
            .collect(),
    }
}

fn read_run(root: Node, file: &str) -> GraceRun {
    GraceRun {
        file: file.to_string(),
        n: u32_attribute(root, "n"),
        run_id: attribute(root, "run-id").unwrap_or_default(),
        started: attribute(root, "started").unwrap_or_default(),
        ended: attribute(root, "ended").unwrap_or_default(),
        status: if attribute(root, "status").as_deref() == Some("completed") {
            "completed".to_string()
        } else {
            "interrupted".to_string()
        },
        start: xs_boolean(root.attribute("start"), false),
        source_revision: attribute(root, "source-revision").unwrap_or_default(),
        dirty: xs_boolean(root.attribute("dirty"), true),
        candidate: attribute(root, "candidate").unwrap_or_default(),
        kernel: attribute(root, "kernel").unwrap_or_default(),
        revision: u32_attribute(root, "revision"),
        contract_sha256: attribute(root, "contract-sha256").unwrap_or_default(),
        preexisting: children_named(root, "Preexisting")
            .into_iter()
            .map(|node| element_text(&node))
            .collect(),
        results: children_named(root, "Result")
            .into_iter()
            .map(read_result)
            .collect(),
    }
}

fn read_result(node: Node) -> GraceRunResult {
    GraceRunResult {
        check: attribute(node, "check").unwrap_or_default(),
        kind: attribute(node, "kind").unwrap_or_default(),
        required: xs_boolean(node.attribute("required"), true),
        definition_sha256: attribute(node, "definition-sha256").unwrap_or_default(),
        definition: child_named(node, "Definition")
            .map(direct_text)
            .unwrap_or_default(),
        verdict: attribute(node, "verdict").unwrap_or_default(),
        claim: attribute(node, "claim"),
        cwd: attribute(node, "cwd"),
        exit: i64_attribute(node, "exit"),
        elapsed_ms: u64_attribute(node, "elapsed-ms"),
        log: attribute(node, "log"),
        log_sha256: attribute(node, "log-sha256"),
        environment: attribute(node, "environment"),
        trace: child_named(node, "Trace").map(read_trace),
        observation: child_named(node, "Observation").map(|element| GraceObservation {
            at: attribute(element, "at").unwrap_or_default(),
            build: attribute(element, "build").unwrap_or_default(),
            environment: attribute(element, "environment").unwrap_or_default(),
            reference: attribute(element, "reference"),
            note: element_text(&element),
        }),
        reason: child_named(node, "Reason").map(|element| element_text(&element)),
    }
}

fn read_trace(node: Node) -> GraceTrace {
    GraceTrace {
        origin: attribute(node, "origin"),
        origin_source: attribute(node, "origin-source").unwrap_or_default(),
        run_echo: xs_boolean(node.attribute("run-echo"), false),
        capture_started: attribute(node, "capture-started").unwrap_or_default(),
        capture_ended: attribute(node, "capture-ended").unwrap_or_default(),
        device: attribute(node, "device"),
        device_source: attribute(node, "device-source").unwrap_or_default(),
        build: attribute(node, "build"),
        build_source: attribute(node, "build-source").unwrap_or_default(),
        matched: u64_attribute(node, "matched").unwrap_or(0),
        expected: u64_attribute(node, "expected").unwrap_or(0),
        missing: children_named(node, "Missing")
            .into_iter()
            .map(|element| element_text(&element))
            .collect(),
        forbidden: children_named(node, "Forbidden")
            .into_iter()
            .map(|element| element_text(&element))
            .collect(),
    }
}

fn read_review(root: Node, file: &str) -> GraceReview {
    GraceReview {
        file: file.to_string(),
        n: u32_attribute(root, "n"),
        revision: u32_attribute(root, "revision"),
        contract_sha256: attribute(root, "contract-sha256").unwrap_or_default(),
        status: if attribute(root, "status").as_deref() == Some("completed") {
            "completed".to_string()
        } else {
            "incomplete".to_string()
        },
        independence: attribute(root, "independence").unwrap_or_default(),
        start_revision: attribute(root, "start-revision").unwrap_or_default(),
        end_revision: attribute(root, "end-revision").unwrap_or_default(),
        candidate: attribute(root, "candidate").unwrap_or_default(),
        findings: children_named(root, "Finding")
            .into_iter()
            .map(|node| GraceFinding {
                id: attribute(node, "id").unwrap_or_default(),
                severity: attribute(node, "severity").unwrap_or_default(),
                status: attribute(node, "status").unwrap_or_default(),
                owner_decision: child_named(node, "OwnerDecision")
                    .map(|decision| element_text(&decision)),
            })
            .collect(),
        adequacy: children_named(root, "Adequacy")
            .into_iter()
            .map(|node| GraceAdequacy {
                check: attribute(node, "check").unwrap_or_default(),
                discriminates: xs_boolean(node.attribute("discriminates"), false),
            })
            .collect(),
        rules: children_named(root, "Rule")
            .into_iter()
            .map(|node| GraceRuleVerdict {
                rule: attribute(node, "rule").unwrap_or_default(),
                verdict: attribute(node, "verdict").unwrap_or_default(),
            })
            .collect(),
    }
}

fn read_close(root: Node, file: &str) -> GraceClose {
    GraceClose {
        file: file.to_string(),
        revision: u32_attribute(root, "revision"),
        contract_sha256: attribute(root, "contract-sha256").unwrap_or_default(),
        date: attribute(root, "date").unwrap_or_default(),
        source_revision: attribute(root, "source-revision").unwrap_or_default(),
        candidate: attribute(root, "candidate").unwrap_or_default(),
        start_revision: attribute(root, "start-revision").unwrap_or_default(),
        brief: attribute(root, "brief"),
        approval: attribute(root, "approval").unwrap_or_default(),
        review: u32_attribute(root, "review"),
        kernel: attribute(root, "kernel").unwrap_or_default(),
        satisfied: children_named(root, "Satisfied")
            .into_iter()
            .map(|node| GraceSatisfied {
                check: attribute(node, "check").unwrap_or_default(),
                kind: attribute(node, "kind").unwrap_or_default(),
                run: u32_attribute(node, "run"),
                claim: attribute(node, "claim").unwrap_or_default(),
            })
            .collect(),
        not_required: children_named(root, "NotRequired")
            .into_iter()
            .map(|node| GraceNotRequired {
                check: attribute(node, "check").unwrap_or_default(),
                verdict: attribute(node, "verdict"),
            })
            .collect(),
    }
}

fn file_names(dir: &Path) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(Result::ok) {
            if entry.file_type().map(|kind| kind.is_file()).unwrap_or(false) {
                if let Ok(name) = entry.file_name().into_string() {
                    names.push(name);
                }
            }
        }
    }
    names
}

/// `approval-<n>.xml`, `run-<n>.xml` or `review-<n>.xml`.
fn record_name(file: &str) -> Option<(RecordSchema, u32)> {
    let stem = file.strip_suffix(".xml")?;
    let (prefix, digits) = stem.rsplit_once('-')?;
    let schema = match prefix {
        "approval" => RecordSchema::Approval,
        "run" => RecordSchema::Run,
        "review" => RecordSchema::Review,
        _ => return None,
    };
    Some((schema, record_number(digits)?))
}

/// `brief-<revision>.html`.
fn brief_revision(file: &str) -> Option<u32> {
    let stem = file.strip_prefix("brief-")?.strip_suffix(".html")?;
    record_number(stem)
}

/// Record numbers are `[1-9][0-9]*`, exactly as the kernel's filename patterns.
fn record_number(digits: &str) -> Option<u32> {
    if digits.is_empty()
        || digits.starts_with('0')
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    digits.parse().ok()
}

fn direct_text(node: Node) -> String {
    node.children()
        .filter(|child| child.is_text())
        .filter_map(|child| child.text())
        .collect()
}

fn u32_attribute(node: Node, name: &str) -> u32 {
    attribute(node, name)
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

fn u64_attribute(node: Node, name: &str) -> Option<u64> {
    attribute(node, name).and_then(|value| value.parse().ok())
}

fn i64_attribute(node: Node, name: &str) -> Option<i64> {
    attribute(node, name).and_then(|value| value.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = include_bytes!("fixtures/C-0019-retire-briefs/contract.xml");

    #[test]
    fn parses_the_c0019_fixture() {
        let contract =
            parse_contract(FIXTURE, "C-0019-retire-briefs").expect("fixture contract parses");
        assert_eq!(contract.id, "C-0019-retire-briefs");
        assert_eq!(contract.revision, 1);
        assert_eq!(
            contract.title,
            "The contract is the approval surface; the separate brief is retired"
        );
        assert_eq!(contract.constraints.len(), 8);
        assert_eq!(contract.acceptance.len(), 9);
        let kinds: Vec<(&str, &str)> = contract
            .checks
            .iter()
            .map(|check| (check.id.as_str(), check.kind.as_str()))
            .collect();
        assert_eq!(
            kinds,
            vec![
                ("CK1", "automated"),
                ("CK2", "automated"),
                ("CK3", "automated"),
                ("CK4", "automated"),
                ("CK5", "trace"),
                ("CK6", "automated"),
                ("CK7", "automated"),
                ("CK8", "trace"),
            ]
        );
        assert!(contract
            .affected
            .protected
            .iter()
            .any(|path| path == ".grace5/kernel-version"));
        assert!(contract.problem.starts_with("The HTML brief was introduced"));
    }

    #[test]
    fn rejects_a_contract_without_title() {
        let src = br#"<Contract id="C-0001-x" revision="1"><Problem>p</Problem></Contract>"#;
        let err = parse_contract(src, "C-0001-x").expect_err("missing title must be rejected");
        assert!(
            err.contains("title"),
            "error should name the title attribute: {err}"
        );
    }

    #[test]
    fn rejects_an_id_that_does_not_match_the_directory() {
        let src =
            br#"<Contract id="C-0002-y" revision="1" title="T"><Problem>p</Problem></Contract>"#;
        let err = parse_contract(src, "C-0003-z").expect_err("id mismatch must be rejected");
        assert!(err.contains("id"), "error should name the id: {err}");
    }

    #[test]
    fn rejects_invalid_xml() {
        let src = br#"<Contract id="C-0004-w" revision="1" title="T"><Problem>"#;
        assert!(parse_contract(src, "C-0004-w").is_err());
    }

    #[test]
    fn preserves_non_ascii_text() {
        let src = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                   <Contract id=\"C-0042-unicode\" revision=\"2\" title=\"Café — 日本語\">\n\
                   <Problem>café — 日本語</Problem>\n\
                   </Contract>"
            .as_bytes();
        let contract = parse_contract(src, "C-0042-unicode").expect("non-ASCII contract parses");
        assert_eq!(contract.problem, "café — 日本語");
        assert_eq!(contract.title, "Café — 日本語");
        assert_eq!(contract.revision, 2);
    }

    #[test]
    fn collapse_trims_and_squeezes_runs() {
        assert_eq!(collapse("  a \t\n b  "), "a b");
        assert_eq!(collapse("one"), "one");
        assert_eq!(collapse(" \t\n "), "");
    }

    #[test]
    fn reads_required_flags_and_expect_defaults() {
        let src = br#"<Contract id="C-0006-opts" revision="1" title="T">
            <Checks>
              <Automated id="CK1" required="false"><Command>a</Command></Automated>
              <Trace id="CK2" required="0"><Command>b</Command><Expect>marker</Expect></Trace>
              <Manual id="CK3"><Procedure>do it</Procedure></Manual>
            </Checks>
          </Contract>"#;
        let contract = parse_contract(src, "C-0006-opts").expect("contract parses");
        assert!(!contract.checks[0].required);
        assert!(!contract.checks[1].required);
        assert!(contract.checks[2].required);
        let expect = &contract.checks[1].expects[0];
        assert_eq!(expect.text, "marker");
        assert_eq!(expect.min, 1);
        assert_eq!(expect.max, None);
        assert_eq!(contract.checks[2].cwd, None);
    }

    #[test]
    fn rejects_a_non_contract_root() {
        let src = br#"<Change id="C-0007-r" revision="1" title="T"/>"#;
        let err = parse_contract(src, "C-0007-r").expect_err("non-Contract root is rejected");
        assert!(err.contains("Contract"), "error should name the root: {err}");
    }

    #[test]
    fn rejects_a_non_numeric_revision() {
        let src = br#"<Contract id="C-0008-n" revision="one" title="T"/>"#;
        let err = parse_contract(src, "C-0008-n").expect_err("non-numeric revision is rejected");
        assert!(
            err.contains("revision"),
            "error should name the revision: {err}"
        );
    }

    #[test]
    fn reads_absent_sections_as_empty() {
        let src = br#"<Contract id="C-0009-s" revision="1" title="T"/>"#;
        let contract = parse_contract(src, "C-0009-s").expect("contract parses");
        assert!(contract.problem.is_empty());
        assert!(contract.change.is_empty());
        assert!(contract.constraints.is_empty());
        assert!(contract.non_goals.is_empty());
        assert!(contract.acceptance.is_empty());
        assert!(contract.affected.paths.is_empty());
        assert!(contract.affected.names.is_empty());
        assert!(contract.affected.protected.is_empty());
        assert!(contract.environments.is_empty());
        assert!(contract.checks.is_empty());
        assert!(contract.decisions.is_empty());
    }
}

#[cfg(test)]
mod records_tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    const CHANGE_ID: &str = "C-0019-retire-briefs";

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> TempDir {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "grace-records-{}-{nanos}-{}",
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

    /// Rebuilds the fixture change directory from the checked-in bytes.
    fn fixture_change() -> TempDir {
        let dir = TempDir::new();
        let change = dir.path().join(CHANGE_ID);
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
        dir
    }

    fn write_record(dir: &TempDir, name: &str, bytes: &[u8]) {
        fs::write(dir.path().join(CHANGE_ID).join("records").join(name), bytes).unwrap();
    }

    fn change_path(dir: &TempDir) -> PathBuf {
        dir.path().join(CHANGE_ID)
    }

    #[test]
    fn parses_the_approval_record() {
        let dir = fixture_change();
        let (records, defects) = parse_records(&change_path(&dir), CHANGE_ID);
        assert!(defects.is_empty(), "unexpected defects: {defects:?}");
        assert_eq!(records.approvals.len(), 1);
        let approval = &records.approvals[0];
        assert_eq!(approval.file, "approval-1.xml");
        assert_eq!(approval.decision, "approved");
        assert_eq!(approval.revision, 1);
        assert_eq!(
            approval.contract_sha256,
            "475e960d00ca71cd5b021dd98fd95438fb267b1997d5375ef1313fcbbb168000"
        );
        assert!(approval.brief_sha256.is_some());
        assert_eq!(approval.bound.len(), 8);
        assert_eq!(approval.statement, "approved");
        assert!(records.log_dirs.is_empty());
    }

    #[test]
    fn parses_the_first_run_and_its_results() {
        let dir = fixture_change();
        let (records, defects) = parse_records(&change_path(&dir), CHANGE_ID);
        assert!(defects.is_empty(), "unexpected defects: {defects:?}");
        let run = records
            .runs
            .iter()
            .find(|run| run.n == 1)
            .expect("run 1 parses");
        assert!(run.start);
        assert_eq!(run.status, "completed");
        // run-1 recorded six results (CK1-CK4, CK6, CK7); run-3 carries all eight.
        assert_eq!(run.results.len(), 6);
        let ck6 = run
            .results
            .iter()
            .find(|result| result.check == "CK6")
            .expect("CK6 ran");
        assert_eq!(ck6.verdict, "failed");
        assert_eq!(ck6.exit, Some(1));
        assert_eq!(ck6.reason.as_deref(), Some("exit 1"));
        let ck1 = run
            .results
            .iter()
            .find(|result| result.check == "CK1")
            .expect("CK1 ran");
        assert_eq!(ck1.elapsed_ms, Some(8108));
        assert_eq!(ck1.log.as_deref(), Some("records/logs/run-1/CK1.log"));
    }

    #[test]
    fn parses_runs_review_and_close() {
        let dir = fixture_change();
        let (records, defects) = parse_records(&change_path(&dir), CHANGE_ID);
        assert!(defects.is_empty(), "unexpected defects: {defects:?}");
        let ns: Vec<u32> = records.runs.iter().map(|run| run.n).collect();
        assert_eq!(ns, vec![1, 3]);
        let review = records.reviews.first().expect("review parses");
        assert_eq!(review.n, 1);
        assert_eq!(review.status, "completed");
        assert!(!review.findings.is_empty());
        assert!(!review.adequacy.is_empty());
        assert!(!review.rules.is_empty());
        let close = records.close.as_ref().expect("close parses");
        assert_eq!(close.review, 1);
        assert_eq!(close.satisfied.len(), 8);
        assert!(records.close_exists);
    }

    #[test]
    fn lists_briefs_by_filename() {
        let dir = fixture_change();
        let (records, defects) = parse_records(&change_path(&dir), CHANGE_ID);
        assert!(defects.is_empty(), "unexpected defects: {defects:?}");
        assert_eq!(records.briefs.len(), 1);
        assert_eq!(records.briefs[0].revision, 1);
        assert_eq!(records.briefs[0].file, "brief-1.html");
    }

    #[test]
    fn rejects_a_run_whose_number_disagrees_with_its_filename() {
        let dir = fixture_change();
        write_record(
            &dir,
            "run-2.xml",
            br#"<Run change="C-0019-retire-briefs" revision="1" contract-sha256="x" n="9" run-id="r" started="" ended="" status="completed" start="true" source-revision="" dirty="false" candidate="" kernel="grace5 0.9.0"/>"#,
        );
        let (records, defects) = parse_records(&change_path(&dir), CHANGE_ID);
        assert!(
            defects.iter().any(|defect| defect.contains("run-2.xml")),
            "defect should name the file: {defects:?}"
        );
        assert_eq!(records.runs.len(), 2);
        assert!(records.runs.iter().all(|run| run.n != 9));
    }

    #[test]
    fn skips_an_unparseable_record() {
        let dir = fixture_change();
        write_record(
            &dir,
            "approval-2.xml",
            br#"<Approval change="C-0019-retire-briefs" revision="2""#,
        );
        let (records, defects) = parse_records(&change_path(&dir), CHANGE_ID);
        assert!(
            defects.iter().any(|defect| defect.contains("approval-2.xml")),
            "defect should name the file: {defects:?}"
        );
        assert_eq!(records.approvals.len(), 1);
    }

    #[test]
    fn reports_an_unknown_record_name() {
        let dir = fixture_change();
        write_record(&dir, "foo.xml", b"<Foo/>");
        let (records, defects) = parse_records(&change_path(&dir), CHANGE_ID);
        assert!(
            defects.iter().any(|defect| defect.contains("foo.xml")),
            "defect should name the file: {defects:?}"
        );
        assert_eq!(records.approvals.len(), 1);
        assert_eq!(records.runs.len(), 2);
    }

    #[test]
    fn keeps_close_exists_when_close_xml_is_unparseable() {
        let dir = fixture_change();
        write_record(&dir, "close.xml", b"<Close");
        let (records, defects) = parse_records(&change_path(&dir), CHANGE_ID);
        assert!(records.close_exists);
        assert!(records.close.is_none());
        assert!(
            defects.iter().any(|defect| defect.contains("close.xml")),
            "defect should name the file: {defects:?}"
        );
    }
}
