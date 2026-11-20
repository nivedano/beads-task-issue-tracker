/**
 * GRACE explorer state (singleton, like `useIssues`).
 *
 * Thin orchestration only: parsing/filtering/sorting live in
 * `~/utils/grace-helpers`, IPC in `~/utils/bd-api`. A generation guard keeps
 * late responses from a previous project or selection out of the refs.
 */
import type { GraceStage, GraceAuthorization, GraceChangeSummary, GraceChangeDetail, GraceListResult } from '~/types/grace'
import { graceList, graceShow, graceCheckChanged } from '~/utils/bd-api'
import { filterGraceChanges, sortGraceChanges, graceStats, type GraceFilters } from '~/utils/grace-helpers'
import { useProjectStorage } from '~/composables/useProjectStorage'

type GraceSort = { field: 'id' | 'title' | 'stage' | 'revision', direction: 'asc' | 'desc' }

// Shared state across all components (singleton pattern)
const changes = ref<GraceChangeSummary[]>([])
const listResult = ref<GraceListResult | null>(null)
const selectedChange = ref<GraceChangeSummary | null>(null)
const detail = ref<GraceChangeDetail | null>(null)
const isLoading = ref(false)
const detailLoading = ref(false)
const error = ref<string | null>(null)

// Per-project persistence (keys match the plan's storage contract)
const search = useProjectStorage<string>('grace:search', '')
const stageFilter = useProjectStorage<GraceStage[]>('grace:filters:stage', [])
const authorizationFilter = useProjectStorage<GraceAuthorization[]>('grace:filters:authorization', [])
const scopeFilter = useProjectStorage<'open' | 'closed' | 'all'>('grace:filters:scope', 'open')
const sort = useProjectStorage<GraceSort>('grace:sort', { field: 'id', direction: 'desc' })

// Two-way facade so components can assign `filters.stage = [...]` and still
// persist through the individual storage refs.
const filters = reactive<GraceFilters>({
  get stage() { return stageFilter.value },
  set stage(value: GraceStage[]) { stageFilter.value = value },
  get authorization() { return authorizationFilter.value },
  set authorization(value: GraceAuthorization[]) { authorizationFilter.value = value },
  get scope() { return scopeFilter.value },
  set scope(value: GraceFilters['scope']) { scopeFilter.value = value },
})

// Stale-response guards for project switches and rapid selection changes
let listGeneration = 0
let detailGeneration = 0

const filteredChanges = computed(() =>
  sortGraceChanges(
    filterGraceChanges(changes.value, filters, search.value),
    sort.value.field,
    sort.value.direction,
  ),
)

const stats = computed(() => graceStats(changes.value))

async function fetchChanges(path: string): Promise<void> {
  const generation = ++listGeneration
  isLoading.value = true
  error.value = null

  try {
    const result = await graceList(path)
    if (generation !== listGeneration) return
    listResult.value = result
    changes.value = result.changes
  } catch (e) {
    if (generation !== listGeneration) return
    // Keep the last known changes on failure; the error surfaces in the panel.
    error.value = e instanceof Error ? e.message : 'Failed to load GRACE changes'
  } finally {
    if (generation === listGeneration) isLoading.value = false
  }
}

async function selectChange(path: string, change: GraceChangeSummary | null): Promise<void> {
  const generation = ++detailGeneration
  selectedChange.value = change

  if (!change) {
    detail.value = null
    detailLoading.value = false
    return
  }

  detail.value = null
  detailLoading.value = true
  error.value = null

  try {
    const result = await graceShow(path, change.id)
    if (generation !== detailGeneration) return
    detail.value = result
    selectedChange.value = result.summary
  } catch (e) {
    if (generation !== detailGeneration) return
    error.value = e instanceof Error ? e.message : 'Failed to load GRACE change'
  } finally {
    if (generation === detailGeneration) detailLoading.value = false
  }
}

/** Cheap fingerprint check; refreshes the list and the selected detail when stale. */
async function refreshIfChanged(path: string): Promise<boolean> {
  let changed: boolean
  try {
    changed = await graceCheckChanged(path)
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Failed to check GRACE changes'
    return false
  }

  if (!changed) return false

  await fetchChanges(path)

  const selected = selectedChange.value
  if (selected) {
    await selectChange(path, changes.value.find(c => c.id === selected.id) ?? selected)
  }

  return true
}

/** Drop all state (no registries, project switch away from grace). */
function clear(): void {
  listGeneration++
  detailGeneration++
  changes.value = []
  listResult.value = null
  selectedChange.value = null
  detail.value = null
  isLoading.value = false
  detailLoading.value = false
  error.value = null
}

export function useGrace() {
  return {
    changes,
    listResult,
    selectedChange,
    detail,
    isLoading,
    detailLoading,
    error,
    search,
    filters,
    // Assign a new object when changing sort — never mutate the stored default in place.
    sort,
    filteredChanges,
    stats,
    fetchChanges,
    selectChange,
    refreshIfChanged,
    clear,
  }
}