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

Trailing sentence punctuation (`. , ; : ! ?`) and unmatched closing brackets or
quotes are trimmed from a match and stay plain prose. Balanced brackets inside a
path (`models/[id]/method.md`) are preserved. A ref is not started inside a word
(`and/or`, `xhttps://…`).

### 2. `hl-docid` — documentation ids

Violet pill: tinted rounded background plus violet foreground. The matched text
is also exposed as a `title`.

- `UC-<digits>` — use cases
- `DR-<SECTION>…-<digits>` — decisions, e.g. `DR-MSR-001`
- `C-<3–6 digits>` — GRACE change ids, e.g. `C-0019`
- `iss-<alnum/hyphen>` — issue ids

All are word-boundary anchored; `C-12`, `C-1234567`, `DR-msr-001` and `xUC-12`
do not match.

### 3. `hl-lexicon` — architecture lexicon terms

Sand/amber accent, medium weight. A dotted token is highlighted only when it is
**defined in the project's term lexicon** (exact membership check). The candidate
shape is `X.Y` or `X.Y.Z` — a leading letter, then one or two dotted segments,
e.g. `M1.2` or `arch.method.v2`. Four-segment chains, decimals (`1.2.3`) and
tokens that merely look dotted (`plain.text` without a lexicon entry) stay plain.

## Lexicon file

Location: `<project>/models/architecture/term-lexicon.yaml`. If the file does not
exist (or is unreadable) only classes 1 and 2 highlight — no error is shown.

Accepted YAML subset (dependency-free parser):

```yaml
# Option A — a `terms:` key with a sequence
terms:
  - M1.2          # comments are stripped
  - arch.method.v2
  - "quoted.term" # surrounding quotes are stripped

# Option B — a bare top-level sequence
- M1.2
- arch.method.v2
```

Entries that do not match the `X.Y` / `X.Y.Z` identifier shape are ignored.
Unsupported structures (mappings, block scalars, other keys) are skipped
gracefully, so a partially understood file still contributes what it can.
Duplicates collapse.

## Theme variables

Added for every theme; neon/flat rules deliberately come after `.dark` because
those themes also carry the `dark` class.

| Theme | `--hl-ref` | `--hl-lexicon` | `--hl-docid` |
|---|---|---|---|
| light (default) | `#3b82f6` | `#b8860b` | `#8b5cf6` |
| `.dark` | `#60a5fa` | `#e0c080` | `#a78bfa` |
| `[data-theme="flat"]` | `#60a5fa` | `#e0c080` | `#a78bfa` |
| `[data-theme="neon"]` | `#00d4ff` | `#ffaa00` | `#bf5fff` |

Class rules (`.hl-ref`, `.hl-lexicon`, `.hl-docid`) live next to the markdown
base styles; scoped `.markdown-base .hl-*` rules keep prose containers from
overriding a span's own color.

## Wiring

`app/plugins/text-highlight.client.ts` registers `v-highlight` with a directive
that keeps a registry of mounted elements. The shared set comes from
`app/composables/useLexicon.ts`; `app/pages/index.vue` calls
`loadLexicon(beadsPath)` whenever the opened project changes. On a lexicon
change the directive re-walks every registered element, unwrapping spans it
created (`data-text-highlight`) and re-tokenizing. Re-running is idempotent:
no nesting, no duplicates.

The directive skips `PRE`, `CODE`, `A`, `SCRIPT`, `STYLE`, `TEXTAREA` and any
existing `span.hl-*` subtree. It clears inside `beforeUpdate` and re-applies in
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
parity route `server/api/grace/lexicon.get.ts`; both canonicalize under the
project root and cap at 1 MiB. The frontend wrapper is `graceReadLexicon` in
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

- Tables (`grace/GraceChecksTable.vue`, markdown tables) are not highlighted.
- Code blocks and inline code are skipped by the directive (`PRE`/`CODE`).
- Log viewers, ID cells, SHA/fingerprint fields and other machine cells are not
  wiring targets.
- Beads issue ids in prose are not a class (`iss-…` is the GRACE/issue
  documentation-id spelling; bare `beads-123`-style tokens are left alone).