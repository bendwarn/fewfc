import type { PlayerId } from '../../app/types/fewfc'
import type { GameRoomAccess } from '../../shared/game-room'

function playerList(value: unknown): PlayerId[] | undefined {
  if (!Array.isArray(value)) {
    return undefined
  }

  const players = value.filter((player): player is PlayerId => player === 'alice' || player === 'bob')

  return players.length > 0 ? players : undefined
}

function roomAccess(value: unknown): GameRoomAccess {
  return value === 'public' ? 'public' : 'private'
}

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const body = await readBody<{
    gameId?: string
    access?: unknown
    players?: unknown
  }>(event)
  const gameId = body.gameId?.trim() || crypto.randomUUID()

  return await callGameRoom(event, gameId, {
    type: 'createGame',
    gameId,
    actorUserId: session.user.id,
    access: roomAccess(body.access),
    players: playerList(body.players),
  })
})
