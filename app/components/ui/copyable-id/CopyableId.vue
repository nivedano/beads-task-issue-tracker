<script setup lang="ts">
import { cn } from '~/lib/utils'

const props = defineProps<{
  value: string
  displayValue?: string
  /** Hide the icon until the surrounding `.group` is hovered or focus is visible. */
  hoverIcon?: boolean
  /** Extra classes merged over the base styling (e.g. text size/color overrides). */
  class?: string
}>()

const copied = ref(false)

const hoverIconClass = computed(() => props.hoverIcon
  ? 'opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 focus-visible:opacity-100 transition-opacity'
  : '')

// The check icon only renders while copied, so it stays visible after the
// pointer leaves instead of fading out with the copy icon.
const checkIconClass = computed(() => props.hoverIcon ? 'opacity-100' : '')

const copyToClipboard = async (event: Event) => {
  event.stopPropagation()
  try {
    await navigator.clipboard.writeText(props.value)
    copied.value = true
    setTimeout(() => {
      copied.value = false
    }, 2000)
  } catch (err) {
    console.error('Failed to copy:', err)
  }
}
</script>

<template>
  <button
    :class="cn('flex items-center gap-1 text-[10px] text-muted-foreground font-mono hover:text-foreground transition-colors', props.class)"
    :title="`Copy ${props.value}`"
    @click="copyToClipboard"
  >
    {{ displayValue ?? value }}
    <svg
      v-if="!copied"
      :class="['w-3 h-3', hoverIconClass]"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
    >
      <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
      <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
    </svg>
    <svg
      v-else
      :class="['w-3 h-3 text-green-500', checkIconClass]"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
    >
      <polyline points="20 6 9 17 4 12" />
    </svg>
  </button>
</template>