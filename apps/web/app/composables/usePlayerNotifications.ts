import type {
  PlayerNotification,
  PlayerNotificationSocketMessage,
} from '#shared/game-room'
import {
  isTransientPlayerNotification,
  mergePlayerNotification,
  normalizePlayerNotifications,
  normalizeHandledPlayerNotificationIds,
  PLAYER_NOTIFICATION_TTL_MS,
  playerNotificationHandledIdStorageKey,
  playerNotificationStorageKey,
  restoreHandledPlayerNotificationIds,
  restorePlayerNotifications,
  serializeHandledPlayerNotificationIds,
  serializePlayerNotifications,
} from '../lib/player-notifications'

import type { LobbyOperations } from '#shared/realtime'

const notificationVisibleDurationMs = 5000

export function usePlayerNotifications() {
  const notifications = ref<PlayerNotification[]>([])
  const visibleNotifications = ref<PlayerNotification[]>([])
  const roomListRevision = ref(0)
  const channel = useRealtimeSocket<LobbyOperations>((value) => {
    const message = value as PlayerNotificationSocketMessage
    if (message.type === 'roomsChanged') roomListRevision.value += 1
    else if (message.type === 'notification') receiveNotification(message.data)
  })
  const connectionState = channel.connectionState
  let retentionExpiryTimer: ReturnType<typeof setTimeout> | undefined
  const notificationTimers = new Map<string, ReturnType<typeof setTimeout>>()
  const handledNotificationIds = new Set<string>()
  let activeUserId = ''
  let storageUsable = true

  function replaceHandledNotificationIds(ids: readonly unknown[]) {
    handledNotificationIds.clear()
    for (const id of normalizeHandledPlayerNotificationIds(ids)) {
      handledNotificationIds.add(id)
    }
  }

  function persistHandledNotificationIds() {
    if (!import.meta.client || !activeUserId) return
    const key = playerNotificationHandledIdStorageKey(activeUserId)
    let stored: string[]
    try {
      stored = restoreHandledPlayerNotificationIds(window.localStorage.getItem(key))
    } catch {
      return
    }

    // 收據獨立於每房只保留一筆的通知清單，避免新訊息取代舊訊息後遺失去重 ID。
    const merged = normalizeHandledPlayerNotificationIds([
      ...stored,
      ...handledNotificationIds,
    ])
    replaceHandledNotificationIds(merged)
    try {
      window.localStorage.setItem(key, serializeHandledPlayerNotificationIds(merged))
    } catch {
      // 儲存空間不可用時，仍以目前頁面的記憶體收據防止一般重試重複顯示。
    }
  }

  function rememberHandledNotification(id: string) {
    if (!id || handledNotificationIds.has(id)) return
    replaceHandledNotificationIds([...handledNotificationIds, id])
    persistHandledNotificationIds()
  }

  function mergeStoredHandledNotificationIds() {
    if (!import.meta.client || !activeUserId) return
    try {
      replaceHandledNotificationIds([
        ...handledNotificationIds,
        ...restoreHandledPlayerNotificationIds(
          window.localStorage.getItem(playerNotificationHandledIdStorageKey(activeUserId)),
        ),
      ])
    } catch {
      // 收據讀取失敗不影響目前頁面的通知處理。
    }
  }

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
    replaceHandledNotificationIds([])
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
    try {
      replaceHandledNotificationIds(restoreHandledPlayerNotificationIds(
        window.localStorage.getItem(playerNotificationHandledIdStorageKey(userId)),
      ))
    } catch {
      // 收據讀取失敗不能阻止正常的房間連線。
    }
  }

  function receiveNotification(notification: PlayerNotification) {
    if (handledNotificationIds.has(notification.id)
      || notifications.value.some(current => current.id === notification.id)
      || visibleNotifications.value.some(current => current.id === notification.id)) return
    if (isTransientPlayerNotification(notification)) {
      dismissRoom(notification.gameId)
      showNotification(notification)
      rememberHandledNotification(notification.id)
      return
    }
    mutateRetainedNotifications(current => mergePlayerNotification(current, notification))
    showNotification(notification)
    rememberHandledNotification(notification.id)
  }

  function connect() {
    if (activeUserId) channel.connect('/api/notifications/socket')
  }

  function disconnect() {
    channel.close()
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
    if (!activeUserId) return
    const notificationKey = playerNotificationStorageKey(activeUserId)
    const handledIdKey = playerNotificationHandledIdStorageKey(activeUserId)
    if (event.key !== null && event.key !== notificationKey && event.key !== handledIdKey) return
    if (event.key === null || event.key === notificationKey) {
      try {
        // 事件的 newValue 可能已落後於目前儲存值，必須重新讀取才不會復活已消耗的房間。
        replaceRetainedNotifications(restorePlayerNotifications(
          window.localStorage.getItem(notificationKey),
        ))
      } catch {
        replaceRetainedNotifications([])
      }
    }
    if (event.key === null || event.key === handledIdKey) mergeStoredHandledNotificationIds()
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
    request: channel.request,
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
