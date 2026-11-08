import { isTauri, logFrontend } from '~/utils/bd-api'

let webviewModule: typeof import('@tauri-apps/api/webview') | null = null
let webviewModuleLoad: Promise<void> | null = null

// Lazily load the Tauri webview module — not available in browser dev mode.
function loadWebviewModule(): Promise<void> {
  if (!webviewModuleLoad) {
    webviewModuleLoad = import('@tauri-apps/api/webview')
      .then((mod) => { webviewModule = mod })
      .catch(() => { /* Not in Tauri environment */ })
  }
  return webviewModuleLoad
}

function applyCssZoom(percent: number) {
  const zoomableContent = document.getElementById('zoomable-content')
  if (zoomableContent) {
    zoomableContent.style.zoom = `${percent}%`
  }
}

export function useZoom() {
  const zoomLevel = useLocalStorage('beads:zoomLevel', 100)
  const minZoom = 75
  const maxZoom = 150
  const step = 5

  const zoomIn = () => {
    if (zoomLevel.value < maxZoom) {
      zoomLevel.value = Math.min(zoomLevel.value + step, maxZoom)
      applyZoom()
    }
  }

  const zoomOut = () => {
    if (zoomLevel.value > minZoom) {
      zoomLevel.value = Math.max(zoomLevel.value - step, minZoom)
      applyZoom()
    }
  }

  const resetZoom = () => {
    zoomLevel.value = 100
    applyZoom()
  }

  /**
   * Prefer the webview's native zoom: unlike CSS `zoom` it scales text as well
   * as layout (real browser-zoom semantics), so the UI actually becomes more
   * readable instead of just larger.
   */
  const applyZoom = async () => {
    if (!import.meta.client) return

    // Clear zoom left on the document by older releases.
    document.documentElement.style.zoom = ''

    if (isTauri()) {
      await loadWebviewModule()
      if (webviewModule) {
        try {
          await webviewModule.getCurrentWebview().setZoom(zoomLevel.value / 100)
          // Drop any CSS zoom so it cannot compound with the native one.
          const zoomableContent = document.getElementById('zoomable-content')
          if (zoomableContent) zoomableContent.style.zoom = ''
          return
        } catch (error) {
          logFrontend('warn', `[zoom] native webview zoom failed, falling back to CSS: ${error}`)
        }
      }
    }

    // Browser dev mode (or unsupported platform): CSS zoom is the best we have.
    applyCssZoom(zoomLevel.value)
  }

  const canZoomIn = computed(() => zoomLevel.value < maxZoom)
  const canZoomOut = computed(() => zoomLevel.value > minZoom)

  // Initialize on mount
  onMounted(() => {
    void applyZoom()
  })

  return {
    zoomLevel,
    zoomIn,
    zoomOut,
    resetZoom,
    canZoomIn,
    canZoomOut,
  }
}
