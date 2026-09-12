import nuxtWorker from '../.output/server/index.mjs'
import { GameRoom } from './durable-objects/game-room'
import { PlayerNotifications } from './durable-objects/player-notifications'
import { ReplayArchive } from './durable-objects/replay-archive'
import { legacyReplayDeleteStatements } from './legacy-purge'
import { managementAuthorized } from './management-auth'
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
  /** 由 Cloudflare Secret 明確控制的維護閘門。 */
  MAINTENANCE_MODE?: string
  /** 從部署帳戶設定注入，僅用來驗證 Wrangler token 的帳戶範圍。 */
  CLOUDFLARE_ACCOUNT_ID?: string
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

async function managementResponse(
  request: Request,
  env: WorkerEnv,
  url: URL,
): Promise<Response | null> {
  if (!url.pathname.startsWith('/internal/legacy-purge/')) return null
  // 對停用的閘門與錯誤密鑰都回傳 404，讓這個受保護的控制介面不會在一般
  // 玩家流量中被發現。
  if (!await managementAuthorized(request, env)) {
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
    sourceGameId?: unknown
    sourceGameIds?: unknown
    replayIds?: unknown
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

  if (url.pathname === '/internal/legacy-purge/room-probe') {
    const hasRoomId = typeof body.roomId === 'string' && body.roomId.trim()
    const hasObjectId = typeof body.objectId === 'string' && body.objectId.trim()
    if (Boolean(hasRoomId) === Boolean(hasObjectId)) {
      return Response.json({ error: 'provide exactly one roomId or objectId' }, { status: 400 })
    }
    const id = hasRoomId
      ? env.GAME_ROOM.idFromName(body.roomId as string)
      : env.GAME_ROOM.idFromString(body.objectId as string)
    return await env.GAME_ROOM.get(id).fetch('https://game-room.internal/manage/probe-legacy', {
      method: 'POST',
    })
  }

  if (url.pathname === '/internal/legacy-purge/replay') {
    const hasObjectId = typeof body.objectId === 'string' && body.objectId.trim()
    if (!hasObjectId) return Response.json({ error: 'an objectId is required' }, { status: 400 })
    const sourceGameId = typeof body.sourceGameId === 'string' ? body.sourceGameId.trim() : ''
    if (!sourceGameId) return Response.json({ error: 'a sourceGameId is required' }, { status: 400 })
    const id = env.REPLAY.idFromString(body.objectId as string)
    return await env.REPLAY.get(id).fetch('https://replay.internal/manage/purge-legacy', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ epoch, sourceGameId }),
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

  if (url.pathname === '/internal/legacy-purge/replay-probe') {
    const hasObjectId = typeof body.objectId === 'string' && body.objectId.trim()
    if (!hasObjectId) return Response.json({ error: 'an objectId is required' }, { status: 400 })
    const id = env.REPLAY.idFromString(body.objectId as string)
    return await env.REPLAY.get(id).fetch('https://replay.internal/manage/probe-legacy', {
      method: 'POST',
    })
  }

  // 先由權威 GameRoom 判定 404，再清理對應的索引列；可回應房間不做任何變更。
  if (url.pathname === '/internal/legacy-purge/room-index') {
    const roomId = typeof body.roomId === 'string' ? body.roomId.trim() : ''
    if (!roomId) return Response.json({ error: 'a roomId is required' }, { status: 400 })
    const room = env.GAME_ROOM.get(env.GAME_ROOM.idFromName(roomId))
    const roomResponse = await room.fetch('https://game-room.internal/manage/purge-legacy', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ epoch }),
    })
    const roomResult = await roomResponse.json() as { status?: unknown }
    const removable = (roomResponse.status === 404 && roomResult.status === 'absent')
      || (roomResponse.status === 500 && roomResult.status === 'broken')
    if (!removable) {
      if (roomResponse.ok && roomResult.status === 'preserved') {
        return Response.json({ status: 'preserved', roomIndexDeleted: 0 })
      }
      return Response.json({ error: 'room management response was unreadable' }, { status: 502 })
    }
    const results = await env.DB.batch([
      env.DB.prepare('DELETE FROM public_game_room WHERE game_id = ?').bind(roomId),
      env.DB.prepare('DELETE FROM game_room_member WHERE game_id = ?').bind(roomId),
    ])
    return Response.json({ status: roomResult.status, roomIndexDeleted: results[0]?.meta.changes ?? 0 })
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
    const sourceGameIds = Array.isArray(body.sourceGameIds)
      ? body.sourceGameIds.filter((value): value is string => typeof value === 'string' && value.trim())
      : []
    const replayIds = Array.isArray(body.replayIds)
      ? body.replayIds.filter((value): value is string => typeof value === 'string' && value.trim())
      : []
    const statements = legacyReplayDeleteStatements(sourceGameIds, replayIds)
    if (statements.length === 0) {
      return Response.json({ playerSavedReplayDeleted: 0, replayArchiveLifecycleDeleted: 0 })
    }
    const results = await env.DB.batch(statements.map(statement => (
      env.DB.prepare(statement.sql).bind(...statement.bindings)
    )))
    return Response.json({
      playerSavedReplayDeleted: results[0]?.meta.changes ?? 0,
      replayArchiveLifecycleDeleted: results[1]?.meta.changes ?? 0,
    })
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
