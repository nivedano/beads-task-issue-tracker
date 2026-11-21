/**
 * End-to-end rendering proof for prose highlighting: the real
 * `GraceContractView` is mounted with the real `v-highlight` directive factory
 * (the same one `app/plugins/text-highlight.client.ts` registers), and the
 * asserted text is the `<Change>` prose of mtext's C-0002 contract.
 *
 * The lexicon Set mirrors what `loadLexicon` produces from the two real
 * documents (mtext + the cited Protocols lexicon), reduced to the terms the
 * fixture prose uses. Mounting the component (rather than calling the
 * tokenizer) is what makes this a consumer-visible proof: the directive runs
 * through Vue's own mount/update hooks over Vue-rendered text nodes.
 */
import { createApp, h, nextTick, ref, withDirectives, type App, type Directive } from 'vue'
import { afterEach, describe, expect, it } from 'vitest'
import GraceContractView from '~/components/grace/GraceContractView.vue'
import { createHighlightDirective } from '~/utils/text-highlight-dom'
import type { GraceContract } from '~/types/grace'

/** Terms present verbatim in the mtext + Protocols lexicons. */
const EXPECTED_TERMS = ['Order', 'SyntaxNode', 'Document', 'Catena', 'title', 'content', 'block']

/** The real `<Change>` sentence of mtext `.grace5/changes/C-0002-title-root`. */
const CHANGE_TEXT =
  "A Document's title Order and content Order root at one SyntaxNode whose allocation is the title; " +
  'the Catena hands off title first; block (UC-0010).'

function contractFixture(): GraceContract {
  return {
    id: 'C-0002-title-root',
    revision: 1,
    title: 'One node roots the title Order and the content Order of a Document',
    problem: 'The title Order roots at a title node of its own beside the content root.',
    change: CHANGE_TEXT,
    constraints: [{ id: 'K1', text: 'The Catena hands off title first.', rule: null }],
    nonGoals: [{ id: 'N1', text: 'A migration of stored Documents.' }],
    acceptance: [{ id: 'A1', text: 'The block reads with UC-0010.', checks: ['CK1'] }],
    affected: { paths: ['packages/metatext/**'], names: ['metatext model'], protected: [] },
    environments: [],
    checks: [],
    decisions: [{ id: 'D1', text: 'The Document keeps its title and content Orders.' }],
  }
}

let mounted: App[] = []

function container(): HTMLElement {
  const host = document.createElement('div')
  host.id = 'mount-root'
  document.body.append(host)
  return host
}

afterEach(() => {
  for (const app of mounted) app.unmount()
  mounted = []
  document.body.innerHTML = ''
})

describe('GraceContractView highlights through the real directive', () => {
  it('marks every lexicon term in the rendered contract prose', async () => {
    const host = container()
    const lexicon = ref<ReadonlySet<string>>(new Set(EXPECTED_TERMS))
    const highlight = createHighlightDirective(lexicon) as unknown as Directive

    const app = createApp({
      render: () => h(GraceContractView, { contract: contractFixture() }),
    })
    app.directive('highlight', highlight)
    app.mount(host)
    mounted.push(app)

    // Mounted hook runs synchronously; one tick settles child patching.
    await nextTick()
    await nextTick()

    const terms = new Set(
      [...host.querySelectorAll('.hl-lexicon')].map(span => span.textContent ?? ''),
    )
    for (const term of EXPECTED_TERMS) expect(terms).toContain(term)

    const docIds = new Set(
      [...host.querySelectorAll('.hl-docid')].map(span => span.textContent ?? ''),
    )
    expect(docIds).toContain('UC-0010')

    // The prose is still intact after the DOM walk.
    expect(host.textContent).toContain(CHANGE_TEXT)
  })

  it('re-applies when the shared lexicon Set changes', async () => {
    const host = container()
    const lexicon = ref<ReadonlySet<string>>(new Set<string>())
    const highlight = createHighlightDirective(lexicon) as unknown as Directive

    const app = createApp({
      render: () => h(GraceContractView, { contract: contractFixture() }),
    })
    app.directive('highlight', highlight)
    app.mount(host)
    mounted.push(app)
    await nextTick()
    expect(host.querySelectorAll('.hl-lexicon').length).toBe(0)

    lexicon.value = new Set(EXPECTED_TERMS)
    await nextTick()
    await nextTick()

    const terms = new Set(
      [...host.querySelectorAll('.hl-lexicon')].map(span => span.textContent ?? ''),
    )
    for (const term of EXPECTED_TERMS) expect(terms).toContain(term)
  })

  it('never highlights inside a <code> subtree', async () => {
    const host = container()
    const lexicon = ref<ReadonlySet<string>>(new Set(EXPECTED_TERMS))
    const highlight = createHighlightDirective(lexicon) as unknown as Directive

    const app = createApp({
      render: () =>
        withDirectives(
          h('div', [h('p', 'Order and block'), h('code', 'Order and block')]),
          [[highlight, undefined]],
        ),
    })
    app.mount(host)
    mounted.push(app)
    await nextTick()
    await nextTick()

    expect(host.querySelectorAll('p .hl-lexicon').length).toBeGreaterThan(0)
    expect(host.querySelector('code')?.querySelectorAll('.hl-lexicon').length).toBe(0)
    expect(host.querySelector('code')?.textContent).toBe('Order and block')
  })
})
