import assert from 'node:assert/strict'
import { describe, test } from 'bun:test'
import {
  completedPendingChoiceSelection,
  isPendingChoiceComplete,
  togglePendingChoiceSelection,
} from './pending-choice-selection'

describe('pending choice selection', () => {
  test('accumulates the required cards before the choice is complete', () => {
    const first = togglePendingChoiceSelection([], 11, 2)
    const second = togglePendingChoiceSelection(first, 12, 2)

    assert.deepEqual(first, [11])
    assert.equal(isPendingChoiceComplete(first, 2), false)
    assert.deepEqual(second, [11, 12])
    assert.equal(isPendingChoiceComplete(second, 2), true)
    assert.equal(completedPendingChoiceSelection(first, 2), undefined)
    assert.deepEqual(completedPendingChoiceSelection(second, 2), [11, 12])
  })

  test('toggles selected cards without exceeding the required count', () => {
    assert.deepEqual(togglePendingChoiceSelection([11], 11, 2), [])
    assert.deepEqual(togglePendingChoiceSelection([11, 12], 13, 2), [11, 12])
  })
})
