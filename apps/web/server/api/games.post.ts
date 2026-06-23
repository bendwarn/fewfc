import type { PlayerId, ViewerId } from '../../app/types/fewfc'

function playerList(value: unknown): PlayerId[] | undefined {
  if (!Array.isArray(value)) {
    return undefined
  }

  const players = value.filter((player): player is PlayerId => player === 'alice' || player === 'bob')

  return players.length > 0 ? players : undefined
}

export default defineEventHandler(async (event) => {
  const body = await readBody<{
    gameId?: string
    players?: unknown
    viewer?: ViewerId
  }>(event)
  const gameId = body.gameId?.trim() || crypto.randomUUID()

  return await callGameRoom(event, gameId, {
    type: 'createGame',
    gameId,
    viewer: body.viewer,
    players: playerList(body.players),
  })
})
