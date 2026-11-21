import { lstat, open, readdir, realpath, stat } from 'node:fs/promises'
import { basename, dirname, isAbsolute, join, relative, resolve, sep, win32 } from 'node:path'

type PathApi = {
  relative: (from: string, to: string) => string
  isAbsolute: (path: string) => boolean
  sep: string
}

/**
 * Whether `resolved` stays inside `root`. Built on `path.relative` so a project
 * that *is* a filesystem root (`D:\`, `/`) is handled correctly — the common
 * `startsWith(root + sep)` check rejects every path under a drive root.
 */
export function isWithinRoot(
  root: string,
  resolved: string,
  path: PathApi = { relative, isAbsolute, sep },
): boolean {
  const inside = path.relative(root, resolved)
  if (inside === '') return true
  if (path.isAbsolute(inside)) return false
  return inside !== '..' && !inside.startsWith(`..${path.sep}`)
}

export const LEXICON_FILE_NAMES: readonly string[] = [
  'term-lexicon.yaml',
  'term-lexicon.yml',
]

/** Directories searched below `models/` are capped at this depth (models/ = 0). */
export const LEXICON_MAX_DEPTH = 8

/** Lexicon reads are capped like notes (`MAX_READ_BYTES` in Rust). */
const MAX_LEXICON_BYTES = 1024 * 1024

/** The first `# Schema: <citation>` header line, at column 0. */
export function schemaCitation(text: string): string | null {
  for (const line of text.split(/\r?\n/)) {
    const match = /^#\s*Schema:\s*(\S+)/.exec(line)
    if (match) return match[1]!
  }
  return null
}

/**
 * Finds every architecture term lexicon below `<root>/models/` in
 * breadth-first order. Entries of every directory are sorted, so the order is
 * deterministic. Hidden directories and `node_modules` are skipped, symlinked
 * directories are never descended (cycle/escape safety), and a symlinked file
 * is accepted only when its canonical path stays inside the canonical project
 * root.
 *
 * Mirrors the Rust `find_project_lexicons` walk; returns the resolved file
 * paths or `[]` when `models/` (or a matching file) is missing.
 */
export async function findLexiconFiles(projectRoot: string): Promise<string[]> {
  let root: string
  try {
    root = await realpath(resolve(projectRoot))
  } catch {
    return []
  }

  const models = resolve(root, 'models')
  try {
    const info = await lstat(models)
    if (info.isSymbolicLink() || !info.isDirectory()) return []
  } catch {
    return []
  }

  const found: string[] = []
  const queue: { dir: string, depth: number }[] = [{ dir: models, depth: 0 }]

  while (queue.length > 0) {
    const current = queue.shift()!

    let entries
    try {
      entries = await readdir(current.dir, { withFileTypes: true })
    } catch {
      continue
    }
    entries.sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0))

    const subdirectories: string[] = []
    for (const entry of entries) {
      const path = resolve(current.dir, entry.name)
      if (entry.isDirectory()) {
        if (
          current.depth < LEXICON_MAX_DEPTH
          && !entry.name.startsWith('.')
          && entry.name !== 'node_modules'
        ) {
          subdirectories.push(path)
        }
        continue
      }
      if (!LEXICON_FILE_NAMES.includes(entry.name)) continue

      let file = path
      if (entry.isSymbolicLink()) {
        try {
          file = await realpath(path)
        } catch {
          continue
        }
        if (!isWithinRoot(root, file)) continue
      }
      try {
        const info = await stat(file)
        if (!info.isFile()) continue
      } catch {
        continue
      }
      found.push(file)
    }

    for (const subdirectory of subdirectories) {
      queue.push({ dir: subdirectory, depth: current.depth + 1 })
    }
  }

  return found
}

/**
 * Resolves the lexicon cited by a lexicon's `# Schema:` header, ONE hop.
 *
 * The citation is relative to the citing file. Leading `..` components climb
 * toward the shared parent; the canonicalized import target must stay inside
 * that canonicalized root. Absolute paths, drive/UNC citations, interior
 * escapes and symlinked citation directories are rejected. The citation may
 * name the schema document beside the lexicon; the lexicon is then looked up
 * in the citation target's directory. Missing files yield `null`.
 */
export async function citedLexicon(file: string, citation: string): Promise<string | null> {
  if (
    citation.length === 0
    || isAbsolute(citation)
    || win32.isAbsolute(citation)
    || citation.startsWith('/')
  ) {
    return null
  }

  const segments = citation.split(/[\\/]+/)
  let leadingParents = 0
  for (const segment of segments) {
    if (segment === '.') continue
    if (segment === '..') {
      leadingParents++
      continue
    }
    break
  }

  let lexiconDir: string
  try {
    lexiconDir = await realpath(dirname(file))
  } catch {
    return null
  }

  let citedRootPath = lexiconDir
  for (let level = 0; level < leadingParents; level++) {
    const parent = dirname(citedRootPath)
    if (parent === citedRootPath) return null
    citedRootPath = parent
  }

  let citedRoot: string
  try {
    citedRoot = await realpath(citedRootPath)
  } catch {
    return null
  }

  const target = resolve(lexiconDir, citation)
  const namesLexicon = LEXICON_FILE_NAMES.includes(basename(target))
  const directory = dirname(target)

  // A symlinked citation directory is never descended.
  try {
    const info = await lstat(directory)
    if (info.isSymbolicLink() || !info.isDirectory()) return null
  } catch {
    return null
  }

  const candidates = namesLexicon
    ? [target]
    : LEXICON_FILE_NAMES.map(name => join(directory, name))

  for (const candidate of candidates) {
    let canonical: string
    try {
      canonical = await realpath(candidate)
    } catch {
      continue
    }
    if (!isWithinRoot(citedRoot, canonical)) continue
    try {
      if (!(await stat(canonical)).isFile()) continue
    } catch {
      continue
    }
    return canonical
  }
  return null
}

async function readCapped(path: string): Promise<string | null> {
  try {
    const info = await stat(path)
    if (!info.isFile()) return null
    const length = Math.min(info.size, MAX_LEXICON_BYTES)
    if (length === 0) return ''
    const handle = await open(path, 'r')
    try {
      const buffer = Buffer.alloc(length)
      const { bytesRead } = await handle.read(buffer, 0, length, 0)
      return buffer.subarray(0, bytesRead).toString('utf8')
    } finally {
      await handle.close()
    }
  } catch {
    return null
  }
}

/**
 * Reads every lexicon of a project (see `findLexiconFiles`), then follows each
 * lexicon's `# Schema:` header citation one hop to a sibling repository's
 * lexicon (the cited schema document sits beside its lexicon). Project files
 * come first in BFS order, imports after them in citation order, and canonical
 * duplicates are read once. Missing or unreadable files are skipped; each read
 * is capped at 1 MiB. Mirrors the Rust `read_project_lexicon`.
 */
export async function readProjectLexicons(projectRoot: string): Promise<string[]> {
  const files = await findLexiconFiles(projectRoot)

  const seen = new Set<string>()
  const texts: string[] = []
  const citations: { file: string, citation: string }[] = []

  for (const file of files) {
    let key: string
    try {
      key = await realpath(file)
    } catch {
      key = file
    }
    if (seen.has(key)) continue
    seen.add(key)

    const text = await readCapped(file)
    if (text === null) continue
    const citation = schemaCitation(text)
    if (citation !== null) citations.push({ file, citation })
    texts.push(text)
  }

  // One hop only: an imported lexicon's own citation is never followed.
  for (const { file, citation } of citations) {
    const imported = await citedLexicon(file, citation)
    if (imported === null || seen.has(imported)) continue
    seen.add(imported)
    const text = await readCapped(imported)
    if (text !== null) texts.push(text)
  }

  return texts
}