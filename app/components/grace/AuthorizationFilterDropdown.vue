<script setup lang="ts">
import type { GraceAuthorization } from '~/types/grace'
import { DropdownMenuCheckboxItem } from '~/components/ui/dropdown-menu'
import { FilterDropdown } from '~/components/ui/filter-dropdown'
import GraceAuthorizationBadge from '~/components/grace/GraceAuthorizationBadge.vue'

const props = defineProps<{
  selectedAuthorizations: GraceAuthorization[]
  open?: boolean
}>()

defineEmits<{
  toggle: [authorization: GraceAuthorization]
  open: []
  close: []
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
  <FilterDropdown
    :open="open"
    mode="checkbox"
    label="Authorization"
    tooltip="Filter by authorization"
    :count="selectedAuthorizations.length"
    content-class="w-56"
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
        <path d="M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6l7-3z" />
        <polyline points="9 12 11 14 15 10" />
      </svg>
    </template>
    <DropdownMenuCheckboxItem
      v-for="authorization in authorizationOptions"
      :key="authorization"
      :model-value="isSelected(authorization)"
      class="text-xs cursor-pointer"
      @select.prevent="$emit('toggle', authorization)"
    >
      <GraceAuthorizationBadge :authorization="authorization" size="sm" />
    </DropdownMenuCheckboxItem>
  </FilterDropdown>
</template>
