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
   * Type zoom rather than UI zoom.
   *
   * `--app-zoom` scales only the type scale of `#zoomable-content` (see the
   * "UI zoom" block in `assets/css/tailwind.css`), while spacing stays pinned
   * to px. Text therefore gets bigger and easier to read without the empty
   * padding around it growing at the same rate. The header sits outside the
   * zoom scope, so it never scales.
   */
  const applyZoom = () => {
    if (!import.meta.client) return

    // Clean up the CSS zoom that older releases applied to the whole content box.
    document.documentElement.style.zoom = ''
    const zoomableContent = document.getElementById('zoomable-content')
    if (zoomableContent) zoomableContent.style.zoom = ''

    document.documentElement.style.setProperty('--app-zoom', String(zoomLevel.value / 100))
  }

  const canZoomIn = computed(() => zoomLevel.value < maxZoom)
  const canZoomOut = computed(() => zoomLevel.value > minZoom)

  // Initialize on mount
  onMounted(() => {
    applyZoom()
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
