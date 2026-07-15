export default defineEventHandler(async (event) => {
  requireDevelopment(event)
  const session = await requireSession(event)
  const gameId = getRouterParam(event, 'id')
  const commandId = getQuery(event).commandId

  if (!gameId) {
    throw createError({ statusCode: 400, statusMessage: 'Missing game id.' })
  }
  if (typeof commandId !== 'string' || !commandId.trim()) {
    throw createError({ statusCode: 400, statusMessage: 'Missing command id.' })
  }

  return await callGameRoom(event, gameId, {
    type: 'inspectDevelopmentRecord',
    actorUserId: session.user.id,
    commandId: commandId.trim(),
  })
})
