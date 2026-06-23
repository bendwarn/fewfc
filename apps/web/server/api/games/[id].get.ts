import type { ViewerId } from '../../../app/types/fewfc'

export default defineEventHandler(async (event) => {
  const gameId = getRouterParam(event, 'id')

  if (!gameId) {
    throw createError({
      statusCode: 400,
      statusMessage: 'Missing game id.',
    })
  }

  const query = getQuery(event)
  const viewer = typeof query.viewer === 'string' ? (query.viewer as ViewerId) : undefined

  return await callGameRoom(event, gameId, {
    type: 'getState',
    viewer,
  })
})
