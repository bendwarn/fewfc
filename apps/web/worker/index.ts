import nuxtWorker from '../.output/server/index.mjs'
import { GameRoom } from './durable-objects/game-room'
import { PlayerNotifications } from './durable-objects/player-notifications'
import { ReplayArchive } from './durable-objects/replay-archive'
import {
  legacyReplayDeleteStatements,
  resetLegacyActiveRoomStatusStatement,
} from './legacy-purge'
import { maintenanceBlocksPlayerMutation, maintenanceEnabled } from './maintenance'
import {
  callPersonalDeckResolution,
  callRuleModuleResolution,
  callRulesCatalog,
} from './rules-engine'

interface WorkerEnv {
  DB: D1Database
  GAME_ROOM: DurableObjectNamespace
  PLAYER_NOTIFICATIONS: DurableObjectNamespace
  REPLAY: DurableObjectNamespace
  APP_ENV: 'development' | 'staging' | 'production'
  /** Explicit deployment-time gate for the one-way legacy cutover. */
  MAINTENANCE_MODE?: string
  /** Dedicated secret for the legacy-purge management endpoints. */
  LEGACY_PURGE_SECRET?: string
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

function managementAuthorized(request: Request, env: WorkerEnv): boolean {
  if (!maintenanceEnabled(env.MAINTENANCE_MODE) || !env.LEGACY_PURGE_SECRET) return false
  const supplied = request.headers.get('x-fewfc-legacy-purge-secret')
  if (!supplied) return false
  const expectedBytes = new TextEncoder().encode(env.LEGACY_PURGE_SECRET)
  const suppliedBytes = new TextEncoder().encode(supplied)
  // `timingSafeEqual` requires equal lengths. Still perform a constant-time
  // comparison on mismatched input so an unauthorised caller cannot use the
  // response timing to learn the secret length.
  const lengthsMatch = expectedBytes.byteLength === suppliedBytes.byteLength
  return lengthsMatch
    ? crypto.subtle.timingSafeEqual(expectedBytes, suppliedBytes)
    : !crypto.subtle.timingSafeEqual(suppliedBytes, suppliedBytes)
}

async function managementResponse(
  request: Request,
  env: WorkerEnv,
  url: URL,
): Promise<Response | null> {
  if (!url.pathname.startsWith('/internal/legacy-purge/')) return null
  // Return 404 for both a disabled gate and a bad secret so this protected
  // control surface is not discoverable during normal player traffic.
  if (!managementAuthorized(request, env)) {
    return Response.json({ error: 'not found' }, { status: 404 })
  }
  if (request.method !== 'POST') {
    return Response.json({ error: 'method not allowed' }, { status: 405 })
  }

  const body = await request.json() as {
    epoch?: unknown
    roomId?: unknown
    replayId?: unknown
    objectId?: unknown
  }
  if (typeof body.epoch !== 'string' || !body.epoch.trim()) {
    return Response.json({ error: 'a purge epoch is required' }, { status: 400 })
  }
  const epoch = body.epoch.trim()

  if (url.pathname === '/internal/legacy-purge/room') {
    const hasRoomId = typeof body.roomId === 'string' && body.roomId.trim()
    const hasObjectId = typeof body.objectId === 'string' && body.objectId.trim()
    if (Boolean(hasRoomId) === Boolean(hasObjectId)) {
      return Response.json({ error: 'provide exactly one roomId or objectId' }, { status: 400 })
    }
    const id = hasRoomId
      ? env.GAME_ROOM.idFromName(body.roomId as string)
      : env.GAME_ROOM.idFromString(body.objectId as string)
    return await env.GAME_ROOM.get(id).fetch('https://game-room.internal/manage/purge-legacy', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ epoch }),
    })
  }

  if (url.pathname === '/internal/legacy-purge/replay') {
    const hasObjectId = typeof body.objectId === 'string' && body.objectId.trim()
    if (!hasObjectId) return Response.json({ error: 'an objectId is required' }, { status: 400 })
    const id = env.REPLAY.idFromString(body.objectId as string)
    return await env.REPLAY.get(id).fetch('https://replay.internal/manage/purge-legacy', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ epoch }),
    })
  }

  if (url.pathname === '/internal/legacy-purge/room-verify') {
    const hasObjectId = typeof body.objectId === 'string' && body.objectId.trim()
    if (!hasObjectId) return Response.json({ error: 'an objectId is required' }, { status: 400 })
    const id = env.GAME_ROOM.idFromString(body.objectId as string)
    return await env.GAME_ROOM.get(id).fetch('https://game-room.internal/manage/verify-legacy', {
      method: 'POST',
    })
  }

  if (url.pathname === '/internal/legacy-purge/replay-verify') {
    const hasObjectId = typeof body.objectId === 'string' && body.objectId.trim()
    if (!hasObjectId) return Response.json({ error: 'an objectId is required' }, { status: 400 })
    const id = env.REPLAY.idFromString(body.objectId as string)
    return await env.REPLAY.get(id).fetch('https://replay.internal/manage/verify-legacy', {
      method: 'POST',
    })
  }

  if (url.pathname === '/internal/legacy-purge/replay-sample') {
    const requestedReplayId = typeof body.replayId === 'string' ? body.replayId : undefined
    if (!requestedReplayId?.trim()) {
      return Response.json({ error: 'a replayId is required' }, { status: 400 })
    }
    const id = env.REPLAY.idFromName(requestedReplayId.trim())
    const response = await env.REPLAY.get(id).fetch('https://replay.internal/?step=0')
    return Response.json({ replayId: requestedReplayId, status: response.status })
  }

  if (url.pathname === '/internal/legacy-purge/replay-index') {
    const results = await env.DB.batch([
      ...legacyReplayDeleteStatements.map(statement => env.DB.prepare(statement)),
    ])
    return Response.json({
      playerSavedReplayDeleted: results[0]?.meta.changes ?? 0,
      replayArchiveLifecycleDeleted: results[1]?.meta.changes ?? 0,
    })
  }

  if (url.pathname === '/internal/legacy-purge/room-index') {
    const result = await env.DB.prepare(resetLegacyActiveRoomStatusStatement).run()
    return Response.json({ roomIndexUpdated: result.meta.changes ?? 0 })
  }

  return Response.json({ error: 'not found' }, { status: 404 })
}

export default {
  async fetch(request: Request, env: WorkerEnv, context: ExecutionContext): Promise<Response> {
    if (!['development', 'staging', 'production'].includes(env.APP_ENV)) {
      return Response.json(
        { error: 'APP_ENV must be development, staging, or production.' },
        { status: 500 },
      )
    }

    globalThis.__fewfcRulesEngine__ = {
      catalog: callRulesCatalog,
      resolvePersonalDeck: callPersonalDeckResolution,
      resolveRuleModules: callRuleModuleResolution,
    }

    const url = new URL(request.url)
    const management = await managementResponse(request, env, url)
    if (management) return management

    if (
      maintenanceEnabled(env.MAINTENANCE_MODE)
      && maintenanceBlocksPlayerMutation(request.method, url.pathname)
    ) {
      return Response.json({ error: 'maintenance is in progress', code: 'maintenanceMode' }, { status: 503 })
    }

    const socket = await websocketResponse(request, env, context, url)

    if (socket) {
      return socket
    }

    if (request.method === 'GET' && url.pathname === '/api/rules/catalog') {
      return Response.json(await callRulesCatalog())
    }

    return await nuxtWorker.fetch(request, env, context)
  },
}

export { GameRoom, PlayerNotifications, ReplayArchive }
