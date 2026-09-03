// Reusable resize logic for centered modal dialogs.
// Not a singleton: each dialog gets its own instance, keyed by `storageKey`.

export type DialogResizeDirection = 'e' | 's' | 'se'

export interface DialogResizeOptions {
  /** localStorage namespace, e.g. `beads:folderPicker` */
  storageKey: string
  defaultWidth: number
  defaultHeight: number
  minWidth?: number
  minHeight?: number
}

const VIEWPORT_MARGIN = 32

// `useLocalStorage` creates its persistence watcher in whatever effect scope is
// active when it is first called. Every other caller invokes it at module top
// level; a key-parameterised composable cannot, so run it in a detached scope
// instead — otherwise the watcher dies with the first dialog that unmounts and
// sizes silently stop being saved.
const persistentScope = effectScope(true)

const CURSORS: Record<DialogResizeDirection, string> = {
  e: 'ew-resize',
  s: 'ns-resize',
  se: 'nwse-resize',
}

export function useDialogResize(options: DialogResizeOptions) {
  const {
    storageKey,
    defaultWidth,
    defaultHeight,
    minWidth = 640,
    minHeight = 420,
  } = options

  const [width, height] = persistentScope.run(() => [
    useLocalStorage(`${storageKey}:width`, defaultWidth),
    useLocalStorage(`${storageKey}:height`, defaultHeight),
  ] as const)!

  const isResizing = ref(false)

  let direction: DialogResizeDirection = 'se'
  let startX = 0
  let startY = 0
  let startWidth = 0
  let startHeight = 0

  const maxWidth = () => (import.meta.client ? window.innerWidth - VIEWPORT_MARGIN : Infinity)
  const maxHeight = () => (import.meta.client ? window.innerHeight - VIEWPORT_MARGIN : Infinity)

  const onMove = (e: MouseEvent) => {
    if (!isResizing.value) return
    // The dialog is centered with translate(-50%, -50%), so each edge moves at
    // half the rate the box grows. Double the delta to keep the handle under
    // the cursor.
    if (direction !== 's') {
      width.value = Math.min(Math.max(startWidth + (e.clientX - startX) * 2, minWidth), maxWidth())
    }
    if (direction !== 'e') {
      height.value = Math.min(Math.max(startHeight + (e.clientY - startY) * 2, minHeight), maxHeight())
    }
  }

  const stopResize = () => {
    if (!isResizing.value) return
    isResizing.value = false
    document.removeEventListener('mousemove', onMove)
    document.removeEventListener('mouseup', stopResize)
    document.body.style.cursor = ''
    document.body.style.userSelect = ''
    document.body.style.webkitUserSelect = ''
  }

  const startResize = (e: MouseEvent, dir: DialogResizeDirection) => {
    e.preventDefault()
    e.stopPropagation()
    window.getSelection()?.removeAllRanges()
    direction = dir
    startX = e.clientX
    startY = e.clientY
    startWidth = width.value
    startHeight = height.value
    isResizing.value = true
    document.addEventListener('mousemove', onMove)
    document.addEventListener('mouseup', stopResize)
    document.body.style.cursor = CURSORS[dir]
    document.body.style.userSelect = 'none'
    document.body.style.webkitUserSelect = 'none'
  }

  // A dialog unmounted mid-drag would otherwise leave the listeners and the
  // body cursor override in place.
  onScopeDispose(stopResize)

  const style = computed(() => ({
    width: `${width.value}px`,
    height: `${height.value}px`,
    maxWidth: `calc(100vw - ${VIEWPORT_MARGIN}px)`,
    maxHeight: `calc(100vh - ${VIEWPORT_MARGIN}px)`,
  }))

  const resetSize = () => {
    width.value = defaultWidth
    height.value = defaultHeight
  }

  return { width, height, style, isResizing: readonly(isResizing), startResize, resetSize }
}
