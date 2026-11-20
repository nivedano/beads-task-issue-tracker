<script setup lang="ts">
import type { GraceAuthorization } from '~/types/grace'
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
import GraceAuthorizationBadge from '~/components/grace/GraceAuthorizationBadge.vue'

const props = defineProps<{
  selectedAuthorizations: GraceAuthorization[]
  open?: boolean
}>()

defineEmits<{
  toggle: [authorization: GraceAuthorization]
  'update:open': [value: boolean]
}>()

const authorizationOptions: GraceAuthorization[] = [
  'approved',
  'unapproved',
  'altered after approval',
  'rejected',
]

const isSelected = (authorization: GraceAuthorization) => props.selectedAuthorizations.includes(authorization)
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
              <path d="M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6l7-3z" />
              <polyline points="9 12 11 14 15 10" />
            </svg>
            Authorization
            <span
              v-if="selectedAuthorizations.length > 0"
              class="ml-0.5 rounded-full bg-primary text-primary-foreground text-[10px] px-1.5 min-w-[18px] text-center"
            >
              {{ selectedAuthorizations.length }}
            </span>
          </Button>
        </DropdownMenuTrigger>
      </TooltipTrigger>
      <TooltipContent>Filter by authorization</TooltipContent>
      <DropdownMenuContent align="start" class="w-56">
        <DropdownMenuCheckboxItem
          v-for="authorization in authorizationOptions"
          :key="authorization"
          :model-value="isSelected(authorization)"
          class="text-xs cursor-pointer"
          @select.prevent="$emit('toggle', authorization)"
        >
          <GraceAuthorizationBadge :authorization="authorization" size="sm" />
        </DropdownMenuCheckboxItem>
      </DropdownMenuContent>
    </DropdownMenu>
  </Tooltip>
</template>
