import { describe, it, expect } from 'vitest'
import { splitPath, getPathSeparator, getFolderName, getParentPath, isRootPath, getPathCrumbs, splitRoot } from '~/utils/path'

// ---------------------------------------------------------------------------
// splitPath
// ---------------------------------------------------------------------------
describe('splitPath', () => {
  it('splits Unix paths', () => {
    expect(splitPath('/home/dev/project')).toEqual(['', 'home', 'dev', 'project'])
  })

  it('splits Windows paths', () => {
    expect(splitPath('C:\\dev\\my-app')).toEqual(['C:', 'dev', 'my-app'])
  })

  it('splits mixed separators', () => {
    expect(splitPath('/path\\to/folder')).toEqual(['', 'path', 'to', 'folder'])
  })

  it('handles single component', () => {
    expect(splitPath('folder')).toEqual(['folder'])
  })

  it('handles trailing separator', () => {
    expect(splitPath('/path/to/')).toEqual(['', 'path', 'to', ''])
  })

  it('handles root path', () => {
    expect(splitPath('/')).toEqual(['', ''])
  })
})

// ---------------------------------------------------------------------------
// getPathSeparator
// ---------------------------------------------------------------------------
describe('getPathSeparator', () => {
  it('detects Unix separator', () => {
    expect(getPathSeparator('/home/dev')).toBe('/')
  })

  it('detects Windows separator', () => {
    expect(getPathSeparator('C:\\dev\\project')).toBe('\\')
  })

  it('defaults to / when no separator present', () => {
    expect(getPathSeparator('folder')).toBe('/')
  })

  it('prefers \\ when both are present', () => {
    expect(getPathSeparator('/path\\to')).toBe('\\')
  })
})

// ---------------------------------------------------------------------------
// getFolderName
// ---------------------------------------------------------------------------
describe('getFolderName', () => {
  it('extracts last component from Unix path', () => {
    expect(getFolderName('/home/dev/project')).toBe('project')
  })

  it('extracts last component from Windows path', () => {
    expect(getFolderName('C:\\dev\\my-app')).toBe('my-app')
  })

  it('returns the string itself for single component', () => {
    expect(getFolderName('project')).toBe('project')
  })

  it('returns the full path for trailing separator', () => {
    // splitPath('/path/to/') → ['', 'path', 'to', ''] → pop() returns ''
    // fallback to path since '' is falsy
    expect(getFolderName('/path/to/')).toBe('/path/to/')
  })
})

// ---------------------------------------------------------------------------
// getParentPath
// ---------------------------------------------------------------------------
describe('getParentPath', () => {
  it('returns parent for Unix path', () => {
    expect(getParentPath('/home/dev/project')).toBe('/home/dev')
  })

  it('returns parent for Windows path', () => {
    expect(getParentPath('C:\\dev\\my-app')).toBe('C:\\dev')
  })

  it('returns separator for root-level path', () => {
    expect(getParentPath('/folder')).toBe('/')
  })

  it('anchors the drive root instead of returning a drive-relative "C:"', () => {
    expect(getParentPath('C:\\folder')).toBe('C:\\')
  })

  it('returns separator for single component', () => {
    expect(getParentPath('folder')).toBe('/')
  })

  it('is idempotent at the Windows drive root', () => {
    expect(getParentPath('C:\\')).toBe('C:\\')
    expect(getParentPath('C:')).toBe('C:\\')
  })

  it('is idempotent at the Unix root', () => {
    expect(getParentPath('/')).toBe('/')
  })

  it('stops at a UNC share instead of walking into \\\\srv', () => {
    // `\\srv` is not a directory — the share is as far up as you can go.
    expect(getParentPath('\\\\srv\\share\\dir')).toBe('\\\\srv\\share')
    expect(getParentPath('\\\\srv\\share')).toBe('\\\\srv\\share')
  })

  it('returns the parent within a UNC share', () => {
    expect(getParentPath('\\\\srv\\share\\dir\\sub')).toBe('\\\\srv\\share\\dir')
  })
})

// ---------------------------------------------------------------------------
// splitRoot
// ---------------------------------------------------------------------------
describe('splitRoot', () => {
  it('splits a Windows drive path', () => {
    expect(splitRoot('C:\\dev\\app')).toEqual({ root: 'C:\\', rest: 'dev\\app' })
  })

  it('anchors a bare drive letter', () => {
    expect(splitRoot('C:')).toEqual({ root: 'C:\\', rest: '' })
  })

  it('treats a UNC share as the root', () => {
    expect(splitRoot('\\\\srv\\share\\dir')).toEqual({ root: '\\\\srv\\share', rest: '\\dir' })
  })

  it('splits a Unix path', () => {
    expect(splitRoot('/home/dev')).toEqual({ root: '/', rest: 'home/dev' })
  })

  it('reports no root for a relative path', () => {
    expect(splitRoot('repos/app')).toEqual({ root: '', rest: 'repos/app' })
  })
})

// ---------------------------------------------------------------------------
// isRootPath
// ---------------------------------------------------------------------------
describe('isRootPath', () => {
  it('recognises the Unix root', () => {
    expect(isRootPath('/')).toBe(true)
  })

  it('recognises Windows drive roots in both forms', () => {
    expect(isRootPath('C:\\')).toBe(true)
    expect(isRootPath('d:/')).toBe(true)
    expect(isRootPath('C:')).toBe(true)
  })

  it('recognises a UNC share as a root', () => {
    expect(isRootPath('\\\\srv\\share')).toBe(true)
    expect(isRootPath('\\\\srv\\share\\')).toBe(true)
  })

  it('rejects non-root paths', () => {
    expect(isRootPath('C:\\dev')).toBe(false)
    expect(isRootPath('/home')).toBe(false)
    expect(isRootPath('\\\\srv\\share\\dir')).toBe(false)
    expect(isRootPath('folder')).toBe(false)
    expect(isRootPath('')).toBe(false)
  })
})

// ---------------------------------------------------------------------------
// getPathCrumbs
// ---------------------------------------------------------------------------
describe('getPathCrumbs', () => {
  it('builds crumbs for a Windows path with an anchored drive root', () => {
    expect(getPathCrumbs('C:\\dev\\my-app')).toEqual([
      { name: 'C:', path: 'C:\\' },
      { name: 'dev', path: 'C:\\dev' },
      { name: 'my-app', path: 'C:\\dev\\my-app' },
    ])
  })

  it('builds crumbs for a Unix path', () => {
    expect(getPathCrumbs('/home/dev/project')).toEqual([
      { name: '/', path: '/' },
      { name: 'home', path: '/home' },
      { name: 'dev', path: '/home/dev' },
      { name: 'project', path: '/home/dev/project' },
    ])
  })

  it('returns a single crumb for a drive root', () => {
    expect(getPathCrumbs('C:\\')).toEqual([{ name: 'C:', path: 'C:\\' }])
  })

  it('builds crumbs for a UNC path with the share as a single root crumb', () => {
    expect(getPathCrumbs('\\\\srv\\share\\dir')).toEqual([
      { name: '\\\\srv\\share', path: '\\\\srv\\share' },
      { name: 'dir', path: '\\\\srv\\share\\dir' },
    ])
  })

  it('builds a single crumb for a relative path', () => {
    expect(getPathCrumbs('~')).toEqual([{ name: '~', path: '~' }])
  })

  it('returns an empty list for an empty path', () => {
    expect(getPathCrumbs('')).toEqual([])
  })
})
