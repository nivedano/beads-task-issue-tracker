<script setup lang="ts">
import type { GraceScope } from '~/types/grace'
import {
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
} from '~/components/ui/dropdown-menu'
import { FilterDropdown } from '~/components/ui/filter-dropdown'

const props = defineProps<{
  scope: GraceScope
  open?: boolean
}>()

defineEmits<{
  select: [scope: GraceScope]
  open: []
  close: []
}>()

const scopeOptions: { value: GraceScope, label: string }[] = [
  { value: 'open', label: 'Open' },
  { value: 'closed', label: 'Closed' },
  { value: 'all', label: 'All' },
]

const triggerLabel = computed(() =>
  props.scope === 'all' ? 'Scope' : props.scope === 'open' ? 'Open' : 'Closed'
)
</script>

<template>
  <FilterDropdown
    :open="open"
    mode="radio"
    :label="triggerLabel"
    tooltip="Filter by scope"
    content-class="w-36"
    @open="$emit('open')"
    @close="$emit('close')"
  >
    <template #icon>
      <svg
        class="w-3.5 h-3.5"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
      >
        <rect x="3" y="4" width="18" height="16" rx="2" />
        <line x1="3" y1="9" x2="21" y2="9" />
      </svg>
    </template>
    <DropdownMenuRadioGroup
      :model-value="scope"
      @update:model-value="$emit('select', $event as GraceScope)"
    >
      <DropdownMenuRadioItem
        v-for="opt in scopeOptions"
        :key="opt.value"
        :value="opt.value"
        class="text-xs cursor-pointer"
      >
        {{ opt.label }}
      </DropdownMenuRadioItem>
    </DropdownMenuRadioGroup>
  </FilterDropdown>
</template>
