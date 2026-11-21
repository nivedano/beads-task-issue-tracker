import { describe, it, expect, vi, beforeEach } from 'vitest'

vi.mock('~/utils/br-api', () => ({
  graceReadLexicon: vi.fn(),
  logFrontend: vi.fn(),
}))

/**
 * Fresh module instance per test: useLexicon holds module-level singleton state
 * (the lexicon ref and the stale-response generation). Dynamic imports are
 * required because `vi.resetModules()` must re-evaluate the composable against
 * freshly created mock functions.
 */
async function loadLexicon() {
  const { useLexicon } = await import('~/composables/useLexicon')
  const api = await import('~/utils/br-api')
  return { composer: useLexicon(), read: vi.mocked(api.graceReadLexicon) }
}

function deferred<T>() {
  return Promise.withResolvers<T>()
}

beforeEach(() => {
  vi.resetModules()
  vi.resetAllMocks()
})

describe('useLexicon', () => {
  it('unions the terms parsed from every returned document', async () => {
    const { composer, read } = await loadLexicon()
    read.mockResolvedValue([
      [
        'schema: term-lexicon/v1',
        'entries:',
        '  - term: title',
        '  - term: block',
        '  - term: replace the text',
      ].join('\n'),
      [
        'schema: term-lexicon/v1',
        'entries:',
        '  - term: Order',
        '  - term: SyntaxNode',
      ].join('\n'),
    ])

    await composer.loadLexicon('/project')

    expect([...composer.lexicon.value].sort()).toEqual([
      'Order',
      'SyntaxNode',
      'block',
      'replace the text',
      'title',
    ])
  })

  it('starts empty and degrades to empty on a failed read', async () => {
    const { composer, read } = await loadLexicon()
    expect(composer.lexicon.value.size).toBe(0)

    read.mockResolvedValue([])
    await composer.loadLexicon('/project')
    expect(composer.lexicon.value.size).toBe(0)

    read.mockRejectedValue(new Error('route failed'))
    await composer.loadLexicon('/other')
    expect(composer.lexicon.value.size).toBe(0)
  })

  it('clears the lexicon without a project path', async () => {
    const { composer, read } = await loadLexicon()
    read.mockResolvedValue(['- term: title\n'])
    await composer.loadLexicon('/project')
    expect(composer.lexicon.value.has('title')).toBe(true)

    await composer.loadLexicon(null)
    expect(composer.lexicon.value.size).toBe(0)
    expect(read).toHaveBeenCalledTimes(1)
  })

  it('discards a stale response after a project switch', async () => {
    const { composer, read } = await loadLexicon()
    const older = deferred<string[]>()
    const newer = deferred<string[]>()
    read.mockReturnValueOnce(older.promise).mockReturnValueOnce(newer.promise)

    const loadOlder = composer.loadLexicon('/project-a')
    const loadNewer = composer.loadLexicon('/project-b')

    newer.resolve(['- term: Order\n'])
    await loadNewer
    older.resolve(['- term: title\n'])
    await loadOlder

    expect([...composer.lexicon.value]).toEqual(['Order'])
  })
})