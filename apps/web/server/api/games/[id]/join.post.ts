export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const gameId = getRouterParam(event, 'id')
  const body = await readBody<{ invite?: string }>(event)

  if (!gameId) {
    throw createError({
      statusCode: 400,
      statusMessage: 'Missing game id.',
    })
  }

  const response = await callGameRoom(event, gameId, {
    type: 'joinGame',
    actorUserId: session.user.id,
    actorName: session.user.name,
    credential: body.invite
      ? { type: 'token', value: body.invite }
      : undefined,
  })

  await updatePublicRoom(event, response)

  return response
})
