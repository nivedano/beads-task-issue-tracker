<script setup lang="ts">
import type { GraceStage, GraceAuthorization, GraceScope } from '~/types/grace'
import type { ColumnConfig } from '~/types/issue'
import { onClickOutside } from '@vueuse/core'
import { Input } from '~/components/ui/input'
import { Button } from '~/components/ui/button'
import { Checkbox } from '~/components/ui/checkbox'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '~/components/ui/tooltip'
import StageFilterDropdown from '~/components/grace/StageFilterDropdown.vue'
import AuthorizationFilterDropdown from '~/components/grace/AuthorizationFilterDropdown.vue'
import ScopeFilterDropdown from '~/components/grace/ScopeFilterDropdown.vue'

const search = defineModel<string>('search', { default: '' })

const props = defineProps<{
  selectedStages: GraceStage[]
  selectedAuthorizations: GraceAuthorization[]
  scope: GraceScope
  columns: ColumnConfig[]
}>()

const emit = defineEmits<{
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

// Track which filter dropdown is currently open (exclusive group)
type FilterType = 'stage' | 'authorization' | 'scope' | null
const activeFilter = ref<FilterType>(null)

const filterButtonsRef = ref<HTMLElement | null>(null)

onClickOutside(filterButtonsRef, () => {
  activeFilter.value = null
}, {
  ignore: ['[data-slot="dropdown-menu-content"]']
})

const handleFilterClick = (filter: FilterType) => {
  activeFilter.value = activeFilter.value === filter ? null : filter
}

// Column settings popover (mirrors issues/ColumnConfig.vue without drag-reorder)
const isColumnPanelOpen = ref(false)
const columnPanelRef = ref<HTMLElement | null>(null)
const columnTriggerRef = ref<HTMLElement | null>(null)

onClickOutside(columnPanelRef, (event) => {
  if (columnTriggerRef.value && (columnTriggerRef.value === event.target || columnTriggerRef.value.contains(event.target as Node))) return
  isColumnPanelOpen.value = false
}, { ignore: [columnTriggerRef] })

const toggleColumn = (columnId: string, checked: boolean) => {
  if (columnId === 'id') return
  emit('update:columns', props.columns.map(col =>
    col.id === columnId ? { ...col, visible: checked } : col
  ))
}

// The ID column is the identity column (it also carries the malformed-defect
// signal, which must never be hideable), so it is not offered here.
const configurableColumns = computed(() => props.columns.filter(col => col.id !== 'id'))
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
        <div
          @pointerdown.capture="handleFilterClick('stage')"
          @keydown.enter.prevent="handleFilterClick('stage')"
          @keydown.space.prevent="handleFilterClick('stage')"
          @keydown.down.prevent="handleFilterClick('stage')"
        >
          <StageFilterDropdown
            :selected-stages="selectedStages"
            :open="activeFilter === 'stage'"
            @close="activeFilter = null"
            @toggle="emit('toggleStage', $event)"
          />
        </div>

        <div
          @pointerdown.capture="handleFilterClick('authorization')"
          @keydown.enter.prevent="handleFilterClick('authorization')"
          @keydown.space.prevent="handleFilterClick('authorization')"
          @keydown.down.prevent="handleFilterClick('authorization')"
        >
          <AuthorizationFilterDropdown
            :selected-authorizations="selectedAuthorizations"
            :open="activeFilter === 'authorization'"
            @close="activeFilter = null"
            @toggle="emit('toggleAuthorization', $event)"
          />
        </div>

        <div
          @pointerdown.capture="handleFilterClick('scope')"
          @keydown.enter.prevent="handleFilterClick('scope')"
          @keydown.space.prevent="handleFilterClick('scope')"
          @keydown.down.prevent="handleFilterClick('scope')"
        >
          <ScopeFilterDropdown
            :scope="scope"
            :open="activeFilter === 'scope'"
            @close="activeFilter = null"
            @select="emit('selectScope', $event)"
          />
        </div>
      </div>

      <div class="relative">
        <!-- The ref must be a plain element: a component ref has no `.contains`,
             which broke onClickOutside's outside-click comparison. -->
        <div ref="columnTriggerRef">
          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                variant="outline"
                size="icon"
                class="h-8 w-8"
                @click="isColumnPanelOpen = !isColumnPanelOpen"
              >
                <svg
                  class="w-3.5 h-3.5"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                >
                  <rect x="3" y="3" width="7" height="7" />
                  <rect x="14" y="3" width="7" height="7" />
                  <rect x="14" y="14" width="7" height="7" />
                  <rect x="3" y="14" width="7" height="7" />
                </svg>
                <span class="sr-only">Column settings</span>
              </Button>
            </TooltipTrigger>
            <TooltipContent>Column settings</TooltipContent>
          </Tooltip>
        </div>

        <div
          v-if="isColumnPanelOpen"
          ref="columnPanelRef"
          class="absolute right-0 top-full mt-1 z-50 w-52 rounded-md border bg-popover text-popover-foreground shadow-md"
        >
          <div class="px-2 py-1.5">
            <span class="text-xs font-semibold">Visible Columns</span>
          </div>
          <div class="h-px bg-border" />
          <div class="py-1">
            <div
              v-for="column in configurableColumns"
              :key="column.id"
              class="flex items-center gap-2 px-2 py-1 text-xs hover:bg-accent/50 rounded-sm mx-1"
            >
              <Checkbox
                :id="`grace-col-${column.id}`"
                :model-value="column.visible"
                class="h-3.5 w-3.5"
                @update:model-value="toggleColumn(column.id, !!$event)"
              />
              <label :for="`grace-col-${column.id}`" class="flex-1 cursor-pointer select-none">{{ column.label }}</label>
            </div>
          </div>
          <div class="h-px bg-border" />
          <div class="p-1">
            <Button variant="ghost" size="sm" class="w-full text-xs h-7 justify-start" @click="emit('resetColumns')">
              Reset to defaults
            </Button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
