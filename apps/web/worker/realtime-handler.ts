import { defineWebSocketHandler, H3Event } from 'h3'
import crossws from 'crossws/adapters/cloudflare'
import type { Hooks, Peer } from 'crossws'
import type { WorkerEnv } from '../server/utils/worker-env'
import type { SocketResult } from '../shared/realtime'

export interface RealtimeIdentity {
  userId: string
  userName: string
  sessionId: string
}

/** CrossWS 的 websocket 是代理；Cloudflare 平台方法必須以原生 socket 作為 this。 */
export function durableSocket(ctx: DurableObjectState, peer: Peer): WebSocket {
  const socket = ctx.getWebSockets().find(socket => socket.deserializeAttachment<{ i?: string }>()?.i === peer.id)
  if (!socket) throw new Error('WebSocket is no longer attached')
  return socket
}

export function attachRealtimeIdentity(socket: WebSocket, request: Request) {
  socket.serializeAttachment({
    ...socket.deserializeAttachment<Record<string, unknown>>(),
    userId: request.headers.get('x-fewfc-user-id') ?? '',
    userName: decodeURIComponent(request.headers.get('x-fewfc-user-name') ?? ''),
    sessionId: request.headers.get('x-fewfc-session-id') ?? '',
  })
}

export async function requireRealtimeIdentity(socket: WebSocket, database: D1Database): Promise<RealtimeIdentity> {
  const identity = socket.deserializeAttachment<RealtimeIdentity>()
  const session = identity?.sessionId && await database.prepare(
    'SELECT id FROM session WHERE id = ? AND user_id = ? AND expires_at > ?',
  ).bind(identity.sessionId, identity.userId, Math.floor(Date.now() / 1000)).first()
  if (!session) throw { statusCode: 401, code: 'sessionExpired', message: '登入已失效，請重新登入。' }
  return identity
}

/** 使用 H3／Nitro 相同的 handler 契約，交由 Cloudflare adapter 保留 DO 休眠能力。 */
export function durableWebSocketHandler(hooks: Partial<Hooks>) {
  const handler = defineWebSocketHandler(hooks)
  return crossws({
    async resolve(request) {
      const response = await handler(new H3Event(request))
      return response.crossws
    },
  })
}

/** 共用既有資料存取邏輯時明確傳入 bindings，不依賴另一個請求留下的全域環境。 */
export function realtimeEvent(env: Pick<WorkerEnv, 'DB' | 'APP_ENV' | 'PLAYER_NOTIFICATIONS'>) {
  const event = new H3Event(new Request('https://realtime.internal/'))
  event.context._platform = { cloudflare: { env } }
  return event
}

export function socketFailure(requestId: string, error: unknown): SocketResult {
  const detail = error as { statusCode?: number; status?: number; data?: { code?: string }; code?: string; message?: string }
  const statusCode = detail?.statusCode ?? detail?.status ?? 500
  return {
    type: 'result', requestId, ok: false,
    error: {
      statusCode,
      code: detail?.data?.code ?? detail?.code ?? 'realtimeFailure',
      message: statusCode < 500 && typeof detail?.message === 'string' ? detail.message : '連線服務暫時無法處理要求，請稍後再試。',
    },
  }
}

export async function socketResponse(requestId: string, response: Response): Promise<SocketResult> {
  const data = await response.json() as Record<string, unknown>
  if (!response.ok) return socketFailure(requestId, { statusCode: response.status, code: data.code, message: data.error })
  // 完整收據含有私人指令內容；公開回覆只需 requestId 與已完成的結果。
  if ('receipt' in data) delete data.receipt
  return { type: 'result', requestId, ok: true, data }
}
