<script setup lang="ts">
import type { GraceStage, GraceAuthorization, GraceScope } from '~/types/grace'
import type { ColumnConfig } from '~/types/issue'
import { Input } from '~/components/ui/input'
import { useExclusiveFilterGroup } from '~/composables/useExclusiveFilterGroup'
import { ColumnConfigPopover } from '~/components/ui/column-config-popover'
import StageFilterDropdown from '~/components/grace/StageFilterDropdown.vue'
import AuthorizationFilterDropdown from '~/components/grace/AuthorizationFilterDropdown.vue'
import ScopeFilterDropdown from '~/components/grace/ScopeFilterDropdown.vue'

const search = defineModel<string>('search', { default: '' })

defineProps<{
  selectedStages: GraceStage[]
  selectedAuthorizations: GraceAuthorization[]
  scope: GraceScope
  columns: ColumnConfig[]
}>()

defineEmits<{
  toggleStage: [stage: GraceStage]
  toggleAuthorization: [authorization: GraceAuthorization]
  selectScope: [scope: GraceScope]
  'update:columns': [columns: ColumnConfig[]]
  resetColumns: []
}>()

// Search is the primary entry point: it takes focus on mount and Ctrl+F brings
// it back from anywhere in the app.
const searchFieldRef = ref<HTMLElement | null>(null)

const focusSearch = (): boolean => {
  const input = searchFieldRef.value?.querySelector('input')
  if (!input) return false
  input.focus()
  input.select()
  return document.activeElement === input
}

defineExpose({ focusSearch })

onMounted(() => {
  // Only claim focus when nothing else has it (a startup dialog, for instance).
  const active = document.activeElement
  if (!active || active === document.body) focusSearch()
})

// Track which filter dropdown is currently open (exclusive group). Opening one
// also closes the column popover, including via the keyboard.
type FilterType = 'stage' | 'authorization' | 'scope'

const columnPopoverRef = ref<InstanceType<typeof ColumnConfigPopover> | null>(null)

const {
  activeFilter,
  filterButtonsRef,
  handleFilterClick,
  onFilterOpen,
  onFilterClose,
} = useExclusiveFilterGroup<FilterType>({
  onAnyOpen: () => columnPopoverRef.value?.close(),
})
</script>

<template>
  <div class="space-y-2">
    <div class="flex items-center gap-2">
      <div ref="searchFieldRef" class="relative flex-1">
        <svg
          class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
        >
          <circle cx="11" cy="11" r="8" />
          <line x1="21" y1="21" x2="16.65" y2="16.65" />
        </svg>
        <Input
          v-model="search"
          type="search"
          placeholder="Search GRACE changes..."
          class="pl-10 h-8 text-xs"
        />
      </div>

      <!-- Filter dropdowns (order matches table columns) -->
      <div ref="filterButtonsRef" class="flex items-center gap-2">
        <div @pointerdown.capture="handleFilterClick('stage')">
          <StageFilterDropdown
            :selected-stages="selectedStages"
            :open="activeFilter === 'stage'"
            @open="onFilterOpen('stage')"
            @close="onFilterClose"
            @toggle="$emit('toggleStage', $event)"
          />
        </div>

        <div @pointerdown.capture="handleFilterClick('authorization')">
          <AuthorizationFilterDropdown
            :selected-authorizations="selectedAuthorizations"
            :open="activeFilter === 'authorization'"
            @open="onFilterOpen('authorization')"
            @close="onFilterClose"
            @toggle="$emit('toggleAuthorization', $event)"
          />
        </div>

        <div @pointerdown.capture="handleFilterClick('scope')">
          <ScopeFilterDropdown
            :scope="scope"
            :open="activeFilter === 'scope'"
            @open="onFilterOpen('scope')"
            @close="onFilterClose"
            @select="$emit('selectScope', $event)"
          />
        </div>
      </div>

      <!-- The GRACE identity column (id) is not configurable. -->
      <ColumnConfigPopover
        ref="columnPopoverRef"
        :columns="columns"
        :exclude-ids="['id']"
        :reorder="false"
        @update:columns="$emit('update:columns', $event)"
        @reset="$emit('resetColumns')"
      />
    </div>
  </div>
</template>
