import type {
  PlayerNotification,
  PlayerNotificationSocketMessage,
} from '../../shared/game-room'
import { mergePlayerNotification } from '../lib/player-notifications'

export function usePlayerNotifications() {
  const notifications = ref<PlayerNotification[]>([])
  const roomListRevision = ref(0)
  const connectionState = ref<'idle' | 'connecting' | 'connected' | 'reconnecting'>('idle')
  let socket: WebSocket | null = null
  let reconnectTimer: ReturnType<typeof setTimeout> | undefined
  let reconnectAttempt = 0
  let shouldConnect = false

  function connect() {
    if (!import.meta.client) {
      return
    }

    shouldConnect = true

    if (socket && socket.readyState <= WebSocket.OPEN) {
      return
    }

    clearTimeout(reconnectTimer)
    connectionState.value = reconnectAttempt > 0 ? 'reconnecting' : 'connecting'
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const nextSocket = new WebSocket(
      `${protocol}//${window.location.host}/api/notifications/socket`,
    )
    socket = nextSocket

    nextSocket.addEventListener('open', () => {
      reconnectAttempt = 0
      connectionState.value = 'connected'
    })

    nextSocket.addEventListener('message', (event) => {
      if (typeof event.data !== 'string' || event.data === 'pong') {
        return
      }

      const message = JSON.parse(event.data) as PlayerNotificationSocketMessage

      if (message.type === 'roomsChanged') {
        roomListRevision.value += 1
        return
      }

      if (message.type === 'notification') {
        notifications.value = mergePlayerNotification(notifications.value, message.data)
      }
    })

    nextSocket.addEventListener('close', () => {
      if (!shouldConnect || socket !== nextSocket) {
        return
      }

      reconnectAttempt += 1
      connectionState.value = 'reconnecting'
      reconnectTimer = setTimeout(
        connect,
        Math.min(1000 * (2 ** (reconnectAttempt - 1)), 10000),
      )
    })

    nextSocket.addEventListener('error', () => {
      nextSocket.close()
    })
  }

  function disconnect() {
    shouldConnect = false
    clearTimeout(reconnectTimer)
    reconnectAttempt = 0
    connectionState.value = 'idle'
    const current = socket
    socket = null

    if (current && current.readyState < WebSocket.CLOSING) {
      current.close(1000, 'session closed')
    }
  }

  function dismiss(id: string) {
    notifications.value = notifications.value.filter((notification) => notification.id !== id)
  }

  function dismissRoom(gameId: string) {
    notifications.value = notifications.value.filter(
      (notification) => notification.gameId !== gameId,
    )
  }

  onBeforeUnmount(disconnect)

  return {
    notifications,
    roomListRevision,
    connectionState,
    connect,
    disconnect,
    dismiss,
    dismissRoom,
  }
}
