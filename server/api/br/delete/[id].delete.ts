import { brDelete } from '../../../utils/br-executor'

export default defineEventHandler(async (event) => {
  const id = getRouterParam(event, 'id')
  const query = getQuery(event)

  if (!id) {
    throw createError({
      statusCode: 400,
      message: 'Issue ID is required',
    })
  }

  const cwd = query.path ? String(query.path) : undefined

  const result = await brDelete(id, cwd)

  if (!result.success) {
    throw createError({
      statusCode: 500,
      message: result.error || 'Failed to delete issue',
    })
  }

  return { success: true, id }
})
