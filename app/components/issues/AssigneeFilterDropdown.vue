<script setup lang="ts">
import { DropdownMenuCheckboxItem } from '~/components/ui/dropdown-menu'
import { FilterDropdown } from '~/components/ui/filter-dropdown'
import { ScrollArea } from '~/components/ui/scroll-area'

const props = defineProps<{
  availableAssignees: string[]
  selectedAssignees: string[]
  open?: boolean
}>()

defineEmits<{
  toggle: [assignee: string]
  open: []
  close: []
}>()

const isSelected = (assignee: string) => props.selectedAssignees.includes(assignee)
</script>

<template>
  <FilterDropdown
    :open="open"
    mode="checkbox"
    label="Assignee"
    tooltip="Filter by assignee"
    :count="selectedAssignees.length"
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
        <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
        <circle cx="12" cy="7" r="4" />
      </svg>
    </template>
    <div v-if="availableAssignees.length === 0" class="px-2 py-3 text-xs text-muted-foreground text-center">
      No assignees found
    </div>
    <ScrollArea v-else class="max-h-64">
      <DropdownMenuCheckboxItem
        v-for="assignee in availableAssignees"
        :key="assignee"
        :model-value="isSelected(assignee)"
        class="text-xs cursor-pointer"
        @select.prevent="$emit('toggle', assignee)"
      >
        <span class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded bg-slate-600 text-white text-xs font-medium">
          <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
            <circle cx="12" cy="7" r="4" />
          </svg>
          {{ assignee }}
        </span>
      </DropdownMenuCheckboxItem>
    </ScrollArea>
  </FilterDropdown>
</template>
