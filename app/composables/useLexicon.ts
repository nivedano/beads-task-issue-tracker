/**
 * Shared architecture term lexicon (singleton).
 *
 * `loadLexicon(projectPath)` is called by `pages/index.vue` when the opened
 * project changes. The `v-highlight` plugin watches `lexicon` and re-walks
 * every mounted prose container, so no component needs the set as a prop.
 *
 * A missing or unreadable lexicon file degrades to an empty set (paths/URLs and
 * doc ids still highlight); only real IPC/route failures are logged.
 */
import { ref } from 'vue'
import { graceReadLexicon, logFrontend } from '~/utils/br-api'
import { parseLexicon } from '~/utils/text-highlight'

const lexicon = ref<ReadonlySet<string>>(new Set<string>())

// Stale-response guard for rapid project switches.
let generation = 0

export async function loadLexicon(projectPath: string | null | undefined): Promise<void> {
  const path = projectPath?.trim() ?? ''
  const request = ++generation

  if (!path) {
    lexicon.value = new Set<string>()
    return
  }

  try {
    const documents = await graceReadLexicon(path)
    if (request !== generation) return
    const merged = new Set<string>()
    for (const document of documents) {
      for (const term of parseLexicon(document)) merged.add(term)
    }
    lexicon.value = merged
  } catch (error) {
    if (request !== generation) return
    lexicon.value = new Set<string>()
    void logFrontend(
      'warn',
      `[lexicon] ${error instanceof Error ? error.message : String(error)}`,
    )
  }
}

export function useLexicon() {
  return { lexicon, loadLexicon }
}