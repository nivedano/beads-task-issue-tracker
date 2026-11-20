# Implementation notes

Prepared 2026-11-17. No brief is written, per the requested retirement. No approval is recorded; execution has not started.

## Bootstrap limitation

The current kernel requires a brief for a new approval: `approval.xsd` requires `brief-sha256`, and `state.obligation` and `close` require a brief for the current revision. This change removes that requirement, but it cannot itself be approved under the old rule without a brief.

2026-11-17: the owner approved the complete prepared set, including this change, on its contract. That statement is the explicit bootstrap decision for this change: no brief is written, and the approval is bound to the contract and check hashes above. The kernel cannot represent this decision until the briefless path is implemented, so the `approval-1.xml` record is written under the new schema after the implementation, before final evidence and review. If the owner instead requires the old-rule path, a one-time brief for this change only is the alternative; none is inferred.

## Scope

- Remove the brief from the approval surface: new approval records bind the contract and check definitions only.
- Keep one findings-only independent contract read before the owner gate; no page, no fidelity parser, no owner-facing brief.
- Keep legacy briefs, approvals and close records readable; validate a brief only when the approval carries `brief-sha256`.
- Remove the active brief documents/template and the brief writer/handoff guidance.
- Keep checks, receipts, independent complete-diff review, protected paths and closure unchanged.

## Open-set sequencing

C-0012, C-0016, C-0017 and C-0018 are prepared with briefs and protect `brief.md` or `templates/**`. This change must execute after they close, or after each change's own approved material revision removes the protection; otherwise its writes to those paths appear in their complete diffs as protected-path violations. Their briefs become history and are not deleted or rewritten.

C-0016 writes feasibility and discrimination guidance into `brief.md`. Before this change deletes that file, the same requirements must be relocated into `skills/grace5-prepare/SKILL.md` and the contract's own sections. If C-0016 closed first, its `brief.md` text is history and C-0019 re-expresses the requirements; if it is still open at C-0019's start, this change waits.

## Independent contract read

Preparation retains the independent finding channel without the artifact. An independent subagent that did not write the contract receives the contract, the cited rule files and read access, and returns findings only: a criterion its check cannot serve, an undefined term, a contradiction, a boundary that does not cover Change, or a cited path that does not exist. The author resolves each finding by editing the contract before the owner gate. No HTML page, `grace5:writer` metadata or fidelity parser is produced. The final independent review remains separate and unchanged.

## Consumer surface

- `src/brief.ts` remains the legacy parser only.
- `src/state.ts` and `src/commands/close.ts` distinguish a legacy approval with `brief-sha256` from a new approval without it.
- `approval.xsd` and `close.xsd` make the brief fields optional; `src/records.ts` writes new records without them.
- `commands.md`, the CLI help and the generated `cli-reference.md` drop the brief refusal phrases and the brief check rule.
- `examples/defective-brief` remains a legacy fixture; `examples-verify.ts` may keep it on the legacy path.
- Conformance FS-12, FS-14, RV-04 and RV-10 are revised or retained as legacy cases; a new `briefless.test.ts` covers the new path.
- `models/architecture/m2-proposal.md` records the brief-metric gap; sysops `grace/optimize/METHOD.md` and `graceopt` are cross-repo consequences and are not changed here.
- Rationale ledger L-04 is retired and L-05 is amended to the contract-only binding.

## Implementation outline

1. `models/architecture/schemas/approval.xsd`: make `brief-sha256` optional; comment that it is legacy. `close.xsd`: make `brief` optional. Keep every other attribute and element.
2. `src/records.ts`: make the approval brief hash and the close brief name optional; write a new close record without `brief`.
3. `src/state.ts`: validate a brief only for an approval with `brief-sha256`; skip the brief step for a new approval; keep the altered-after-approval detection and every Bound check.
4. `src/commands/close.ts`: do not require a brief for a new approval; include `brief` in the close record only when legacy.
5. `src/commands/run.ts`: ensure a new briefless revision is not held by a brief defect; a legacy brief still holds once its revision is approved.
6. `src/brief.ts`: keep as the legacy parser; no new brief is authored.
7. `skills/grace5-prepare/SKILL.md`: remove the independent brief writer and the brief owner-gate step; the owner reads the contract and answers approve/reject. Add the findings-only independent contract read: a subagent that did not write the contract receives the contract, cited rules and read access, returns findings, and the author resolves them before the owner gate. Keep the interview, contract, checks and owner gate.
8. Relocate C-0016's feasibility and discrimination requirements from `brief.md` into `skills/grace5-prepare/SKILL.md` and the contract sections before deleting `brief.md`.
9. `skills/grace5-orchestrate/SKILL.md`, `grace5-execute/SKILL.md`, `grace5-review/SKILL.md`: remove brief handoff references; the reviewer still receives the contract, complete diff, receipts and cited rules.
10. `models/architecture/method.md`, `authorization.md`, `evidence-records.md`, `commands.md`, `conformance.md`, `m2-proposal.md`, `README.md`, `AGENTS.md`: state the contract-only approval, the independent contract read, legacy brief handling and the measurement consequence.
11. Delete `models/architecture/brief.md` and `models/architecture/templates/brief.html` from the active surface; keep `models/architecture/examples/**` untouched as historical fixtures.
12. Amend rationale-ledger L-04 (retired) and L-05 (contract-only binding).
13. Add `src/conformance/briefless.test.ts` for the new path and the legacy path; update existing brief-dependent tests so historical cases still pass.
14. Regenerate `models/architecture/cli-reference.md` and `packages/`; run CK1-CK8.

## Independent contract read: resolved findings

A findings-only independent read of the 2026-11-17 contract returned two mechanical issues before the owner gate, both resolved in the contract:

- A2 named CK3 and CK7, which validate records but do not exercise legacy `run`/`close`; A2 now names CK2 and CK6.
- CK5 did not inspect `models/architecture/commands.md`, so a stale brief refusal phrase there would pass; CK5 now includes that file.
- The read's K7/D10 observation was checked against the seven contracts: all seven protect `models/architecture/templates/**`, and C-0012, C-0014, C-0017 and C-0018 protect `models/architecture/brief.md`; K7 and D10 now state that set exactly.

## Measurement consequence

The control center's M2 metrics read brief metadata (`grace5.brief.*`, `grace5.brief.writer_self`). New changes will have no brief, so those metrics become unavailable for them. `models/architecture/m2-proposal.md` records the gap; `grace/optimize/METHOD.md` and `graceopt` are outside this repository and are not changed here.

## Verification focus

- CK2/CK6: a new change with no brief approves, starts, runs, reviews and closes; a legacy brief still validates and a mismatched legacy brief still refuses.
- CK5/CK8: the active guidance and surface no longer instructs or carries a brief, and the prepare skill names the independent findings-only contract read.
- CK6: the new test covers the briefless approval/run/close path, the legacy brief path, the altered-after-approval path and the unbound-check path.
- CK3/CK7: examples and every closed change remain clean.
- CK1/CK4: typecheck, generated packages and CLI reference remain current.
- The change registers no version, pin, tag or migration row.
