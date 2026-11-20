import { brReady, unwrapBrEnvelope } from '../../utils/br-executor'
import { transformIssue } from '../../utils/br-transformers'
import type { BrRawIssue } from '../../utils/br-transformers'

export default defineEventHandler(async (event) => {
  const query = getQuery(event)
  const cwd = query.path ? String(query.path) : undefined

  const result = await brReady(cwd)

  if (!result.success) {
    throw createError({
      statusCode: 500,
      message: result.error || 'Failed to get ready issues',
    })
  }

  const issues = unwrapBrEnvelope<BrRawIssue>(result.data).map(transformIssue)

  return issues
})
