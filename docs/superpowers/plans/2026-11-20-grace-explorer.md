# GRACE Explorer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a read-only GRACE 5 artifact/state explorer beside the beads issue explorer, shown as center-panel tabs only when both registries exist in the opened project.

**Architecture:** New Rust `grace` module parses `.grace5/` directly (roxmltree + sha2), deriving display-level lifecycle stage and kernel-equivalent authorization; four new Tauri commands feed a `useGrace` composable and `app/components/grace/` components mirroring the issues explorer; `index.vue` gains a capability-aware tab strip and grace-only project path.

**Tech Stack:** Rust (tauri 2, roxmltree, sha2, serde), Vue 3 / Nuxt 4 SPA, TypeScript, vitest, shadcn-vue primitives already in `app/components/ui/`.

**Spec:** `docs/superpowers/specs/2026-11-20-grace-explorer-design.md` (approved 2026-11-20). Executors read both. Kernel semantics reference: sibling repo `D:/repos/github.com/nivedano/grace5` (`src/contract.ts:86-89`, `src/state.ts:128-163`, `src/records.ts:157-266`, `src/xml.ts:143-146`) — read-only reference, never a runtime dependency.

## Global Constraints

- Read-only: no code path writes to `.grace5/` anywhere.
- No git operations in grace parsing; no bun/grace5 installation required at runtime.
- Beads registry means `.beads/` only (this fork has no `.tracker/` backend; `.claude/codebase-map.md` is stale — do not trust it for backend facts).
- Tab strip renders only when both registries exist; single registry renders its explorer bare.
- Authorization is kernel-equivalent: SHA-256 over raw `contract.xml` bytes with ONLY `\r\n` pairs → `\n`, byte-preserving (never decode to UTF-8 string first); approval for the contract's CURRENT revision; branch order: none → `unapproved`, decision `rejected` → `rejected`, hash mismatch → `altered after approval`, else `approved`.
- Stage is progress, not verdict: `closed` → `reviewed` → `executing` → `approved` → `prepared`; may differ from authorization; both shown.
- Malformed is structural only (parse failure; missing contract `id`/`revision`/`title`; id ≠ directory; record number ≠ filename; `closed/` without `close.xml`; unknown record filenames). No XSD, no semantic re-verification. Malformed never hides valid changes. Standing note: "structural read-only view — grace5 check/status remains authoritative".
- Web mode: grace commands throw "GRACE explorer is only available in the desktop app" (bd-command pattern); `fs_list` server route gains `hasGrace` parity.
- Text semantics mirror kernel `collapse`: trim, inner `[ \t\n\r]+` runs → one space; applied to attribute values and element text.
- Latest result per check = highest-numbered run containing that check (kernel `checkState` rule). Counts display-level: passed among required, no candidate/dirty/hash re-verification.
- Per-project storage keys: `explorerTab`, `grace:filters:stage`, `grace:filters:authorization`, `grace:filters:scope`, `grace:search`, `grace:sort`, `grace:columns`.
- Reuse existing patterns: `bd-api.ts` invoke wrappers, `useProjectStorage`, hand-rolled tab strip styled like the mobile nav buttons (`index.vue:1221-1244`), `ui/` primitives, `renderMarkdown`, `openUrl`. No new UI dependency.
- Fixtures copied into this repo (provenance noted); no runtime reads of the sibling repo.
- Pin: `.grace5/kernel-version` trimmed; supported pin `0.9.0`; mismatch or missing → warning banner, still parse.
- `grace_read_file`: canonicalized containment inside `<project>/.grace5/`, 1 MiB cap (1048576 bytes), truncation reported, UTF-8 lossy.

## Review Focus

1. Non-ASCII and CRLF contract bytes: authorization hash must equal the kernel's (byte-level, not string-level) — Task 3 tests.
2. `grace_read_file` traversal: `../`, absolute paths outside `.grace5`, symlink escape must be refused — Task 4 tests.
3. Grace-only project: zero bd CLI / git invocations (PathSelector count scan included) and no stuck "Loading…" — Task 6 tab-rule tests + Task 10 smoke with log inspection.
4. A malformed change beside valid ones: valid ones still listed, malformed flagged — Task 3 test.
5. Pin mismatch / missing `kernel-version`: banner shown, changes still readable, no crash — Task 3 tests + Task 10 smoke.

## File Structure

**Create (Rust):** `src-tauri/src/grace/mod.rs` (commands + re-exports), `model.rs` (serde types, camelCase), `parse.rs` (contract + records, structural defects), `state.rs` (hash, authorization, stage, summaries), `fs.rs` (safe read, fingerprint, watch targets), `src-tauri/src/grace/fixtures/` (copied C-0019 artifacts + README provenance).
**Modify (Rust):** `src-tauri/Cargo.toml` (roxmltree, sha2), `src-tauri/src/lib.rs` (`mod grace;`, command registration, `fs_list` hasGrace, `start_watching` multi-target).
**Create (frontend):** `app/types/grace.ts`, `app/utils/grace-helpers.ts`, `tests/utils/grace-helpers.test.ts`, `app/composables/useGrace.ts`, `app/components/grace/`: `GraceListPanel.vue`, `GraceToolbar.vue`, `GraceTable.vue`, `GraceStageBadge.vue`, `GraceAuthorizationBadge.vue`, `GraceFilterChips.vue`, `StageFilterDropdown.vue`, `AuthorizationFilterDropdown.vue`, `ScopeFilterDropdown.vue`, `GraceDetail.vue`, `GraceLifecycleStepper.vue`, `GraceContractView.vue`, `GraceChecksTable.vue`, `GraceRecordsTimeline.vue`, `GraceNotesView.vue`.
**Modify (frontend):** `app/utils/bd-api.ts` (grace wrappers, `hasGrace` types, `probeProjectCapabilities`), `server/api/fs/list.get.ts` (hasGrace), `app/pages/index.vue` (tabs, panel swap, capability-aware lifecycle, mobile), `app/components/dashboard/FolderPicker.vue` (gate + badge), `PathSelector.vue` (scan skip + badge), `DashboardContent.vue` (grace-only empty state), `CHANGELOG.md`, `README.md`.

**Issue tracking (AGENTS.md):** before Task 1, create and claim a bd issue: `bd create "GRACE explorer: read-only .grace5 artifact/state explorer"`, then `bd update <id> --status in_progress`. If the repo has no bd state, follow `bd onboard`; if the bd binary is absent, report it and continue without blocking. Close the issue in Task 11.

---

### Task 1: Contract parsing (Rust)

**Files:**
- Modify: `src-tauri/Cargo.toml` (add `roxmltree = "0.20"`, `sha2 = "0.10"`)
- Create: `src-tauri/src/grace/mod.rs`, `src-tauri/src/grace/model.rs`, `src-tauri/src/grace/parse.rs`
- Create: `src-tauri/src/grace/fixtures/C-0019-retire-briefs/contract.xml` (copy verbatim from `D:/repos/github.com/nivedano/grace5/.grace5/changes/closed/C-0019-retire-briefs/contract.xml`), `fixtures/README.md` (provenance: grace5 repo, kernel 0.9.0, copied 2026-11-20)
- Modify: `src-tauri/src/lib.rs` (add `mod grace;` only)

**Interfaces:**
- Produces: `model::GraceContract { id: String, revision: u32, title: String, problem: String, change: String, constraints: Vec<GraceConstraint>, non_goals: Vec<GraceItem>, acceptance: Vec<GraceCriterion>, affected: GraceAffected, environments: Vec<GraceEnvironment>, checks: Vec<GraceCheck>, decisions: Vec<GraceItem> }` with `GraceItem { id, text }`, `GraceConstraint { id, text, rule: Option<String> }`, `GraceCriterion { id, text, checks: Vec<String> }` (checks = whitespace-split `checks` attribute), `GraceAffected { paths, names, protected: Vec<String> }`, `GraceEnvironment { id, device: Option<String>, build: Option<String>, text }`, `GraceCheck { id, kind: String, required: bool, environment: Option<String>, cwd: Option<String>, budget: Option<u64>, command: Option<String>, procedure: Option<String>, expects: Vec<GraceExpect>, forbids: Vec<String>, origin: Option<String>, log: Option<String> }`, `GraceExpect { text, min: u64, max: Option<u64> }`. All `#[derive(Serialize, Clone, Debug)] #[serde(rename_all = "camelCase")]`.
- Produces: `parse::parse_contract(bytes: &[u8], dir_name: &str) -> Result<GraceContract, String>` — Err is the structural malformed reason. Also `parse::collapse(value: &str) -> String` and `parse::element_text(el: &roxmltree::Node) -> String` (concatenate descendant text nodes in document order, then collapse).

- [ ] **Step 1: Copy fixture + write failing tests** in `parse.rs` `#[cfg(test)]`: (a) `parse_contract(include_bytes!("fixtures/C-0019-retire-briefs/contract.xml"), "C-0019-retire-briefs")` succeeds; assert id, revision 1, title equals the fixture's `title` attribute, 8 constraints, 9 acceptance criteria, checks contain CK1..CK8 with kinds, `affected.protected` contains `.grace5/kernel-version`; (b) missing `title` attribute → Err containing "title"; (c) root id ≠ dir_name → Err containing "id"; (d) invalid XML → Err; (e) non-ASCII: inline contract bytes with `<Problem>café — 日本語</Problem>` parse with text preserved exactly after collapse; (f) collapse: `"  a \t\n b  "` → `"a b"`.
- [ ] **Step 2:** `cargo test grace::parse` from `src-tauri/` — expect FAIL (module missing).
- [ ] **Step 3:** Implement model types + `parse_contract`: roxmltree parse; root must be `Contract` with non-empty `id`/`revision`/`title` (revision parsed as u32); id must equal dir_name; sections extracted leniently (missing section → empty vec / empty string); `required` defaults true (`xs:boolean` "false"/"0" → false); `Expect` min default 1; text via `element_text`, attributes via `collapse`.
- [ ] **Step 4:** `cargo test grace::parse` — expect PASS (6 tests).
- [ ] **Step 5:** Commit `feat(grace): contract parsing with structural validation`.

### Task 2: Records parsing (Rust)

**Files:**
- Modify: `src-tauri/src/grace/model.rs`, `parse.rs`
- Create: fixtures `records/approval-1.xml`, `records/run-1.xml`, `records/run-3.xml`, `records/review-1.xml`, `records/close.xml`, `records/brief-1.html`, `notes.md` under `fixtures/C-0019-retire-briefs/` (copy verbatim from the same grace5 change dir; skip `logs/`)

**Interfaces:**
- Produces model types (camelCase serde): `GraceApproval { file, revision: u32, contract_sha256, brief_sha256: Option<String>, date: Option<String>, decision: String, statement, bound: Vec<GraceBound> }`, `GraceBound { check, definition_sha256 }`; `GraceRun { file, n: u32, run_id, started, ended, status, start: bool, source_revision, dirty: bool, candidate, kernel, revision: u32, contract_sha256, preexisting: Vec<String>, results: Vec<GraceRunResult> }`; `GraceRunResult { check, kind, required: bool, definition_sha256, definition, verdict, claim: Option<String>, cwd: Option<String>, exit: Option<i64>, elapsed_ms: Option<u64>, log: Option<String>, log_sha256: Option<String>, environment: Option<String>, trace: Option<GraceTrace>, observation: Option<GraceObservation>, reason: Option<String> }`; `GraceTrace { origin: Option<String>, origin_source, run_echo: bool, capture_started, capture_ended, device: Option<String>, device_source, build: Option<String>, build_source, matched: u64, expected: u64, missing: Vec<String>, forbidden: Vec<String> }`; `GraceObservation { at, build, environment, reference: Option<String>, note }`; `GraceReview { file, n: u32, revision: u32, contract_sha256, status, independence, start_revision, end_revision, candidate, findings: Vec<GraceFinding>, adequacy: Vec<GraceAdequacy>, rules: Vec<GraceRuleVerdict> }`; `GraceFinding { id, severity, status, owner_decision: Option<String> }`; `GraceAdequacy { check, discriminates: bool }`; `GraceRuleVerdict { rule, verdict }`; `GraceClose { file, revision: u32, contract_sha256, date, source_revision, candidate, start_revision, brief: Option<String>, approval, review: u32, kernel, satisfied: Vec<GraceSatisfied>, not_required: Vec<GraceNotRequired> }`; `GraceSatisfied { check, kind, run: u32, claim }`; `GraceNotRequired { check, verdict: Option<String> }`; `GraceBrief { revision: u32, file }`.
- Produces: `parse::Records { approvals: Vec<GraceApproval>, runs: Vec<GraceRun>, reviews: Vec<GraceReview>, close: Option<GraceClose>, close_exists: bool, briefs: Vec<GraceBrief>, log_dirs: Vec<u32> }` and `parse::parse_records(change_dir: &Path, change_id: &str) -> (Records, Vec<String>)` (defects).

- [ ] **Step 1: Write failing tests** (tempdir assembled from `include_bytes!` fixtures): (a) approval-1 parses: decision "approved", revision 1, contract_sha256 `475e960d00ca71cd5b021dd98fd95438fb267b1997d5375ef1313fcbbb168000`, brief_sha256 present, 8 Bound entries, statement "approved"; (b) run-1: n 1, start true, status completed, 7 results, CK6 verdict "failed" exit 1 reason "exit 1", CK1 elapsed_ms 8108 log "records/logs/run-1/CK1.log"; (c) run-3 + review-1 + close parse: close.review 1, 8 Satisfied; review findings/adequacy/rules non-empty; runs sorted ascending n; (d) briefs lists brief-1.html revision 1; (e) defect: `run-2.xml` whose root `n="9"` → defect mentioning filename, record skipped; (f) defect: unparseable `approval-2.xml` → defect, skipped; (g) defect: unknown `foo.xml` → defect; (h) `close_exists` true when close.xml present even if unparseable (then close None + defect).
- [ ] **Step 2:** `cargo test grace::parse` — expect FAIL on new tests.
- [ ] **Step 3:** Implement `parse_records`: enumerate `records/` entries; filename patterns `approval-(\d+).xml`, `run-(\d+).xml`, `review-(\d+).xml`, `close.xml`, `brief-(\d+).html`; root attribute names exactly as kernel (`change`, `revision`, `contract-sha256`, `decision`, `brief-sha256`, `date`, `n`, `run-id`, `started`, `ended`, `status`, `start`, `source-revision`, `dirty`, `candidate`, `kernel`, `independence`, `start-revision`, `end-revision`, `approval`, `review`, `brief`); children `Statement`, `Bound`, `Preexisting`, `Result` (attrs `check`, `kind`, `required`, `definition-sha256`, `verdict`, `claim`, `cwd`, `exit`, `elapsed-ms`, `log`, `log-sha256`, `environment`; children `Definition`, `Trace`, `Observation`, `Reason`), `Finding` (+`OwnerDecision`), `Adequacy`, `Rule`, `Satisfied`, `NotRequired`, `Missing`, `Forbidden`; record `change` attr must equal change_id else defect; number must equal filename number else defect; `log_dirs` from `records/logs/run-<n>/` directory names; briefs recorded by filename only (no HTML parsing).
- [ ] **Step 4:** `cargo test grace::parse` — expect PASS.
- [ ] **Step 5:** Commit `feat(grace): records parsing with structural defects`.

### Task 3: State derivation + grace_list (Rust)

**Files:**
- Create: `src-tauri/src/grace/state.rs`
- Modify: `src-tauri/src/grace/mod.rs` (command), `model.rs` (`GraceChangeSummary`, `GraceListResult`)

**Interfaces:**
- Produces: `state::contract_sha256(bytes: &[u8]) -> String` (lowercase hex); `state::authorization(contract_revision: u32, contract_hash: &str, approvals: &[GraceApproval]) -> &'static str`; `state::summarize(id: &str, in_closed_dir: bool, contract: Option<&GraceContract>, contract_hash: Option<&str>, records: &parse::Records, malformed: Option<String>) -> GraceChangeSummary`; `model::GraceChangeSummary { id, title: Option<String>, revision: Option<u32>, stage: String, authorization: String, malformed: Option<String>, in_closed_dir: bool, closed: bool, checks_passed: u32, checks_required: u32, review_status: Option<String>, review_open_findings: u32, problem: Option<String> }`; `model::GraceListResult { has_grace: bool, pin: Option<String>, pin_supported: bool, open_count: u32, closed_count: u32, changes: Vec<GraceChangeSummary> }`; command `grace_list(path: String) -> Result<GraceListResult, String>` (registered in lib.rs invoke_handler in Task 4).
- Consumes: Tasks 1-2 parsers.

- [ ] **Step 1: Write failing tests:** (a) kernel vector: `contract_sha256(include_bytes!("fixtures/C-0019-retire-briefs/contract.xml"))` == `475e960d00ca71cd5b021dd98fd95438fb267b1997d5375ef1313fcbbb168000` (kernel-generated value from approval-1.xml). If this fails, STOP and report — do not adjust the expectation; (b) CRLF invariance: same bytes with every `\n` → `\r\n` hash identically; (c) non-ASCII vector: minimal inline contract containing `café 日本語`, expected hex computed once at authoring time via `bun -e "const b=Buffer.from(SRC,'utf8');const n=Buffer.from(b.toString('latin1').replace(/\r\n/g,'\n'),'latin1');console.log(require('crypto').createHash('sha256').update(n).digest('hex'))"` and hardcoded — SRC must be the exact UTF-8 contract text used in the Rust test, byte-identical; (d) authorization branch order: no approval for revision → unapproved; approval decision rejected → rejected (even with matching hash); approved + hash mismatch → "altered after approval"; approved + match → approved; approval for OTHER revision only → unapproved; (e) stage ladder: close_exists → closed; completed review → reviewed; runs → executing; authorization approved → approved; else prepared; (f) counts: latest run containing each check wins (run-1 CK6 failed, run-3 CK6 passed → passed counts); checks_required = contract required checks; (g) tempdir list: one valid change + one malformed (missing title) → both returned, malformed flagged, valid intact; closed/ subdir change counted closed; (h) pin: missing kernel-version → pin None + pin_supported false; "0.9.0" → supported; "0.8.0" → unsupported; (i) no `.grace5` → has_grace false, empty list, no error.
- [ ] **Step 2:** `cargo test grace::state` — expect FAIL.
- [ ] **Step 3:** Implement: `contract_sha256` = sha2::Sha256 over bytes after replacing only adjacent `b"\r\n"` pairs with `b"\n"` (byte loop or windows; never String-decode); authorization per kernel branch order using approval where `revision == contract_revision`; stage per ladder (closed from close_exists; reviewed from any review status completed; executing from non-empty runs; approved from authorization == "approved"; else prepared); review_status/open findings from highest-n review; summarize wires it together; `grace_list` walks `.grace5/changes/` (skip `closed` entry) + `.grace5/changes/closed/`, reads contract bytes once per change (hash + parse), records, defects; open_count = changes not closed, closed_count = closed; pin read+trim; a change's malformed = its contract parse error, else the first records defect when any; summary.problem = the contract Problem text (search surface).
- [ ] **Step 4:** `cargo test grace` — expect PASS.
- [ ] **Step 5:** Commit `feat(grace): kernel-equivalent authorization, stage derivation, grace_list`.

### Task 4: Remaining commands + watcher + fs_list (Rust)

**Files:**
- Create: `src-tauri/src/grace/fs.rs`
- Modify: `src-tauri/src/grace/mod.rs` (`grace_show`, `grace_read_file`, `grace_check_changed`), `model.rs` (`GraceChangeDetail`, `GraceFileContent`), `src-tauri/src/lib.rs` (register 4 commands in invoke_handler; `DirectoryEntry`/`FsListResult` + `list_directory` hasGrace; `start_watching` multi-target)

**Interfaces:**
- Produces: `model::GraceChangeDetail { summary: GraceChangeSummary, dir: String, contract: Option<GraceContract>, approvals: Vec<GraceApproval>, runs: Vec<GraceRun>, reviews: Vec<GraceReview>, close: Option<GraceClose>, briefs: Vec<GraceBrief>, notes_md: Option<String>, log_dirs: Vec<u32>, defects: Vec<String>, latest_results: Vec<GraceRunResult> }`; `model::GraceFileContent { content: String, truncated: bool, size_bytes: u64 }`.
- Produces: `fs::read_grace_file(project: &Path, rel_path: &str) -> Result<GraceFileContent, String>`; `fs::grace_fingerprint(project: &Path) -> Option<(SystemTime, u64)>` (max mtime + file count over `.grace5` tree, metadata only); `fs::watch_targets(project: &Path) -> Vec<PathBuf>` (existing `.beads` and/or `.grace5` dirs).
- Produces commands: `grace_show(path: String, id: String) -> Result<GraceChangeDetail, String>` (id looked up in changes/ then changes/closed/; unknown id → Err); `grace_read_file(path: String, rel_path: String) -> Result<GraceFileContent, String>`; `grace_check_changed(path: String) -> Result<bool, String>` (static `GRACE_LAST_FINGERPRINT: Mutex<HashMap<String, (SystemTime, u64)>>`, compare-and-update like `bd_check_changed`).
- Consumes: Tasks 1-3. `latest_results`: for each contract check, result from the highest-n run containing it (kernel checkState rule).

- [ ] **Step 1: Write failing tests:** (a) `read_grace_file` reads `changes/closed/C-0019-retire-briefs/notes.md` from a tempdir copy; (b) refuses `../outside.md`, absolute path outside `.grace5`, and a symlink inside `.grace5` pointing outside (canonicalized containment); (c) file > 1 MiB → truncated true, content at most 1 MiB (lossy UTF-8 decode may shift the byte boundary), size_bytes actual; (d) missing file → Err; (e) `grace_fingerprint` changes when a file is touched/added, None without `.grace5`; (f) `watch_targets`: beads-only → [.beads], grace-only → [.grace5], both → both, neither → empty; (g) `grace_show` on fixture tempdir: summary.closed true, contract present, notes_md contains fixture text, briefs len 1, latest_results CK6 verdict passed (run-3 supersedes run-1); (h) `grace_show` unknown id → Err; (i) `list_directory` tempdir: entry with `.grace5` → hasGrace true, without → false; current-dir hasGrace likewise.
- [ ] **Step 2:** `cargo test grace` — expect FAIL on new tests.
- [ ] **Step 3:** Implement fs.rs + commands; register `grace::grace_list, grace::grace_show, grace::grace_read_file, grace::grace_check_changed` in the lib.rs invoke_handler list; add `has_grace` (`#[serde(rename = "hasGrace")]`) to DirectoryEntry + FsListResult and set it in `list_directory` (`.grace5` is_dir, same places has_beads is set); rework `start_watching`: targets from `fs::watch_targets`, empty → Err "no .beads or .grace5 directory found at: {path}", watch each target (beads: recursive iff project_uses_dolt else non-recursive; grace: always recursive), event emission/debounce unchanged.
- [ ] **Step 4:** `cargo test` (full suite) — expect PASS.
- [ ] **Step 5:** Commit `feat(grace): show/read/changed commands, dual watcher, fs_list hasGrace`.

### Task 5: Frontend types + API wrappers + web parity

**Files:**
- Create: `app/types/grace.ts`
- Modify: `app/utils/bd-api.ts` (wrappers + `DirectoryEntry`/`FsListResult` hasGrace + `probeProjectCapabilities`), `server/api/fs/list.get.ts` (hasGrace)

**Interfaces:**
- Produces: TS types mirroring Task 1-4 serde output EXACTLY (camelCase): `GraceStage = 'prepared'|'approved'|'executing'|'reviewed'|'closed'`; `GraceAuthorization = 'unapproved'|'approved'|'rejected'|'altered after approval'`; `GraceCheck`, `GraceExpect`, `GraceItem`, `GraceConstraint`, `GraceCriterion`, `GraceEnvironment`, `GraceAffected`, `GraceContract`, `GraceBound`, `GraceApproval`, `GraceTrace`, `GraceObservation`, `GraceRunResult`, `GraceRun`, `GraceFinding`, `GraceAdequacy`, `GraceRuleVerdict`, `GraceReview`, `GraceSatisfied`, `GraceNotRequired`, `GraceClose`, `GraceBrief`, `GraceChangeSummary`, `GraceChangeDetail`, `GraceListResult`, `GraceFileContent` (field names/types per the Rust model blocks).
- Produces in bd-api.ts: `graceList(path: string): Promise<GraceListResult>`, `graceShow(path: string, id: string): Promise<GraceChangeDetail>`, `graceReadFile(path: string, relPath: string): Promise<GraceFileContent>`, `graceCheckChanged(path: string): Promise<boolean>` — each `if (isTauri()) invoke<T>('grace_*', {...})` else `throw new Error('GRACE explorer is only available in the desktop app')`; `probeProjectCapabilities(path: string): Promise<{ hasBeads: boolean; hasGrace: boolean }>` via two `fsExists` calls (`join` with platform separator via existing path utils; web mode: hasBeads false/hasGrace false). `DirectoryEntry`/`FsListResult` gain `hasGrace: boolean`.
- Consumes: Task 4 command names/args (`path`, `id`, `relPath`; Tauri 2 maps camelCase JS argument names to snake_case Rust parameters, so pass `{ path, relPath }` for `rel_path`).

- [ ] **Step 1:** Write types + wrappers + server route hasGrace (mirror the Rust detection: for each entry and for the current dir, check whether a `.grace5` subdirectory exists; dot-entries themselves stay unlisted).
- [ ] **Step 2:** `bun run build` — expect success (typecheck via nuxt generate).
- [ ] **Step 3:** Commit `feat(grace): frontend types, API wrappers, fs_list web parity`.

### Task 6: Pure helpers (TDD)

**Files:**
- Create: `app/utils/grace-helpers.ts`, `tests/utils/grace-helpers.test.ts`

**Interfaces:**
- Produces: `GraceFilters { stage: GraceStage[]; authorization: GraceAuthorization[]; scope: 'open'|'closed'|'all' }`; `filterGraceChanges(changes: GraceChangeSummary[], filters: GraceFilters, search: string): GraceChangeSummary[]`; `sortGraceChanges(changes: GraceChangeSummary[], field: 'id'|'title'|'stage'|'revision', direction: 'asc'|'desc'): GraceChangeSummary[]`; `graceStats(changes: GraceChangeSummary[]): { open: number; closed: number }`; `resolveExplorerTab(stored: 'issues'|'grace', caps: { hasBeads: boolean; hasGrace: boolean }): { tab: 'issues'|'grace'|null; showStrip: boolean }`.
- Consumes: `naturalCompare` from `app/utils/issue-helpers.ts` (id sort).

- [ ] **Step 1: Write failing tests** (~24 cases): filter — empty filters + empty search passes all; scope 'open' excludes `closed || inClosedDir`, 'closed' includes only those, 'all' both; stage/authorization arrays OR within facet, AND across facets; search case-insensitive across id, title, problem (null title/problem tolerated); malformed changes pass filters (never hidden). sort — id uses naturalCompare (C-0002 before C-0010), desc reverses; title localeCompare with nulls last; stage ladder order prepared<approved<executing<reviewed<closed; revision numeric, nulls last. stats — open/closed counts. resolveExplorerTab — both registries: tab=stored, showStrip true; beads-only: tab issues, showStrip false (stored grace ignored); grace-only: tab grace, showStrip false; neither: tab null, showStrip false.
- [ ] **Step 2:** `bun run test tests/utils/grace-helpers.test.ts` — expect FAIL.
- [ ] **Step 3:** Implement helpers (pure, no Vue imports).
- [ ] **Step 4:** `bun run test` (full) — expect PASS, existing 265+ unaffected.
- [ ] **Step 5:** Commit `feat(grace): pure filter/sort/stats/tab-rule helpers with tests`.

### Task 7: useGrace composable

**Files:**
- Create: `app/composables/useGrace.ts`

**Interfaces:**
- Produces `useGrace()` returning: `changes: Ref<GraceChangeSummary[]>`, `listResult: Ref<GraceListResult|null>`, `selectedChange: Ref<GraceChangeSummary|null>`, `detail: Ref<GraceChangeDetail|null>`, `isLoading: Ref<boolean>`, `detailLoading: Ref<boolean>`, `error: Ref<string|null>`, `search: Ref<string>` (useProjectStorage 'grace:search'), `filters` reactive { stage, authorization, scope } via useProjectStorage keys 'grace:filters:stage' (default []), 'grace:filters:authorization' (default []), 'grace:filters:scope' (default 'open'), `sort: Ref<{ field: 'id'|'title'|'stage'|'revision'; direction: 'asc'|'desc' }>` via useProjectStorage 'grace:sort' (default { field:'id', direction:'desc' }); filteredChanges applies filterGraceChanges then sortGraceChanges with sort.value, `filteredChanges: ComputedRef<GraceChangeSummary[]>` (filterGraceChanges + sortGraceChanges), `stats: ComputedRef<{open,closed}>`, `fetchChanges(path: string): Promise<void>` (graceList; sets isLoading false in finally; error surfaced, changes kept stale on failure), `selectChange(path: string, c: GraceChangeSummary|null): Promise<void>` (null clears detail; else graceShow into detail, detailLoading guarded), `refreshIfChanged(path: string): Promise<boolean>` (graceCheckChanged → fetchChanges + re-fetch selected detail if any), `clear(): void`.
- Consumes: Tasks 5-6. Pattern reference: `app/composables/useIssues.ts` (module-level singleton refs like useIssues, generation guard for path switches).

- [ ] **Step 1:** Implement composable (thin reactive wrapper; all logic in helpers/API).
- [ ] **Step 2:** `bun run build` — expect success.
- [ ] **Step 3:** Commit `feat(grace): useGrace composable with per-project persistence`.

### Task 8: GRACE list-side components

**Files:**
- Create: `app/components/grace/GraceStageBadge.vue`, `GraceAuthorizationBadge.vue`, `GraceFilterChips.vue`, `StageFilterDropdown.vue`, `AuthorizationFilterDropdown.vue`, `ScopeFilterDropdown.vue`, `GraceToolbar.vue`, `GraceTable.vue`, `GraceListPanel.vue`

**Interfaces:**
- Consumes: `app/types/grace.ts`, `useGrace`, `ui/` primitives (Badge, Button, Input, DropdownMenu*, Table*, ScrollArea, Tooltip), patterns from `issues/StatusBadge.vue`, `issues/StatusFilterDropdown.vue`, `issues/IssuesToolbar.vue`, `issues/IssueTable.vue`, `issues/IssueListPanel.vue`, `issues/FilterChips.vue`.
- Produces: `GraceListPanel` props `{ filteredChanges: GraceChangeSummary[], selectedId: string|null, filters: GraceFilters, search: string, sortField, sortDirection, isLoading: boolean, pinWarning: string|null }` (total count derived from filteredChanges.length), emits `update:search, update:filters, select(change), sort(field, direction), clear-filters`; exposes `focusSearch()`. Internally: GraceToolbar (search input + 3 filter dropdowns + column-config popover persisted via useProjectStorage 'grace:columns') + GraceFilterChips + GraceTable.
- Badge maps: stage prepared→secondary/outline, approved→primary/blue, executing→warning, reviewed→accent, closed→success/muted; authorization approved→success, rejected→error, "altered after approval"→warning, unapproved→muted. Style classes mirror StatusBadge.
- GraceTable columns (default visible: id, title, stage, authorization, checks, review, revision): checks cell `passed/required`, review cell `status (open findings)` or "—"; sortable headers emit sort; row click emits select; selected row highlighted like IssueTable; malformed rows show error badge + tooltip with the defect, never hidden.

- [ ] **Step 1:** Implement badges + dropdowns + chips (mirror issues counterparts; checkbox lists from the GraceStage/GraceAuthorization unions; scope dropdown open/closed/all).
- [ ] **Step 2:** Implement GraceToolbar + GraceTable + GraceListPanel (pass-through emits like IssueListPanel; no data logic).
- [ ] **Step 3:** `bun run build` — expect success.
- [ ] **Step 4:** Commit `feat(grace): explorer list components (toolbar, filters, table, badges)`.

### Task 9: GRACE detail components

**Files:**
- Create: `app/components/grace/GraceLifecycleStepper.vue`, `GraceContractView.vue`, `GraceChecksTable.vue`, `GraceRecordsTimeline.vue`, `GraceNotesView.vue`, `GraceDetail.vue`

**Interfaces:**
- Consumes: types, `renderMarkdown` (`app/utils/markdown.ts`), `openUrl` (`app/utils/open-url.ts`), `graceReadFile`, `layout/CollapsibleSection.vue` pattern, `ui/` Dialog/Badge/Separator/ScrollArea/Tooltip.
- Produces: `GraceDetail` props `{ detail: GraceChangeDetail|null, isLoading: boolean, pinWarning: string|null }`, emits `open-file(relPath: string)`; renders: header (CopyableId-style id, revision, GraceStageBadge, GraceAuthorizationBadge, title); GraceLifecycleStepper (5 nodes Prepare→Approve→Execute→Review→Close; lit through the stage index; authorization badge shown separately — progress ≠ authority); banners: malformed (error, defect text), pinWarning (warning), standing muted note "Structural read-only view — grace5 check/status remains authoritative"; collapsible sections: Contract (GraceContractView: Problem, Change, Constraints with rule citations, NonGoals, Acceptance with checks refs, Affected paths/names/protected, Decisions), Checks (GraceChecksTable: definition fields + latest verdict badge, claim, exit, elapsed, reason; trace matched/expected/missing/forbidden; observation at/build/environment/note), Records (GraceRecordsTimeline: approvals per revision with decision/statement/bound hashes/brief-sha256; runs ascending n, expandable per-check receipts incl. log open button; reviews with findings table severity/status/ownerDecision, adequacy, rule verdicts; close satisfied/notRequired; briefs listed with open-externally), Notes (GraceNotesView: renderMarkdown, "no notes" empty state).
- Log viewing: in-app Dialog with `<pre>` content from `graceReadFile` (relPath from result.log), truncation notice when truncated; raw artifact open (contract.xml, brief HTML, notes.md) via `openUrl` with the absolute path (`detail.dir` + relative). Brief HTML NEVER rendered inline.

- [ ] **Step 1:** Implement stepper + contract view + checks table.
- [ ] **Step 2:** Implement records timeline + notes + GraceDetail wrapper + log dialog.
- [ ] **Step 3:** `bun run build` — expect success.
- [ ] **Step 4:** Commit `feat(grace): detail view (stepper, contract, checks, records timeline, notes)`.

### Task 10: App integration + capability-aware lifecycle

**Files:**
- Modify: `app/pages/index.vue`, `app/components/dashboard/FolderPicker.vue`, `app/components/dashboard/PathSelector.vue`, `app/components/dashboard/DashboardContent.vue`, `app/composables/useChangeDetection.ts` (grace branch), polling wiring in index.vue

**Interfaces:**
- Consumes: everything above. Key existing anchors: center main `index.vue:1044-1098`, right aside `1101-1216`, mobile strip `1221-1244`, `mobilePanel` ref `:193`, `handleSelectIssue` `:688-702`, `handlePathChange` `:586-666`, mount `:400-475`, `showOnboarding` `:140-143`; FolderPicker gate `:476`, badges `:437-451`; PathSelector count scan `:33` (`bdList({path, includeAll:true})`).
- Produces: `caps = ref({hasBeads:false, hasGrace:false})` probed via `probeProjectCapabilities` on mount and path change (before fetch decisions); `explorerTabStorage = useProjectStorage<'issues'|'grace'>('explorerTab','issues')`; `effective = computed(() => resolveExplorerTab(explorerTabStorage.value, caps.value))`; strip buttons styled exactly like the mobile nav strip (flex border-b border-border bg-card; active text-primary border-b-2 border-primary), labels `Issues ({{ filteredIssues.length }})` / `GRACE ({{ grace.stats.open }})` (useGrace stats), rendered only when `effective.showStrip`; center renders IssueListPanel or GraceListPanel by `effective.tab`; right aside gate extended with `selectedChange`, content GraceDetail when grace tab active (IssuePreview otherwise); `handleSelectChange` mirrors handleSelectIssue (mobile → mobilePanel 'details', desktop → isRightSidebarOpen); mobile: `mobilePanel` union gains 'grace', strip gains GRACE button under showStrip, grace panel renders GraceListPanel, details branch renders GraceDetail when grace tab active.
- Capability-aware lifecycle: mount + handlePathChange run beads machinery (bdCleanupStaleLocks, bdResetMtime, migration checks, refs migration, probe registration, fetchIssues/fetchStats) ONLY when caps.hasBeads; grace fetch when caps.hasGrace; isLoading resolves when all applicable fetches settle; dashboard empty-state card in DashboardContent when `!caps.hasBeads && caps.hasGrace` ("No beads registry in this project — GRACE explorer active"); PathSelector scan probes `.beads` per project (cached Map) before bdList — grace-only projects skipped entirely (no bd CLI, no git), GRACE badge shown instead of count; FolderPicker: `hasGrace` from fsList, gate `:disabled="!hasBeads && !hasGrace"`, GRACE badge beside Beads/Dolt (entry + current dir).
- Refresh wiring: startListening called when either registry exists (Rust watches both); useChangeDetection handler on `beads-changed`: beads flow gated by caps.hasBeads, plus `grace.refreshIfChanged(path)` gated by caps.hasGrace; adaptive poll cheap-check runs bdCheckChanged (hasBeads) and graceCheckChanged (hasGrace), refreshing whichever changed; project switch clears grace state (generation guard like useIssues).

- [ ] **Step 1:** Wire caps + tab strip + panel/aside swap + mobile (desktop and mobile branches).
- [ ] **Step 2:** Capability-aware mount/handlePathChange + dashboard empty state + PathSelector skip + FolderPicker gate/badge.
- [ ] **Step 3:** Refresh wiring (watcher handler + polling).
- [ ] **Step 4:** Gates: `cargo test` (src-tauri), `bun run test`, `bun run build` — all PASS.
- [ ] **Step 5:** Smoke via `bun run tauri:dev` (report visual limits honestly if a display is unavailable): (1) grace-only: open `D:/repos/github.com/nivedano/grace5` — admitted via FolderPicker, GRACE explorer bare (no strip), 19 closed changes listed with scope 'all', detail renders contract/records/briefs/notes, log dialog opens, no bd CLI calls in backend log, no stuck Loading; (2) dual: temp beads project with `.grace5` fixture copied in (include one deliberately malformed change dir and one with kernel-version `0.8.0` in a second temp copy) — strip appears with counts, tab persists across project switch, filters/sort/search work, malformed flagged not hidden, pin banner shown, touching a `.grace5` file refreshes the list; (3) beads-only: unchanged behavior, no strip, no grace calls.
- [ ] **Step 6:** Commit `feat(grace): tab-strip integration, grace-only project path, live refresh`.

### Task 11: Docs, gates, land the plane

**Files:**
- Modify: `README.md` (GRACE explorer feature section: what it shows, tab rule, read-only guarantee, desktop-only), `CHANGELOG.md` (entry matching the file's existing format; do not bump package.json version unless the changelog convention requires it)

- [ ] **Step 1:** Write docs; delete any throwaway smoke scripts/fixtures created outside the repo fixtures dir.
- [ ] **Step 2:** Full gates: `cargo test`, `bun run test`, `bun run build` — all PASS; report outputs.
- [ ] **Step 3:** `bd close <feature-id>` (and any sub-issues filed).
- [ ] **Step 4:** Land the plane per AGENTS.md: `git pull --rebase` → `bd sync` → `git push` → `git status` MUST show up to date with origin; clear stashes; verify all changes committed AND pushed. If push fails, resolve and retry.
- [ ] **Step 5:** Handoff note: what shipped, fixture provenance, known limits (structural validation only, display-level counts, desktop-only grace).
