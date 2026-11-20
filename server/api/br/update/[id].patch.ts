import { brUpdate } from '../../../utils/br-executor'
import { transformIssue, priorityToNumber } from '../../../utils/br-transformers'

export default defineEventHandler(async (event) => {
  const id = getRouterParam(event, 'id')
  const body = await readBody(event)
  const query = getQuery(event)

  if (!id) {
    throw createError({
      statusCode: 400,
      message: 'Issue ID is required',
    })
  }

  const cwd = query.path ? String(query.path) : undefined

  // Convert priority from "p3" format to "3" for br CLI
  const priority = body.priority ? priorityToNumber(body.priority) : undefined

  const result = await brUpdate(
    id,
    {
      title: body.title,
      description: body.description,
      type: body.type,
      status: body.status,
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
      message: result.error || 'Failed to update issue',
    })
  }

  // update returns an array, extract first element
  const issueData = Array.isArray(result.data) ? result.data[0] : result.data

  // Transform br CLI response to match Issue type interface
  return issueData ? transformIssue(issueData as Parameters<typeof transformIssue>[0]) : null
})
