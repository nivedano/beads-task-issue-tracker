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

/**
 * Parses the subset of YAML the architecture term lexicon uses:
 *
 * ```yaml
 * terms:
 *   - M1.2
 *   - arch.method.v2
 * ```
 *
 * A top-level sequence (`- term` at column 0) is accepted too. Comments and
 * surrounding quotes are stripped; entries that are not X.Y / X.Y.Z shaped are
 * ignored. Unsupported structures are skipped, never thrown on, so a partially
 * understood file still contributes what it can.
 */
export function parseLexicon(yaml: string): Set<string> {
  const terms = new Set<string>()
  if (!yaml) return terms

  type Mode = 'root' | 'terms' | 'other'
  let mode: Mode = 'root'

  for (const rawLine of yaml.split(/\r?\n/)) {
    const line = stripComment(rawLine)
    const trimmed = line.trim()
    if (trimmed.length === 0) continue
    // YAML document markers carry no terms.
    if (trimmed === '---' || trimmed === '...') continue

    if (trimmed.startsWith('- ') || trimmed === '-') {
      if (mode === 'root' || mode === 'terms') {
        const term = scalar(trimmed.slice(1))
        if (IDENTIFIER.test(term)) terms.add(term)
      }
      continue
    }

    const key = /^([A-Za-z_][\w-]*)\s*:\s*(.*)$/.exec(trimmed)
    if (key) {
      const [, name, inline] = key
      mode = name === 'terms' && (inline ?? '').trim().length === 0 ? 'terms' : 'other'
      continue
    }

    // Indented free text is a block scalar under some other key.
    if (mode === 'root') mode = 'other'
  }

  return terms
}