# GRACE Explorer — User Smoke Checklist & Handoff

Shipped: a read-only GRACE 5 explorer beside the beads issue explorer (`feat(grace)` commits
`d0927b3`..`9fbb8f7`, on top of the plan/spec docs). This document is the manual verification
script for the feature and the handoff for the parked follow-ups. The workspace reports that fed
it were removed after landing; everything actionable is here.

## What shipped (one paragraph)

The app parses `.grace5/` directly (no grace5 install, no git): a center tab strip
`Issues (n) | GRACE (n)` appears only when a project has **both** `.beads/` and `.grace5/`; with a
single registry that explorer renders bare. The GRACE changes table shows stage, kernel-equivalent
authorization, latest `passed/required` checks, review status/findings and revision, with stage /
authorization / open-closed filters, search, per-project sort and column config, and keyboard row
navigation. The detail panel shows the lifecycle stepper, contract sections, checks with latest
recorded verdicts and an in-app log viewer, records timeline (approvals, runs, reviews, close,
briefs), and rendered `notes.md`. Live refresh covers `.grace5/` via the existing watcher and
adaptive polling; grace-only projects are admissible and never run `bd`/`git`. Nothing is written
to `.grace5/`, and `grace5 check`/`status` remains authoritative (structural display only).

## Fixtures

Real grace-only project (owner repo, 19 closed changes, pin 0.9.0):

- `D:/repos/github.com/nivedano/grace5`

Throwaway fixtures (OS temp; safe to delete after verification):

- `C:/Users/yehor/AppData/Local/Temp/grace-task10-graceonly` — grace-only; copy of
  `grace5/.grace5` plus one valid open change `changes/C-0100-smoke-open` and one malformed
  `changes/closed/C-0900-broken` (contract missing the required `title`); pin 0.9.0.
- `C:/Users/yehor/AppData/Local/Temp/grace-task10-dual` — the same `.grace5` plus a real
  `.beads` workspace (`br init`).
- `C:/Users/yehor/AppData/Local/Temp/grace-task10-dual-pin080` — same as dual with
  `.grace5/kernel-version` = `0.8.0`.

Beads-only project for shape 3: any existing beads checkout (e.g. one already in the project
list).

## What to look for, by shape

### 1) Grace-only — `D:/repos/github.com/nivedano/grace5`

1. **Admission**: Add Project → navigate to the repo. The picker header shows a violet **GRACE**
   badge (no beads badge) and **Add Project is enabled** (desktop only; web mode shows no grace
   affordance).
2. **Bare explorer**: the center shows the GRACE explorer with **no Issues|GRACE strip**.
3. **Sidebar**: left dashboard shows “No beads registry in this project — GRACE explorer active”
   (never “Loading...”). The project list row shows a **GRACE** chip instead of an issue count.
4. **No bd/git**: the backend log shows no `br`/`bd` invocation for this project, and
   `[watcher] Started watching: …\grace5\.grace5`.
5. **List**: scope defaults to Open → 0 rows. Switch scope to **All** → **19 closed changes**.
6. **Detail** (`C-0019-retire-briefs`): stepper lit to CLOSED; contract sections Problem, Change,
   Constraints (with rule citations), Non-Goals, Acceptance (check chips), Affected
   (paths/names/protected), Environments, Decisions; Checks CK1..CK8 with latest verdicts;
   Records (approval-1, run-3, review-1, close, brief-1); Notes rendered markdown; the standing
   note includes the authorization bound (“matches the kernel for schema-valid records…”).
7. **Log dialog**: “View log” on a check opens the in-app dialog (no truncation at this size).
8. **Artifact open**: “Open contract.xml”, “Open notes.md”, a brief’s “Open externally” →
   system default app. (The host `.txt` handler was known broken during development; verify with
   your real handlers.)
9. **Live refresh**: append a newline to `notes.md` → list/detail refresh within ~1 s
   (watch the debug panel too). Ctrl+F focuses the GRACE search box.

### 2) Dual fixtures — strip, filters, malformed, pin

1. Add `grace-task10-dual`: strip appears `Issues (n) | GRACE (1)`; GRACE lists the single open
   change by default.
2. Switch to another project and back: the strip disappears/reappears and the **GRACE tab is
   remembered per project** (also across app restarts).
3. Scope All → 20 closed (19 valid + `C-0900-broken`) + 1 open. Exercise stage/authorization
   filters, search (`smoke`, `broken`), sort by title/stage/revision asc/desc, column config
   (toggle, reset), and Clear (chips disappear, scope resets to All after Clear).
4. `C-0900-broken` is listed with the red defect icon/tooltip and shows the “Structural defects”
   banner in detail — **never hidden**.
5. Keyboard: Tab into the table, arrow up/down moves the focus ring, Enter opens the detail.
6. Add `grace-task10-dual-pin080`: amber banner in list **and** detail:
   `GRACE state pin 0.8.0 — explorer targets 0.9.0; structural view only`. Changes still display.
7. Touch a `.grace5` file in the dual project → GRACE refreshes; the **issues list does not
   reload** (debug panel shows no beads fetch).
8. Sanity: duplicate ids cannot occur in these fixtures; the list flags a closed-side duplicate
   with the kernel defect string and keeps both rows (covered by Rust tests).
9. Empty project: open a folder with neither registry → sidebar says “No beads or GRACE registry
   in this project” with the `.beads`/`.grace5` hint (no permanent “Loading...”).

### 3) Beads-only — unchanged behavior

1. No strip; Issues renders as before; no GRACE entry anywhere; backend log shows **no
   `grace_*` calls**.
2. Resize below 1024 px: mobile nav shows Dashboard/Issues/Details only.
3. Grace-only mobile (shape 1 at narrow width): nav shows a GRACE entry (no Issues entry),
   dashboard shows the grace-only card, selecting a change navigates to the details panel.

### Badge appearance per theme

Check Classic Light, Classic Dark, Dark Flat, and Neon: EXECUTING and ALTERED AFTER APPROVAL use
amber with **black** text (AA-safe); PREPARED/APPROVED/REJECTED/REVIEWED/CLOSED stay legible.
A full migration of these fixed palette classes to theme tokens is parked (see Follow-ups).

## Known limits (accepted)

- Structural validation only: malformed means parse/shape defects; no XSD or semantic re-check and
  no re-verification of verdicts, counts, or closure obligations.
- Check counts and review adequacy are display-level.
- GRACE is desktop-only; the web build keeps beads only and shows no grace affordance.
- `useGrace` has one shared `error` ref: a detail-fetch failure can also render the list banner
  (`GRACE error: …`) until the next successful fetch.
- The 3 s watcher self-write cooldown can delay a grace event that arrives right after a beads
  poll; the 30 s safety poll still refreshes it.
- On the degraded path (watcher unavailable) the 1 s fast check uses a per-project fingerprint
  memo (~1 s TTL), so a change can be observed up to ~1 s late.

## Follow-ups (not in this branch)

1. **`open_image_file` quoting** (`src-tauri/src/lib.rs` ~`open_image_file`): it still passes the
   path as a plain `cmd /C start` argument; apply the same `raw_arg` + quoting used by
   `grace_open_file` (an `&` in a filename can split the command). Pre-existing.
2. **`issues/ColumnConfig.vue` outside-click bug**: the popover trigger ref points at a component
   instance, so `onClickOutside`’s `.contains` check can throw; put the ref on a wrapper element
   (same fix as `GraceToolbar`). Pre-existing.
3. **Duplicate-id detail banner**: `grace_show` resolves the open-side directory, so opening the
   flagged closed-side duplicate shows the open-side detail without a duplicate defect; add the
   ~6-line location check in `load_change`/`grace_show` so the detail banner agrees with the list.
4. **Badge theme-token migration**: replace the fixed palette badge classes with theme tokens;
   needs visual review in all four themes.
5. **Release step**: `CHANGELOG.md` currently has an `## [Unreleased]` section. The release commit
   must stamp it with the version + date and bump `package.json`, `src-tauri/Cargo.toml` and
   `src-tauri/tauri.conf.json` together (repo convention).
6. **Symlink-guard tests**: the list-time symlink flagging test self-skips where directory
   symlinks cannot be created (Windows without the privilege); it needs a capable CI runner to
   execute.
7. **GUI smoke**: this checklist itself (the development session had no GUI input channel).

After verification, delete the three temp fixture directories listed above.