<script setup lang="ts">
import { DropdownMenuCheckboxItem } from '~/components/ui/dropdown-menu'
import { FilterDropdown } from '~/components/ui/filter-dropdown'
import { ScrollArea } from '~/components/ui/scroll-area'
import LabelBadge from '~/components/issues/LabelBadge.vue'

const props = defineProps<{
  availableLabels: string[]
  selectedLabels: string[]
  open?: boolean
}>()

defineEmits<{
  toggle: [label: string]
  open: []
  close: []
}>()

const isSelected = (label: string) => props.selectedLabels.includes(label)
</script>

<template>
  <FilterDropdown
    :open="open"
    mode="checkbox"
    label="Labels"
    tooltip="Filter by label"
    :count="selectedLabels.length"
    content-class="w-48"
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
        <path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z" />
        <line x1="7" y1="7" x2="7.01" y2="7" />
      </svg>
    </template>
    <div v-if="availableLabels.length === 0" class="px-2 py-3 text-xs text-muted-foreground text-center">
      No labels found
    </div>
    <ScrollArea v-else class="max-h-64">
      <DropdownMenuCheckboxItem
        v-for="label in availableLabels"
        :key="label"
        :model-value="isSelected(label)"
        class="text-xs cursor-pointer"
        @select.prevent="$emit('toggle', label)"
      >
        <LabelBadge :label="label" size="sm" />
      </DropdownMenuCheckboxItem>
    </ScrollArea>
  </FilterDropdown>
</template>
