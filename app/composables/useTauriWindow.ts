let windowModule: typeof import('@tauri-apps/api/window') | null = null
let setTitlePermissionDeniedLogged = false

// Pre-load the Tauri window module
if (import.meta.client) {
  import('@tauri-apps/api/window').then(mod => {
    windowModule = mod
  }).catch(() => {
    // Not in Tauri environment
  })
}

export function useTauriWindow() {
  const startDragging = () => {
    if (windowModule) {
      windowModule.getCurrentWindow().startDragging().catch(() => {
        // Ignore drag failures in unsupported environments.
      })
    }
  }

  const setWindowTitle = (title: string) => {
    if (windowModule) {
      windowModule.getCurrentWindow().setTitle(title).catch((error) => {
        // Some capability profiles may deny changing title; do not break app render.
        if (!setTitlePermissionDeniedLogged) {
          setTitlePermissionDeniedLogged = true
          console.warn('Unable to set window title:', error)
        }
      })
    }
  }

  // Window controls for the in-app title bar (undecorated windows).
  const minimizeWindow = () => {
    windowModule?.getCurrentWindow().minimize().catch(() => {})
  }

  const toggleMaximizeWindow = () => {
    windowModule?.getCurrentWindow().toggleMaximize().catch(() => {})
  }

  const closeWindow = () => {
    windowModule?.getCurrentWindow().close().catch(() => {})
  }

  const isWindowMaximized = async (): Promise<boolean> => {
    if (!windowModule) return false
    try {
      return await windowModule.getCurrentWindow().isMaximized()
    } catch {
      return false
    }
  }

  /** Subscribe to resize events; resolves to an unlisten function. */
  const onWindowResized = async (handler: () => void): Promise<() => void> => {
    if (!windowModule) return () => {}
    try {
      return await windowModule.getCurrentWindow().onResized(handler)
    } catch {
      return () => {}
    }
  }

  return {
    startDragging,
    setWindowTitle,
    minimizeWindow,
    toggleMaximizeWindow,
    closeWindow,
    isWindowMaximized,
    onWindowResized,
  }
}
