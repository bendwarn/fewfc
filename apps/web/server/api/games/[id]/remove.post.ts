export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const gameId = getRouterParam(event, 'id')
  const body = await readBody<{ userId?: string }>(event)

  if (!gameId || !body.userId) {
    throw createError({ statusCode: 400, statusMessage: 'Missing game id or player.' })
  }

  const response = await callGameRoom(event, gameId, {
    type: 'removePlayer',
    actorUserId: session.user.id,
    targetUserId: body.userId,
  })

  await updatePublicRoom(event, response)
  return response
})
