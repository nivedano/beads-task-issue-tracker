<script setup lang="ts">
import type { IssuePriority } from '~/types/issue'
import { DropdownMenuCheckboxItem } from '~/components/ui/dropdown-menu'
import { FilterDropdown } from '~/components/ui/filter-dropdown'
import PriorityBadge from '~/components/issues/PriorityBadge.vue'

const props = defineProps<{
  selectedPriorities: IssuePriority[]
  open?: boolean
}>()

defineEmits<{
  toggle: [priority: IssuePriority]
  open: []
  close: []
}>()

const priorityOptions: { value: IssuePriority; label: string }[] = [
  { value: 'p0', label: 'P0 - Critical' },
  { value: 'p1', label: 'P1 - High' },
  { value: 'p2', label: 'P2 - Medium' },
  { value: 'p3', label: 'P3 - Low' },
  { value: 'p4', label: 'P4 - Minimal' },
]

const isSelected = (priority: IssuePriority) => props.selectedPriorities.includes(priority)
</script>

<template>
  <FilterDropdown
    :open="open"
    mode="checkbox"
    label="Priority"
    tooltip="Filter by priority"
    :count="selectedPriorities.length"
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
        <path d="M22 12h-4l-3 9L9 3l-3 9H2" />
      </svg>
    </template>
    <DropdownMenuCheckboxItem
      v-for="opt in priorityOptions"
      :key="opt.value"
      :model-value="isSelected(opt.value)"
      class="text-xs cursor-pointer"
      @select.prevent="$emit('toggle', opt.value)"
    >
      <PriorityBadge :priority="opt.value" size="sm" />
    </DropdownMenuCheckboxItem>
  </FilterDropdown>
</template>
