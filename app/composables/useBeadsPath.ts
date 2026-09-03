import { fsExists } from '~/utils/bd-api'
import { clearProjectStorageCache, migrateProjectStorageNamespace } from '~/composables/useProjectStorage'
import { normalizePath } from '~/utils/favorites-helpers'

const DEFAULT_PATH = '.'

// Shared state across all components
const beadsPath = ref<string>(DEFAULT_PATH)
const isInitialized = ref(false)
const hasStoredPath = ref(false)
const isValidating = ref(false)

export function useBeadsPath() {
  // Initialize from localStorage only once (client-side)
  if (import.meta.client && !isInitialized.value) {
    const stored = localStorage.getItem('beads:path')
    if (stored) {
      try {
        // Normalize so a path stored before the Windows verbatim-path fix
        // (`\\?\C:\dev\app`) still matches the migrated project list, which the
        // sidebar compares against by string equality.
        const rawPath = JSON.parse(stored)
        const parsedPath = normalizePath(rawPath)
        if (parsedPath !== rawPath) {
          // Write the migration through: useProjectStorage reads `beads:path`
          // straight from localStorage to namespace per-project settings, so a
          // stale raw value there would key them off the old hash.
          migrateProjectStorageNamespace(rawPath, parsedPath)
          localStorage.setItem('beads:path', JSON.stringify(parsedPath))
        }
        // Temporarily set the path, will validate async
        beadsPath.value = parsedPath
        hasStoredPath.value = true

        // Validate path exists asynchronously
        if (parsedPath !== DEFAULT_PATH) {
          isValidating.value = true
          fsExists(parsedPath).then((exists) => {
            if (!exists) {
              // Path doesn't exist, reset to default
              beadsPath.value = DEFAULT_PATH
              hasStoredPath.value = false
              localStorage.setItem('beads:path', JSON.stringify(DEFAULT_PATH))
              console.warn(`[useBeadsPath] Stored path "${parsedPath}" does not exist, resetting to default`)
            }
            isValidating.value = false
          }).catch(() => {
            isValidating.value = false
          })
        }
      } catch {
        beadsPath.value = DEFAULT_PATH
        hasStoredPath.value = false
      }
    } else {
      hasStoredPath.value = false
    }
    isInitialized.value = true
  }

  // Watch for changes and persist to localStorage
  watch(
    beadsPath,
    (newValue) => {
      if (import.meta.client) {
        localStorage.setItem('beads:path', JSON.stringify(newValue))
      }
    },
    { immediate: false }
  )

  const setPath = (path: string) => {
    const newPath = path ? normalizePath(path) : DEFAULT_PATH
    // Update path in localStorage FIRST (before reloading project storage)
    beadsPath.value = newPath
    hasStoredPath.value = true
    if (import.meta.client) {
      localStorage.setItem('beads:path', JSON.stringify(newPath))
    }
    // Now reload project storage with the new path
    clearProjectStorageCache()
  }

  const resetPath = () => {
    beadsPath.value = DEFAULT_PATH
    hasStoredPath.value = true
  }

  // Clear path completely - used when removing last favorite to show onboarding
  const clearPath = () => {
    beadsPath.value = DEFAULT_PATH
    hasStoredPath.value = false
    if (import.meta.client) {
      localStorage.removeItem('beads:path')
    }
  }

  const isCustomPath = computed(() => beadsPath.value !== DEFAULT_PATH)

  return {
    beadsPath: readonly(beadsPath),
    setPath,
    resetPath,
    clearPath,
    isCustomPath,
    hasStoredPath: readonly(hasStoredPath),
  }
}
