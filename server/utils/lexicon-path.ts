import { isAbsolute, relative, sep } from 'node:path'

type PathApi = {
  relative: (from: string, to: string) => string
  isAbsolute: (path: string) => boolean
  sep: string
}

/**
 * Whether `resolved` stays inside `root`. Built on `path.relative` so a project
 * that *is* a filesystem root (`D:\`, `/`) is handled correctly — the common
 * `startsWith(root + sep)` check rejects every path under a drive root.
 */
export function isWithinRoot(
  root: string,
  resolved: string,
  path: PathApi = { relative, isAbsolute, sep },
): boolean {
  const inside = path.relative(root, resolved)
  if (inside === '') return true
  if (path.isAbsolute(inside)) return false
  return inside !== '..' && !inside.startsWith(`..${path.sep}`)
}
