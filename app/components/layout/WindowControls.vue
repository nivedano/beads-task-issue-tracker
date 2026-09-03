<script setup lang="ts">
/**
 * Minimize / maximize / close buttons for the in-app title bar. Only rendered
 * where the window runs undecorated (Windows).
 */
const { minimizeWindow, toggleMaximizeWindow, closeWindow, isWindowMaximized, onWindowResized } = useTauriWindow()

const isMaximized = ref(false)
let unlisten: (() => void) | null = null

const refreshState = async () => {
  isMaximized.value = await isWindowMaximized()
}

onMounted(async () => {
  await refreshState()
  unlisten = await onWindowResized(() => { refreshState() })
})

onBeforeUnmount(() => {
  unlisten?.()
  unlisten = null
})

const handleToggleMaximize = () => {
  toggleMaximizeWindow()
  // The resize event lands after the transition; refresh defensively too.
  window.setTimeout(refreshState, 120)
}

const buttonClass = 'flex h-full min-h-8 w-11 items-center justify-center text-foreground/70 transition-colors hover:bg-accent hover:text-accent-foreground'
</script>

<template>
  <div class="app-no-drag flex items-stretch">
    <button
      type="button"
      :class="buttonClass"
      aria-label="Minimize"
      @click="minimizeWindow"
    >
      <svg class="w-3 h-3" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.2">
        <line x1="2" y1="6" x2="10" y2="6" />
      </svg>
    </button>

    <button
      type="button"
      :class="buttonClass"
      :aria-label="isMaximized ? 'Restore' : 'Maximize'"
      @click="handleToggleMaximize"
    >
      <!-- Restore: two offset squares, mirroring the native Windows glyph -->
      <svg v-if="isMaximized" class="w-3 h-3" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.2">
        <rect x="2" y="4" width="6" height="6" />
        <path d="M4 4V2h6v6H8" />
      </svg>
      <svg v-else class="w-3 h-3" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.2">
        <rect x="2.5" y="2.5" width="7" height="7" />
      </svg>
    </button>

    <button
      type="button"
      class="flex h-full min-h-8 w-11 items-center justify-center text-foreground/70 transition-colors hover:bg-red-600 hover:text-white"
      aria-label="Close"
      @click="closeWindow"
    >
      <svg class="w-3 h-3" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.2">
        <line x1="2.5" y1="2.5" x2="9.5" y2="9.5" />
        <line x1="9.5" y1="2.5" x2="2.5" y2="9.5" />
      </svg>
    </button>
  </div>
</template>
