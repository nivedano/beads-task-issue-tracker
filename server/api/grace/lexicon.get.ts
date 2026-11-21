import { readProjectLexicons } from '../../utils/lexicon-path'

/**
 * Reads every architecture term lexicon for prose highlighting: all
 * `term-lexicon.yaml`/`.yml` files under `<project>/models/` plus each file's
 * one-hop `# Schema:` citation to a sibling repository's lexicon.
 * Mirrors the Rust `read_lexicon` command (see `readProjectLexicons` for the
 * discovery, citation-boundary and 1 MiB cap rules). Always returns an array;
 * a missing or unreadable project yields `[]` and highlighting degrades to the
 * two structural classes.
 */
export default defineEventHandler(async (event) => {
  const query = getQuery(event)
  const project = typeof query.path === 'string' ? query.path.trim() : ''
  if (!project) return []

  try {
    return await readProjectLexicons(project)
  } catch {
    return []
  }
})