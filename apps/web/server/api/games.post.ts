import type { PlayerId } from '../../app/types/fewfc'

function playerList(value: unknown): PlayerId[] | undefined {
  if (!Array.isArray(value)) {
    return undefined
  }

  const players = value.filter((player): player is PlayerId => player === 'alice' || player === 'bob')

  return players.length > 0 ? players : undefined
}

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const body = await readBody<{
    gameId?: string
    players?: unknown
  }>(event)
  const gameId = body.gameId?.trim() || crypto.randomUUID()

  return await callGameRoom(event, gameId, {
    type: 'createGame',
    gameId,
    actorUserId: session.user.id,
    players: playerList(body.players),
  })
})
