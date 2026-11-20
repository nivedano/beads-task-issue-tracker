<script setup lang="ts">
import type { GraceStage } from '~/types/grace'
import { graceStageLadder } from '~/utils/grace-helpers'
import { DropdownMenuCheckboxItem } from '~/components/ui/dropdown-menu'
import { FilterDropdown } from '~/components/ui/filter-dropdown'
import GraceStageBadge from '~/components/grace/GraceStageBadge.vue'

const props = defineProps<{
  selectedStages: GraceStage[]
  open?: boolean
}>()

defineEmits<{
  toggle: [stage: GraceStage]
  open: []
  close: []
}>()

const isSelected = (stage: GraceStage) => props.selectedStages.includes(stage)
</script>

<template>
  <FilterDropdown
    :open="open"
    mode="checkbox"
    label="Stage"
    tooltip="Filter by stage"
    :count="selectedStages.length"
    content-class="w-44"
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
        <polyline points="3 17 9 11 13 15 21 7" />
        <polyline points="15 7 21 7 21 13" />
      </svg>
    </template>
    <DropdownMenuCheckboxItem
      v-for="stage in graceStageLadder"
      :key="stage"
      :model-value="isSelected(stage)"
      class="text-xs cursor-pointer"
      @select.prevent="$emit('toggle', stage)"
    >
      <GraceStageBadge :stage="stage" size="sm" />
    </DropdownMenuCheckboxItem>
  </FilterDropdown>
</template>
