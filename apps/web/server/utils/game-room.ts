import { createError, type H3Event } from 'h3'
import type { GameRoomRequest, GameRoomResponse } from '../../shared/game-room'

interface WorkerEnv {
  GAME_ROOM: DurableObjectNamespace
}

declare global {
  // Nitro's Cloudflare module stores the Worker env here while handling requests.
  // eslint-disable-next-line no-var
  var __env__: WorkerEnv | undefined
}

function workerEnv(event: H3Event): WorkerEnv {
  const platformEnv = (
    event.context as {
      _platform?: {
        cloudflare?: {
          env?: WorkerEnv
        }
      }
    }
  )._platform?.cloudflare?.env
  const env = platformEnv ?? globalThis.__env__

  if (!env?.GAME_ROOM) {
    throw createError({
      statusCode: 501,
      statusMessage: 'GameRoom Durable Object is only available through Wrangler/Cloudflare.',
    })
  }

  return env
}

export async function callGameRoom(
  event: H3Event,
  gameId: string,
  body: GameRoomRequest,
): Promise<GameRoomResponse> {
  const env = workerEnv(event)
  const objectId = env.GAME_ROOM.idFromName(gameId)
  const room = env.GAME_ROOM.get(objectId)
  const response = await room.fetch('https://game-room.internal/', {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
    },
    body: JSON.stringify(body),
  })

  if (!response.ok) {
    throw createError({
      statusCode: response.status,
      statusMessage: await response.text(),
    })
  }

  return (await response.json()) as GameRoomResponse
}
