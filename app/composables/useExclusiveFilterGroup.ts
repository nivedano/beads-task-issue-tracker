import { onClickOutside } from '@vueuse/core'

export interface ExclusiveFilterGroupOptions {
  /** Called whenever a filter is opened — pointer or keyboard. */
  onAnyOpen?: () => void
}

/**
 * Exclusive filter group shared by both explorer toolbars: one filter menu is
 * open at a time, and an outside click closes it. Dropdown content is portaled
 * to #zoomable-content, so it is ignored by the outside-click check.
 */
export function useExclusiveFilterGroup<T extends string>(
  options: ExclusiveFilterGroupOptions = {},
) {
  const activeFilter = ref<T | null>(null)
  const filterButtonsRef = ref<HTMLElement | null>(null)

  onClickOutside(filterButtonsRef, () => {
    activeFilter.value = null
  }, {
    ignore: ['[data-slot="dropdown-menu-content"]']
  })

  const onFilterOpen = (filter: T) => {
    activeFilter.value = filter
    options.onAnyOpen?.()
  }

  const onFilterClose = () => {
    activeFilter.value = null
  }

  // The wrapper's pointerdown capture toggles: a second press on the open
  // filter closes it, any other filter replaces it.
  const handleFilterClick = (filter: T) => {
    if (activeFilter.value === filter) {
      activeFilter.value = null
      return
    }
    onFilterOpen(filter)
  }

  return {
    activeFilter,
    filterButtonsRef,
    handleFilterClick,
    onFilterOpen,
    onFilterClose,
  }
}