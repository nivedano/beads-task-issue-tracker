<script setup lang="ts">
import type { GraceStage } from '~/types/grace'
import { graceStageLadder } from '~/utils/grace-helpers'
import { Button } from '~/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuCheckboxItem,
  DropdownMenuTrigger,
} from '~/components/ui/dropdown-menu'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '~/components/ui/tooltip'
import GraceStageBadge from '~/components/grace/GraceStageBadge.vue'

const props = defineProps<{
  selectedStages: GraceStage[]
  open?: boolean
}>()

defineEmits<{
  toggle: [stage: GraceStage]
  close: []
}>()

const isSelected = (stage: GraceStage) => props.selectedStages.includes(stage)
</script>

<template>
  <Tooltip>
    <DropdownMenu :open="open" :modal="false">
      <TooltipTrigger as-child>
        <DropdownMenuTrigger as-child>
          <Button variant="outline" size="sm" class="h-8 text-xs gap-1">
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
            Stage
            <span
              v-if="selectedStages.length > 0"
              class="ml-0.5 rounded-full bg-primary text-primary-foreground text-[10px] px-1.5 min-w-[18px] text-center"
            >
              {{ selectedStages.length }}
            </span>
          </Button>
        </DropdownMenuTrigger>
      </TooltipTrigger>
      <TooltipContent>Filter by stage</TooltipContent>
      <DropdownMenuContent align="start" class="w-44" @escape-key-down="$emit('close')">
        <DropdownMenuCheckboxItem
          v-for="stage in graceStageLadder"
          :key="stage"
          :model-value="isSelected(stage)"
          class="text-xs cursor-pointer"
          @select.prevent="$emit('toggle', stage)"
        >
          <GraceStageBadge :stage="stage" size="sm" />
        </DropdownMenuCheckboxItem>
      </DropdownMenuContent>
    </DropdownMenu>
  </Tooltip>
</template>
