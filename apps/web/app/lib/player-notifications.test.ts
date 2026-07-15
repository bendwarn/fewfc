import { describe, expect, test } from 'bun:test'
import type { PlayerNotification } from '../../shared/game-room'
import { mergePlayerNotification } from './player-notifications'

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
    createdAt: `2026-07-15T00:00:${id.padStart(2, '0')}Z`,
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

  test('keeps at most eight distinct rooms', () => {
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
    ])
  })
})
