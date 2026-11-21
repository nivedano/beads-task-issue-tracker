import { open, realpath, stat } from 'node:fs/promises'
import { resolve } from 'node:path'
import { findLexiconFile } from '../../utils/lexicon-path'

const MAX_LEXICON_BYTES = 1024 * 1024

/**
 * Reads the project's architecture term lexicon for prose highlighting.
 * Mirrors the Rust `read_lexicon` command: discovers `term-lexicon.yaml`/`.yml`
 * anywhere under `<project>/models/` (see `findLexiconFile` for the discovery
 * and tie-break rules), caps the read at 1 MiB and returns `null` when no
 * lexicon exists or it is unreadable.
 */
export default defineEventHandler(async (event) => {
  const query = getQuery(event)
  const project = typeof query.path === 'string' ? query.path.trim() : ''
  if (!project) return null

  try {
    const root = await realpath(resolve(project))
    const discovered = await findLexiconFile(root)
    if (!discovered) return null

    const info = await stat(discovered)
    if (!info.isFile()) return null

    const length = Math.min(info.size, MAX_LEXICON_BYTES)
    if (length === 0) return ''

    const handle = await open(discovered, 'r')
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