export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const gameId = getRouterParam(event, 'id')

  if (!gameId) {
    throw createError({
      statusCode: 400,
      statusMessage: 'Missing game id.',
    })
  }

  const { deck } = await effectiveDeckForUser(event, session.user.id)
  const response = await callGameRoom(event, gameId, {
    type: 'startGame',
    actorUserId: session.user.id,
    deckList: deck,
  })

  await updatePublicRoom(event, response)

  return response
})
