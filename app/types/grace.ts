/**
 * GRACE 5 explorer types.
 *
 * Mirrors the Rust `grace` model (src-tauri/src/grace/model.rs) exactly —
 * serde `rename_all = "camelCase"`, so Rust `snake_case` fields arrive here
 * camelCased (`contract_sha256` → `contractSha256`, `non_goals` → `nonGoals`).
 * `Option<T>` is `T | null`, `Vec<T>` is `T[]`, numeric fields are `number`.
 */

/** Display-level lifecycle stage (progress, not authorization). */
export type GraceStage = 'prepared' | 'approved' | 'executing' | 'reviewed' | 'closed'

/** Kernel-equivalent authorization verdict for the contract's current revision. */
export type GraceAuthorization = 'unapproved' | 'approved' | 'rejected' | 'altered after approval'

export interface GraceItem {
  id: string
  text: string
}

export interface GraceConstraint {
  id: string
  text: string
  rule: string | null
}

export interface GraceCriterion {
  id: string
  text: string
  /** Whitespace-split `checks` attribute. */
  checks: string[]
}

export interface GraceAffected {
  paths: string[]
  names: string[]
  protected: string[]
}

export interface GraceEnvironment {
  id: string
  device: string | null
  build: string | null
  text: string
}

export interface GraceExpect {
  text: string
  min: number
  max: number | null
}

export interface GraceCheck {
  id: string
  kind: string
  required: boolean
  environment: string | null
  cwd: string | null
  budget: number | null
  command: string | null
  procedure: string | null
  expects: GraceExpect[]
  forbids: string[]
  origin: string | null
  log: string | null
}

export interface GraceContract {
  id: string
  revision: number
  title: string
  problem: string
  change: string
  constraints: GraceConstraint[]
  nonGoals: GraceItem[]
  acceptance: GraceCriterion[]
  affected: GraceAffected
  environments: GraceEnvironment[]
  checks: GraceCheck[]
  decisions: GraceItem[]
}

export interface GraceBound {
  check: string
  definitionSha256: string
}

export interface GraceApproval {
  file: string
  revision: number
  contractSha256: string
  briefSha256: string | null
  date: string | null
  decision: string
  statement: string
  bound: GraceBound[]
}

export interface GraceTrace {
  origin: string | null
  originSource: string
  runEcho: boolean
  captureStarted: string
  captureEnded: string
  device: string | null
  deviceSource: string
  build: string | null
  buildSource: string
  matched: number
  expected: number
  missing: string[]
  forbidden: string[]
}

export interface GraceObservation {
  at: string
  build: string
  environment: string
  reference: string | null
  note: string
}

export interface GraceRunResult {
  check: string
  kind: string
  required: boolean
  definitionSha256: string
  definition: string
  verdict: string
  claim: string | null
  cwd: string | null
  exit: number | null
  elapsedMs: number | null
  log: string | null
  logSha256: string | null
  environment: string | null
  trace: GraceTrace | null
  observation: GraceObservation | null
  reason: string | null
}

export interface GraceRun {
  file: string
  n: number
  runId: string
  started: string
  ended: string
  status: string
  start: boolean
  sourceRevision: string
  dirty: boolean
  candidate: string
  kernel: string
  revision: number
  contractSha256: string
  preexisting: string[]
  results: GraceRunResult[]
}

export interface GraceFinding {
  id: string
  severity: string
  status: string
  ownerDecision: string | null
}

export interface GraceAdequacy {
  check: string
  discriminates: boolean
}

export interface GraceRuleVerdict {
  rule: string
  verdict: string
}

export interface GraceReview {
  file: string
  n: number
  revision: number
  contractSha256: string
  status: string
  independence: string
  startRevision: string
  endRevision: string
  candidate: string
  findings: GraceFinding[]
  adequacy: GraceAdequacy[]
  rules: GraceRuleVerdict[]
}

export interface GraceSatisfied {
  check: string
  kind: string
  run: number
  claim: string
}

export interface GraceNotRequired {
  check: string
  verdict: string | null
}

export interface GraceClose {
  file: string
  revision: number
  contractSha256: string
  date: string
  sourceRevision: string
  candidate: string
  startRevision: string
  brief: string | null
  approval: string
  review: number
  kernel: string
  satisfied: GraceSatisfied[]
  notRequired: GraceNotRequired[]
}

export interface GraceBrief {
  revision: number
  file: string
}

export interface GraceChangeSummary {
  id: string
  title: string | null
  revision: number | null
  stage: GraceStage
  authorization: GraceAuthorization
  malformed: string | null
  inClosedDir: boolean
  closed: boolean
  checksPassed: number
  checksRequired: number
  reviewStatus: string | null
  reviewOpenFindings: number
  problem: string | null
}

export interface GraceChangeDetail {
  summary: GraceChangeSummary
  dir: string
  contract: GraceContract | null
  approvals: GraceApproval[]
  runs: GraceRun[]
  reviews: GraceReview[]
  close: GraceClose | null
  briefs: GraceBrief[]
  notesMd: string | null
  logDirs: number[]
  defects: string[]
  latestResults: GraceRunResult[]
}

export interface GraceListResult {
  hasGrace: boolean
  pin: string | null
  pinSupported: boolean
  openCount: number
  closedCount: number
  changes: GraceChangeSummary[]
}

export interface GraceFileContent {
  content: string
  truncated: boolean
  sizeBytes: number
}
