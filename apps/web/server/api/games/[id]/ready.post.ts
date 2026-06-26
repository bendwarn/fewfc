export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const gameId = getRouterParam(event, 'id')

  if (!gameId) {
    throw createError({
      statusCode: 400,
      statusMessage: 'Missing game id.',
    })
  }

  return await callGameRoom(event, gameId, {
    type: 'readyGame',
    actorUserId: session.user.id,
  })
})
