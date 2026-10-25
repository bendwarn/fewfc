import { DurableObject } from 'cloudflare:workers'
import type { PlayerNotification } from '../../shared/game-room'
import type { WorkerEnv } from '../../server/utils/worker-env'
import { parseSocketRequest } from '../../shared/realtime'
import { attachRealtimeIdentity, durableSocket, durableWebSocketHandler, requireRealtimeIdentity, socketFailure } from '../realtime-handler'
import { executeLobbyRequest } from '../lobby-requests'
import { maintenanceEnabled } from '../maintenance'

interface SocketAttachment {
  userId: string
}

interface NotificationEnvelope {
  targetUserId: string
  notification: PlayerNotification
}

const playerNotificationReceiptLimit = 256
const playerNotificationReceiptPrefix = 'playerNotificationReceipts:'

export class PlayerNotifications extends DurableObject<WorkerEnv & { DB: D1Database; MAINTENANCE_MODE?: string }> {
  private notificationOperationTail: Promise<void> = Promise.resolve()
  private readonly fallbackReceipts = new Map<string, string[]>()
  private readonly socketHandler = durableWebSocketHandler({
    open: peer => { attachRealtimeIdentity(durableSocket(this.ctx, peer), peer.request) },
    message: async (peer, message) => {
      const wire = message.text()
      if (wire === 'ping') { peer.send('pong'); return }
      let requestId = 'invalid'
      try {
        const raw = wire.length <= 65_536 ? JSON.parse(wire) : undefined
        if (typeof raw?.requestId === 'string' && raw.requestId.length <= 200) requestId = raw.requestId
        const request = parseSocketRequest(raw, 'lobby')
        if (!request) throw { statusCode: 400, code: 'invalidRequest', message: '無效的連線請求。' }
        requestId = request.requestId
        const actor = await requireRealtimeIdentity(durableSocket(this.ctx, peer), this.env.DB)
        if (request.operation !== 'list' && maintenanceEnabled(this.env.MAINTENANCE_MODE)) {
          throw { statusCode: 503, code: 'maintenanceMode', message: '系統維護中，請稍後再試。' }
        }
        const data = await executeLobbyRequest(this.env, actor, request)
        peer.send(JSON.stringify({ type: 'result', requestId, ok: true, data }))
      } catch (error) {
        peer.send(JSON.stringify(socketFailure(requestId, error)))
        if ((error as { statusCode?: number })?.statusCode === 401) peer.close(4001, 'session expired')
      }
    },
  })

  async fetch(request: Request): Promise<Response> {
    if (request.method === 'GET') {
      if (request.headers.get('Upgrade')?.toLowerCase() !== 'websocket') {
        return Response.json({ error: 'websocket upgrade required' }, { status: 426 })
      }

      const userId = request.headers.get('x-fewfc-user-id')

      if (!userId) {
        return Response.json({ error: 'authenticated player required' }, { status: 401 })
      }

      return await this.socketHandler.handleDurableUpgrade(this, request)
    }

    if (request.method === 'POST') {
      const roomsChanged = new URL(request.url).pathname.endsWith('/rooms-changed')
      if (roomsChanged) {
        const message = JSON.stringify({ type: 'roomsChanged' })
        for (const socket of this.ctx.getWebSockets()) {
          if (socket.readyState === WebSocket.OPEN) socket.send(message)
        }
        return Response.json({ delivered: true })
      }

      const envelope = await request.json() as NotificationEnvelope
      return await this.serializeNotification(async () => {
        const notification = envelope?.notification
        const targetUserId = envelope?.targetUserId
        const notificationId = notification?.id
        const receiptKey = typeof targetUserId === 'string' && targetUserId.length > 0
          ? await this.receiptKey(targetUserId) : null
        const deliveredIds = receiptKey && typeof notificationId === 'string' && notificationId.length > 0
          ? await this.readReceipts(receiptKey, targetUserId)
          : []
        if (typeof notificationId === 'string' && deliveredIds.includes(notificationId)) {
          return Response.json({ delivered: true, duplicate: true })
        }

        const message = JSON.stringify({ type: 'notification', data: notification })
        for (const socket of this.ctx.getWebSockets()) {
          if (socket.readyState !== WebSocket.OPEN) continue
          const attachment = socket.deserializeAttachment() as SocketAttachment | null
          if (attachment?.userId !== targetUserId) continue
          socket.send(message)
        }

        if (receiptKey && typeof notificationId === 'string' && notificationId.length > 0) {
          await this.writeReceipts(receiptKey, targetUserId, deliveredIds, notificationId)
        }
        return Response.json({ delivered: true })
      })
    }

    return Response.json({ error: 'method not allowed' }, { status: 405 })
  }

  private async serializeNotification<T>(operation: () => Promise<T>): Promise<T> {
    const previous = this.notificationOperationTail
    let release = () => {}
    this.notificationOperationTail = new Promise<void>(resolve => { release = resolve })
    await previous
    try {
      return await operation()
    } finally {
      release()
    }
  }

  private async receiptKey(targetUserId: string): Promise<string> {
    const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(targetUserId))
    const hex = Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, '0')).join('')
    return `${playerNotificationReceiptPrefix}${hex}`
  }

  private async readReceipts(key: string, targetUserId: string): Promise<string[]> {
    if (!this.ctx.storage) return this.fallbackReceipts.get(targetUserId) ?? []
    const stored = await this.ctx.storage.get<unknown>(key)
    return Array.isArray(stored) && stored.length <= playerNotificationReceiptLimit
      && stored.every(id => typeof id === 'string') ? stored : []
  }

  private async writeReceipts(
    key: string,
    targetUserId: string,
    deliveredIds: string[],
    notificationId: string,
  ): Promise<void> {
    const next = [...deliveredIds.filter(id => id !== notificationId), notificationId]
      .slice(-playerNotificationReceiptLimit)
    if (!this.ctx.storage) {
      this.fallbackReceipts.set(targetUserId, next)
      return
    }
    await this.ctx.storage.put(key, next)
  }

  async webSocketMessage(socket: WebSocket, message: string | ArrayBuffer) {
    await this.socketHandler.handleDurableMessage(this, socket, message)
  }

  async webSocketClose(socket: WebSocket, code: number, reason: string, wasClean: boolean) {
    await this.socketHandler.handleDurableClose(this, socket, code, reason, wasClean)
  }
}
