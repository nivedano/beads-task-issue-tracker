//! Serialized shapes for the GRACE explorer (camelCase on the wire).

use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceItem {
    pub id: String,
    pub text: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceConstraint {
    pub id: String,
    pub text: String,
    pub rule: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceCriterion {
    pub id: String,
    pub text: String,
    pub checks: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceAffected {
    pub paths: Vec<String>,
    pub names: Vec<String>,
    pub protected: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceEnvironment {
    pub id: String,
    pub device: Option<String>,
    pub build: Option<String>,
    pub text: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceExpect {
    pub text: String,
    pub min: u64,
    pub max: Option<u64>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceCheck {
    pub id: String,
    pub kind: String,
    pub required: bool,
    pub environment: Option<String>,
    pub cwd: Option<String>,
    pub budget: Option<u64>,
    pub command: Option<String>,
    pub procedure: Option<String>,
    pub expects: Vec<GraceExpect>,
    pub forbids: Vec<String>,
    pub origin: Option<String>,
    pub log: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceContract {
    pub id: String,
    pub revision: u32,
    pub title: String,
    pub problem: String,
    pub change: String,
    pub constraints: Vec<GraceConstraint>,
    pub non_goals: Vec<GraceItem>,
    pub acceptance: Vec<GraceCriterion>,
    pub affected: GraceAffected,
    pub environments: Vec<GraceEnvironment>,
    pub checks: Vec<GraceCheck>,
    pub decisions: Vec<GraceItem>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceBound {
    pub check: String,
    pub definition_sha256: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceApproval {
    pub file: String,
    pub revision: u32,
    pub contract_sha256: String,
    pub brief_sha256: Option<String>,
    pub date: Option<String>,
    pub decision: String,
    pub statement: String,
    pub bound: Vec<GraceBound>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceTrace {
    pub origin: Option<String>,
    pub origin_source: String,
    pub run_echo: bool,
    pub capture_started: String,
    pub capture_ended: String,
    pub device: Option<String>,
    pub device_source: String,
    pub build: Option<String>,
    pub build_source: String,
    pub matched: u64,
    pub expected: u64,
    pub missing: Vec<String>,
    pub forbidden: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceObservation {
    pub at: String,
    pub build: String,
    pub environment: String,
    pub reference: Option<String>,
    pub note: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceRunResult {
    pub check: String,
    pub kind: String,
    pub required: bool,
    pub definition_sha256: String,
    pub definition: String,
    pub verdict: String,
    pub claim: Option<String>,
    pub cwd: Option<String>,
    pub exit: Option<i64>,
    pub elapsed_ms: Option<u64>,
    pub log: Option<String>,
    pub log_sha256: Option<String>,
    pub environment: Option<String>,
    pub trace: Option<GraceTrace>,
    pub observation: Option<GraceObservation>,
    pub reason: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceRun {
    pub file: String,
    pub n: u32,
    pub run_id: String,
    pub started: String,
    pub ended: String,
    pub status: String,
    pub start: bool,
    pub source_revision: String,
    pub dirty: bool,
    pub candidate: String,
    pub kernel: String,
    pub revision: u32,
    pub contract_sha256: String,
    pub preexisting: Vec<String>,
    pub results: Vec<GraceRunResult>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceFinding {
    pub id: String,
    pub severity: String,
    pub status: String,
    pub owner_decision: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceAdequacy {
    pub check: String,
    pub discriminates: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceRuleVerdict {
    pub rule: String,
    pub verdict: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceReview {
    pub file: String,
    pub n: u32,
    pub revision: u32,
    pub contract_sha256: String,
    pub status: String,
    pub independence: String,
    pub start_revision: String,
    pub end_revision: String,
    pub candidate: String,
    pub findings: Vec<GraceFinding>,
    pub adequacy: Vec<GraceAdequacy>,
    pub rules: Vec<GraceRuleVerdict>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceSatisfied {
    pub check: String,
    pub kind: String,
    pub run: u32,
    pub claim: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceNotRequired {
    pub check: String,
    pub verdict: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceClose {
    pub file: String,
    pub revision: u32,
    pub contract_sha256: String,
    pub date: String,
    pub source_revision: String,
    pub candidate: String,
    pub start_revision: String,
    pub brief: Option<String>,
    pub approval: String,
    pub review: u32,
    pub kernel: String,
    pub satisfied: Vec<GraceSatisfied>,
    pub not_required: Vec<GraceNotRequired>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceBrief {
    pub revision: u32,
    pub file: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceChangeSummary {
    pub id: String,
    pub title: Option<String>,
    pub revision: Option<u32>,
    pub stage: String,
    pub authorization: String,
    pub malformed: Option<String>,
    pub in_closed_dir: bool,
    pub closed: bool,
    pub checks_passed: u32,
    pub checks_required: u32,
    pub review_status: Option<String>,
    pub review_open_findings: u32,
    pub problem: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GraceListResult {
    pub has_grace: bool,
    pub pin: Option<String>,
    pub pin_supported: bool,
    pub open_count: u32,
    pub closed_count: u32,
    pub changes: Vec<GraceChangeSummary>,
}
