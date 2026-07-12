import { createError, type H3Event } from 'h3'
import type { GameRoomRequest, GameRoomResponse } from '../../shared/game-room'
import { workerEnv, type DurableObjectNamespaceBinding } from './worker-env'

function gameRoomNamespace(event: H3Event): DurableObjectNamespaceBinding {
  const namespace = workerEnv(event).GAME_ROOM

  if (!namespace) {
    throw createError({
      statusCode: 501,
      statusMessage: 'GameRoom Durable Object is only available through Wrangler/Cloudflare.',
    })
  }

  return namespace
}

export async function callGameRoom(
  event: H3Event,
  gameId: string,
  body: GameRoomRequest,
): Promise<GameRoomResponse> {
  const namespace = gameRoomNamespace(event)
  const objectId = namespace.idFromName(gameId)
  const room = namespace.get(objectId)
  const response = await room.fetch('https://game-room.internal/', {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
    },
    body: JSON.stringify(body),
  })

  if (!response.ok) {
    const error = await readGameRoomError(response)
    throw createError({
      statusCode: response.status,
      statusMessage: error.message,
      data: { code: error.code },
    })
  }

  return (await response.json()) as GameRoomResponse
}

async function readGameRoomError(response: Response): Promise<{ message: string, code?: string }> {
  try {
    const body = await response.json() as { error?: unknown, code?: unknown }
    if (typeof body.error === 'string' && body.error.trim()) {
      return {
        message: body.error.trim(),
        code: typeof body.code === 'string' ? body.code : undefined,
      }
    }
  } catch {
    // A malformed internal response must not be copied into a public error message.
  }

  return { message: '房間服務暫時無法處理要求，請稍後再試。' }
}
