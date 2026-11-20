GRACE 5 fixture provenance
===========================

`C-0019-retire-briefs/` holds verbatim byte copies from
`D:/repos/github.com/nivedano/grace5/.grace5/changes/closed/C-0019-retire-briefs/`
in the grace5 kernel repository, kernel version `0.9.0`, copied on 2026-11-20:

- `contract.xml`
- `notes.md`
- `records/approval-1.xml`
- `records/run-1.xml`
- `records/run-3.xml`
- `records/review-1.xml`
- `records/close.xml`
- `records/brief-1.html`

(`records/logs/` was deliberately not copied; the record parser only derives
log directory names from it.)

The copies are unmodified: line endings, encoding and content are identical.
`contract.xml` hashes to `475e960d00ca71cd5b021dd98fd95438fb267b1997d5375ef1313fcbbb168000`,
the hash recorded in the change's `records/approval-1.xml`. They are used only by
Rust unit tests; no runtime code reads this directory.
