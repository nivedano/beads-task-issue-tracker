<script setup lang="ts">
import { Button } from '~/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from '~/components/ui/dropdown-menu'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '~/components/ui/tooltip'
import type { GraceScope } from '~/types/grace'

defineProps<{
  scope: GraceScope
  open?: boolean
}>()

defineEmits<{
  select: [scope: GraceScope]
  'update:open': [value: boolean]
}>()

const scopeOptions: { value: GraceScope, label: string }[] = [
  { value: 'open', label: 'Open' },
  { value: 'closed', label: 'Closed' },
  { value: 'all', label: 'All' },
]
</script>

<template>
  <Tooltip>
    <DropdownMenu :open="open" :modal="false" @update:open="$emit('update:open', $event)">
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
              <rect x="3" y="4" width="18" height="16" rx="2" />
              <line x1="3" y1="9" x2="21" y2="9" />
            </svg>
            {{ scope === 'all' ? 'Scope' : scope === 'open' ? 'Open' : 'Closed' }}
          </Button>
        </DropdownMenuTrigger>
      </TooltipTrigger>
      <TooltipContent>Filter by scope</TooltipContent>
      <DropdownMenuContent align="start" class="w-36">
        <DropdownMenuRadioGroup :model-value="scope" @update:model-value="$emit('select', $event as GraceScope)">
          <DropdownMenuRadioItem
            v-for="opt in scopeOptions"
            :key="opt.value"
            :value="opt.value"
            class="text-xs cursor-pointer"
          >
            {{ opt.label }}
          </DropdownMenuRadioItem>
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  </Tooltip>
</template>
