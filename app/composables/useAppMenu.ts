// Global state for dialog/panel visibility
const showUpdateDialog = ref(false)
const showAboutDialog = ref(false)
const showSettingsDialog = ref(false)
const showDebugPanel = ref(false)
let menuInitialized = false

// macOS keeps the native application menu. Windows/Linux render the same
// commands in the in-app title bar (AppMenuBar) instead of a native menu row.
const { isMacOS } = usePlatform()
const usesInAppMenuBar = !isMacOS

export function useAppMenu() {
  const openAbout = () => { showAboutDialog.value = true }
  const openSettings = () => { showSettingsDialog.value = true }
  const openUpdate = () => { showUpdateDialog.value = true }
  const toggleLogs = () => { showDebugPanel.value = !showDebugPanel.value }

  const initializeNativeMenu = async () => {
    try {
      const { Menu, Submenu, MenuItem, PredefinedMenuItem } = await import('@tauri-apps/api/menu')

      // App menu items
      const aboutItem = await MenuItem.new({
        text: 'About Beads Task-Issue Tracker',
        action: openAbout,
      })
      const separator1 = await PredefinedMenuItem.new({ item: 'Separator' })

      const settingsItem = await MenuItem.new({
        text: 'Settings...',
        accelerator: 'CmdOrCtrl+,',
        action: openSettings,
      })

      const checkUpdateItem = await MenuItem.new({
        text: 'Check for Update...',
        action: openUpdate,
      })

      const showLogsItem = await MenuItem.new({
        text: 'Show Logs...',
        accelerator: 'CmdOrCtrl+Shift+L',
        action: toggleLogs,
      })

      const separator2 = await PredefinedMenuItem.new({ item: 'Separator' })
      const servicesItem = await PredefinedMenuItem.new({ item: 'Services' })
      const separator3 = await PredefinedMenuItem.new({ item: 'Separator' })
      const hideItem = await PredefinedMenuItem.new({ item: 'Hide' })
      const hideOthersItem = await PredefinedMenuItem.new({ item: 'HideOthers' })
      const showAllItem = await PredefinedMenuItem.new({ item: 'ShowAll' })
      const separator4 = await PredefinedMenuItem.new({ item: 'Separator' })
      const quitItem = await PredefinedMenuItem.new({ item: 'Quit' })

      const appMenu = await Submenu.new({
        text: 'Beads Task-Issue Tracker',
        items: [
          aboutItem,
          separator1,
          settingsItem,
          checkUpdateItem,
          showLogsItem,
          separator2,
          servicesItem,
          separator3,
          hideItem,
          hideOthersItem,
          showAllItem,
          separator4,
          quitItem,
        ],
      })

      // Edit menu
      const undoItem = await PredefinedMenuItem.new({ item: 'Undo' })
      const redoItem = await PredefinedMenuItem.new({ item: 'Redo' })
      const editSeparator1 = await PredefinedMenuItem.new({ item: 'Separator' })
      const cutItem = await PredefinedMenuItem.new({ item: 'Cut' })
      const copyItem = await PredefinedMenuItem.new({ item: 'Copy' })
      const pasteItem = await PredefinedMenuItem.new({ item: 'Paste' })
      const selectAllItem = await PredefinedMenuItem.new({ item: 'SelectAll' })

      const editMenu = await Submenu.new({
        text: 'Edit',
        items: [
          undoItem,
          redoItem,
          editSeparator1,
          cutItem,
          copyItem,
          pasteItem,
          selectAllItem,
        ],
      })

      // Window menu - use Maximize (Tauri displays as "Zoom" on macOS)
      const minimizeItem = await PredefinedMenuItem.new({ item: 'Minimize' })
      const maximizeItem = await PredefinedMenuItem.new({ item: 'Maximize' })
      const windowSeparator = await PredefinedMenuItem.new({ item: 'Separator' })
      const closeItem = await PredefinedMenuItem.new({ item: 'CloseWindow' })

      const windowMenu = await Submenu.new({
        text: 'Window',
        items: [
          minimizeItem,
          maximizeItem,
          windowSeparator,
          closeItem,
        ],
      })

      // Create and set the menu
      const menu = await Menu.new({
        items: [appMenu, editMenu, windowMenu],
      })

      await menu.setAsAppMenu()
    } catch (error) {
      console.error('Failed to initialize app menu:', error)
    }
  }

  // Without a native menu there are no menu accelerators, so the two shortcuts
  // the File menu advertises have to be handled by the webview.
  const registerAccelerators = () => {
    window.addEventListener('keydown', (event) => {
      if (!(event.ctrlKey || event.metaKey) || event.altKey) return

      if (!event.shiftKey && event.key === ',') {
        event.preventDefault()
        openSettings()
        return
      }

      if (event.shiftKey && event.key.toLowerCase() === 'l') {
        event.preventDefault()
        toggleLogs()
      }
    })
  }

  const initializeMenu = async () => {
    // Only initialize once and only in Tauri environment
    if (menuInitialized) return
    if (typeof window === 'undefined' || (!window.__TAURI__ && !window.__TAURI_INTERNALS__)) return

    menuInitialized = true

    if (usesInAppMenuBar) {
      registerAccelerators()
      return
    }

    await initializeNativeMenu()
  }

  return {
    showUpdateDialog,
    showAboutDialog,
    showSettingsDialog,
    showDebugPanel,
    usesInAppMenuBar,
    openAbout,
    openSettings,
    openUpdate,
    toggleLogs,
    initializeMenu,
  }
}
