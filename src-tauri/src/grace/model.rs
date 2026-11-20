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