import { describe, it, expect, afterEach } from 'vitest'
import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join, posix, win32 } from 'node:path'
import {
  citedLexicon,
  findLexiconFiles,
  isWithinRoot,
  LEXICON_MAX_DEPTH,
  readProjectLexicons,
  schemaCitation,
} from '../../server/utils/lexicon-path'

describe('isWithinRoot', () => {
  it('accepts a project that is a Windows drive root', () => {
    expect(
      isWithinRoot('D:\\', 'D:\\models\\architecture\\term-lexicon.yaml', win32),
    ).toBe(true)
  })

  it('rejects a path on another drive from a drive root', () => {
    expect(isWithinRoot('D:\\', 'C:\\models\\architecture\\term-lexicon.yaml', win32)).toBe(false)
  })

  it('accepts a project at the POSIX root', () => {
    expect(isWithinRoot('/', '/models/architecture/term-lexicon.yaml', posix)).toBe(true)
  })

  it('rejects escapes, siblings and similarly named prefixes', () => {
    expect(isWithinRoot('/proj', '/proj/../x', posix)).toBe(false)
    expect(isWithinRoot('/proj', '/other/models/x.yaml', posix)).toBe(false)
    expect(isWithinRoot('/proj', '/project/models/x.yaml', posix)).toBe(false)
    expect(isWithinRoot('/proj', '/proj/models/x.yaml', posix)).toBe(true)
  })
})

describe('schemaCitation', () => {
  it('takes the first column-0 Schema citation only', () => {
    const text = [
      '# Term lexicon — mText',
      '# Schema: ../../../metasteme-protocols/models/architecture/core/term-lexicon-schema.md (term-lexicon/v1).',
      '# Schema: ignored.md',
      '  # Schema: indented.md',
    ].join('\n')
    expect(schemaCitation(text)).toBe(
      '../../../metasteme-protocols/models/architecture/core/term-lexicon-schema.md',
    )
    expect(schemaCitation('Schema: not-a-comment.md')).toBeNull()
    expect(schemaCitation('# Schema:')).toBeNull()
  })
})

describe('findLexiconFiles', () => {
  const created: string[] = []

  async function project(): Promise<string> {
    const dir = await mkdtemp(join(tmpdir(), 'lexicon-web-'))
    created.push(dir)
    return dir
  }

  async function writeLexicon(root: string, relative: string, contents: string) {
    const path = join(root, relative)
    await mkdir(dirname(path), { recursive: true })
    await writeFile(path, contents)
  }

  async function contentsOf(paths: string[]): Promise<string[]> {
    return Promise.all(paths.map(path => readFile(path, 'utf8')))
  }

  afterEach(async () => {
    await Promise.all(created.splice(0).map(dir => rm(dir, { recursive: true, force: true })))
  })

  it('returns [] when models/ is missing', async () => {
    const root = await project()
    expect(await findLexiconFiles(root)).toEqual([])
  })

  it('finds a lexicon in a nested subdirectory', async () => {
    const root = await project()
    await writeLexicon(root, 'models/deep/nested/term-lexicon.yaml', 'M1.2: gloss\n')
    expect(await contentsOf(await findLexiconFiles(root))).toEqual(['M1.2: gloss\n'])
  })

  it('accepts the .yml file name', async () => {
    const root = await project()
    await writeLexicon(root, 'models/architecture/term-lexicon.yml', '- M1.2\n')
    expect(await contentsOf(await findLexiconFiles(root))).toEqual(['- M1.2\n'])
  })

  it('returns every lexicon in breadth-first order', async () => {
    const root = await project()
    await writeLexicon(root, 'models/deep/nested/term-lexicon.yaml', 'deep\n')
    await writeLexicon(root, 'models/architecture/term-lexicon.yaml', 'shallow\n')
    expect(await contentsOf(await findLexiconFiles(root))).toEqual(['shallow\n', 'deep\n'])
  })

  it('keeps sorted order at equal depth', async () => {
    const root = await project()
    await writeLexicon(root, 'models/b/x/term-lexicon.yaml', 'b\n')
    await writeLexicon(root, 'models/a/x/term-lexicon.yaml', 'a\n')
    expect(await contentsOf(await findLexiconFiles(root))).toEqual(['a\n', 'b\n'])
  })

  it('skips hidden and node_modules directories', async () => {
    const root = await project()
    await writeLexicon(root, 'models/.hidden/term-lexicon.yaml', 'hidden\n')
    await writeLexicon(root, 'models/node_modules/pkg/term-lexicon.yaml', 'modules\n')
    expect(await findLexiconFiles(root)).toEqual([])
  })

  it('does not descend symlinked directories', async () => {
    const root = await project()
    const outside = await project()
    await writeFile(join(outside, 'term-lexicon.yaml'), 'outside\n')
    await mkdir(join(root, 'models'), { recursive: true })
    try {
      await symlink(outside, join(root, 'models', 'linked'), 'junction')
    } catch {
      return // symlinks unavailable in this environment
    }
    expect(await findLexiconFiles(root)).toEqual([])
  })

  it('rejects a symlinked file escaping the project', async () => {
    const root = await project()
    const outside = await project()
    await writeFile(join(outside, 'lexicon.yaml'), 'outside\n')
    const target = join(root, 'models', 'architecture', 'term-lexicon.yaml')
    await mkdir(dirname(target), { recursive: true })
    try {
      await symlink(join(outside, 'lexicon.yaml'), target, 'file')
    } catch {
      return // symlinks unavailable in this environment
    }
    expect(await findLexiconFiles(root)).toEqual([])
  })

  it('caps the search depth', async () => {
    const root = await project()
    const segments = Array.from({ length: LEXICON_MAX_DEPTH }, (_, level) => `d${level}`)
    await writeLexicon(root, join('models', ...segments, 'term-lexicon.yaml'), 'eight\n')
    expect(await contentsOf(await findLexiconFiles(root))).toEqual(['eight\n'])

    const tooDeep = await project()
    const deeper = [...segments, `d${LEXICON_MAX_DEPTH}`]
    await writeLexicon(tooDeep, join('models', ...deeper, 'term-lexicon.yaml'), 'nine\n')
    expect(await findLexiconFiles(tooDeep)).toEqual([])
  })
})

describe('readProjectLexicons', () => {
  const created: string[] = []

  async function project(): Promise<string> {
    const dir = await mkdtemp(join(tmpdir(), 'lexicon-import-'))
    created.push(dir)
    return dir
  }

  async function writeAt(root: string, relative: string, contents: string) {
    const path = join(root, relative)
    await mkdir(dirname(path), { recursive: true })
    await writeFile(path, contents)
  }

  /** `<workspace>/repos/mtext` citing `<workspace>/repos/metasteme-protocols`. */
  async function citedLayout(): Promise<{ workspace: string, project: string, core: string }> {
    const workspace = await project()
    const root = join(workspace, 'repos')
    const sibling = join(root, 'mtext')
    const core = join(root, 'metasteme-protocols', 'models', 'architecture', 'core')
    await mkdir(core, { recursive: true })
    await writeFile(join(core, 'term-lexicon-schema.md'), '# schema\n')
    await writeFile(
      join(core, 'term-lexicon.yaml'),
      '# Term lexicon\n# Schema: term-lexicon-schema.md\n- term: Order\n',
    )
    return { workspace, project: sibling, core }
  }

  afterEach(async () => {
    await Promise.all(created.splice(0).map(dir => rm(dir, { recursive: true, force: true })))
  })

  it('unions two project lexicons', async () => {
    const root = await project()
    await writeAt(root, 'models/a/term-lexicon.yaml', '- term: title\n')
    await writeAt(root, 'models/b/term-lexicon.yaml', '- term: Order\n')
    expect(await readProjectLexicons(root)).toEqual(['- term: title\n', '- term: Order\n'])
  })

  it('imports the lexicon cited by a schema header', async () => {
    const { project: root, core } = await citedLayout()
    const projectText = [
      '# Term lexicon',
      '# Schema: ../../../metasteme-protocols/models/architecture/core/term-lexicon-schema.md (term-lexicon/v1).',
      '- term: title',
      '',
    ].join('\n')
    await writeAt(root, 'models/architecture/term-lexicon.yaml', projectText)

    const texts = await readProjectLexicons(root)
    expect(texts).toHaveLength(2)
    expect(texts[0]).toBe(projectText)
    expect(await readFile(join(core, 'term-lexicon.yaml'), 'utf8')).toBe(texts[1])
  })

  it('uses a citation that names the lexicon file directly', async () => {
    const { project: root } = await citedLayout()
    await writeAt(
      root,
      'models/architecture/term-lexicon.yaml',
      '# Schema: ../../../metasteme-protocols/models/architecture/core/term-lexicon.yaml\n- term: title\n',
    )
    const texts = await readProjectLexicons(root)
    expect(texts).toHaveLength(2)
    expect(texts[1]).toContain('term: Order')
  })

  it('rejects an absolute citation', async () => {
    const { project: root, core } = await citedLayout()
    await writeAt(
      root,
      'models/architecture/term-lexicon.yaml',
      `# Schema: ${join(core, 'term-lexicon.yaml')}\n- term: title\n`,
    )
    expect(await readProjectLexicons(root)).toHaveLength(1)
  })

  it('rejects a citation that escapes the cited root through ..', async () => {
    const { workspace, project: root } = await citedLayout()
    await writeAt(workspace, 'outside/term-lexicon.yaml', '- term: leaked\n')
    await writeAt(
      root,
      'models/architecture/term-lexicon.yaml',
      '# Schema: ../../../metasteme-protocols/models/architecture/../../../../outside/term-lexicon.yaml\n- term: title\n',
    )
    expect(await readProjectLexicons(root)).toHaveLength(1)
  })

  it('rejects a symlinked citation escape', async () => {
    const { workspace, project: root } = await citedLayout()
    const outside = await project()
    await writeFile(join(outside, 'term-lexicon.yaml'), '- term: leaked\n')
    await rm(join(workspace, 'repos', 'metasteme-protocols'), { recursive: true, force: true })
    try {
      await symlink(outside, join(workspace, 'repos', 'metasteme-protocols'), 'junction')
    } catch {
      return // symlinks unavailable in this environment
    }
    await writeAt(
      root,
      'models/architecture/term-lexicon.yaml',
      '# Schema: ../../../metasteme-protocols/term-lexicon-schema.md\n- term: title\n',
    )
    expect(await readProjectLexicons(root)).toHaveLength(1)
  })

  it('skips a missing cited lexicon', async () => {
    const { project: root, core } = await citedLayout()
    await rm(join(core, 'term-lexicon.yaml'))
    await writeAt(
      root,
      'models/architecture/term-lexicon.yaml',
      '# Schema: ../../../metasteme-protocols/models/architecture/core/term-lexicon-schema.md\n- term: title\n',
    )
    expect(await readProjectLexicons(root)).toHaveLength(1)
  })

  it('follows citations one hop only', async () => {
    const { project: root, core } = await citedLayout()
    await writeAt(core, 'other/term-lexicon.yaml', '- term: third\n')
    await writeAt(
      core,
      'term-lexicon.yaml',
      '# Schema: other/term-lexicon-schema.md\n- term: Order\n',
    )
    await writeAt(
      root,
      'models/architecture/term-lexicon.yaml',
      '# Schema: ../../../metasteme-protocols/models/architecture/core/term-lexicon-schema.md\n- term: title\n',
    )
    const texts = await readProjectLexicons(root)
    expect(texts).toHaveLength(2)
    expect(texts.every(text => !text.includes('term: third'))).toBe(true)
  })

  it('reads duplicate canonical paths once', async () => {
    const { project: root } = await citedLayout()
    const citation = '# Schema: ../../../metasteme-protocols/models/architecture/core/term-lexicon-schema.md\n'
    await writeAt(root, 'models/a/term-lexicon.yaml', `${citation}- term: a\n`)
    await writeAt(root, 'models/b/term-lexicon.yaml', `${citation}- term: b\n`)
    const texts = await readProjectLexicons(root)
    expect(texts).toHaveLength(3)
    expect(texts.filter(text => text.includes('term: Order'))).toHaveLength(1)
  })

  it('caps each lexicon read at one MiB', async () => {
    const root = await project()
    const contents = 'a'.repeat(1024 * 1024 + 10)
    await writeAt(root, 'models/architecture/term-lexicon.yaml', contents)
    const texts = await readProjectLexicons(root)
    expect(texts).toHaveLength(1)
    expect(texts[0]).toHaveLength(1024 * 1024)
  })

  it('returns [] for a missing project', async () => {
    expect(await readProjectLexicons(join(tmpdir(), 'lexicon-does-not-exist-xyz'))).toEqual([])
  })
})

describe('citedLexicon', () => {
  const created: string[] = []

  afterEach(async () => {
    await Promise.all(created.splice(0).map(dir => rm(dir, { recursive: true, force: true })))
  })

  it('returns null for a citation that does not resolve', async () => {
    const root = await mkdtemp(join(tmpdir(), 'lexicon-cited-'))
    created.push(root)
    await mkdir(join(root, 'models', 'architecture'), { recursive: true })
    const file = join(root, 'models', 'architecture', 'term-lexicon.yaml')
    await writeFile(file, '# Schema: ../../../missing/term-lexicon.yaml\n')
    expect(await citedLexicon(file, '../../../missing/term-lexicon.yaml')).toBeNull()
    expect(await citedLexicon(file, 'C:\\other\\term-lexicon.yaml')).toBeNull()
    expect(await citedLexicon(file, '\\\\server\\share\\term-lexicon.yaml')).toBeNull()
  })
})