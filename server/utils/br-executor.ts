import { exec } from 'node:child_process'
import { promisify } from 'node:util'

const execAsync = promisify(exec)

export interface BrExecutorOptions {
  args?: string[]
  cwd?: string
}

export interface BrResult<T = unknown> {
  success: boolean
  data?: T
  error?: string
}

/**
 * Unwrap the paginated envelope returned by br >= 0.1.30 for `list` commands.
 * br list --json returns {"issues": [...], "total": N, ...} instead of a flat array.
 * Other commands (show, ready, search) still return flat arrays.
 */
export function unwrapBrEnvelope<T = unknown>(data: unknown): T[] {
  if (Array.isArray(data)) {
    return data as T[]
  }
  if (data && typeof data === 'object' && 'issues' in data) {
    const envelope = data as { issues: unknown[] }
    if (Array.isArray(envelope.issues)) {
      return envelope.issues as T[]
    }
  }
  return []
}

/**
 * Execute a br CLI command and return JSON output.
 * br is the only supported client.
 * @param command - The br command to execute
 * @param options - Options including args and working directory
 */
export async function executeBr<T = unknown>(
  command: string,
  options: BrExecutorOptions = {}
): Promise<BrResult<T>> {
  // Priority: options.cwd > BEADS_PATH env > process.cwd()
  const workingDir = options.cwd || process.env.BEADS_PATH || process.cwd()
  const args = options.args?.join(' ') || ''
  const fullCommand = `br ${command} ${args} --json`.trim()

  try {
    const { stdout, stderr } = await execAsync(fullCommand, {
      cwd: workingDir,
      env: {
        ...process.env,
        BEADS_PATH: workingDir,
      },
    })

    if (stderr && !stdout) {
      return {
        success: false,
        error: stderr.trim(),
      }
    }

    try {
      const data = JSON.parse(stdout) as T
      return {
        success: true,
        data,
      }
    } catch {
      // If output is not JSON, return raw output
      return {
        success: true,
        data: stdout.trim() as unknown as T,
      }
    }
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error)
    return {
      success: false,
      error: errorMessage,
    }
  }
}

/**
 * Execute br list command with optional filters
 */
export async function brList(
  filters?: {
    status?: string[]
    type?: string[]
    priority?: string[]
    assignee?: string
    includeAll?: boolean
  },
  cwd?: string
) {
  const args: string[] = []

  // --all includes closed issues (used for search)
  if (filters?.includeAll) {
    args.push('--all')
  }

  if (filters?.status?.length) {
    args.push(`--status=${filters.status.join(',')}`)
  }
  if (filters?.type?.length) {
    args.push(`--type=${filters.type.join(',')}`)
  }
  if (filters?.priority?.length) {
    args.push(`--priority=${filters.priority.join(',')}`)
  }
  if (filters?.assignee) {
    args.push(`--assignee=${filters.assignee}`)
  }

  return executeBr('list', { args, cwd })
}

/**
 * Execute br show command
 */
export async function brShow(id: string, cwd?: string) {
  return executeBr('show', { args: [id], cwd })
}

/**
 * Execute br create command
 */
/**
 * Escape double quotes in a string for shell arguments
 */
function escapeQuotes(str: string): string {
  return str.replace(/"/g, '\\"')
}

export async function brCreate(
  title: string,
  options?: {
    description?: string
    type?: string
    priority?: string
    assignee?: string
    labels?: string[]
    externalRef?: string
    estimate?: number
    design?: string
    acceptance?: string
    notes?: string
  },
  cwd?: string
) {
  const args: string[] = [`"${title}"`]

  if (options?.description) {
    args.push(`--description="${escapeQuotes(options.description)}"`)
  }
  if (options?.type) {
    args.push(`--type=${options.type}`)
  }
  if (options?.priority) {
    args.push(`--priority=${options.priority}`)
  }
  if (options?.assignee) {
    args.push(`--assignee=${options.assignee}`)
  }
  if (options?.labels?.length) {
    args.push(`--labels=${options.labels.join(',')}`)
  }
  if (options?.externalRef) {
    args.push(`--external-ref="${escapeQuotes(options.externalRef)}"`)
  }
  if (options?.estimate) {
    args.push(`--estimate=${options.estimate}`)
  }
  if (options?.design) {
    args.push(`--design="${escapeQuotes(options.design)}"`)
  }
  if (options?.acceptance) {
    args.push(`--acceptance="${escapeQuotes(options.acceptance)}"`)
  }
  if (options?.notes) {
    args.push(`--notes="${escapeQuotes(options.notes)}"`)
  }

  return executeBr('create', { args, cwd })
}

/**
 * Execute br update command
 */
export async function brUpdate(
  id: string,
  updates: {
    title?: string
    description?: string
    type?: string
    status?: string
    priority?: string
    assignee?: string
    labels?: string[]
    externalRef?: string
    estimate?: number
    design?: string
    acceptance?: string
    notes?: string
  },
  cwd?: string
) {
  const args: string[] = [id]

  if (updates.title) {
    args.push(`--title="${escapeQuotes(updates.title)}"`)
  }
  if (updates.description) {
    args.push(`--description="${escapeQuotes(updates.description)}"`)
  }
  if (updates.type) {
    args.push(`--type=${updates.type}`)
  }
  if (updates.status) {
    args.push(`--status=${updates.status}`)
  }
  if (updates.priority) {
    args.push(`--priority=${updates.priority}`)
  }
  if (updates.assignee) {
    args.push(`--assignee=${updates.assignee}`)
  }
  if (updates.labels?.length) {
    args.push(`--labels=${updates.labels.join(',')}`)
  }
  if (updates.externalRef) {
    args.push(`--external-ref="${escapeQuotes(updates.externalRef)}"`)
  }
  if (updates.estimate) {
    args.push(`--estimate=${updates.estimate}`)
  }
  if (updates.design) {
    args.push(`--design="${escapeQuotes(updates.design)}"`)
  }
  if (updates.acceptance) {
    args.push(`--acceptance="${escapeQuotes(updates.acceptance)}"`)
  }
  if (updates.notes) {
    args.push(`--notes="${escapeQuotes(updates.notes)}"`)
  }

  return executeBr('update', { args, cwd })
}

/**
 * Execute br close command
 */
export async function brClose(id: string, cwd?: string) {
  return executeBr('close', { args: [id], cwd })
}

/**
 * Execute br status command (dashboard stats)
 */
export async function brStatus(cwd?: string) {
  return executeBr('status', { cwd })
}

/**
 * Execute br count command (grouped counts)
 */
export async function brCount(cwd?: string) {
  return executeBr('count', { cwd })
}

/**
 * Execute br ready command (available work)
 */
export async function brReady(cwd?: string) {
  return executeBr('ready', { cwd })
}

/**
 * Execute br delete command
 */
export async function brDelete(id: string, cwd?: string) {
  return executeBr('delete', { args: [id, '--force'], cwd })
}

/**
 * Execute br comments add command
 */
export async function brCommentsAdd(id: string, content: string, cwd?: string) {
  // Escape double quotes in content
  const escapedContent = content.replace(/"/g, '\\"')
  return executeBr('comments add', { args: [id, `"${escapedContent}"`], cwd })
}
