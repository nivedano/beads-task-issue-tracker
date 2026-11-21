/**
 * DOM side of prose highlighting: clears and re-applies `hl-*` spans over the
 * text nodes of a bound element, and the `v-highlight` directive factory.
 *
 * The markdown sanitizer never lets spans through, so highlighting is a
 * post-render DOM walk (see `~/utils/markdown`).
 *
 * Text-node identity matters: on `{{ }}` surfaces Vue keeps a reference to the
 * original text node and patches its value. The walk therefore never replaces
 * that node — it keeps the first plain segment in place (or empties it when the
 * first segment is a highlight) and inserts the remaining pieces after it. The
 * directive clears before every update, inside `beforeUpdate`, so Vue always
 * patches a plain, connected text node.
 */
import { watch, type Ref } from 'vue'
import { HIGHLIGHT_CLASS_NAMES, tokenizeHighlights } from './text-highlight'

/** Subtrees that are never rewritten: code/logs, links, scripts and our spans. */
const SKIP_SELECTOR = [
  'pre',
  'code',
  'a',
  'script',
  'style',
  'textarea',
  'table',
  // A bound element never walks into another bound element.
  '[data-highlight-root]',
  ...HIGHLIGHT_CLASS_NAMES.map(cls => `.${cls}`),
].join(', ')

interface HighlightRecord {
  /** The original text node; its identity is preserved across walks. */
  node: Text
  /** Nodes appended after `node`, in document order (spans and plain runs). */
  inserted: Node[]
}

const recordsByElement = new WeakMap<HTMLElement, HighlightRecord[]>()

/**
 * Restores every text node this module rewrote back to its full plain text and
 * removes the pieces it inserted. Author-supplied `hl-*` markup is untouched.
 */
export function clearHighlights(root: HTMLElement): void {
  const records = recordsByElement.get(root)
  if (records) {
    for (const record of records) {
      let text = record.node.nodeValue ?? ''
      for (const inserted of record.inserted) text += inserted.textContent ?? ''
      record.node.nodeValue = text
      for (const inserted of record.inserted) inserted.parentNode?.removeChild(inserted)
    }
    recordsByElement.delete(root)
  }
}

function applyToTextNode(node: Text, lexicon: ReadonlySet<string>): HighlightRecord | null {
  const segments = tokenizeHighlights(node.nodeValue ?? '', lexicon)
  if (!segments.some(segment => segment.cls !== undefined)) return null

  const first = segments[0]!
  const document = node.ownerDocument
  const inserted: Node[] = []

  // Keep the original node as the leading plain run (or empty when the text
  // starts with a highlight); Vue's vnode reference stays connected.
  node.nodeValue = first.cls === undefined ? first.text : ''

  for (const segment of segments.slice(first.cls === undefined ? 1 : 0)) {
    if (segment.cls === undefined) {
      inserted.push(document.createTextNode(segment.text))
      continue
    }
    const span = document.createElement('span')
    span.className = segment.cls
    span.textContent = segment.text
    // Every highlight span reveals the matched text on hover.
    span.title = segment.text
    inserted.push(span)
  }

  node.after(...inserted)
  return { node, inserted }
}

/**
 * Re-highlights `root` from scratch. Idempotent: the previous walk is fully
 * restored first, so repeated calls neither nest nor duplicate. Lexicon terms
 * that left the set lose their span on the next call.
 */
export function highlightElement(root: HTMLElement, lexicon: ReadonlySet<string>): void {
  clearHighlights(root)

  const walker = root.ownerDocument.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      const parent = node.parentElement
      if (!parent) return NodeFilter.FILTER_REJECT
      const skipped = parent.closest(SKIP_SELECTOR)
      // The bound element itself is tagged as a root; only descendants that
      // belong to a *different* skip subtree are refused.
      if (skipped && skipped !== root) return NodeFilter.FILTER_REJECT
      return NodeFilter.FILTER_ACCEPT
    },
  })

  // Snapshot first: inserting nodes during the walk would otherwise revisit
  // pieces created for an earlier text node.
  const nodes: Text[] = []
  while (walker.nextNode()) nodes.push(walker.currentNode as Text)

  const records: HighlightRecord[] = []
  for (const node of nodes) {
    const record = applyToTextNode(node, lexicon)
    if (record) records.push(record)
  }
  if (records.length > 0) recordsByElement.set(root, records)
}

export interface HighlightDirective {
  created: (el: HTMLElement) => void
  mounted: (el: HTMLElement) => void
  beforeUpdate: (el: HTMLElement) => void
  updated: (el: HTMLElement) => void
  unmounted: (el: HTMLElement) => void
}

/**
 * Directive factory: one registry per app, watching the shared lexicon Set and
 * re-walking every mounted element when it changes.
 */
export function createHighlightDirective(lexicon: Ref<ReadonlySet<string>>): HighlightDirective {
  const registry = new Set<HTMLElement>()

  watch(
    lexicon,
    () => {
      for (const element of registry) {
        if (element.isConnected) highlightElement(element, lexicon.value)
      }
    },
    { flush: 'post' },
  )

  const tagRoot = (element: HTMLElement) => {
    element.dataset.highlightRoot = 'true'
  }

  return {
    created(element) {
      // Children are created before the parent mounts, so tagging here keeps an
      // outer walk from entering a nested root even on first mount.
      tagRoot(element)
    },
    mounted(element) {
      tagRoot(element)
      registry.add(element)
      highlightElement(element, lexicon.value)
    },
    beforeUpdate(element) {
      // Runs before Vue patches children, so interpolated text nodes are plain
      // and connected when Vue writes into them.
      clearHighlights(element)
    },
    updated(element) {
      if (registry.has(element)) highlightElement(element, lexicon.value)
    },
    unmounted(element) {
      clearHighlights(element)
      registry.delete(element)
      delete element.dataset.highlightRoot
    },
  }
}