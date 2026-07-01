export default defineEventHandler(async (event) => {
  requireDevelopment(event)
  const session = await requireSession(event)
  const gameId = getRouterParam(event, 'id')

  if (!gameId) {
    throw createError({ statusCode: 400, statusMessage: 'Missing game id.' })
  }

  return await callGameRoom(event, gameId, {
    type: 'seedEndgameFixture',
    actorUserId: session.user.id,
  })
})
