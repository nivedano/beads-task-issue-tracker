<script setup lang="ts">
import type { GraceStage, GraceAuthorization, GraceChangeSummary, GraceSortField } from '~/types/grace'
import type { ColumnConfig } from '~/types/issue'
import { useProjectStorage } from '~/composables/useProjectStorage'
import type { GraceFilters } from '~/utils/grace-helpers'
import GraceToolbar from '~/components/grace/GraceToolbar.vue'
import GraceFilterChips from '~/components/grace/GraceFilterChips.vue'
import GraceTable from '~/components/grace/GraceTable.vue'

const DEFAULT_COLUMNS: ColumnConfig[] = [
  { id: 'id', label: 'ID', visible: true, sortable: true },
  { id: 'title', label: 'Title', visible: true, sortable: true },
  { id: 'stage', label: 'Stage', visible: true, sortable: true },
  { id: 'authorization', label: 'Authorization', visible: true, sortable: false },
  { id: 'checks', label: 'Checks', visible: true, sortable: false },
  { id: 'review', label: 'Review', visible: true, sortable: false },
  { id: 'revision', label: 'Revision', visible: true, sortable: true },
]

const search = defineModel<string>('search', { required: true })

const props = defineProps<{
  filteredChanges: GraceChangeSummary[]
  selectedId?: string | null
  filters: GraceFilters
  sortField: GraceSortField
  sortDirection: 'asc' | 'desc'
  isLoading: boolean
  pinWarning: string | null
  /** Optional: a failed list fetch (the list would otherwise look empty). */
  error?: string | null
}>()

const emit = defineEmits<{
  'update:filters': [filters: GraceFilters]
  select: [change: GraceChangeSummary]
  sort: [field: GraceSortField, direction: 'asc' | 'desc']
  'clear-filters': []
}>()

const columns = useProjectStorage<ColumnConfig[]>('grace:columns', DEFAULT_COLUMNS)

// Reconcile the stored config against the canonical columns: unknown ids are
// dropped (a renamed/removed column), new defaults are appended in their
// canonical position, and the identity column stays first and visible — it
// carries the malformed-defect signal, which config must never hide.
const tableColumns = computed<ColumnConfig[]>(() => {
  const savedVisibility = new Map(columns.value.map(column => [column.id, column.visible]))
  return DEFAULT_COLUMNS.map((column, index) => ({
    ...column,
    visible: index === 0 ? true : (savedVisibility.get(column.id) ?? column.visible),
  }))
})

const toolbarRef = ref<InstanceType<typeof GraceToolbar> | null>(null)

const focusSearch = () => {
  return toolbarRef.value?.focusSearch() ?? false
}

defineExpose({ focusSearch })

// Facet toggles replace the array (never mutate the stored one in place) so the
// per-project storage watch and the computed filter both see a new value.
const toggleStage = (stage: GraceStage) => {
  const stageNext = props.filters.stage.includes(stage)
    ? props.filters.stage.filter(s => s !== stage)
    : [...props.filters.stage, stage]
  emit('update:filters', { ...props.filters, stage: stageNext })
}

const toggleAuthorization = (authorization: GraceAuthorization) => {
  const authorizationNext = props.filters.authorization.includes(authorization)
    ? props.filters.authorization.filter(a => a !== authorization)
    : [...props.filters.authorization, authorization]
  emit('update:filters', { ...props.filters, authorization: authorizationNext })
}

const selectScope = (scope: GraceFilters['scope']) => {
  emit('update:filters', { ...props.filters, scope })
}

const updateColumns = (next: ColumnConfig[]) => {
  columns.value = next
}

const resetColumns = () => {
  columns.value = DEFAULT_COLUMNS.map(col => ({ ...col }))
}

const handleSort = (field: GraceSortField, direction: 'asc' | 'desc') => {
  emit('sort', field, direction)
}
</script>

<template>
  <div class="p-4 border-b border-border space-y-3">
    <div
      v-if="pinWarning"
      class="flex items-start gap-2 rounded border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs text-amber-600 dark:text-amber-400"
    >
      <svg
        class="w-3.5 h-3.5 mt-0.5 shrink-0"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
      >
        <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
        <line x1="12" y1="9" x2="12" y2="13" />
        <line x1="12" y1="17" x2="12.01" y2="17" />
      </svg>
      <span>{{ pinWarning }}</span>
    </div>

    <!-- `error` is useGrace's shared ref, so it can carry a list OR a detail
         failure — keep the noun neutral. A failed fetch must not read as "0 changes". -->
    <div
      v-if="error"
      class="flex items-start gap-2 rounded border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive"
    >
      <svg
        class="w-3.5 h-3.5 mt-0.5 shrink-0"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
      >
        <circle cx="12" cy="12" r="10" />
        <line x1="12" y1="8" x2="12" y2="12" />
        <line x1="12" y1="16" x2="12.01" y2="16" />
      </svg>
      <span>GRACE error: {{ error }}</span>
    </div>

    <GraceToolbar
      ref="toolbarRef"
      v-model:search="search"
      :selected-stages="filters.stage"
      :selected-authorizations="filters.authorization"
      :scope="filters.scope"
      :columns="columns"
      @toggle-stage="toggleStage"
      @toggle-authorization="toggleAuthorization"
      @select-scope="selectScope"
      @update:columns="updateColumns"
      @reset-columns="resetColumns"
    />

    <!-- The chip row and the loading note share one line: neither may add a
         row above the table, where the table is the sole scroll owner. -->
    <div
      v-if="isLoading || filters.stage.length > 0 || filters.authorization.length > 0 || filters.scope !== 'all'"
      class="flex flex-wrap items-center gap-1.5"
    >
      <GraceFilterChips
        :stage-filters="filters.stage"
        :authorization-filters="filters.authorization"
        :scope="filters.scope"
        @remove-stage="toggleStage"
        @remove-authorization="toggleAuthorization"
        @reset-scope="selectScope('all')"
        @clear-all="emit('clear-filters')"
      />
      <span v-if="isLoading" class="ml-auto text-xs text-muted-foreground">Loading…</span>
    </div>
  </div>

  <!-- No padding/scroller here: GraceTable owns the only scroll surface, fills
       the pane edge to edge, and flex-1 + min-h-0 is what makes h-full resolve. -->
  <div class="flex-1 min-h-0">
    <GraceTable
      :changes="filteredChanges"
      :columns="tableColumns"
      :selected-id="selectedId"
      :sort-field="sortField"
      :sort-direction="sortDirection"
      @select="emit('select', $event)"
      @sort="handleSort"
    />
  </div>
</template>
