import { brList, unwrapBrEnvelope } from '../../utils/br-executor'
import { transformIssue, priorityToNumber } from '../../utils/br-transformers'
import type { BrRawIssue } from '../../utils/br-transformers'

export default defineEventHandler(async (event) => {
  const query = getQuery(event)

  // Convert priority from "p3" format to "3" format for br CLI
  const priorityFilters = query.priority
    ? String(query.priority).split(',').map(priorityToNumber)
    : undefined

  const filters = {
    status: query.status ? String(query.status).split(',') : undefined,
    type: query.type ? String(query.type).split(',') : undefined,
    priority: priorityFilters,
    assignee: query.assignee ? String(query.assignee) : undefined,
    includeAll: query.all === 'true',
  }

  const cwd = query.path ? String(query.path) : undefined

  const result = await brList(filters, cwd)

  if (!result.success) {
    throw createError({
      statusCode: 500,
      message: result.error || 'Failed to list issues',
    })
  }

  const issues = unwrapBrEnvelope<BrRawIssue>(result.data).map(transformIssue)

  return issues
})
