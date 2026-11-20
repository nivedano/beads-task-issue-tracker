/**
 * Pure helper functions for the GRACE explorer.
 *
 * No Vue imports: filtering, sorting, counts and the tab rule are plain
 * functions so they stay testable and reusable from the composable/components.
 */
import type { GraceStage, GraceAuthorization, GraceChangeSummary } from '~/types/grace'
import { naturalCompare } from '~/utils/issue-helpers'

export interface GraceFilters {
  stage: GraceStage[]
  authorization: GraceAuthorization[]
  scope: 'open' | 'closed' | 'all'
}

/**
 * Lifecycle ladder, earliest → latest. The same order drives stage sorting
 * and the detail view's stepper.
 */
export const graceStageLadder: readonly GraceStage[] = ['prepared', 'approved', 'executing', 'reviewed', 'closed']

/** A change counts as closed when it is marked closed or lives in `closed/`. */
function isClosedChange(change: GraceChangeSummary): boolean {
  return change.closed || change.inClosedDir
}

/**
 * Apply scope + stage/authorization facets + free-text search.
 * Facets are ORed within themselves and ANDed across each other; malformed
 * changes are ordinary rows here — the list flags them, it never hides them.
 */
export function filterGraceChanges(
  changes: GraceChangeSummary[],
  filters: GraceFilters,
  search: string,
): GraceChangeSummary[] {
  const query = search.trim().toLowerCase()

  return changes.filter((change) => {
    if (filters.scope !== 'all' && isClosedChange(change) !== (filters.scope === 'closed')) return false
    if (filters.stage.length > 0 && !filters.stage.includes(change.stage)) return false
    if (filters.authorization.length > 0 && !filters.authorization.includes(change.authorization)) return false
    if (!query) return true
    return [change.id, change.title, change.problem]
      .some(value => value != null && value.toLowerCase().includes(query))
  })
}

/** Compare nullable values; null always sorts last regardless of direction. */
function compareNullable<T>(
  a: T | null,
  b: T | null,
  compare: (x: T, y: T) => number,
  direction: number,
): number {
  if (a === null && b === null) return 0
  if (a === null) return 1
  if (b === null) return -1
  return compare(a, b) * direction
}

/**
 * Sort a copy of the list; ties fall back to natural id order so the order
 * stays stable across refreshes.
 */
export function sortGraceChanges(
  changes: GraceChangeSummary[],
  field: 'id' | 'title' | 'stage' | 'revision',
  direction: 'asc' | 'desc',
): GraceChangeSummary[] {
  const dir = direction === 'asc' ? 1 : -1

  return [...changes].sort((a, b) => {
    let primary = 0

    switch (field) {
      case 'id':
        primary = naturalCompare(a.id.toLowerCase(), b.id.toLowerCase()) * dir
        break
      case 'title':
        primary = compareNullable(a.title, b.title, (x, y) => x.localeCompare(y), dir)
        break
      case 'stage':
        primary = (graceStageLadder.indexOf(a.stage) - graceStageLadder.indexOf(b.stage)) * dir
        break
      case 'revision':
        primary = compareNullable(a.revision, b.revision, (x, y) => x - y, dir)
        break
    }

    return primary !== 0 ? primary : naturalCompare(a.id.toLowerCase(), b.id.toLowerCase())
  })
}

/** Display-level counts: a change is closed when closed or stored in `closed/`. */
export function graceStats(changes: GraceChangeSummary[]): { open: number; closed: number } {
  let closed = 0
  for (const change of changes) {
    if (isClosedChange(change)) closed++
  }
  return { open: changes.length - closed, closed }
}

/**
 * Pick the explorer shown in the center panel.
 * The tab strip exists only when both registries are present; with a single
 * registry its explorer renders bare and the stored tab is ignored.
 */
export function resolveExplorerTab(
  stored: 'issues' | 'grace',
  caps: { hasBeads: boolean; hasGrace: boolean },
): { tab: 'issues' | 'grace' | null, showStrip: boolean } {
  if (caps.hasBeads && caps.hasGrace) return { tab: stored, showStrip: true }
  if (caps.hasBeads) return { tab: 'issues', showStrip: false }
  if (caps.hasGrace) return { tab: 'grace', showStrip: false }
  return { tab: null, showStrip: false }
}
