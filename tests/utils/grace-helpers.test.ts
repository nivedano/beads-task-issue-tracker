import { describe, it, expect } from 'vitest'
import {
  filterGraceChanges,
  sortGraceChanges,
  graceStats,
  resolveExplorerTab,
  graceLocation,
  toGraceRelative,
  type GraceFilters,
} from '~/utils/grace-helpers'
import type { GraceChangeSummary } from '~/types/grace'

/** Baseline summary; tests override only the fields they exercise. */
function change(id: string, overrides: Partial<GraceChangeSummary> = {}): GraceChangeSummary {
  return {
    id,
    title: null,
    revision: null,
    stage: 'prepared',
    authorization: 'unapproved',
    malformed: null,
    inClosedDir: false,
    closed: false,
    checksPassed: 0,
    checksRequired: 0,
    reviewStatus: null,
    reviewOpenFindings: 0,
    problem: null,
    ...overrides,
  }
}

const noFilters: GraceFilters = { stage: [], authorization: [], scope: 'all' }

// ---------------------------------------------------------------------------
// filterGraceChanges
// ---------------------------------------------------------------------------
describe('filterGraceChanges', () => {
  it('returns every change when filters are empty and search is blank', () => {
    const changes = [change('C-0001'), change('C-0002'), change('C-0003')]
    expect(filterGraceChanges(changes, noFilters, '').map(c => c.id)).toEqual(['C-0001', 'C-0002', 'C-0003'])
  })

  it("scope 'open' excludes closed changes and changes in the closed/ directory", () => {
    const changes = [
      change('C-0001'),
      change('C-0002', { closed: true }),
      change('C-0003', { inClosedDir: true }),
    ]
    expect(filterGraceChanges(changes, { ...noFilters, scope: 'open' }, '').map(c => c.id)).toEqual(['C-0001'])
  })

  it("scope 'closed' keeps only closed changes and changes in the closed/ directory", () => {
    const changes = [
      change('C-0001'),
      change('C-0002', { closed: true }),
      change('C-0003', { inClosedDir: true }),
    ]
    expect(filterGraceChanges(changes, { ...noFilters, scope: 'closed' }, '').map(c => c.id))
      .toEqual(['C-0002', 'C-0003'])
  })

  it("scope 'all' keeps both open and closed changes", () => {
    const changes = [
      change('C-0001'),
      change('C-0002', { closed: true }),
      change('C-0003', { inClosedDir: true }),
    ]
    expect(filterGraceChanges(changes, noFilters, '').map(c => c.id)).toEqual(['C-0001', 'C-0002', 'C-0003'])
  })

  it('ORs stages within the stage facet', () => {
    const changes = [
      change('C-0001', { stage: 'prepared' }),
      change('C-0002', { stage: 'approved' }),
      change('C-0003', { stage: 'reviewed' }),
    ]
    const filters: GraceFilters = { ...noFilters, stage: ['prepared', 'reviewed'] }
    expect(filterGraceChanges(changes, filters, '').map(c => c.id)).toEqual(['C-0001', 'C-0003'])
  })

  it('ORs authorizations within the authorization facet', () => {
    const changes = [
      change('C-0001', { authorization: 'approved' }),
      change('C-0002', { authorization: 'rejected' }),
      change('C-0003', { authorization: 'unapproved' }),
    ]
    const filters: GraceFilters = { ...noFilters, authorization: ['approved', 'unapproved'] }
    expect(filterGraceChanges(changes, filters, '').map(c => c.id)).toEqual(['C-0001', 'C-0003'])
  })

  it('ANDs facets together', () => {
    const changes = [
      change('C-0001', { stage: 'approved', authorization: 'approved' }),
      change('C-0002', { stage: 'approved', authorization: 'rejected' }),
      change('C-0003', { stage: 'closed', authorization: 'approved', closed: true }),
    ]
    const filters: GraceFilters = { stage: ['approved'], authorization: ['approved'], scope: 'open' }
    expect(filterGraceChanges(changes, filters, '').map(c => c.id)).toEqual(['C-0001'])
  })

  it('matches ids case-insensitively', () => {
    const changes = [change('C-0019-retire-briefs'), change('C-0020-other')]
    expect(filterGraceChanges(changes, noFilters, 'c-0019').map(c => c.id)).toEqual(['C-0019-retire-briefs'])
  })

  it('matches titles case-insensitively', () => {
    const changes = [change('C-0001', { title: 'Retire Briefs' }), change('C-0002', { title: 'Other work' })]
    expect(filterGraceChanges(changes, noFilters, 'retire').map(c => c.id)).toEqual(['C-0001'])
  })

  it('matches problems case-insensitively', () => {
    const changes = [
      change('C-0001', { problem: 'Briefs are stale' }),
      change('C-0002', { problem: 'Something else' }),
    ]
    expect(filterGraceChanges(changes, noFilters, 'STALE').map(c => c.id)).toEqual(['C-0001'])
  })

  it('tolerates null titles and problems', () => {
    const changes = [change('C-0001'), change('C-0002', { title: 'Has title', problem: 'Has problem' })]
    expect(filterGraceChanges(changes, noFilters, 'zzz').map(c => c.id)).toEqual([])
    expect(filterGraceChanges(changes, noFilters, 'c-0001').map(c => c.id)).toEqual(['C-0001'])
  })

  it('treats malformed changes as ordinary rows', () => {
    const changes = [change('C-0001', { malformed: 'missing contract id' }), change('C-0002')]
    expect(filterGraceChanges(changes, noFilters, '').map(c => c.id)).toEqual(['C-0001', 'C-0002'])
    expect(filterGraceChanges(changes, { ...noFilters, stage: ['prepared'] }, '').map(c => c.id)).toEqual(['C-0001', 'C-0002'])
    expect(filterGraceChanges(changes, { ...noFilters, stage: ['closed'] }, '')).toEqual([])
    expect(filterGraceChanges(changes, noFilters, 'c-0001').map(c => c.id)).toEqual(['C-0001'])
  })

  it('ANDs search with a non-empty facet', () => {
    const changes = [
      change('C-0001', { title: 'Retire briefs' }),
      change('C-0002', { title: 'Retire logs', stage: 'approved' }),
    ]
    expect(filterGraceChanges(changes, noFilters, 'retire').map(c => c.id)).toEqual(['C-0001', 'C-0002'])
    expect(filterGraceChanges(changes, { ...noFilters, stage: ['prepared'] }, 'retire').map(c => c.id))
      .toEqual(['C-0001'])
    expect(filterGraceChanges(changes, { ...noFilters, stage: ['closed'] }, 'retire')).toEqual([])
  })

  it('treats a whitespace-only search as no search', () => {
    const changes = [change('C-0001'), change('C-0002')]
    expect(filterGraceChanges(changes, noFilters, '   ').map(c => c.id)).toEqual(['C-0001', 'C-0002'])
  })
})

// ---------------------------------------------------------------------------
// sortGraceChanges
// ---------------------------------------------------------------------------
describe('sortGraceChanges', () => {
  it('sorts ids naturally in ascending order', () => {
    const changes = [change('C-0010'), change('C-0002'), change('C-0100')]
    expect(sortGraceChanges(changes, 'id', 'asc').map(c => c.id)).toEqual(['C-0002', 'C-0010', 'C-0100'])
  })

  it('reverses the id order when descending', () => {
    const changes = [change('C-0002'), change('C-0010'), change('C-0100')]
    expect(sortGraceChanges(changes, 'id', 'desc').map(c => c.id)).toEqual(['C-0100', 'C-0010', 'C-0002'])
  })

  it('sorts titles with localeCompare and keeps nulls last when ascending', () => {
    const changes = [
      change('C-0003', { title: null }),
      change('C-0002', { title: 'beta' }),
      change('C-0001', { title: 'Alpha' }),
    ]
    expect(sortGraceChanges(changes, 'title', 'asc').map(c => c.id)).toEqual(['C-0001', 'C-0002', 'C-0003'])
  })

  it('keeps null titles last when descending', () => {
    const changes = [
      change('C-0003', { title: null }),
      change('C-0002', { title: 'beta' }),
      change('C-0001', { title: 'Alpha' }),
    ]
    expect(sortGraceChanges(changes, 'title', 'desc').map(c => c.id)).toEqual(['C-0002', 'C-0001', 'C-0003'])
  })

  it('sorts stages along the ladder when ascending', () => {
    const changes = [
      change('C-0003', { stage: 'closed' }),
      change('C-0001', { stage: 'prepared' }),
      change('C-0002', { stage: 'executing' }),
    ]
    expect(sortGraceChanges(changes, 'stage', 'asc').map(c => c.id)).toEqual(['C-0001', 'C-0002', 'C-0003'])
  })

  it('reverses the stage ladder when descending', () => {
    const changes = [
      change('C-0001', { stage: 'prepared' }),
      change('C-0003', { stage: 'closed' }),
      change('C-0002', { stage: 'executing' }),
    ]
    expect(sortGraceChanges(changes, 'stage', 'desc').map(c => c.id)).toEqual(['C-0003', 'C-0002', 'C-0001'])
  })

  it('sorts revisions numerically with nulls last when ascending', () => {
    const changes = [
      change('C-0003', { revision: null }),
      change('C-0002', { revision: 3 }),
      change('C-0001', { revision: 1 }),
    ]
    expect(sortGraceChanges(changes, 'revision', 'asc').map(c => c.id)).toEqual(['C-0001', 'C-0002', 'C-0003'])
  })

  it('keeps null revisions last when descending', () => {
    const changes = [
      change('C-0003', { revision: null }),
      change('C-0001', { revision: 1 }),
      change('C-0002', { revision: 3 }),
    ]
    expect(sortGraceChanges(changes, 'revision', 'desc').map(c => c.id)).toEqual(['C-0002', 'C-0001', 'C-0003'])
  })

  it('breaks ties by natural id order', () => {
    const changes = [
      change('C-0010', { stage: 'approved' }),
      change('C-0002', { stage: 'approved' }),
    ]
    expect(sortGraceChanges(changes, 'stage', 'asc').map(c => c.id)).toEqual(['C-0002', 'C-0010'])
  })

  it('does not mutate the input array', () => {
    const changes = [change('C-0010'), change('C-0002')]
    const snapshot = changes.map(c => c.id)
    sortGraceChanges(changes, 'id', 'asc')
    expect(changes.map(c => c.id)).toEqual(snapshot)
  })
})

// ---------------------------------------------------------------------------
// graceStats
// ---------------------------------------------------------------------------
describe('graceStats', () => {
  it('counts open changes as everything not closed', () => {
    const changes = [
      change('C-0001'),
      change('C-0002', { closed: true }),
      change('C-0003', { inClosedDir: true }),
      change('C-0004'),
    ]
    expect(graceStats(changes)).toEqual({ open: 2, closed: 2 })
  })

  it('returns zeros for an empty list', () => {
    expect(graceStats([])).toEqual({ open: 0, closed: 0 })
  })
})

// ---------------------------------------------------------------------------
// resolveExplorerTab
// ---------------------------------------------------------------------------
describe('resolveExplorerTab', () => {
  it('honours the stored tab when both registries exist', () => {
    expect(resolveExplorerTab('grace', { hasBeads: true, hasGrace: true }))
      .toEqual({ tab: 'grace', showStrip: true })
    expect(resolveExplorerTab('issues', { hasBeads: true, hasGrace: true }))
      .toEqual({ tab: 'issues', showStrip: true })
  })

  it('forces the issues tab for a beads-only project', () => {
    expect(resolveExplorerTab('grace', { hasBeads: true, hasGrace: false }))
      .toEqual({ tab: 'issues', showStrip: false })
  })

  it('forces the grace tab for a grace-only project', () => {
    expect(resolveExplorerTab('issues', { hasBeads: false, hasGrace: true }))
      .toEqual({ tab: 'grace', showStrip: false })
  })

  it('renders no explorer when neither registry exists', () => {
    expect(resolveExplorerTab('grace', { hasBeads: false, hasGrace: false }))
      .toEqual({ tab: null, showStrip: false })
  })
})

// ---------------------------------------------------------------------------
// graceLocation / toGraceRelative
// ---------------------------------------------------------------------------
describe('graceLocation', () => {
  it('splits an absolute Windows change dir into project root and change prefix', () => {
    expect(graceLocation('D:/p/.grace5/changes/closed/C-x')).toEqual({
      projectPath: 'D:/p',
      changePrefix: ['changes', 'closed', 'C-x'],
    })
  })

  it('splits an absolute Windows change dir written with backslashes', () => {
    expect(graceLocation('D:\\p\\.grace5\\changes\\C-x')).toEqual({
      projectPath: 'D:/p',
      changePrefix: ['changes', 'C-x'],
    })
  })

  it('splits an absolute POSIX change dir', () => {
    expect(graceLocation('/srv/p/.grace5/changes/C-x')).toEqual({
      projectPath: '/srv/p',
      changePrefix: ['changes', 'C-x'],
    })
  })

  it('splits at the .grace5 segment, not at a .grace5 substring', () => {
    expect(graceLocation('D:/my.grace5repo/.grace5/changes/C-x')).toEqual({
      projectPath: 'D:/my.grace5repo',
      changePrefix: ['changes', 'C-x'],
    })
  })

  it('returns null for relative or empty dirs', () => {
    expect(graceLocation('changes/closed/C-x')).toBeNull()
    expect(graceLocation('.grace5/changes/C-x')).toBeNull()
    expect(graceLocation('')).toBeNull()
    expect(graceLocation(null)).toBeNull()
  })
})

describe('toGraceRelative', () => {
  const location = graceLocation('D:/p/.grace5/changes/closed/C-x')!

  it('composes a run log path below .grace5', () => {
    expect(toGraceRelative(location, 'records/logs/run-1/CK1.log'))
      .toBe('changes/closed/C-x/records/logs/run-1/CK1.log')
  })

  it('normalizes backslash separators in the recorded path', () => {
    expect(toGraceRelative(location, 'records\\run-1.xml'))
      .toBe('changes/closed/C-x/records/run-1.xml')
  })

  it('returns null without a location or a path', () => {
    expect(toGraceRelative(null, 'records/run-1.xml')).toBeNull()
    expect(toGraceRelative(location, '')).toBeNull()
  })
})
