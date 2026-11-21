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
 * same start offset (leftmost match still wins across offsets).
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
    // Upper bound of three segments; leading letter keeps decimals out.
    pattern: String.raw`(?<![\w.-])[A-Za-z][A-Za-z0-9_-]*(?:\.[A-Za-z0-9][A-Za-z0-9_-]*){1,2}(?!\.[A-Za-z0-9])`,
  },
] as const satisfies readonly { cls: HighlightClass, token: string, pattern: string }[]

export const HIGHLIGHT_CLASS_NAMES: readonly HighlightClass[] =
  HIGHLIGHT_CLASSES.map(definition => definition.cls)

const COMBINED = new RegExp(
  HIGHLIGHT_CLASSES.map(definition => `(?<${definition.token}>${definition.pattern})`).join('|'),
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
 * Splits prose into plain text and highlight segments in one pass.
 * Leftmost match wins; ref > docid > lexicon at the same offset. Lexicon
 * candidates are only tagged when they are members of `lexicon`.
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

  COMBINED.lastIndex = 0
  let cursor = 0
  let match: RegExpExecArray | null

  while ((match = COMBINED.exec(text)) !== null) {
    const groups = match.groups ?? {}
    const raw = match[0]
    const start = match.index
    const ref = groups.ref
    const docid = groups.docid

    let cls: HighlightClass | undefined
    let value = raw

    if (ref !== undefined) {
      value = trimRef(raw)
      if (value.length > 0) cls = 'hl-ref'
    } else if (docid !== undefined) {
      cls = 'hl-docid'
    } else if (groups.lexicon !== undefined && lexicon.has(raw)) {
      cls = 'hl-lexicon'
    }

    if (cls === undefined) continue

    push(text.slice(cursor, start))
    push(value, cls)
    // Anything trimmed from the end of the ref goes back to plain prose.
    if (value.length < raw.length) push(raw.slice(value.length))
    cursor = start + raw.length
  }

  push(text.slice(cursor))
  return segments
}

const IDENTIFIER = /^[A-Za-z][A-Za-z0-9_-]*(?:\.[A-Za-z0-9][A-Za-z0-9_-]*){1,2}$/

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

/** Identifier-shaped scalar, or null. */
function term(raw: string): string | null {
  const value = scalar(raw)
  return IDENTIFIER.test(value) ? value : null
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

/** Term from a sequence item: `- M1.2`, `- "M1.2"` or `- M1.2: gloss`. */
function itemTerm(payload: string): string | null {
  const raw = payload.trim()
  if (raw.length === 0) return null
  const mapping = splitKey(raw)
  return mapping ? term(mapping.key) : term(raw)
}

/** Identifiers of a flow sequence (`terms: [M1.2, M1.3]`), or null. */
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
 * - M1.2                     # top-level sequence of scalars
 * terms:                     # terms: sequence (scalars or single-key mappings)
 *   - M1.2: gloss
 * M1.2: gloss                # top-level mapping, key is the term
 * M1.3:
 *   definition text          # ignored until the next column-0 key
 * ```
 *
 * Keys/scalars that are not X.Y / X.Y.Z shaped are ignored. Indented lines
 * belong to the current entry (a definition block or a nested mapping) and
 * contribute nothing unless they are entries of the `terms:` collection itself.
 * Document markers, comments and surrounding quotes are handled; unsupported
 * structures are skipped, never thrown on, so a partially understood file still
 * contributes what it can.
 */
export function parseLexicon(yaml: string): Set<string> {
  const terms = new Set<string>()
  if (!yaml) return terms

  // Whether the open block is the `terms:` collection, and the indent at which
  // its entries live (deeper lines belong to the current entry).
  let inTerms = false
  let entryIndent = -1

  for (const rawLine of yaml.split(/\r?\n/)) {
    const line = stripComment(rawLine)
    if (line.trim().length === 0) continue
    const indent = (INDENT.exec(line)?.[0] ?? '').length
    const trimmed = line.trim()

    // YAML document markers carry no terms.
    if (trimmed === '---' || trimmed === '...') {
      inTerms = false
      entryIndent = -1
      continue
    }

    if (indent === 0) {
      inTerms = false
      entryIndent = -1

      if (trimmed.startsWith('- ') || trimmed === '-') {
        const found = itemTerm(trimmed.slice(1))
        if (found) terms.add(found)
        continue
      }

      const entry = splitKey(trimmed)
      if (!entry) continue
      const name = scalar(entry.key)
      if (name === 'terms') {
        const flow = flowTerms(entry.value)
        if (flow) {
          for (const item of flow) terms.add(item)
        } else if (entry.value.length === 0) {
          inTerms = true
        }
        continue
      }
      const found = term(name)
      if (found) terms.add(found)
      continue
    }

    // Indented lines belong to the open entry/key and are skipped, except for
    // the entries of the `terms:` collection itself.
    if (!inTerms) continue
    if (entryIndent < 0) entryIndent = indent
    if (indent > entryIndent) continue
    if (indent < entryIndent) {
      inTerms = false
      entryIndent = -1
      continue
    }

    let found: string | null
    if (trimmed.startsWith('- ') || trimmed === '-') {
      found = itemTerm(trimmed.slice(1))
    } else {
      const entry = splitKey(trimmed)
      found = entry ? term(entry.key) : null
    }
    if (found) terms.add(found)
  }

  return terms
}