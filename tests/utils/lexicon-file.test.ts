import { describe, it, expect, afterEach } from 'vitest'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { parseLexicon } from '~/utils/text-highlight'
import { readProjectLexicons } from '../../server/utils/lexicon-path'

/**
 * End-to-end check for the desktop read path: a project root on disk gets real
 * lexicon files, `readProjectLexicons` mirrors the Rust discovery + cited-import
 * walk, and the frontend parser turns each file content into the highlight set.
 * The Tauri IPC boundary itself is exercised by the Rust tests in
 * `src-tauri/src/grace`.
 */
describe('lexicon file discovery and parsing', () => {
  const created: string[] = []

  async function project(): Promise<string> {
    const dir = await mkdtemp(join(tmpdir(), 'lexicon-project-'))
    created.push(dir)
    return dir
  }

  afterEach(async () => {
    await Promise.all(created.splice(0).map(dir => rm(dir, { recursive: true, force: true })))
  })

  it('parses a mapping-shaped lexicon nested under models/', async () => {
    const root = await project()
    const file = join(root, 'models', 'deep', 'nested', 'term-lexicon.yaml')
    await mkdir(dirname(file), { recursive: true })
    await writeFile(
      file,
      ['M1.2: first gloss', 'M1.3:', '  A definition block M9.9 must not leak.', ''].join('\n'),
    )

    const discovered = await readProjectLexicons(root)
    expect(discovered).toHaveLength(1)
    expect([...parseLexicon(discovered[0]!)]).toEqual(['M1.2', 'M1.3'])
  })

  it('parses a lexicon at the documented architecture location', async () => {
    const root = await project()
    const file = join(root, 'models', 'architecture', 'term-lexicon.yaml')
    await mkdir(dirname(file), { recursive: true })
    await writeFile(
      file,
      ['---', 'terms:', '  - M1.2        # comment', '  - "arch.method.v2"', '...', ''].join('\n'),
    )

    const documents = await readProjectLexicons(root)
    expect(documents).toHaveLength(1)
    expect([...parseLexicon(documents[0]!)]).toEqual(['M1.2', 'arch.method.v2'])
  })

  it('parses the self-describing entries: schema used by real lexicons', async () => {
    const root = await project()
    const file = join(root, 'models', 'architecture', 'term-lexicon.yaml')
    await mkdir(dirname(file), { recursive: true })
    await writeFile(
      file,
      [
        '# Term lexicon — mText (surface population).',
        'schema: term-lexicon/v1',
        'tree: mtext',
        'owner: mtext',
        'entries:',
        '',
        '  # ---- document domain ----',
        '  - term: block',
        '    namespace: mtext',
        '    area: document-domain',
        '    definition: models/architecture/outliner.md#blocks',
        '    maps_to: mdm:SyntaxNode',
        '    relation: surface_of',
        '    note: \'classification: surface reading of mdm:SyntaxNode\'',
        '',
        '  # ---- protocol ----',
        '  - term: x.mtext.move',
        '    namespace: protocol',
        '    area: operations',
        '    kind: protocol-act',
        '',
        '  - term: replace the text',
        '    namespace: mtext',
        '    area: operations',
        '    kind: operation',
        '',
        '  - term: root-entry',
        '    namespace: mtext',
        '    area: operations',
        '    kind: refusal',
        '',
        '  - term: NOT_LOADED',
        '    namespace: platform',
        '',
      ].join('\n'),
    )

    const discovered = await readProjectLexicons(root)
    expect(discovered).toHaveLength(1)
    expect([...parseLexicon(discovered[0]!)]).toEqual([
      'block',
      'x.mtext.move',
      'replace the text',
      'root-entry',
      'NOT_LOADED',
    ])
  })

  it('yields an empty set when the project has no lexicon file', async () => {
    const root = await project()
    const discovered = await readProjectLexicons(root)
    expect(discovered).toEqual([])
    expect(parseLexicon('')).toEqual(new Set())
  })
})