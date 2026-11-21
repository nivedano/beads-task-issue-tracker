import { lstat, readdir, realpath, stat } from 'node:fs/promises'
import { isAbsolute, relative, resolve, sep } from 'node:path'

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

/**
 * Finds the project's architecture term lexicon by breadth-first walk of
 * `<root>/models/`, shallowest file first. Entries of every directory are
 * sorted, so a tie at the same depth resolves to the lexicographically smallest
 * path relative to `models/`. Hidden directories and `node_modules` are
 * skipped, symlinked directories are never descended (cycle/escape safety), and
 * a symlinked file is accepted only when its canonical path stays inside the
 * canonical project root.
 *
 * Mirrors the Rust `find_project_lexicon` walk; returns the resolved file path
 * or `null` when `models/` (or a matching file) is missing.
 */
export async function findLexiconFile(projectRoot: string): Promise<string | null> {
  let root: string
  try {
    root = await realpath(resolve(projectRoot))
  } catch {
    return null
  }

  const models = resolve(root, 'models')
  try {
    const info = await lstat(models)
    if (info.isSymbolicLink() || !info.isDirectory()) return null
  } catch {
    return null
  }

  let best: { depth: number, relative: string, path: string } | null = null
  const queue: { dir: string, depth: number }[] = [{ dir: models, depth: 0 }]

  while (queue.length > 0) {
    const current = queue.shift()!
    if (best !== null && current.depth > best.depth) break

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

      const rel = relative(models, path).split(sep).join('/')
      const better =
        best === null
        || current.depth < best.depth
        || (current.depth === best.depth && rel < best.relative)
      if (better) best = { depth: current.depth, relative: rel, path: file }
    }

    for (const subdirectory of subdirectories) {
      queue.push({ dir: subdirectory, depth: current.depth + 1 })
    }
  }

  return best?.path ?? null
}