/**
 * Path utilities for cross-platform compatibility (Windows/macOS/Linux)
 * Preserves the native path separator format of the platform.
 */

/**
 * Split a path into its components (works with both / and \)
 */
export function splitPath(path: string): string[] {
  return path.split(/[/\\]/)
}

/**
 * Detect which separator is used in a path
 */
export function getPathSeparator(path: string): string {
  return path.includes('\\') ? '\\' : '/'
}

/**
 * Get the folder name from a path (last component)
 */
export function getFolderName(path: string): string {
  const parts = splitPath(path)
  return parts.pop() || path
}

/**
 * Split a path into the root you cannot navigate above and the rest.
 *
 * A UNC share is a root in its own right: `\\srv` is not a directory, so
 * `\\srv\share` is as far up as the picker can go. A bare drive letter is
 * always returned anchored (`C:\`) because `C:` alone is drive-*relative* on
 * Windows and resolves to the current directory on that drive.
 *
 * `C:\dev\app` → `{ root: 'C:\', rest: 'dev\app' }`
 * `\\srv\share\dir` → `{ root: '\\srv\share', rest: '\dir' }`
 * `/home/dev` → `{ root: '/', rest: 'home/dev' }`
 * `repos/app` → `{ root: '', rest: 'repos/app' }`
 */
export function splitRoot(path: string): { root: string, rest: string } {
  const unc = /^\\\\([^\\/]+)[\\/]([^\\/]+)/.exec(path)
  if (unc) {
    return { root: `\\\\${unc[1]}\\${unc[2]}`, rest: path.slice(unc[0].length) }
  }

  const drive = /^([a-zA-Z]:)[\\/]?/.exec(path)
  if (drive) {
    return { root: `${drive[1]}\\`, rest: path.slice(drive[0].length) }
  }

  if (path.startsWith('/')) {
    return { root: '/', rest: path.slice(1) }
  }

  return { root: '', rest: path }
}

function segmentsOf(rest: string): string[] {
  return rest.split(/[/\\]/).filter(Boolean)
}

/**
 * True for a filesystem root: `/` on Unix, `C:\` (or bare `C:`) on Windows,
 * `\\server\share` for a UNC share.
 */
export function isRootPath(path: string): boolean {
  if (!path) return false
  const { root, rest } = splitRoot(path)
  return root !== '' && segmentsOf(rest).length === 0
}

/**
 * Get the parent path, preserving the original separator.
 *
 * Idempotent at a root, so the picker's "up" button becomes a no-op there
 * instead of walking into an unresolvable path like `C:` or `\\srv`.
 */
export function getParentPath(path: string): string {
  const { root, rest } = splitRoot(path)
  const segments = segmentsOf(rest)

  if (segments.length === 0) return root || '/'
  segments.pop()

  const separator = getPathSeparator(path)
  if (root === '') return segments.join(separator) || separator
  if (segments.length === 0) return root

  return /[\\/]$/.test(root)
    ? `${root}${segments.join(separator)}`
    : `${root}${separator}${segments.join(separator)}`
}

/**
 * Breadcrumb segments for a path, each with the absolute path it navigates to.
 * `C:\dev\app` → [{ name: 'C:', path: 'C:\' }, { name: 'dev', path: 'C:\dev' },
 *                 { name: 'app', path: 'C:\dev\app' }]
 */
export function getPathCrumbs(path: string): Array<{ name: string, path: string }> {
  if (!path) return []

  const separator = getPathSeparator(path)
  const { root, rest } = splitRoot(path)
  const crumbs: Array<{ name: string, path: string }> = []

  // The root is one crumb; its label drops the trailing separator (`C:`) but
  // the path it navigates to keeps it (`C:\`).
  let current = root
  if (root !== '') {
    crumbs.push({ name: root === '/' ? '/' : root.replace(/[\\/]+$/, ''), path: root })
  }

  for (const segment of segmentsOf(rest)) {
    current = current !== '' && !/[\\/]$/.test(current)
      ? `${current}${separator}${segment}`
      : `${current}${segment}`
    crumbs.push({ name: segment, path: current })
  }

  return crumbs
}
