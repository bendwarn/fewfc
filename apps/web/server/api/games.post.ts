import type { GameRoomAccess, GameRoomCapacity } from '../../shared/game-room'
import { normalizeServerRuleModules } from '../utils/rule-modules'

function roomAccess(value: unknown): GameRoomAccess {
  return value === 'public' ? 'public' : 'private'
}

function roomCapacity(value: unknown): GameRoomCapacity {
  return value === 4 ? 4 : 2
}

function roomCode(): string {
  const alphabet = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789'
  const bytes = new Uint8Array(7)
  crypto.getRandomValues(bytes)

  return [...bytes]
    .map((byte) => alphabet[byte % alphabet.length])
    .join('')
}

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const body = await readBody<{
    name?: string
    access?: unknown
    capacity?: unknown
    enabledRuleModules?: unknown
  }>(event)
  const gameId = crypto.randomUUID()
  const invitation = {
    roomCode: roomCode(),
    inviteToken: crypto.randomUUID(),
  }
  const name = body.name?.trim() || `${session.user.name || '玩家'}的房間`
  const response = await callGameRoom(event, gameId, {
    type: 'createGame',
    gameId,
    actorUserId: session.user.id,
    actorName: session.user.name,
    access: roomAccess(body.access),
    capacity: roomCapacity(body.capacity),
    name,
    enabledRuleModules: normalizeServerRuleModules(body.enabledRuleModules),
    invitation,
  })

  if (!response.metadata.members.some((member) => member.userId === session.user.id)) {
    throw createError({
      statusCode: 409,
      statusMessage: 'Room code collision. Create the room again.',
    })
  }

  await upsertPublicRoom(event, response, {
    name,
  })

  return response
})
