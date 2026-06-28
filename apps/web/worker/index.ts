import nuxtWorker from '../.output/server/index.mjs'
import { GameRoom } from './durable-objects/game-room'
import { PlayerNotifications } from './durable-objects/player-notifications'

interface WorkerEnv {
  GAME_ROOM: DurableObjectNamespace
  PLAYER_NOTIFICATIONS: DurableObjectNamespace
  APP_ENV: 'development' | 'staging' | 'production'
}

interface AuthSessionResponse {
  user?: {
    id?: string
    name?: string
  }
}

async function authenticatedUser(
  request: Request,
  env: WorkerEnv,
  context: ExecutionContext,
): Promise<{ id: string; name: string } | null> {
  const headers = new Headers()
  const cookie = request.headers.get('cookie')

  if (cookie) {
    headers.set('cookie', cookie)
  }

  const response = await nuxtWorker.fetch(new Request(
    new URL('/api/auth/get-session', request.url),
    { headers },
  ), env, context)

  if (!response.ok) {
    return null
  }

  const session = await response.json() as AuthSessionResponse

  if (!session.user?.id) {
    return null
  }

  return {
    id: session.user.id,
    name: session.user.name || '玩家',
  }
}

async function websocketResponse(
  request: Request,
  env: WorkerEnv,
  context: ExecutionContext,
  url: URL,
): Promise<Response | null> {
  if (request.headers.get('Upgrade')?.toLowerCase() !== 'websocket') {
    return null
  }

  const gameMatch = url.pathname.match(/^\/api\/games\/([^/]+)\/socket$/)
  const notifications = url.pathname === '/api/notifications/socket'

  if (!gameMatch && !notifications) {
    return null
  }

  const user = await authenticatedUser(request, env, context)

  if (!user) {
    return Response.json({ error: 'authentication required' }, { status: 401 })
  }

  if (notifications) {
    const id = env.PLAYER_NOTIFICATIONS.idFromName('global')
    return await env.PLAYER_NOTIFICATIONS.get(id).fetch(
      'https://player-notifications.internal/socket',
      {
        headers: {
          Upgrade: 'websocket',
          'x-fewfc-user-id': user.id,
        },
      },
    )
  }

  const gameId = decodeURIComponent(gameMatch?.[1] ?? '')
  const id = env.GAME_ROOM.idFromName(gameId)

  return await env.GAME_ROOM.get(id).fetch('https://game-room.internal/socket', {
    headers: {
      Upgrade: 'websocket',
      'x-fewfc-user-id': user.id,
      'x-fewfc-user-name': encodeURIComponent(user.name),
    },
  })
}

export default {
  async fetch(request: Request, env: WorkerEnv, context: ExecutionContext): Promise<Response> {
    if (!['development', 'staging', 'production'].includes(env.APP_ENV)) {
      return Response.json(
        { error: 'APP_ENV must be development, staging, or production.' },
        { status: 500 },
      )
    }

    const socket = await websocketResponse(request, env, context, new URL(request.url))

    if (socket) {
      return socket
    }

    return await nuxtWorker.fetch(request, env, context)
  },
}

export { GameRoom, PlayerNotifications }
