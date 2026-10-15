import { expect, test } from 'bun:test'
import {
  nextHighestActiveGameVersion,
  shouldApplyActiveGameVersion,
  shouldApplyRoomMetadataVersion,
} from './active-game-version'

test('active Game Record versions reject only stale delivery for the same Game Instance', () => {
  const newest = { gameInstanceId: 'game-1', recordSequence: 8 }

  expect(shouldApplyActiveGameVersion(newest, { gameInstanceId: 'game-1', recordSequence: 7 })).toBe(false)
  expect(shouldApplyActiveGameVersion(newest, { gameInstanceId: 'game-1', recordSequence: 8 })).toBe(true)
  expect(shouldApplyActiveGameVersion(newest, { gameInstanceId: 'game-2', recordSequence: 1 })).toBe(true)
  expect(nextHighestActiveGameVersion(newest, { gameInstanceId: 'game-1', recordSequence: 7 })).toEqual(newest)
})

test('room metadata rejects older same-room snapshots but accepts equal and new rooms', () => {
  const newest = { gameId: 'game-1', updatedAt: '2026-10-16T01:00:00.002Z' }

  expect(shouldApplyRoomMetadataVersion(newest, {
    gameId: 'game-1', updatedAt: '2026-10-16T01:00:00.001Z',
  })).toBe(false)
  expect(shouldApplyRoomMetadataVersion(newest, newest)).toBe(true)
  expect(shouldApplyRoomMetadataVersion(newest, {
    gameId: 'game-1', updatedAt: '2026-10-16T01:00:00.003Z',
  })).toBe(true)
  expect(shouldApplyRoomMetadataVersion(newest, {
    gameId: 'game-2', updatedAt: '2026-10-15T01:00:00.000Z',
  })).toBe(true)
})
