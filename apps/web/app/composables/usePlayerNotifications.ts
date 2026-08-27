import type {
  PlayerNotification,
  PlayerNotificationSocketMessage,
} from '../../shared/game-room'
import {
  isTransientPlayerNotification,
  mergePlayerNotification,
  normalizePlayerNotifications,
  PLAYER_NOTIFICATION_TTL_MS,
  playerNotificationStorageKey,
  restorePlayerNotifications,
  serializePlayerNotifications,
} from '../lib/player-notifications'

const notificationVisibleDurationMs = 5000

export function usePlayerNotifications() {
  const notifications = ref<PlayerNotification[]>([])
  const visibleNotifications = ref<PlayerNotification[]>([])
  const roomListRevision = ref(0)
  const connectionState = ref<'idle' | 'connecting' | 'connected' | 'reconnecting'>('idle')
  let socket: WebSocket | null = null
  let reconnectTimer: ReturnType<typeof setTimeout> | undefined
  let retentionExpiryTimer: ReturnType<typeof setTimeout> | undefined
  const notificationTimers = new Map<string, ReturnType<typeof setTimeout>>()
  let reconnectAttempt = 0
  let shouldConnect = false
  let activeUserId = ''
  let storageUsable = true

  function clearNotificationTimer(id: string) {
    const timer = notificationTimers.get(id)
    if (timer) clearTimeout(timer)
    notificationTimers.delete(id)
  }

  function hideNotification(id: string) {
    clearNotificationTimer(id)
    visibleNotifications.value = visibleNotifications.value.filter(notification => notification.id !== id)
  }

  function scheduleRetentionExpiry() {
    clearTimeout(retentionExpiryTimer)
    const expiry = notifications.value.reduce<number | undefined>((soonest, notification) => {
      const value = Date.parse(notification.createdAt) + PLAYER_NOTIFICATION_TTL_MS
      return soonest === undefined || value < soonest ? value : soonest
    }, undefined)
    if (expiry === undefined) return
    retentionExpiryTimer = setTimeout(() => {
      mutateRetainedNotifications(current => current)
    }, Math.max(0, expiry - Date.now()))
  }

  function replaceRetainedNotifications(next: readonly PlayerNotification[]) {
    notifications.value = normalizePlayerNotifications(next)
    scheduleRetentionExpiry()
  }

  function showNotification(notification: PlayerNotification) {
    const next = mergePlayerNotification(visibleNotifications.value, notification)
    if (next === visibleNotifications.value) return
    const replaced = visibleNotifications.value.find(item => item.gameId === notification.gameId)
    if (replaced) clearNotificationTimer(replaced.id)
    visibleNotifications.value = next
    clearNotificationTimer(notification.id)
    notificationTimers.set(notification.id, setTimeout(() => hideNotification(notification.id), notificationVisibleDurationMs))
  }

  function mutateRetainedNotifications(
    mutation: (current: PlayerNotification[]) => PlayerNotification[],
  ) {
    const apply = (current: PlayerNotification[]) => {
      const next = mutation(current)
      replaceRetainedNotifications(next)
      return notifications.value
    }

    if (!import.meta.client || !activeUserId || !storageUsable) {
      return apply(notifications.value)
    }
    let current: PlayerNotification[]
    try {
      // 每次寫入先以目前儲存值為基礎，避免另一個分頁已消耗的通知復活。
      current = restorePlayerNotifications(
        window.localStorage.getItem(playerNotificationStorageKey(activeUserId)),
      )
    } catch {
      storageUsable = false
      return apply(notifications.value)
    }
    const next = mutation(current)
    try {
      window.localStorage.setItem(
        playerNotificationStorageKey(activeUserId),
        serializePlayerNotifications(next),
      )
      replaceRetainedNotifications(next)
      return next
    } catch {
      // 儲存空間不可用時，通知仍只在目前頁面可用。
      storageUsable = false
      return apply(notifications.value)
    }
  }

  function setUserId(userId: string) {
    if (activeUserId === userId) return
    disconnect()
    activeUserId = userId
    storageUsable = true
    visibleNotifications.value.forEach(notification => clearNotificationTimer(notification.id))
    visibleNotifications.value = []
    notifications.value = []
    clearTimeout(retentionExpiryTimer)

    if (!import.meta.client || !userId) return
    try {
      replaceRetainedNotifications(restorePlayerNotifications(window.localStorage.getItem(playerNotificationStorageKey(userId))))
    } catch {
      // 讀取失敗不能阻止正常的房間連線。
    }
  }

  function receiveNotification(notification: PlayerNotification) {
    if (isTransientPlayerNotification(notification)) {
      dismissRoom(notification.gameId)
      showNotification(notification)
      return
    }
    mutateRetainedNotifications(current => mergePlayerNotification(current, notification))
    showNotification(notification)
  }

  function connect() {
    if (!import.meta.client || !activeUserId) {
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
    const socketUserId = activeUserId

    nextSocket.addEventListener('open', () => {
      if (socket !== nextSocket || activeUserId !== socketUserId) return
      reconnectAttempt = 0
      connectionState.value = 'connected'
    })

    nextSocket.addEventListener('message', (event) => {
      if (socket !== nextSocket || activeUserId !== socketUserId) return
      if (typeof event.data !== 'string' || event.data === 'pong') {
        return
      }

      let message: PlayerNotificationSocketMessage
      try {
        message = JSON.parse(event.data) as PlayerNotificationSocketMessage
      } catch {
        return
      }

      if (message.type === 'roomsChanged') {
        roomListRevision.value += 1
        return
      }

      if (message.type === 'notification') {
        receiveNotification(message.data)
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
    hideNotification(id)
  }

  function dismissRoom(gameId: string) {
    mutateRetainedNotifications(current => current.filter(
      (notification) => notification.gameId !== gameId,
    ))
  }

  function handleStorage(event: StorageEvent) {
    if (!activeUserId || (event.key !== null && event.key !== playerNotificationStorageKey(activeUserId))) return
    try {
      // 事件的 newValue 可能已落後於目前儲存值，必須重新讀取才不會復活已消耗的房間。
      replaceRetainedNotifications(restorePlayerNotifications(
        window.localStorage.getItem(playerNotificationStorageKey(activeUserId)),
      ))
    } catch {
      replaceRetainedNotifications([])
    }
  }

  if (import.meta.client) {
    window.addEventListener('storage', handleStorage)
  }

  onBeforeUnmount(() => {
    disconnect()
    clearTimeout(retentionExpiryTimer)
    notificationTimers.forEach(timer => clearTimeout(timer))
    notificationTimers.clear()
    if (import.meta.client) window.removeEventListener('storage', handleStorage)
  })

  return {
    notifications,
    visibleNotifications,
    roomListRevision,
    connectionState,
    connect,
    disconnect,
    dismiss,
    dismissRoom,
    setUserId,
  }
}
