import { brCreate } from '../../utils/br-executor'
import { transformIssue, priorityToNumber } from '../../utils/br-transformers'

export default defineEventHandler(async (event) => {
  const body = await readBody(event)
  const query = getQuery(event)

  if (!body.title) {
    throw createError({
      statusCode: 400,
      message: 'Title is required',
    })
  }

  const cwd = query.path ? String(query.path) : undefined

  // Convert priority from "p3" format to "3" for br CLI
  const priority = body.priority ? priorityToNumber(body.priority) : undefined

  const result = await brCreate(
    body.title,
    {
      description: body.description,
      type: body.type,
      priority,
      assignee: body.assignee,
      labels: body.labels,
      externalRef: body.externalRef,
      estimate: body.estimateMinutes,
      design: body.designNotes,
      acceptance: body.acceptanceCriteria,
      notes: body.workingNotes,
    },
    cwd
  )

  if (!result.success) {
    throw createError({
      statusCode: 500,
      message: result.error || 'Failed to create issue',
    })
  }

  // Transform br CLI response to match Issue type interface
  return result.data ? transformIssue(result.data as Parameters<typeof transformIssue>[0]) : null
})
