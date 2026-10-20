import { expect, test } from 'bun:test'
import { battleRecordCurrentTurnNumber } from './battle-record-timeline'

test('preparing and finished games do not expose a current turn in the timeline', () => {
  expect(battleRecordCurrentTurnNumber('Preparing', 1)).toBeNull()
  expect(battleRecordCurrentTurnNumber('Finished', 4)).toBeNull()
})

test('an in-progress game exposes its active turn number', () => {
  expect(battleRecordCurrentTurnNumber('InProgress', 1)).toBe(1)
})

test('a missing replay frame has no active turn', () => {
  expect(battleRecordCurrentTurnNumber(undefined, undefined)).toBeNull()
})
