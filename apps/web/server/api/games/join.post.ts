export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const body = await readBody<{ code?: string }>(event)
  const roomCode = body.code?.trim().toUpperCase()

  if (!roomCode) {
    throw createError({
      statusCode: 400,
      statusMessage: 'Missing room code.',
    })
  }

  const gameId = await gameIdForRoomCode(event, roomCode)
  if (!gameId) {
    throw createError({
      statusCode: 404,
      statusMessage: 'Room not found.',
    })
  }

  const response = await callGameRoom(event, gameId, {
    type: 'joinGame',
    actorUserId: session.user.id,
    actorName: session.user.name,
    credential: {
      type: 'code',
      value: roomCode,
    },
  })

  await updatePublicRoom(event, response)

  return response
})
