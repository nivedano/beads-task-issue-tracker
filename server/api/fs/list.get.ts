import { readdir, stat } from 'node:fs/promises'
import { join, resolve } from 'node:path'
import { homedir } from 'node:os'

export interface DirectoryEntry {
  name: string
  path: string
  isDirectory: boolean
  hasBeads: boolean
  hasGrace: boolean
}

/** Whether `dir` contains a subdirectory named `name` (e.g. a `.beads` registry). */
async function hasSubdirectory(dir: string, name: string): Promise<boolean> {
  try {
    const entryStat = await stat(join(dir, name))
    return entryStat.isDirectory()
  } catch {
    // Subdirectory doesn't exist
    return false
  }
}

export default defineEventHandler(async (event) => {
  const query = getQuery(event)
  let requestedPath = query.path ? String(query.path) : homedir()

  // Handle special paths
  if (requestedPath === '~') {
    requestedPath = homedir()
  }

  const targetPath = resolve(requestedPath)

  try {
    const entries = await readdir(targetPath, { withFileTypes: true })

    const directories: DirectoryEntry[] = []

    for (const entry of entries) {
      // Skip hidden files/folders (starting with .)
      if (entry.name.startsWith('.')) continue

      if (entry.isDirectory()) {
        const fullPath = join(targetPath, entry.name)

        // Check which registries this directory contains
        const [hasBeads, hasGrace] = await Promise.all([
          hasSubdirectory(fullPath, '.beads'),
          hasSubdirectory(fullPath, '.grace5'),
        ])

        directories.push({
          name: entry.name,
          path: fullPath,
          isDirectory: true,
          hasBeads,
          hasGrace,
        })
      }
    }

    // Sort: beads projects first, then alphabetically
    directories.sort((a, b) => {
      if (a.hasBeads && !b.hasBeads) return -1
      if (!a.hasBeads && b.hasBeads) return 1
      return a.name.localeCompare(b.name)
    })

    // Check which registries the current directory contains
    const [currentHasBeads, currentHasGrace] = await Promise.all([
      hasSubdirectory(targetPath, '.beads'),
      hasSubdirectory(targetPath, '.grace5'),
    ])

    return {
      currentPath: targetPath,
      hasBeads: currentHasBeads,
      hasGrace: currentHasGrace,
      entries: directories,
    }
  } catch (error) {
    throw createError({
      statusCode: 500,
      message: `Cannot read directory: ${error instanceof Error ? error.message : 'Unknown error'}`,
    })
  }
})
