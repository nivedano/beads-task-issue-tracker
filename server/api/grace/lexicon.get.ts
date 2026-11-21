import { open, realpath, stat } from 'node:fs/promises'
import { resolve } from 'node:path'
import { isWithinRoot } from '../../utils/lexicon-path'

const MAX_LEXICON_BYTES = 1024 * 1024
const LEXICON_RELATIVE = ['models', 'architecture', 'term-lexicon.yaml'] as const

/**
 * Reads `<project>/models/architecture/term-lexicon.yaml` for prose
 * highlighting. Mirrors the Rust `read_lexicon` command: resolves symlinks,
 * refuses anything that escapes the project, caps the read at 1 MiB and
 * returns `null` when the file is missing or not a regular file.
 */
export default defineEventHandler(async (event) => {
  const query = getQuery(event)
  const project = typeof query.path === 'string' ? query.path.trim() : ''
  if (!project) return null

  try {
    const root = await realpath(resolve(project))
    const target = resolve(root, ...LEXICON_RELATIVE)
    const resolved = await realpath(target)
    if (!isWithinRoot(root, resolved)) return null

    const info = await stat(resolved)
    if (!info.isFile()) return null

    const length = Math.min(info.size, MAX_LEXICON_BYTES)
    if (length === 0) return ''

    const handle = await open(resolved, 'r')
    try {
      const buffer = Buffer.alloc(length)
      const { bytesRead } = await handle.read(buffer, 0, length, 0)
      return buffer.subarray(0, bytesRead).toString('utf8')
    } finally {
      await handle.close()
    }
  } catch {
    // Missing file or unreadable project: highlighting degrades gracefully.
    return null
  }
})