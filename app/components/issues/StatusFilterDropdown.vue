<script setup lang="ts">
import type { IssueStatus } from '~/types/issue'
import { DropdownMenuCheckboxItem } from '~/components/ui/dropdown-menu'
import { FilterDropdown } from '~/components/ui/filter-dropdown'
import StatusBadge from '~/components/issues/StatusBadge.vue'

const props = defineProps<{
  selectedStatuses: IssueStatus[]
  open?: boolean
}>()

defineEmits<{
  toggle: [status: IssueStatus]
  open: []
  close: []
}>()

const statusOptions: { value: IssueStatus; label: string }[] = [
  { value: 'open', label: 'Open' },
  { value: 'in_progress', label: 'In Progress' },
  { value: 'blocked', label: 'Blocked' },
  { value: 'closed', label: 'Closed' },
  { value: 'deferred', label: 'Deferred' },
  { value: 'pinned', label: 'Pinned' },
  { value: 'hooked', label: 'Hooked' },
]

const isSelected = (status: IssueStatus) => props.selectedStatuses.includes(status)
</script>

<template>
  <FilterDropdown
    :open="open"
    mode="checkbox"
    label="Status"
    tooltip="Filter by status"
    :count="selectedStatuses.length"
    content-class="w-40"
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
        <circle cx="12" cy="12" r="10" />
        <polyline points="12 6 12 12 16 14" />
      </svg>
    </template>
    <DropdownMenuCheckboxItem
      v-for="opt in statusOptions"
      :key="opt.value"
      :model-value="isSelected(opt.value)"
      class="text-xs cursor-pointer"
      @select.prevent="$emit('toggle', opt.value)"
    >
      <StatusBadge :status="opt.value" size="sm" />
    </DropdownMenuCheckboxItem>
  </FilterDropdown>
</template>
