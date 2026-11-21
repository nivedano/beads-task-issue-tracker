import { describe, it, expect, afterEach } from 'vitest'
import { mkdir, mkdtemp, readFile, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join, posix, win32 } from 'node:path'
import {
  findLexiconFile,
  isWithinRoot,
  LEXICON_MAX_DEPTH,
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

describe('findLexiconFile', () => {
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

  async function contentOf(path: string | null): Promise<string | null> {
    return path === null ? null : readFile(path, 'utf8')
  }

  afterEach(async () => {
    await Promise.all(created.splice(0).map(dir => rm(dir, { recursive: true, force: true })))
  })

  it('returns null when models/ is missing', async () => {
    const root = await project()
    expect(await findLexiconFile(root)).toBeNull()
  })

  it('finds a lexicon in a nested subdirectory', async () => {
    const root = await project()
    await writeLexicon(root, 'models/deep/nested/term-lexicon.yaml', 'M1.2: gloss\n')
    expect(await contentOf(await findLexiconFile(root))).toBe('M1.2: gloss\n')
  })

  it('accepts the .yml file name', async () => {
    const root = await project()
    await writeLexicon(root, 'models/architecture/term-lexicon.yml', '- M1.2\n')
    expect(await contentOf(await findLexiconFile(root))).toBe('- M1.2\n')
  })

  it('prefers the shallowest lexicon', async () => {
    const root = await project()
    await writeLexicon(root, 'models/deep/nested/term-lexicon.yaml', 'deep\n')
    await writeLexicon(root, 'models/architecture/term-lexicon.yaml', 'shallow\n')
    expect(await contentOf(await findLexiconFile(root))).toBe('shallow\n')
  })

  it('breaks equal-depth ties by the smallest relative path', async () => {
    const root = await project()
    await writeLexicon(root, 'models/b/x/term-lexicon.yaml', 'b\n')
    await writeLexicon(root, 'models/a/x/term-lexicon.yaml', 'a\n')
    expect(await contentOf(await findLexiconFile(root))).toBe('a\n')
  })

  it('skips hidden and node_modules directories', async () => {
    const root = await project()
    await writeLexicon(root, 'models/.hidden/term-lexicon.yaml', 'hidden\n')
    await writeLexicon(root, 'models/node_modules/pkg/term-lexicon.yaml', 'modules\n')
    expect(await findLexiconFile(root)).toBeNull()
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
    expect(await findLexiconFile(root)).toBeNull()
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
    expect(await findLexiconFile(root)).toBeNull()
  })

  it('caps the search depth', async () => {
    const root = await project()
    const segments = Array.from({ length: LEXICON_MAX_DEPTH }, (_, level) => `d${level}`)
    await writeLexicon(root, join('models', ...segments, 'term-lexicon.yaml'), 'eight\n')
    expect(await contentOf(await findLexiconFile(root))).toBe('eight\n')

    const tooDeep = await project()
    const deeper = [...segments, `d${LEXICON_MAX_DEPTH}`]
    await writeLexicon(tooDeep, join('models', ...deeper, 'term-lexicon.yaml'), 'nine\n')
    expect(await findLexiconFile(tooDeep)).toBeNull()
  })
})