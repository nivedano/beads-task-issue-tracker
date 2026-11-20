# GRACE Explorer — Design

Date: 2026-11-20 · Status: approved by owner 2026-11-20 (approach A, read-only, no cross-linking, kernel-equivalent authorization) · App: beads-task-issue-tracker (Nuxt 4 SPA + Tauri 2)

## Intent and why

The owner runs GRACE 5 changes (kernel repo: `D:/repos/github.com/nivedano/grace5`, version 0.9.0) in pilot repositories alongside beads tasks. Today the tracker shows only beads issues; GRACE state under `.grace5/` is visible only through the kernel CLI. This feature adds a read-only explorer for GRACE artifacts and lifecycle state with the same ergonomics as the issue explorer — filters, sortable table, detail view — inside the existing desktop app, without touching kernel authority.

## Goals

- Center panel hosts a tab strip `Issues (n) | GRACE (n open)` when the opened project contains both a beads registry (`.beads/`) and a `.grace5/` folder. With only one registry, that explorer renders alone, no strip.
- GRACE explorer mirrors the issues explorer: search, filter dropdowns + active-filter chips, sortable table, right-panel detail view.
- Detail view: lifecycle stepper (Prepare → Approve → Execute → Review → Close), the contract's eight sections, checks with latest recorded verdicts, records timeline (approvals, runs with per-check receipts, reviews with findings, close), rendered `notes.md`, and open-raw-file/log actions.
- Grace-only projects can be added and opened (today the Add Project gate requires `.beads`).
- Live refresh: `.grace5/` is watched like `.beads/`; adaptive polling covers it too.
- Per-project persistence: active explorer tab, grace filters, sort, column config.

## Non-goals

- No writes to `.grace5/` of any kind: no approve, run, observe, close, or notes editing from the UI. Kernel gates stay CLI-only.
- No re-verification of kernel verdicts (hash bindings, counts rules, closure obligations). The explorer displays recorded data and flags contract-revision drift; the kernel remains the source of truth.
- No cross-linking between GRACE changes and beads issues (no linking convention exists yet; explicitly deferred by owner).
- No GRACE 4 (`.grace/`) support.
- No grace KPI dashboard: the left dashboard stays beads-centric; tab counts are the only grace aggregate.

## Anti-goals

- A second source of truth for approval state. Approval is never inferred; the UI shows the recorded decision verbatim.
- Any git operation in grace parsing (the kernel's `evaluate` uses git; the explorer must not).
- Requiring bun or a grace5 installation on the user's machine.

## Constraints

- Single-page app: `app/pages/index.vue` orchestrates three panels. Center `<main>` is lines ~1044-1098; right detail `<aside>` ~1101-1216; mobile branch ~1218-1381 with a hand-rolled button tab strip (`mobilePanel` state).
- No Tabs primitive exists in `app/components/ui/`; the app's only tab precedent is the hand-rolled mobile strip. Reuse that pattern; no new UI dependency.
- `fs_list` (src-tauri/src/lib.rs ~3249-3318) skips dot-prefixed entries and reports `hasBeads`/`usesDolt` per directory and for the current directory. Add `hasGrace` (`.grace5/` is a directory) to `DirectoryEntry` and `FsListResult`, and to the web-mode route `server/api/fs/list.get.ts` for parity.
- The project-add gate is `FolderPicker.vue:476` `:disabled="!hasBeads"` (NOT PrerequisitesCard, which is informational onboarding). Gate becomes `!hasBeads && !hasGrace`; add a GRACE badge beside the Beads/Dolt badges.
- Verified: this fork has no built-in tracker backend — `src-tauri/src/` holds only `lib.rs` and `main.rs`; nothing reads `.tracker/`. `.claude/codebase-map.md` is stale on this point; do not trust it for backend facts. Beads registry means `.beads/` only.
- Grace-only projects need a capability-aware path, not just fetch gating: `PathSelector.vue:33` calls `bdList({path, includeAll:true})` for EVERY listed project's issue count (which runs `bd sync` — a git operation); grace-only projects must be skipped in that scan and show a GRACE badge instead of a count. `isLoading` currently clears inside `fetchIssues` and the dashboard shows "Loading…" while `stats` is null; for grace-only projects loading must complete via the grace fetch and the dashboard shows an empty-state card ("No beads registry in this project — GRACE explorer active"), no grace KPIs. Mount and `handlePathChange` skip migration checks, refs migration, probe registration and beads mtime reset when `.beads` is absent.
- `start_watching` (lib.rs ~4730-4908) hard-fails when `.beads` is absent and watches only `.beads`. It must support grace-only projects (watch `.grace5`) and dual projects (watch both). `handlePathChange` (index.vue ~586-666) and mount currently assume beads: skip beads fetch/watch when `.beads` is absent.
- `read_text_file`/`write_text_file` are sandboxed to `.beads/attachments/*.md` and cannot serve grace artifacts. New read-only command `grace_read_file` canonicalizes and validates the path stays inside `<project>/.grace5/`, with a size cap for logs (truncation reported, never silent).
- Grace state layout is version-pinned (`.grace5/kernel-version`). The explorer targets the 0.9.0 layout; a different pin shows a visible warning banner but still attempts to read (read-only browsing must not brick on pin mismatch).
- XML parsing: add `roxmltree` (read-only, zero-transitive-dep choice) to `src-tauri/Cargo.toml`. Malformed is structural only: XML parse failure; missing contract root `id`/`revision`/`title` attributes; change id not matching its directory name; record number not matching its filename; a `closed/` directory without `close.xml`; unknown record filenames. No XSD validation and no semantic defect reproduction (criterion/check references, counts, closure obligations). A standing note in the GRACE tab states: structural read-only view — `grace5 check`/`status` remains authoritative. Malformed changes are flagged and never hide valid ones.
- Frontend data access follows `app/utils/bd-api.ts`: `isTauri()` → `invoke`, else web fallback or throw. Per-project settings follow `useProjectStorage` (`beads:proj:{hash}:{key}`).
- No runtime dependency on the grace5 sibling repo. Test fixtures are copied into this repo.

## Approaches

### A — Center tab strip (chosen, owner's proposal refined)

Tab strip above the center panel, rendered only when both registries exist; active tab persisted per project; right detail panel swaps content with the tab. GRACE explorer mirrors the issues explorer structure (toolbar + chips + table; detail in right panel). Strip reuses the mobile-nav hand-rolled button pattern.
Tradeoff: one context at a time; cleanest fit for the existing three-panel layout and the owner's explicit tab semantics.

### B — Unified table with registry filter facet (rejected)

One explorer, `All / Issues / GRACE` scope toggle. The two schemas share almost no columns or filters, so this becomes two tables wearing a toggle, and the "one registry → no chrome" rule degenerates into a hidden filter.

### C — Split view, grace panel beside/below issues (rejected)

Both registries visible at once, but 1400×900 with three panels is already dense; contract text needs width and a half-height table serves neither explorer.

### Data access — native Rust parsing (chosen over CLI bridge)

New Rust module parses `.grace5/` directly (contracts, records, notes). The alternative — shelling to `grace5 status` — was rejected: the kernel is unpublished (pilots run from a checkout), requires bun + a configured path, emits plain text, and its full evaluation depends on git. Native parsing has no install requirement and no git calls.

## Decided by owner

- Approach A with the stated tab semantics (strip only when two or more registries; single registry renders bare).
- Read-only explorer.
- No cross-linking for now.
- Kernel-equivalent authorization (option a): `sha2` crate, raw-byte SHA-256 with CRLF→LF only, approval for the contract's current revision, the kernel's four values and branch order. The explorer must never disagree with `grace5 status` on authorization.

## Director calls (defaults the owner may reject)

- **Web mode: grace commands are desktop-only**, throwing "only available in the desktop app" exactly like the bd commands in bd-api.ts. The `fs_list` server route still gains `hasGrace` so folder-picker badges work on web; the grace explorer itself shows a desktop-only notice in web mode.
- Authorization is kernel-equivalent (owner decision): SHA-256 over the raw `contract.xml` bytes with ONLY `\r\n` pairs replaced by `\n`, byte-preserving (latin1 round-trip equivalent — never decode to a UTF-8 string first; non-ASCII contract text would mismatch); approval record looked up by the contract's CURRENT revision; branch order: no such approval → `unapproved`; decision `rejected` → `rejected`; recorded `contract-sha256` ≠ current hash → `altered after approval`; else `approved` (kernel refs: grace5 `src/contract.ts:86-89`, `src/state.ts:128-133`). The authorization filter uses exactly these four values.
- Stage is lifecycle PROGRESS, not verdict: `closed` (close.xml exists) → `reviewed` (a completed review record exists) → `executing` (run records exist) → `approved` (authorization is approved) → `prepared` (contract exists, no approval for the current revision). Governing approval always means the approval record for the contract's current revision. Stage and authorization may legitimately differ (e.g. a reviewed change whose current revision was rejected): both are shown, neither hides the other; the stepper lights progress, the authorization badge shows authority. Check counts and review adequacy stay display-level — never re-verified.
- Legacy `records/brief-<rev>.html` pages are part of the records model and timeline, listed with an open-externally action (raw HTML in the OS browser via the existing `openUrl` util); never rendered inline. Legacy approvals display `brief-sha256` when present.
- Table columns: id, title (contract `title` attribute), stage, authorization, checks (latest passed/required counts), review (status + open findings count), revision. Default sort: id descending. Open changes shown by default; closed reachable via the open/closed filter (mirrors issues defaulting to open statuses).
- Filters: stage, authorization, open/closed, plus search across id/title/Problem text. Same dropdown + chip style as issues.
- Detection trigger: presence of the `.grace5/` directory itself (matches the owner's "contains the grace5 folder" wording), not merely `changes/`.
- Per-project storage keys: `explorerTab`, `grace:filters:*`, `grace:sort`, `grace:columns`.
- Pure frontend logic (stage derivation, filtering, sorting) lives in `app/utils/grace-helpers.ts` with vitest coverage, matching the existing pure-helper test strategy. Rust grace module carries `#[cfg(test)]` fixture tests; fixtures copied from grace5's worked examples and closed changes.
- Mobile: the existing mobile strip gains the GRACE entry under the same two-registry rule; detail navigation reuses `mobilePanel = 'details'`.

## Left to the builder

- Exact Rust module layout and command names (`grace_list`, `grace_show`, `grace_read_file`, change-detection integration), watcher event wiring (extend `beads-changed` vs separate `grace-changed`), and adaptive-polling mtime integration.
- Component decomposition under `app/components/grace/` (list panel, toolbar, table, detail, badges, stepper, records timeline), XML→TypeScript type shapes, and the `useGrace` composable mirroring `useIssues` patterns.
- Malformed-change and pin-mismatch presentation, log viewing UX within the size cap, keyboard-navigation parity, and empty states.
- Smoke verification plan: grace-only project (the grace5 repo itself), dual-registry project (beads project with a `.grace5/` fixture), beads-only project (no strip, unchanged behavior).

## Verification

- `cargo test` in src-tauri (grace module fixture tests), `bun run test` (vitest grace-helpers), `bun run build` or typecheck as the repo's gates define.
- Manual UI smoke via `tauri dev` on the three project shapes above; verify tab strip appearance rule, filters, detail rendering, live refresh on `.grace5/` file change, and grace-only project admission.
