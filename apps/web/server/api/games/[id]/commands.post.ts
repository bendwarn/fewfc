import type { GameRoomRequest } from '../../../../shared/game-room'
import type { ViewerId } from '../../../../app/types/fewfc'

export default defineEventHandler(async (event) => {
  const gameId = getRouterParam(event, 'id')

  if (!gameId) {
    throw createError({
      statusCode: 400,
      statusMessage: 'Missing game id.',
    })
  }

  const body = await readBody<{
    commandId?: string
    viewer?: ViewerId
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
    viewer: body.viewer,
    action: body.action,
  })
})
