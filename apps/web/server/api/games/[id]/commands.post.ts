import type { GameRoomRequest, PlayableActionsResponse } from '../../../../shared/game-room'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const gameId = getRouterParam(event, 'id')

  if (!gameId) {
    throw createError({
      statusCode: 400,
      statusMessage: 'Missing game id.',
    })
  }

  const body = await readBody<{
    commandId?: string
    gameInstanceId?: string
    transactionId?: string
    action?: Extract<GameRoomRequest, { type: 'submitCommand' }>['action']
  }>(event)

  if (!body.action) {
    throw createError({
      statusCode: 400,
      statusMessage: 'Missing command action.',
    })
  }
  if (!body.commandId?.trim() || !body.gameInstanceId?.trim() || !body.transactionId?.trim()) {
    throw createError({
      statusCode: 400,
      statusMessage: 'Missing command identity.',
    })
  }

  const request: GameRoomRequest = {
    type: 'submitCommand',
    commandId: body.commandId.trim(),
    gameInstanceId: body.gameInstanceId.trim(),
    transactionId: body.transactionId.trim(),
    actorUserId: session.user.id,
    action: body.action,
  }

  if (body.action.type === 'playableActions') {
    return await callGameRoom<PlayableActionsResponse>(event, gameId, request)
  }

  const response = await callGameRoom(event, gameId, request)

  await updatePublicRoom(event, response)

  return response
})
