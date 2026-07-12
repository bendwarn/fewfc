import { describe, expect, test } from 'bun:test'
import {
  completedPendingChoiceSelection,
  isPendingChoiceComplete,
  togglePendingChoiceSelection,
} from './pending-choice-selection'

describe('pending choice selection', () => {
  test('accumulates the required cards before the choice is complete', () => {
    const first = togglePendingChoiceSelection([], 11, 2)
    const second = togglePendingChoiceSelection(first, 12, 2)

    expect(first).toEqual([11])
    expect(isPendingChoiceComplete(first, 2)).toBe(false)
    expect(second).toEqual([11, 12])
    expect(isPendingChoiceComplete(second, 2)).toBe(true)
    expect(completedPendingChoiceSelection(first, 2)).toBe(undefined)
    expect(completedPendingChoiceSelection(second, 2)).toEqual([11, 12])
  })

  test('toggles selected cards without exceeding the required count', () => {
    expect(togglePendingChoiceSelection([11], 11, 2)).toEqual([])
    expect(togglePendingChoiceSelection([11, 12], 13, 2)).toEqual([11, 12])
  })
})
