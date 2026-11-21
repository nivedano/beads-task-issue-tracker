import { describe, it, expect } from 'vitest'
import { nextTick, ref } from 'vue'
import { tokenizeHighlights, parseLexicon, type HighlightSegment } from '~/utils/text-highlight'
import { highlightElement, createHighlightDirective } from '~/utils/text-highlight-dom'

const REF = 'hl-ref'
const DOCID = 'hl-docid'
const LEX = 'hl-lexicon'

function seg(text: string, cls?: string): HighlightSegment {
  return cls ? { text, cls: cls as HighlightSegment['cls'] } : { text }
}

describe('tokenizeHighlights', () => {
  it('returns an empty list for empty text', () => {
    expect(tokenizeHighlights('', new Set())).toEqual([])
  })

  it('returns one plain segment when nothing matches', () => {
    expect(tokenizeHighlights('just prose here', new Set())).toEqual([seg('just prose here')])
  })

  it('marks http, https and www URLs as refs', () => {
    expect(tokenizeHighlights('see http://example.com/a and https://x.dev/b', new Set())).toEqual([
      seg('see '),
      seg('http://example.com/a', REF),
      seg(' and '),
      seg('https://x.dev/b', REF),
    ])
    expect(tokenizeHighlights('www.example.com/a/b', new Set())).toEqual([
      seg('www.example.com/a/b', REF),
    ])
  })

  it('marks Windows drive and UNC paths as refs', () => {
    expect(tokenizeHighlights('at D:\\repos\\app and C:/Users/me', new Set())).toEqual([
      seg('at '),
      seg('D:\\repos\\app', REF),
      seg(' and '),
      seg('C:/Users/me', REF),
    ])
    expect(tokenizeHighlights('\\\\server\\share\\file.txt', new Set())).toEqual([
      seg('\\\\server\\share\\file.txt', REF),
    ])
  })

  it('marks unix absolute, dot-relative and home-relative paths', () => {
    expect(tokenizeHighlights('/usr/local/bin and ./src/a.ts and ../lib/b.ts and ~/notes', new Set())).toEqual([
      seg('/usr/local/bin', REF),
      seg(' and '),
      seg('./src/a.ts', REF),
      seg(' and '),
      seg('../lib/b.ts', REF),
      seg(' and '),
      seg('~/notes', REF),
    ])
  })

  it('marks bare relative file paths and trailing-slash directories', () => {
    expect(tokenizeHighlights('models/architecture/method.md and models/architecture/', new Set())).toEqual([
      seg('models/architecture/method.md', REF),
      seg(' and '),
      seg('models/architecture/', REF),
    ])
  })

  it('trims trailing sentence punctuation from refs', () => {
    expect(tokenizeHighlights('See https://example.com/x.', new Set())).toEqual([
      seg('See '),
      seg('https://example.com/x', REF),
      seg('.'),
    ])
    expect(tokenizeHighlights('at /usr/local/bin:', new Set())).toEqual([
      seg('at '),
      seg('/usr/local/bin', REF),
      seg(':'),
    ])
  })

  it('trims unmatched closing brackets and quotes from refs', () => {
    expect(tokenizeHighlights('(see models/architecture/method.md)', new Set())).toEqual([
      seg('(see '),
      seg('models/architecture/method.md', REF),
      seg(')'),
    ])
    expect(tokenizeHighlights('"https://example.com/x"', new Set())).toEqual([
      seg('"'),
      seg('https://example.com/x', REF),
      seg('"'),
    ])
  })

  it('keeps balanced brackets inside a ref', () => {
    expect(tokenizeHighlights('models/a/[id]/b.md', new Set())).toEqual([
      seg('models/a/[id]/b.md', REF),
    ])
  })

  it('keeps a ref whole when it contains a doc id or dotted segments', () => {
    expect(tokenizeHighlights('https://example.com/C-0019', new Set())).toEqual([
      seg('https://example.com/C-0019', REF),
    ])
    expect(tokenizeHighlights('models/architecture/M1.2.md', new Set(['M1.2']))).toEqual([
      seg('models/architecture/M1.2.md', REF),
    ])
  })

  it('marks documentation ids as doc ids', () => {
    expect(tokenizeHighlights('Fixes UC-12, DR-MSR-001, C-0019 and iss-1234.', new Set())).toEqual([
      seg('Fixes '),
      seg('UC-12', DOCID),
      seg(', '),
      seg('DR-MSR-001', DOCID),
      seg(', '),
      seg('C-0019', DOCID),
      seg(' and '),
      seg('iss-1234', DOCID),
      seg('.'),
    ])
  })

  it('rejects malformed or embedded doc ids', () => {
    for (const text of ['UC-', 'C-12', 'C-1234567', 'DR-msr-001', 'xUC-12', 'xc-0019']) {
      expect(tokenizeHighlights(text, new Set())).toEqual([seg(text)])
    }
  })

  it('never highlights a partial doc id token', () => {
    for (const text of ['UC-12-3', 'DR-MSR-001-followup']) {
      expect(tokenizeHighlights(text, new Set())).toEqual([seg(text)])
    }
  })

  it('consumes the lowercase change slug on C ids', () => {
    expect(tokenizeHighlights('see C-0019-retire-briefs.', new Set())).toEqual([
      seg('see '),
      seg('C-0019-retire-briefs', DOCID),
      seg('.'),
    ])
  })

  it('marks lexicon terms only when they are defined', () => {
    expect(tokenizeHighlights('uses M1.2 but not M1.3', new Set(['M1.2']))).toEqual([
      seg('uses '),
      seg('M1.2', LEX),
      seg(' but not M1.3'),
    ])
    expect(tokenizeHighlights('arch.method.v2', new Set(['arch.method.v2']))).toEqual([
      seg('arch.method.v2', LEX),
    ])
  })

  it('marks single-word, PascalCase, SCREAMING_SNAKE and hyphenated terms', () => {
    const lexicon = new Set(['Order', 'SyntaxNode', 'UNTITLED_NODE', 'title-root'])
    expect(tokenizeHighlights('Order, SyntaxNode, UNTITLED_NODE, title-root', lexicon)).toEqual([
      seg('Order', LEX),
      seg(', '),
      seg('SyntaxNode', LEX),
      seg(', '),
      seg('UNTITLED_NODE', LEX),
      seg(', '),
      seg('title-root', LEX),
    ])
  })

  it('marks phrase terms with spaces, preferring the longest term', () => {
    const lexicon = new Set(['move', 'move up', 'move up by one', 'replace the text'])
    expect(tokenizeHighlights('move up by one, then replace the text', lexicon)).toEqual([
      seg('move up by one', LEX),
      seg(', then '),
      seg('replace the text', LEX),
    ])
    expect(tokenizeHighlights('move up', lexicon)).toEqual([seg('move up', LEX)])
  })

  it('marks dotted terms of any depth', () => {
    expect(tokenizeHighlights('a.b.c.d and M1.2', new Set(['a.b.c.d', 'M1.2']))).toEqual([
      seg('a.b.c.d', LEX),
      seg(' and '),
      seg('M1.2', LEX),
    ])
  })

  it('rejects lexicon partial-token matches', () => {
    const lexicon = new Set(['Order', 'M1.2', 'title-root', 'root'])
    expect(tokenizeHighlights('Ordering aOrder Order.', lexicon)).toEqual([
      seg('Ordering aOrder '),
      seg('Order', LEX),
      seg('.'),
    ])
    expect(tokenizeHighlights('M1.2.3 M1.2', lexicon)).toEqual([
      seg('M1.2.3 '),
      seg('M1.2', LEX),
    ])
    expect(tokenizeHighlights('title-root-protocol title-root', lexicon)).toEqual([
      seg('title-root-protocol '),
      seg('title-root', LEX),
    ])
  })

  it('leaves terms plain when the lexicon does not define them', () => {
    expect(tokenizeHighlights('Order and title-root', new Set(['block']))).toEqual([
      seg('Order and title-root'),
    ])
  })

  it('keeps ref > docid > lexicon precedence at the same offset', () => {
    expect(tokenizeHighlights('models/a/M1.2.md', new Set(['M1.2']))).toEqual([
      seg('models/a/M1.2.md', REF),
    ])
    expect(tokenizeHighlights('C-0019', new Set(['C-0019']))).toEqual([
      seg('C-0019', DOCID),
    ])
  })

  it('rebuilds the term pattern when the set changes', () => {
    expect(tokenizeHighlights('Order and block', new Set(['Order']))).toEqual([
      seg('Order', LEX),
      seg(' and block'),
    ])
    expect(tokenizeHighlights('Order and block', new Set(['block']))).toEqual([
      seg('Order and '),
      seg('block', LEX),
    ])
    const first = new Set(['Order'])
    expect(tokenizeHighlights('Order and block', first)).toEqual([
      seg('Order', LEX),
      seg(' and block'),
    ])
    expect(tokenizeHighlights('Order and block', new Set(['Order', 'block']))).toEqual([
      seg('Order', LEX),
      seg(' and '),
      seg('block', LEX),
    ])
  })

  it('escapes regex metacharacters in terms', () => {
    expect(tokenizeHighlights('CodeMirror 6 and Node.js', new Set(['CodeMirror 6', 'Node.js']))).toEqual([
      seg('CodeMirror 6', LEX),
      seg(' and '),
      seg('Node.js', LEX),
    ])
  })

  it('does not match inside words', () => {
    expect(tokenizeHighlights('xhttps://example.com and aD:\\repos and/or', new Set())).toEqual([
      seg('xhttps://example.com and aD:\\repos and/or'),
    ])
  })

  it('consumes the whole extension and never stops mid-token', () => {
    expect(tokenizeHighlights('foo/bar.typescript', new Set())).toEqual([
      seg('foo/bar.typescript', REF),
    ])
    expect(tokenizeHighlights('a/b/c', new Set())).toEqual([seg('a/b/c')])
    expect(tokenizeHighlights('models/architecture/and', new Set())).toEqual([
      seg('models/architecture/and'),
    ])
    expect(tokenizeHighlights('see models/architecture/ now', new Set())).toEqual([
      seg('see '),
      seg('models/architecture/', REF),
      seg(' now'),
    ])
  })

  it('bounds path segments to 64 characters', () => {
    const tooLong = 'a'.repeat(65)
    expect(tokenizeHighlights(`models/${tooLong}.md`, new Set())).toEqual([
      seg(`models/${tooLong}.md`),
    ])
    const atLimit = 'a'.repeat(64)
    expect(tokenizeHighlights(`models/${atLimit}.md`, new Set())).toEqual([
      seg(`models/${atLimit}.md`, REF),
    ])
  })
})

describe('parseLexicon', () => {
  it('parses a top-level sequence of terms of any shape', () => {
    const yaml = [
      '# Architecture lexicon',
      '- M1.2',
      "- 'M1.3'  # quoted entry",
      '- "arch.method.v2"',
      '- Order',
      '- NOT_LOADED',
      '- not a term',
      '',
    ].join('\n')
    expect([...parseLexicon(yaml)]).toEqual([
      'M1.2',
      'M1.3',
      'arch.method.v2',
      'Order',
      'NOT_LOADED',
      'not a term',
    ])
  })

  it('parses a terms: key with an indented sequence', () => {
    const yaml = ['terms:', '  - M1.2', '  - M1.2.3 # nested', ''].join('\n')
    expect([...parseLexicon(yaml)]).toEqual(['M1.2', 'M1.2.3'])
  })

  it('ignores unsupported structures instead of failing', () => {
    const yaml = [
      'categories:',
      '  - M9.9',
      'M1.2: a mapping entry',
      'terms:',
      '  - M1.2',
      'notes: |',
      '  free text',
    ].join('\n')
    expect([...parseLexicon(yaml)]).toEqual(['M1.2'])
  })

  it('returns an empty set for garbage input', () => {
    expect(parseLexicon(':::\n  -: [unclosed')).toEqual(new Set())
    expect(parseLexicon('')).toEqual(new Set())
  })

  it('handles CRLF line endings', () => {
    expect([...parseLexicon('- M1.2\r\n- M1.3\r\n')]).toEqual(['M1.2', 'M1.3'])
  })

  it('deduplicates repeated entries', () => {
    expect([...parseLexicon('- M1.2\n- M1.2')]).toEqual(['M1.2'])
  })

  it('ignores document markers and parses the documented fixture', () => {
    const yaml = [
      '---',
      '# Architecture term lexicon',
      'terms:',
      '  - M1.2          # comments are stripped',
      '  - arch.method.v2',
      '  - "quoted.term" # surrounding quotes are stripped',
      '...',
      '',
    ].join('\n')
    expect([...parseLexicon(yaml)]).toEqual(['M1.2', 'arch.method.v2', 'quoted.term'])
  })

  it('parses a bare list that starts with a document marker', () => {
    expect([...parseLexicon('---\n- M1.2\n- M1.3\n...\n')]).toEqual(['M1.2', 'M1.3'])
  })

  it('parses a top-level mapping whose keys are terms', () => {
    const yaml = [
      '---',
      '# Architecture term lexicon',
      'M1.2: the first milestone gloss',
      '"arch.method.v2": "quoted key, quoted gloss # not a comment"',
      "M1.2.3: 'single-quoted gloss'",
      '...',
      '',
    ].join('\n')
    expect([...parseLexicon(yaml)]).toEqual(['M1.2', 'arch.method.v2', 'M1.2.3'])
  })

  it('ignores mapping definition blocks and nested keys', () => {
    const yaml = [
      'M1.2:',
      '  A free-text definition that mentions M9.9 in passing.',
      '  nested: M9.8',
      '  M9.7: nested key',
      'M1.3: short gloss',
      '  - M9.6',
      'M1.4:',
      '  ```',
      '  code block',
      '  ```',
      '',
    ].join('\n')
    expect([...parseLexicon(yaml)]).toEqual(['M1.2', 'M1.3', 'M1.4'])
  })

  it('parses a sequence of single-key mappings', () => {
    const yaml = ['- M1.2: gloss text', '- arch.method.v2:', '- "M1.3": quoted'].join('\n')
    expect([...parseLexicon(yaml)]).toEqual(['M1.2', 'arch.method.v2', 'M1.3'])
  })

  it('parses a terms: block with mapping items and nested descriptions', () => {
    const yaml = [
      '---',
      'terms:',
      '  - M1.2: gloss text',
      '  - M1.3:',
      '      description: nested text',
      '      M9.9: must not leak',
      '  - "arch.method.v2"  # quoted scalar',
      '...',
      '',
    ].join('\n')
    expect([...parseLexicon(yaml)]).toEqual(['M1.2', 'M1.3', 'arch.method.v2'])
  })

  it('parses a terms: block written as a mapping', () => {
    const yaml = ['terms:', '  M1.2: gloss', '  M1.3:'].join('\n')
    expect([...parseLexicon(yaml)]).toEqual(['M1.2', 'M1.3'])
  })

  it('parses a flow sequence on the terms: key', () => {
    expect([...parseLexicon('terms: [M1.2, "M1.3", Order, not a term]')]).toEqual([
      'M1.2',
      'M1.3',
      'Order',
      'not a term',
    ])
  })

  it('ignores structural keys but keeps non-dotted term keys', () => {
    const yaml = [
      'schema: term-lexicon/v1',
      'tree: mtext',
      'owner: mtext',
      'version: 2',
      'notes:',
      '  owner: architecture',
      '  M9.9: not a term key',
      'title: plain text',
      '',
    ].join('\n')
    expect([...parseLexicon(yaml)]).toEqual(['title'])
  })

  it('parses the self-describing entries: schema and ignores entry fields', () => {
    const yaml = [
      '# Term lexicon',
      'schema: term-lexicon/v1',
      'tree: mtext',
      'owner: mtext',
      'entries:',
      '',
      '  # ---- document domain ----',
      '  - term: block',
      '    namespace: mtext',
      '    definition: models/architecture/outliner.md#blocks',
      '    maps_to: mdm:SyntaxNode',
      '    collisions:',
      '      - reading: a programming-language block',
      '        rule: qualify',
      '',
      '  - term: replace the text',
      '    namespace: mtext',
      '    note: \'classification: local operation\'',
      '',
      '  - term: UNTITLED_NODE',
      '    namespace: platform',
      '',
    ].join('\n')
    expect([...parseLexicon(yaml)]).toEqual(['block', 'replace the text', 'UNTITLED_NODE'])
  })

  it('parses non-dotted terms in every supported shape', () => {
    const yaml = [
      'Order: the boundary structure',
      '- SyntaxNode',
      'terms:',
      '  - UNTITLED_NODE',
      '  - x.mtext.move: gloss',
      '  - "replace the text"',
      '  NOT_LOADED: mapping form',
      '',
    ].join('\n')
    expect([...parseLexicon(yaml)]).toEqual([
      'Order',
      'SyntaxNode',
      'UNTITLED_NODE',
      'x.mtext.move',
      'replace the text',
      'NOT_LOADED',
    ])
  })

  it('drops empty and over-long values', () => {
    const long = 'a'.repeat(121)
    const yaml = [
      '- ""',
      "- ''",
      '- ',
      `- ${long}`,
      '- Reasonable term',
      '',
    ].join('\n')
    expect([...parseLexicon(yaml)]).toEqual(['Reasonable term'])
  })

  it('parses every shape mixed in one document', () => {
    const yaml = [
      'version: 2',
      'terms:',
      '  - M1.2',
      '  - M1.3: gloss',
      'M1.4: top-level gloss',
      'M1.5:',
      '  definition',
      '- M1.6',
      '',
    ].join('\n')
    expect([...parseLexicon(yaml)]).toEqual(['M1.2', 'M1.3', 'M1.4', 'M1.5', 'M1.6'])
  })
})

describe('highlightElement', () => {
  function mount(html: string): HTMLElement {
    const el = document.createElement('div')
    el.innerHTML = html
    document.body.appendChild(el)
    return el
  }

  it('wraps refs, doc ids and lexicon terms in spans', () => {
    const el = mount('<p>See UC-12 at models/x.md and M1.2</p>')
    highlightElement(el, new Set(['M1.2']))
    const spans = [...el.querySelectorAll('span')]
    expect(spans.map(s => [s.className, s.textContent])).toEqual([
      ['hl-docid', 'UC-12'],
      ['hl-ref', 'models/x.md'],
      ['hl-lexicon', 'M1.2'],
    ])
    expect(spans[0]!.getAttribute('title')).toBe('UC-12')
    expect(spans[1]!.getAttribute('title')).toBe('models/x.md')
    expect(spans[2]!.getAttribute('title')).toBe('M1.2')
    el.remove()
  })

  it('leaves markdown table content plain', () => {
    const el = mount(
      '<table><thead><tr><th>C-0019</th></tr></thead>' +
        '<tbody><tr><td>models/x.md and M1.2</td></tr></tbody></table><p>C-0019</p>',
    )
    highlightElement(el, new Set(['M1.2']))
    expect(el.querySelectorAll('table .hl-ref, table .hl-docid, table .hl-lexicon')).toHaveLength(0)
    expect(el.querySelectorAll('table span')).toHaveLength(0)
    expect(el.querySelectorAll('p .hl-docid')).toHaveLength(1)
    el.remove()
  })

  it('skips pre, code, a, script, style and existing highlight spans', () => {
    const el = mount(
      '<pre>UC-1</pre><code>UC-2</code><a href="#">C-0019</a>' +
        '<script>UC-3</script><span class="hl-ref">UC-4</span><p>UC-5</p>',
    )
    highlightElement(el, new Set())
    const spans = [...el.querySelectorAll('span')]
    expect(spans).toHaveLength(2)
    expect(spans[0]!.className).toBe('hl-ref')
    expect(spans[1]!.className).toBe('hl-docid')
    expect(spans[1]!.textContent).toBe('UC-5')
    el.remove()
  })

  it('is idempotent when re-run on already highlighted markup', () => {
    const el = mount('<p>See UC-12 at models/x.md</p>')
    highlightElement(el, new Set())
    const once = el.innerHTML
    highlightElement(el, new Set())
    expect(el.innerHTML).toBe(once)
    expect(el.querySelectorAll('.hl-docid')).toHaveLength(1)
    expect(el.querySelectorAll('.hl-ref')).toHaveLength(1)
    el.remove()
  })

  it('replaces stale highlights when the lexicon changes', () => {
    const el = mount('<p>M1.2 and M1.3</p>')
    highlightElement(el, new Set(['M1.2']))
    expect([...el.querySelectorAll('.hl-lexicon')].map(s => s.textContent)).toEqual(['M1.2'])

    highlightElement(el, new Set(['M1.3']))
    expect([...el.querySelectorAll('.hl-lexicon')].map(s => s.textContent)).toEqual(['M1.3'])

    highlightElement(el, new Set())
    expect(el.querySelectorAll('.hl-lexicon')).toHaveLength(0)
    expect(el.textContent).toBe('M1.2 and M1.3')
    el.remove()
  })
})

describe('createHighlightDirective', () => {
  it('keeps the original text node and never duplicates interpolated text', () => {
    const lexicon = ref<ReadonlySet<string>>(new Set(['M1.2']))
    const directive = createHighlightDirective(lexicon)

    const el = document.createElement('div')
    const paragraph = document.createElement('p')
    const node = document.createTextNode('See UC-12 and M1.2')
    paragraph.appendChild(node)
    el.appendChild(paragraph)
    document.body.appendChild(el)

    directive.mounted(el)
    expect(el.querySelectorAll('span')).toHaveLength(2)
    expect(node.isConnected).toBe(true)

    // Vue's update order: beforeUpdate clears, then the text node is patched.
    directive.beforeUpdate(el)
    node.nodeValue = 'See UC-99 and M1.2'
    directive.updated(el)
    expect(el.textContent).toBe('See UC-99 and M1.2')
    expect([...el.querySelectorAll('.hl-docid')].map(span => span.textContent)).toEqual(['UC-99'])
    expect(node.isConnected).toBe(true)

    directive.unmounted(el)
    expect(el.textContent).toBe('See UC-99 and M1.2')
    el.remove()
  })

  it('never walks into a nested highlight root', async () => {
    const lexicon = ref<ReadonlySet<string>>(new Set<string>())
    const directive = createHighlightDirective(lexicon)

    const outer = document.createElement('div')
    outer.innerHTML = 'outer M1.2 <span id="inner">inner M1.2</span>'
    const inner = outer.querySelector('#inner') as HTMLElement
    document.body.appendChild(outer)

    // Vue mounts children before parents: the inner root is registered first.
    directive.mounted(inner)
    directive.mounted(outer)

    lexicon.value = new Set(['M1.2'])
    await nextTick()
    expect([...outer.querySelectorAll('.hl-lexicon')].map(span => span.textContent)).toEqual([
      'M1.2',
      'M1.2',
    ])
    expect(inner.querySelectorAll('.hl-lexicon')).toHaveLength(1)

    directive.unmounted(outer)
    expect(outer.textContent).toBe('outer M1.2 inner M1.2')
    expect(inner.querySelectorAll('.hl-lexicon')).toHaveLength(1)

    directive.unmounted(inner)
    expect(outer.textContent).toBe('outer M1.2 inner M1.2')
    outer.remove()
  })

  it('keeps nested roots intact when unmounted inner first', async () => {
    const lexicon = ref<ReadonlySet<string>>(new Set(['M1.2']))
    const directive = createHighlightDirective(lexicon)

    const outer = document.createElement('div')
    outer.innerHTML = 'outer M1.2 <span id="inner">inner M1.2</span>'
    const inner = outer.querySelector('#inner') as HTMLElement
    document.body.appendChild(outer)

    directive.mounted(inner)
    directive.mounted(outer)
    expect(outer.textContent).toBe('outer M1.2 inner M1.2')

    directive.unmounted(inner)
    expect(outer.textContent).toBe('outer M1.2 inner M1.2')

    lexicon.value = new Set(['M1.3'])
    await nextTick()
    expect(outer.textContent).toBe('outer M1.2 inner M1.2')
    expect(outer.querySelectorAll('.hl-lexicon')).toHaveLength(0)

    directive.unmounted(outer)
    expect(outer.textContent).toBe('outer M1.2 inner M1.2')
    outer.remove()
  })

  it('re-walks every mounted element when the lexicon set changes', async () => {
    const lexicon = ref<ReadonlySet<string>>(new Set<string>())
    const directive = createHighlightDirective(lexicon)

    const el = document.createElement('div')
    el.textContent = 'uses M1.2'
    document.body.appendChild(el)

    directive.mounted(el)
    expect(el.querySelector('.hl-lexicon')).toBeNull()

    lexicon.value = new Set(['M1.2'])
    await nextTick()
    expect(el.querySelector('.hl-lexicon')?.textContent).toBe('M1.2')

    directive.unmounted(el)
    expect(el.querySelector('.hl-lexicon')).toBeNull()
    expect(el.textContent).toBe('uses M1.2')
    lexicon.value = new Set(['M1.3'])
    await nextTick()
    // Unmounted elements leave the registry: nothing is re-highlighted.
    expect(el.querySelector('.hl-lexicon')).toBeNull()

    el.remove()
  })
})