import type { GameRoomRequest } from '../../../../shared/game-room'

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
    action?: GameRoomRequest extends { type: 'submitCommand'; action: infer Action } ? Action : never
  }>(event)

  if (!body.action) {
    throw createError({
      statusCode: 400,
      statusMessage: 'Missing command action.',
    })
  }

  return await callGameRoom(event, gameId, {
    type: 'submitCommand',
    commandId: body.commandId?.trim() || crypto.randomUUID(),
    actorUserId: session.user.id,
    action: body.action,
  })
})
