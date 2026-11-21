/**
 * Prose highlighting classes, tokenizer and the dependency-free lexicon parser.
 *
 * Pure module: no DOM, no Vue, no I/O. The DOM walk lives in
 * `~/utils/text-highlight-dom`; styles and theme variables in
 * `app/assets/css/tailwind.css`. See `docs/text-highlighting.md`.
 */

export type HighlightClass = 'hl-ref' | 'hl-lexicon' | 'hl-docid'

export interface HighlightSegment {
  text: string
  cls?: HighlightClass
}

/**
 * Class definitions as ordered data. The order is the precedence order at the
 * same start offset (leftmost match still wins across offsets). The lexicon
 * class carries no static pattern: its alternatives are built from the term
 * set supplied to `tokenizeHighlights` (see `lexiconPattern`).
 */
const SEGMENT = String.raw`[A-Za-z0-9_.()\[\]{}-]{1,64}`
// A path match may not stop in the middle of a token: no trailing word char,
// no further slash, and no `.alnum` continuation (sentence periods are allowed).
const REF_END = String.raw`(?![A-Za-z0-9_/-])(?!\.[A-Za-z0-9])`

export const HIGHLIGHT_CLASSES = [
  {
    cls: 'hl-ref',
    token: 'ref',
    pattern: String.raw`(?<![\w/\\])(?:${[
      // Absolute URLs and bare www hosts
      String.raw`https?://[^\s<>"'\x60]+`,
      String.raw`www\.[^\s<>"'\x60]+`,
      // Windows drive paths, UNC shares
      String.raw`[A-Za-z]:[\\/][^\s<>"'\x60]+`,
      String.raw`\\\\[^\s<>"'\x60]+`,
      // Unix absolute paths (at least one slash after the leading segment)
      String.raw`/(?:[^\s/<>"'\x60]+/)+[^\s/<>"'\x60]*`,
      // Explicitly relative paths
      String.raw`\.\.?/[^\s<>"'\x60]+`,
      String.raw`~/[^\s<>"'\x60]+`,
      // Bare relative file paths (whole extension) and dir paths ending in a
      // slash. Brackets are allowed so routes like `[id]` stay whole; each
      // segment is bounded so pathological tokens cannot trigger a long scan.
      String.raw`${SEGMENT}(?:/${SEGMENT})+\.[A-Za-z0-9]+${REF_END}`,
      String.raw`${SEGMENT}(?:/${SEGMENT})+/${REF_END}`,
    ].join('|')})`,
  },
  {
    cls: 'hl-docid',
    token: 'docid',
    // C ids also consume their lowercase change slug (the artifact dir name).
    pattern: String.raw`\b(?:UC-\d+(?![A-Za-z0-9-])|DR-(?:[A-Z][A-Z0-9]*-)*\d+(?![A-Za-z0-9-])|C-\d{3,6}(?:-[a-z0-9]+)*(?![A-Za-z0-9-])|iss-[A-Za-z0-9]+(?:-[A-Za-z0-9]+)*)\b`,
  },
  {
    cls: 'hl-lexicon',
    token: 'lexicon',
    pattern: null,
  },
] as const satisfies readonly { cls: HighlightClass, token: string, pattern: string | null }[]

export const HIGHLIGHT_CLASS_NAMES: readonly HighlightClass[] =
  HIGHLIGHT_CLASSES.map(definition => definition.cls)

const STATIC_COMBINED = new RegExp(
  HIGHLIGHT_CLASSES
    .filter(definition => definition.pattern !== null)
    .map(definition => `(?<${definition.token}>${definition.pattern})`)
    .join('|'),
  'g',
)

const SENTENCE_PUNCTUATION = '.,;:!?'
const BRACKET_PAIRS: Record<string, string> = { ')': '(', ']': '[', '}': '{' }

function countChar(value: string, char: string): number {
  let count = 0
  for (const candidate of value) {
    if (candidate === char) count++
  }
  return count
}

/**
 * Drops sentence punctuation and unmatched closing brackets from a greedy ref
 * match. Balanced brackets inside the path are preserved. Quotes never reach
 * this function: the ref patterns exclude them from the match charset.
 */
export function trimRef(raw: string): string {
  let value = raw
  for (;;) {
    let changed = false
    while (value.length > 0 && SENTENCE_PUNCTUATION.includes(value[value.length - 1]!)) {
      value = value.slice(0, -1)
      changed = true
    }
    const last = value[value.length - 1]
    if (last !== undefined) {
      const opener = BRACKET_PAIRS[last]
      if (opener !== undefined) {
        if (countChar(value, last) > countChar(value, opener)) {
          value = value.slice(0, -1)
          changed = true
        }
      }
    }
    if (!changed) return value
  }
}

/**
 * A lexicon term never matches inside a longer token: the character before it
 * and the character after it must not continue the same word/dotted/hyphenated
 * run. A trailing sentence period is allowed (`Order.` highlights `Order`),
 * while a dotted continuation is not (`M1.2` does not match in `M1.2.3`).
 */
const TERM_LEAD = String.raw`(?<![A-Za-z0-9_.-])`
const TERM_TRAIL = String.raw`(?![A-Za-z0-9_-])(?!\.[A-Za-z0-9])`

const LEXICON_PATTERNS = new WeakMap<ReadonlySet<string>, RegExp | null>()

function escapeRegExpLiteral(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

/**
 * Combined alternation of every defined term, longest first so a phrase wins
 * over its prefix (`move up` before `move`). Compiled once per set identity
 * and cached: the DOM walk calls the tokenizer per text node.
 */
function lexiconPattern(lexicon: ReadonlySet<string>): RegExp | null {
  const cached = LEXICON_PATTERNS.get(lexicon)
  if (cached !== undefined || LEXICON_PATTERNS.has(lexicon)) return cached ?? null

  const terms = [...lexicon]
    .filter(term => term.length > 0)
    .sort((left, right) => right.length - left.length || (left === right ? 0 : left < right ? -1 : 1))

  let pattern: RegExp | null = null
  if (terms.length > 0) {
    const alternatives = terms.map(escapeRegExpLiteral).join('|')
    pattern = new RegExp(`${TERM_LEAD}(?:${alternatives})${TERM_TRAIL}`, 'g')
  }
  LEXICON_PATTERNS.set(lexicon, pattern)
  return pattern
}

/**
 * Splits prose into plain text and highlight segments in one pass.
 * Leftmost match wins; ref > docid > lexicon at the same offset. Lexicon terms
 * are exactly the members of `lexicon`, whatever their shape.
 */
export function tokenizeHighlights(
  text: string,
  lexicon: ReadonlySet<string> = new Set<string>(),
): HighlightSegment[] {
  if (!text) return []

  const segments: HighlightSegment[] = []
  const push = (value: string, cls?: HighlightClass) => {
    if (!value) return
    const previous = segments[segments.length - 1]
    if (cls === undefined) {
      if (previous && previous.cls === undefined) previous.text += value
      else segments.push({ text: value })
      return
    }
    segments.push({ text: value, cls })
  }

  const lexiconRegex = lexicon.size > 0 ? lexiconPattern(lexicon) : null
  const nextStatic = (from: number): RegExpExecArray | null => {
    STATIC_COMBINED.lastIndex = from
    return STATIC_COMBINED.exec(text)
  }
  const nextLexicon = (from: number): RegExpExecArray | null => {
    if (lexiconRegex === null) return null
    lexiconRegex.lastIndex = from
    return lexiconRegex.exec(text)
  }

  let cursor = 0
  let staticMatch = nextStatic(0)
  let lexiconMatch = nextLexicon(0)

  while (staticMatch !== null || lexiconMatch !== null) {
    // A losing pattern may still point inside already-consumed prose.
    if (staticMatch !== null && staticMatch.index < cursor) staticMatch = nextStatic(cursor)
    if (lexiconMatch !== null && lexiconMatch.index < cursor) lexiconMatch = nextLexicon(cursor)
    if (staticMatch === null && lexiconMatch === null) break

    // Leftmost wins; at the same offset the static classes precede the lexicon.
    const useStatic = lexiconMatch === null
      || (staticMatch !== null && staticMatch.index <= lexiconMatch.index)
    const match = (useStatic ? staticMatch : lexiconMatch)!
    const raw = match[0]
    const start = match.index

    let cls: HighlightClass | undefined
    let value = raw

    if (useStatic) {
      const groups = match.groups ?? {}
      if (groups.ref !== undefined) {
        value = trimRef(raw)
        if (value.length > 0) cls = 'hl-ref'
      } else if (groups.docid !== undefined) {
        cls = 'hl-docid'
      }
      if (cls === undefined) {
        staticMatch = nextStatic(start + 1)
        continue
      }
      staticMatch = nextStatic(start + raw.length)
    } else {
      cls = 'hl-lexicon'
      lexiconMatch = nextLexicon(start + raw.length)
    }

    push(text.slice(cursor, start))
    push(value, cls)
    // Anything trimmed from the end of the ref goes back to plain prose.
    if (value.length < raw.length) push(raw.slice(value.length))
    cursor = start + raw.length
  }

  push(text.slice(cursor))
  return segments
}

/** Removes a YAML comment that is not inside a quoted scalar. */
function stripComment(line: string): string {
  let quote: string | null = null
  for (let index = 0; index < line.length; index++) {
    const char = line[index]
    if (quote !== null) {
      if (char === quote) quote = null
      continue
    }
    if (char === '"' || char === "'") {
      quote = char
      continue
    }
    if (char === '#') return line.slice(0, index)
  }
  return line
}

function scalar(raw: string): string {
  const value = raw.trim()
  if (value.length >= 2) {
    const first = value[0]
    if ((first === '"' || first === "'") && value[value.length - 1] === first) {
      return value.slice(1, -1).trim()
    }
  }
  return value
}

const INDENT = /^[ \t]*/

const MAX_TERM_LENGTH = 120

/** Top-level keys that describe the file, never a term. */
const STRUCTURAL_KEYS: Record<string, true> = {
  schema: true,
  tree: true,
  owner: true,
  version: true,
  notes: true,
  description: true,
  terms: true,
  entries: true,
  categories: true,
  metadata: true,
}

/** A term scalar of any shape, or null when empty or unreasonably long. */
function term(raw: string): string | null {
  const value = scalar(raw)
  if (value.length === 0 || value.length > MAX_TERM_LENGTH) return null
  return value
}

/**
 * Splits `key: value` / `key:` at the first plain colon. `M1.2:x` and
 * `http://…` are scalars, not mappings, so they yield null.
 */
function splitKey(text: string): { key: string, value: string } | null {
  let quote: string | null = null
  for (let index = 0; index < text.length; index++) {
    const char = text[index]
    if (quote !== null) {
      if (char === quote) quote = null
      continue
    }
    if (char === '"' || char === "'") {
      quote = char
      continue
    }
    if (char !== ':') continue
    const rest = text.slice(index + 1)
    if (rest.length === 0 || rest[0] === ' ') {
      return { key: text.slice(0, index), value: rest.trim() }
    }
    return null
  }
  return null
}

/**
 * Term from a sequence item: `- block`, `- "replace the text"`, `- M1.2: gloss`
 * or the self-describing `- term: block`. `termFieldOnly` marks the `entries:`
 * collection, whose mapping items carry the term in the `term:` field and whose
 * other fields (`namespace:`, `area:`, …) are not terms.
 */
function itemTerm(payload: string, termFieldOnly: boolean): string | null {
  const raw = payload.trim()
  if (raw.length === 0) return null
  const mapping = splitKey(raw)
  if (!mapping) return term(raw)
  const key = scalar(mapping.key)
  if (key === 'term') return term(mapping.value)
  return termFieldOnly ? null : term(key)
}

/** Term from a mapping line inside a collection (`M1.2: gloss` / `term: block`). */
function mappingTerm(payload: string, termFieldOnly: boolean): string | null {
  const mapping = splitKey(payload)
  if (!mapping) return null
  const key = scalar(mapping.key)
  if (key === 'term') return term(mapping.value)
  return termFieldOnly ? null : term(key)
}

/** Terms of a flow sequence (`terms: [block, "replace the text"]`), or null. */
function flowTerms(raw: string): string[] | null {
  const value = raw.trim()
  if (!value.startsWith('[') || !value.endsWith(']')) return null
  return value
    .slice(1, -1)
    .split(',')
    .map(part => term(part))
    .filter((part): part is string => part !== null)
}

/**
 * Parses the architecture term lexicon. Accepted shapes:
 *
 * ```yaml
 * # 1. top-level sequence of scalars
 * - block
 * - replace the text
 *
 * # 2. terms: block — scalars or single-key mappings
 * terms:
 *   - block
 *   - "replace the text"
 *   - x.mtext.move: gloss
 *
 * # 3. top-level mapping whose keys are the terms
 * block: gloss text
 *
 * # 4. the self-describing term-lexicon/v1 wrapper
 * schema: term-lexicon/v1
 * entries:
 *   - term: replace the text
 *     namespace: mtext
 * ```
 *
 * Terms may have any shape (a single word, PascalCase, SCREAMING_SNAKE,
 * hyphenated, dotted, or a phrase with spaces) up to 120 characters; empty
 * values are ignored. Indented lines belong to the open entry (its definition
 * block or fields) and contribute nothing unless they are entries of the
 * `terms:` / `entries:` collection itself. Structural top-level keys
 * (`schema:`, `tree:`, `owner:`, `version:`, `notes:`, `terms:`, `entries:` …)
 * are never terms. Document markers, comments and surrounding quotes are
 * handled; unsupported structures are skipped, never thrown on, so a partially
 * understood file still contributes what it can.
 */
export function parseLexicon(yaml: string): Set<string> {
  const terms = new Set<string>()
  if (!yaml) return terms

  // Which collection is open, and the indent at which its entries live
  // (deeper lines belong to the current entry).
  let collection: 'terms' | 'entries' | null = null
  let entryIndent = -1

  for (const rawLine of yaml.split(/\r?\n/)) {
    const line = stripComment(rawLine)
    if (line.trim().length === 0) continue
    const indent = (INDENT.exec(line)?.[0] ?? '').length
    const trimmed = line.trim()

    // YAML document markers carry no terms.
    if (trimmed === '---' || trimmed === '...') {
      collection = null
      entryIndent = -1
      continue
    }

    if (indent === 0) {
      collection = null
      entryIndent = -1

      if (trimmed.startsWith('- ') || trimmed === '-') {
        const found = itemTerm(trimmed.slice(1), false)
        if (found) terms.add(found)
        continue
      }

      const entry = splitKey(trimmed)
      if (!entry) continue
      const name = scalar(entry.key)

      if (name === 'terms' || name === 'entries') {
        const flow = flowTerms(entry.value)
        if (flow) {
          for (const item of flow) terms.add(item)
        } else if (entry.value.length === 0) {
          collection = name
        }
        continue
      }
      if (STRUCTURAL_KEYS[name] === true) continue
      const found = term(name)
      if (found) terms.add(found)
      continue
    }

    // Indented lines belong to the open entry/key and are skipped, except for
    // the entries of the `terms:` / `entries:` collection itself.
    if (collection === null) continue
    if (entryIndent < 0) entryIndent = indent
    if (indent > entryIndent) continue
    if (indent < entryIndent) {
      collection = null
      entryIndent = -1
      continue
    }

    const termFieldOnly = collection === 'entries'
    const found = (trimmed.startsWith('- ') || trimmed === '-')
      ? itemTerm(trimmed.slice(1), termFieldOnly)
      : mappingTerm(trimmed, termFieldOnly)
    if (found) terms.add(found)
  }

  return terms
}