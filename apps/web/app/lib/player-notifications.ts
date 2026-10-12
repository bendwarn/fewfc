import type { PlayerNotification } from '../../shared/game-room'

export const PLAYER_NOTIFICATION_STORAGE_PREFIX = 'fewfc.player-notifications.v1:'
export const PLAYER_NOTIFICATION_HANDLED_ID_STORAGE_PREFIX = 'fewfc.player-notification-handled.v1:'
export const PLAYER_NOTIFICATION_TTL_MS = 7 * 24 * 60 * 60 * 1000
export const PLAYER_NOTIFICATION_ROOM_LIMIT = 100
export const PLAYER_NOTIFICATION_HANDLED_ID_LIMIT = 4096

const notificationKinds = new Set<PlayerNotification['kind']>([
  'gameStarted',
  'yourTurn',
  'roomChanged',
  'removed',
  'dissolved',
  'seatPromoted',
])

interface StoredPlayerNotifications {
  version: 1
  notifications: PlayerNotification[]
}

interface StoredHandledPlayerNotificationIds {
  version: 1
  ids: string[]
}

export function playerNotificationStorageKey(userId: string): string {
  return `${PLAYER_NOTIFICATION_STORAGE_PREFIX}${userId}`
}

export function playerNotificationHandledIdStorageKey(userId: string): string {
  return `${PLAYER_NOTIFICATION_HANDLED_ID_STORAGE_PREFIX}${userId}`
}

export function normalizeHandledPlayerNotificationIds(ids: readonly unknown[]): string[] {
  const seen = new Set<string>()
  const normalized: string[] = []
  for (const id of ids) {
    if (typeof id !== 'string' || id.length === 0 || id.length > 256 || seen.has(id)) continue
    seen.add(id)
    normalized.push(id)
  }
  return normalized.slice(-PLAYER_NOTIFICATION_HANDLED_ID_LIMIT)
}

export function restoreHandledPlayerNotificationIds(serialized: string | null): string[] {
  if (!serialized) return []

  try {
    const stored = JSON.parse(serialized) as Partial<StoredHandledPlayerNotificationIds>
    if (stored.version !== 1 || !Array.isArray(stored.ids)) return []
    return normalizeHandledPlayerNotificationIds(stored.ids)
  } catch {
    return []
  }
}

export function serializeHandledPlayerNotificationIds(ids: readonly string[]): string {
  const stored: StoredHandledPlayerNotificationIds = {
    version: 1,
    ids: normalizeHandledPlayerNotificationIds(ids),
  }
  return JSON.stringify(stored)
}

export function isTransientPlayerNotification(notification: PlayerNotification): boolean {
  return notification.kind === 'removed' || notification.kind === 'dissolved'
}

function isPlayerNotification(value: unknown): value is PlayerNotification {
  if (!value || typeof value !== 'object') return false
  const notification = value as Record<string, unknown>
  return typeof notification.id === 'string'
    && typeof notification.gameId === 'string'
    && typeof notification.message === 'string'
    && typeof notification.createdAt === 'string'
    && notificationKinds.has(notification.kind as PlayerNotification['kind'])
    && Number.isFinite(Date.parse(notification.createdAt))
}

export function normalizePlayerNotifications(
  notifications: readonly PlayerNotification[],
  now = Date.now(),
): PlayerNotification[] {
  const cutoff = now - PLAYER_NOTIFICATION_TTL_MS
  const seenRoomIds = new Set<string>()

  return notifications.filter((notification) => {
    if (!isPlayerNotification(notification)
      || isTransientPlayerNotification(notification)
      || Date.parse(notification.createdAt) <= cutoff
      || seenRoomIds.has(notification.gameId)) {
      return false
    }
    seenRoomIds.add(notification.gameId)
    return true
  }).slice(0, PLAYER_NOTIFICATION_ROOM_LIMIT)
}

export function restorePlayerNotifications(serialized: string | null, now = Date.now()): PlayerNotification[] {
  if (!serialized) return []

  try {
    const stored = JSON.parse(serialized) as Partial<StoredPlayerNotifications>
    if (stored.version !== 1 || !Array.isArray(stored.notifications)) return []
    return normalizePlayerNotifications(stored.notifications, now)
  } catch {
    return []
  }
}

export function serializePlayerNotifications(notifications: readonly PlayerNotification[]): string {
  const stored: StoredPlayerNotifications = {
    version: 1,
    notifications: normalizePlayerNotifications(notifications),
  }
  return JSON.stringify(stored)
}

export function mergePlayerNotification(
  notifications: PlayerNotification[],
  incoming: PlayerNotification,
): PlayerNotification[] {
  if (notifications.some(notification => notification.id === incoming.id)) return notifications
  const current = notifications.find(notification => notification.gameId === incoming.gameId)
  // 補位通知必須讓當事人看見，不能被同房的一般更新立刻覆蓋。
  if (current?.kind === 'seatPromoted' && incoming.kind === 'roomChanged') {
    return notifications
  }
  return [
    incoming,
    ...notifications.filter(notification => notification.gameId !== incoming.gameId),
  ].slice(0, PLAYER_NOTIFICATION_ROOM_LIMIT)
}
