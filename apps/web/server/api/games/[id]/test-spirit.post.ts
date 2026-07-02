export default defineEventHandler(async (event) => {
  requireDevelopment(event)
  const session = await requireSession(event)
  const gameId = getRouterParam(event, 'id')

  if (!gameId) {
    throw createError({ statusCode: 400, statusMessage: 'Missing game id.' })
  }

  const spirit = getQuery(event).spirit === 'Fire' ? 'Fire' : 'Metal'
  return await callGameRoom(event, gameId, {
    type: 'seedSpiritFixture',
    actorUserId: session.user.id,
    spirit,
  })
})
