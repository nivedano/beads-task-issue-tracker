import { createHighlightDirective } from '~/utils/text-highlight-dom'
import { useLexicon } from '~/composables/useLexicon'

/**
 * `v-highlight` — post-render prose highlighting for paths/URLs, doc ids and
 * architecture lexicon terms. Registered client-side only: it walks real DOM.
 */
export default defineNuxtPlugin((nuxtApp) => {
  const { lexicon } = useLexicon()
  nuxtApp.vueApp.directive('highlight', createHighlightDirective(lexicon))
})