import { describe, it, expect, vi, beforeEach } from 'vitest'
import type { GraceChangeSummary, GraceChangeDetail, GraceListResult } from '~/types/grace'

const { settings } = vi.hoisted(() => ({ settings: new Map<string, unknown>() }))

vi.mock('~/composables/useProjectStorage', async () => {
  const { ref } = await import('vue')
  return {
    useProjectStorage: <T>(key: string, defaultValue: T) => {
      if (!settings.has(key)) settings.set(key, defaultValue)
      return ref(settings.get(key) as T)
    },
  }
})

vi.mock('~/utils/br-api', () => ({
  graceList: vi.fn(),
  graceShow: vi.fn(),
  graceCheckChanged: vi.fn(),
}))

function change(id: string, overrides: Partial<GraceChangeSummary> = {}): GraceChangeSummary {
  return {
    id,
    title: null,
    revision: null,
    stage: 'prepared',
    authorization: 'unapproved',
    authorizationNote: null,
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

function detail(summary: GraceChangeSummary): GraceChangeDetail {
  return {
    summary,
    dir: `/project/.grace5/changes/${summary.id}`,
    contract: null,
    approvals: [],
    runs: [],
    reviews: [],
    close: null,
    briefs: [],
    notesMd: null,
    notesTruncated: false,
    logDirs: [],
    defects: [],
    latestResults: [],
  }
}

function listResult(changes: GraceChangeSummary[]): GraceListResult {
  return { hasGrace: true, pin: null, pinSupported: true, openCount: changes.length, closedCount: 0, changes }
}

function deferred<T>() {
  return Promise.withResolvers<T>()
}

/**
 * Fresh module instance per test: useGrace holds module-level singleton state.
 * Dynamic imports are required because `vi.resetModules()` must re-evaluate the
 * composable against freshly created mock functions.
 */
async function loadGrace() {
  const { useGrace } = await import('~/composables/useGrace')
  const api = await import('~/utils/br-api')
  return {
    grace: useGrace(),
    graceList: vi.mocked(api.graceList),
    graceShow: vi.mocked(api.graceShow),
    graceCheckChanged: vi.mocked(api.graceCheckChanged),
  }
}

beforeEach(() => {
  vi.resetModules()
  vi.resetAllMocks()
  settings.clear()
})

describe('useGrace', () => {
  it('discards the newest fetch when an older fetch settles last', async () => {
    const { grace, graceList } = await loadGrace()
    const older = deferred<GraceListResult>()
    const newer = deferred<GraceListResult>()
    graceList.mockReturnValueOnce(older.promise).mockReturnValueOnce(newer.promise)

    const fetchOlder = grace.fetchChanges('/project-a')
    const fetchNewer = grace.fetchChanges('/project-b')

    newer.resolve(listResult([change('C-0002')]))
    await fetchNewer
    older.resolve(listResult([change('C-0001')]))
    await fetchOlder

    expect(grace.changes.value.map(c => c.id)).toEqual(['C-0002'])
    expect(grace.isLoading.value).toBe(false)
  })

  it('aborts a refresh whose fingerprint check settles after a clear', async () => {
    const { grace, graceList, graceCheckChanged } = await loadGrace()
    const check = deferred<boolean>()
    graceCheckChanged.mockReturnValue(check.promise)
    graceList.mockResolvedValue(listResult([change('C-0001')]))

    const refresh = grace.refreshIfChanged('/project-a')
    grace.clear()
    check.resolve(true)

    await expect(refresh).resolves.toBe(false)
    expect(graceList).not.toHaveBeenCalled()
    expect(grace.changes.value).toEqual([])
    expect(grace.selectedChange.value).toBeNull()
    expect(grace.detail.value).toBeNull()
  })

  it('does not re-select the old project after a clear lands during the refresh fetch', async () => {
    const { grace, graceList, graceShow, graceCheckChanged } = await loadGrace()
    const oldChange = change('C-0001')
    graceShow.mockResolvedValue(detail(oldChange))
    await grace.selectChange('/project-a', oldChange)

    graceCheckChanged.mockResolvedValue(true)
    const oldListFetch = deferred<GraceListResult>()
    graceList.mockReturnValue(oldListFetch.promise)

    const refresh = grace.refreshIfChanged('/project-a')
    await vi.waitFor(() => expect(graceList).toHaveBeenCalledTimes(1))

    // Project switch: clear, then the new project selects its own change.
    grace.clear()
    const newChange = change('C-9999')
    graceShow.mockResolvedValue(detail(newChange))
    await grace.selectChange('/project-b', newChange)

    oldListFetch.resolve(listResult([change('C-0002')]))
    await expect(refresh).resolves.toBe(false)

    expect(graceShow).toHaveBeenCalledTimes(2)
    expect(graceShow).toHaveBeenLastCalledWith('/project-b', 'C-9999')
    expect(grace.selectedChange.value?.id).toBe('C-9999')
    expect(grace.detail.value?.summary.id).toBe('C-9999')
  })

  it('refreshes the list and the selected detail when the fingerprint changed', async () => {
    const { grace, graceList, graceShow, graceCheckChanged } = await loadGrace()
    const before = change('C-0001', { checksPassed: 1 })
    const after = change('C-0001', { checksPassed: 4, stage: 'executing' })

    graceList.mockResolvedValueOnce(listResult([before]))
    graceShow.mockResolvedValueOnce(detail(before))
    await grace.fetchChanges('/project')
    await grace.selectChange('/project', before)

    graceCheckChanged.mockResolvedValue(true)
    graceList.mockResolvedValueOnce(listResult([after]))
    graceShow.mockResolvedValueOnce(detail(after))

    await expect(grace.refreshIfChanged('/project')).resolves.toBe(true)
    expect(grace.changes.value[0]!.checksPassed).toBe(4)
    expect(grace.selectedChange.value?.checksPassed).toBe(4)
    expect(grace.detail.value?.summary.stage).toBe('executing')
    expect(graceShow).toHaveBeenLastCalledWith('/project', 'C-0001')
  })

  it('reports a failed list fetch as not refreshed and keeps the list error', async () => {
    const { grace, graceList, graceShow, graceCheckChanged } = await loadGrace()
    const selected = change('C-0001')
    graceShow.mockResolvedValueOnce(detail(selected))
    await grace.selectChange('/project', selected)
    graceShow.mockClear()

    graceCheckChanged.mockResolvedValue(true)
    graceList.mockRejectedValueOnce(new Error('grace_list exploded'))

    await expect(grace.refreshIfChanged('/project')).resolves.toBe(false)
    expect(grace.error.value).toBe('grace_list exploded')
    expect(graceShow).not.toHaveBeenCalled()
    expect(grace.selectedChange.value?.id).toBe('C-0001')
    expect(grace.detail.value?.summary.id).toBe('C-0001')
  })
})
