import { DurableObject } from 'cloudflare:workers'
import type { PlayerNotification } from '../../shared/game-room'

interface SocketAttachment {
  userId: string
}

interface NotificationEnvelope {
  targetUserId: string
  notification: PlayerNotification
}

export class PlayerNotifications extends DurableObject {
  async fetch(request: Request): Promise<Response> {
    if (request.method === 'GET') {
      if (request.headers.get('Upgrade')?.toLowerCase() !== 'websocket') {
        return Response.json({ error: 'websocket upgrade required' }, { status: 426 })
      }

      const userId = request.headers.get('x-fewfc-user-id')

      if (!userId) {
        return Response.json({ error: 'authenticated player required' }, { status: 401 })
      }

      const pair = new WebSocketPair()
      const [client, server] = Object.values(pair)
      this.ctx.acceptWebSocket(server)
      server.serializeAttachment({ userId } satisfies SocketAttachment)

      return new Response(null, { status: 101, webSocket: client })
    }

    if (request.method === 'POST') {
      const roomsChanged = new URL(request.url).pathname.endsWith('/rooms-changed')
      const envelope = roomsChanged
        ? null
        : await request.json() as NotificationEnvelope
      const message = roomsChanged
        ? JSON.stringify({ type: 'roomsChanged' })
        : JSON.stringify({
            type: 'notification',
            data: envelope?.notification,
          })

      for (const socket of this.ctx.getWebSockets()) {
        if (socket.readyState !== WebSocket.OPEN) {
          continue
        }

        if (!roomsChanged) {
          const attachment = socket.deserializeAttachment() as SocketAttachment | null

          if (attachment?.userId !== envelope?.targetUserId) {
            continue
          }
        }

        socket.send(message)
      }

      return Response.json({ delivered: true })
    }

    return Response.json({ error: 'method not allowed' }, { status: 405 })
  }

  webSocketMessage(socket: WebSocket, message: string | ArrayBuffer) {
    if (typeof message === 'string' && message === 'ping') {
      socket.send('pong')
    }
  }
}
