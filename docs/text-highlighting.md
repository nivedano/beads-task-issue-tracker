# Prose syntax highlighting

GRACE explorer notes/contracts/records and issue prose (descriptions, comments,
design notes, acceptance criteria, working notes) get three kinds of inline
highlighting. The set of classes, the detection procedure and the styles are
separate layers:

| Layer | Where |
|---|---|
| Class definitions (data) | `app/utils/text-highlight.ts` → `HIGHLIGHT_CLASSES` |
| Detection (single-pass tokenizer) | `app/utils/text-highlight.ts` → `tokenizeHighlights` |
| DOM application | `app/utils/text-highlight-dom.ts` + `v-highlight` directive |
| Per-class styles + theme vars | `app/assets/css/tailwind.css` |

Sanitized markdown never lets `<span>` through (`renderMarkdown`'s DOMPurify
allow-list), so highlighting is always a **post-render DOM walk** over text
nodes, never injected HTML.

## Classes

Precedence at the same start offset is **ref > docid > lexicon**; across
offsets the leftmost match wins. Plain text is emitted untouched.

### 1. `hl-ref` — local paths and URLs

Underlined, blue foreground.

Matches:

- `http://…`, `https://…` and `www.…`
- Windows drive paths `C:\…` and `C:/…`
- UNC shares `\\server\share\…`
- Unix absolute paths `/seg/seg…`
- Explicit relative paths `./…`, `../…`, `~/…`
- Bare relative file paths carrying an extension (`models/architecture/method.md`)
- Bare directory paths ending in `/` (`models/architecture/`)

Bare relative matches are bounded: each path segment is 1–64 characters, the
extension is consumed whole (`foo/bar.typescript` highlights in full), and an
end guard refuses to stop mid-token (`a/b/c`, `models/architecture/and` stay
plain; `models/architecture/` followed by a space still highlights). Quotes are
never part of a ref match; trailing sentence punctuation (`. , ; : ! ?`) and
unmatched closing brackets are trimmed from a match and stay plain prose.
Balanced brackets inside a path (`models/[id]/method.md`) are preserved. A ref
is not started inside a word (`xhttps://…`).

Known limitation: a prose fragment that is itself path-shaped with a trailing
slash (e.g. `and/or/`) is indistinguishable from a directory path and is
highlighted. Accepted iteration-1 tradeoff; see the exclusions below.

### 2. `hl-docid` — documentation ids

Violet pill: tinted rounded background plus violet foreground. Every `hl-*`
span (all three classes) carries a `title` with the matched text.

- `UC-<digits>` — use cases
- `DR-<SECTION>…-<digits>` — decisions, e.g. `DR-MSR-001`
- `C-<3–6 digits>` plus the optional lowercase change slug — GRACE change ids,
  e.g. `C-0019` and `C-0019-retire-briefs` highlight whole (the slug is the
  artifact directory name)
- `iss-<alnum/hyphen>` — issue ids

All are word-boundary anchored and partial tokens never match: `C-12`,
`C-1234567`, `DR-msr-001`, `xUC-12`, `UC-12-3` and `DR-MSR-001-followup` stay
plain.

### 3. `hl-lexicon` — architecture lexicon terms

Sand/amber accent, medium weight. A dotted token is highlighted only when it is
**defined in the project's term lexicon** (exact membership check). The candidate
shape is `X.Y` or `X.Y.Z` — a leading letter, then one or two dotted segments,
e.g. `M1.2` or `arch.method.v2`. Four-segment chains, decimals (`1.2.3`) and
tokens that merely look dotted (`plain.text` without a lexicon entry) stay plain.

## Lexicon file

Discovery: the app walks `<project>/models/` breadth-first and reads the first
`term-lexicon.yaml` (or `term-lexicon.yml`) it finds. The shallowest file wins;
at equal depth the lexicographically smallest path relative to `models/` wins.
Directories starting with `.` and `node_modules` are skipped, symlinked
directories are never descended, the search stops 8 directory levels below
`models/`, and a symlinked file is read only when it resolves inside the
project. If no file exists (or it is unreadable) only classes 1 and 2
highlight — no error is shown.

Accepted YAML shapes (dependency-free parser). Only identifier-shaped keys or
scalars become terms — `X.Y` / `X.Y.Z`, e.g. `M1.2` or `arch.method.v2`;
anything else (`version:`, `notes:`, prose) is ignored:

```yaml
# 1. top-level sequence of scalars
- M1.2
- arch.method.v2

# 2. terms: block — scalars or single-key mappings
terms:
  - M1.2          # comments are stripped
  - "quoted.term" # surrounding quotes are stripped
  - M1.3: gloss

# 3. top-level mapping whose keys are the terms
M1.2: gloss text
M1.3:
  Free-text definition (ignored, as is every indented line under a term).
```

A `terms:` block may also be written as a mapping or as a flow sequence
(`terms: [M1.2, M1.3]`). Definition text and nested keys never contribute
terms: indented lines belong to the current key/entry until the next column-0
key. YAML document markers (`---`, `...`) carry no terms and are skipped, CRLF
files parse the same, and unsupported structures are skipped gracefully, so a
partially understood file still contributes what it can. Duplicates collapse.

## Theme variables

Added for every theme; neon/flat rules deliberately come after `.dark` because
those themes also carry the `dark` class.

| Theme | `--hl-ref` | `--hl-lexicon` | `--hl-docid` |
|---|---|---|---|
| light (default) | `#2563eb` | `#8a6508` | `#7c3aed` |
| `.dark` | `#60a5fa` | `#e0c080` | `#a78bfa` |
| `[data-theme="flat"]` | `#60a5fa` | `#e0c080` | `#a78bfa` |
| `[data-theme="neon"]` | `#00d4ff` | `#ffaa00` | `#bf5fff` |

Light-theme values are darkened to reach ≥ 4.5:1 contrast on `#ffffff`
(5.17, 5.32 and 5.70 respectively); dark, flat and neon keep the lighter
accents on their dark backgrounds.

Class rules (`.hl-ref`, `.hl-lexicon`, `.hl-docid`) live next to the markdown
base styles; scoped `.markdown-base .hl-*` rules keep prose containers from
overriding a span's own color.

## Wiring

`app/plugins/text-highlight.client.ts` registers `v-highlight` with a directive
that keeps a registry of mounted elements. The shared set comes from
`app/composables/useLexicon.ts`; `app/pages/index.vue` calls
`loadLexicon(beadsPath)` whenever the opened project changes. On a lexicon
change the directive re-walks every registered element, restoring the text
nodes it rewrote (tracked per root in a `WeakMap`) and re-tokenizing. Re-running
is idempotent: no nesting, no duplicates.

The directive skips `PRE`, `CODE`, `A`, `SCRIPT`, `STYLE`, `TEXTAREA`,
`TABLE` and any existing `span.hl-*` subtree, so markdown tables in
`GraceNotesView`/`LinkifiedText` stay plain. A registered root is tagged with
`data-highlight-root`; an outer walk never enters a nested root, and unmounting
either root in either order leaves the other's text intact. It clears inside `beforeUpdate` and re-applies in
`updated`, preserving the original text-node identity so Vue's interpolated
`{{ }}` prose patches the node it still holds a reference to.

Bound surfaces:

- `ui/linkified-text/LinkifiedText.vue` (issue description, comments, design
  notes, acceptance criteria, working notes, dashboard prerequisites)
- `grace/GraceNotesView.vue` (rendered notes markdown)
- `grace/GraceContractView.vue` (problem/change/constraints/goals/acceptance/
  affected/decisions prose)
- `grace/GraceRecordsTimeline.vue` (approval statements, run reason and
  observation note)

Backend reads: Rust `read_lexicon` (`src-tauri/src/grace/mod.rs`) and the web
parity route `server/api/grace/lexicon.get.ts`; both discover the file under
`models/` with the same rules, canonicalize under the project root and cap at
1 MiB. The route's walk and containment check live in
`server/utils/lexicon-path.ts` (`path.relative` semantics) so a project that
*is* a filesystem root (`D:\`, `/`) is accepted; symlink escapes are refused by
`realpath` + containment. The frontend wrapper is `graceReadLexicon` in
`app/utils/br-api.ts` (invoke in Tauri, `$fetch` in web).

## Adding a class

1. Append a `{ cls, token, pattern }` entry to `HIGHLIGHT_CLASSES` in
   `app/utils/text-highlight.ts` (order = precedence at the same offset).
2. Add `--hl-<name>` theme vars to every theme block in `tailwind.css` and a
   `.hl-<name>` rule.
3. Add the class to `HighlightClass`, extend tokenizer tests, and mention it in
   the table above.

The directive picks up the new class automatically through
`HIGHLIGHT_CLASS_NAMES`.

## Iteration-1 exclusions

- Tables (`grace/GraceChecksTable.vue`, markdown tables) are not highlighted;
  the directive skips the whole `TABLE` subtree.
- Path-shaped prose with a trailing slash (`and/or/`) highlights as a directory.
  Accepted tradeoff: distinguishing it from a real directory needs sentence
  context the tokenizer deliberately does not model.
- Code blocks and inline code are skipped by the directive (`PRE`/`CODE`).
- Log viewers, ID cells, SHA/fingerprint fields and other machine cells are not
  wiring targets.
- Beads issue ids in prose are not a class (`iss-…` is the GRACE/issue
  documentation-id spelling; bare `beads-123`-style tokens are left alone).