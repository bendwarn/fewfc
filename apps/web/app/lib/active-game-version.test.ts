import { expect, test } from 'bun:test'
import { nextHighestActiveGameVersion, shouldApplyActiveGameVersion } from './active-game-version'

test('active Game Record versions reject only stale delivery for the same Game Instance', () => {
  const newest = { gameInstanceId: 'game-1', recordSequence: 8 }

  expect(shouldApplyActiveGameVersion(newest, { gameInstanceId: 'game-1', recordSequence: 7 })).toBe(false)
  expect(shouldApplyActiveGameVersion(newest, { gameInstanceId: 'game-1', recordSequence: 8 })).toBe(true)
  expect(shouldApplyActiveGameVersion(newest, { gameInstanceId: 'game-2', recordSequence: 1 })).toBe(true)
  expect(nextHighestActiveGameVersion(newest, { gameInstanceId: 'game-1', recordSequence: 7 })).toEqual(newest)
})
