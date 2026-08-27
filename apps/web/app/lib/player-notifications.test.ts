import { describe, expect, test } from 'bun:test'
import type { PlayerNotification } from '../../shared/game-room'
import {
  PLAYER_NOTIFICATION_ROOM_LIMIT,
  PLAYER_NOTIFICATION_TTL_MS,
  mergePlayerNotification,
  playerNotificationStorageKey,
  restorePlayerNotifications,
  serializePlayerNotifications,
} from './player-notifications'

function notification(
  id: string,
  gameId: string,
  kind: PlayerNotification['kind'],
  message: string,
): PlayerNotification {
  return {
    id,
    gameId,
    kind,
    message,
    createdAt: new Date(Date.UTC(2026, 7, 27, 12, 0, Number.parseInt(id, 10) || 0)).toISOString(),
  }
}

describe('mergePlayerNotification', () => {
  test('replaces an existing notification from the same room with the latest event', () => {
    const previous = notification('1', 'room-a', 'gameStarted', '房間 A 已開始。')
    const latest = notification('2', 'room-a', 'yourTurn', '房間 A 輪到你行動。')

    expect(mergePlayerNotification([previous], latest)).toEqual([latest])
  })

  test('keeps notifications from different rooms in newest-first order', () => {
    const roomA = notification('1', 'room-a', 'gameStarted', '房間 A 已開始。')
    const roomB = notification('2', 'room-b', 'yourTurn', '房間 B 輪到你行動。')

    expect(mergePlayerNotification([roomA], roomB)).toEqual([roomB, roomA])
  })

  test('does not let a same-room update hide a seat promotion', () => {
    const promoted = notification('1', 'room-a', 'seatPromoted', '已補為玩家，請準備')
    const changed = notification('2', 'room-a', 'roomChanged', '房間 A 已更新。')

    expect(mergePlayerNotification([promoted], changed)).toEqual([promoted])
  })

  test('keeps at most 100 distinct rooms', () => {
    const existing = [
      notification('8', 'room-8', 'yourTurn', '房間 8'),
      notification('7', 'room-7', 'yourTurn', '房間 7'),
      notification('6', 'room-6', 'yourTurn', '房間 6'),
      notification('5', 'room-5', 'yourTurn', '房間 5'),
      notification('4', 'room-4', 'yourTurn', '房間 4'),
      notification('3', 'room-3', 'yourTurn', '房間 3'),
      notification('2', 'room-2', 'yourTurn', '房間 2'),
      notification('1', 'room-1', 'yourTurn', '房間 1'),
    ]
    const latest = notification('9', 'room-9', 'gameStarted', '房間 9 已開始。')

    expect(mergePlayerNotification(existing, latest).map(item => item.gameId)).toEqual([
      'room-9',
      'room-8',
      'room-7',
      'room-6',
      'room-5',
      'room-4',
      'room-3',
      'room-2',
      'room-1',
    ])
  })

  test('evicts the oldest room when the 100-room cap is exceeded', () => {
    const existing = Array.from({ length: PLAYER_NOTIFICATION_ROOM_LIMIT }, (_, index) => (
      notification(String(index), `room-${index}`, 'yourTurn', `房間 ${index}`)
    ))
    const latest = notification('new', 'room-new', 'gameStarted', '新房間')

    const merged = mergePlayerNotification(existing, latest)

    expect(merged).toHaveLength(PLAYER_NOTIFICATION_ROOM_LIMIT)
    expect(merged[0]).toEqual(latest)
    expect(merged.some(item => item.gameId === 'room-99')).toBe(false)
  })
})

describe('retained Player Notifications', () => {
  const now = Date.parse('2026-08-27T12:00:00.000Z')

  test('restores only current ordinary entries and keeps one entry per room', () => {
    const newest = { ...notification('2', 'room-a', 'yourTurn', '新的通知'), createdAt: '2026-08-27T11:00:00.000Z' }
    const old = { ...notification('1', 'room-a', 'gameStarted', '舊的通知'), createdAt: '2026-08-27T10:00:00.000Z' }
    const expired = { ...notification('3', 'room-b', 'roomChanged', '過期通知'), createdAt: new Date(now - PLAYER_NOTIFICATION_TTL_MS - 1).toISOString() }
    const removed = { ...notification('4', 'room-c', 'removed', '已被移除'), createdAt: '2026-08-27T11:00:00.000Z' }

    expect(restorePlayerNotifications(JSON.stringify({ version: 1, notifications: [newest, old, expired, removed] }), now)).toEqual([newest])
  })

  test('expires an entry at the seven-day boundary', () => {
    const boundary = { ...notification('1', 'room-a', 'yourTurn', '輪到你'), createdAt: new Date(now - PLAYER_NOTIFICATION_TTL_MS).toISOString() }

    expect(restorePlayerNotifications(JSON.stringify({ version: 1, notifications: [boundary] }), now)).toEqual([])
  })

  test('rejects corrupt storage without affecting the current page', () => {
    expect(restorePlayerNotifications('{not json', now)).toEqual([])
    expect(restorePlayerNotifications(JSON.stringify({ version: 2, notifications: [] }), now)).toEqual([])
    expect(restorePlayerNotifications(JSON.stringify({ version: 1, notifications: [{ gameId: 'room-a' }] }), now)).toEqual([])
  })

  test('serializes a versioned value and keeps account keys separate', () => {
    const currentNow = Date.now()
    const current = { ...notification('1', 'room-a', 'yourTurn', '輪到你'), createdAt: new Date(currentNow).toISOString() }

    expect(playerNotificationStorageKey('player-a')).not.toBe(playerNotificationStorageKey('player-b'))
    expect(restorePlayerNotifications(serializePlayerNotifications([current]), currentNow)).toEqual([current])
  })
})
