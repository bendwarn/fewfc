import { describe, expect, test } from 'bun:test'
import type { PublicGameState } from '../types/fewfc'
import {
  lastCompletedTurnDiscardForPlayer,
  latestSharedTurnDiscard,
  personalPileCounts,
  publicCardRefCount,
  sharedPileCounts,
} from './battlefield-presentation'

type BattlefieldPileState = Pick<PublicGameState,
  | 'deckCount'
  | 'discard'
  | 'playerDecks'
  | 'playerDiscards'
  | 'lastCompletedTurnDiscards'
  | 'turnNumber'>

const fireThree = { id: 7, label: '火 3', element: 'Fire' as const, level: 3, secretStrategies: [] }
const waterTwo = { id: 8, label: '水 2', element: 'Water' as const, level: 2, secretStrategies: [] }

function state(overrides: Partial<BattlefieldPileState> = {}): BattlefieldPileState {
  return {
    turnNumber: 6,
    deckCount: 18,
    discard: [fireThree],
    playerDecks: [{ player: 'alice', cards: { kind: 'hidden', count: 12 } }],
    playerDiscards: [{ player: 'alice', cards: [fireThree, waterTwo] }],
    lastCompletedTurnDiscards: [
      { player: 'alice', card: fireThree, turnNumber: 4 },
      { player: 'bob', card: waterTwo, turnNumber: 5 },
    ],
    ...overrides,
  }
}

describe('battlefield pile presentation', () => {
  test('counts known, partially known, and hidden Card refs', () => {
    expect(publicCardRefCount({ kind: 'known', cards: [fireThree] })).toBe(1)
    expect(publicCardRefCount({ kind: 'partiallyKnown', cards: [fireThree, null] })).toBe(2)
    expect(publicCardRefCount({ kind: 'hidden', count: 3 })).toBe(3)
    expect(publicCardRefCount(undefined)).toBe(0)
  })

  test('presents Personal and shared pile counts independently', () => {
    expect(personalPileCounts(state(), 'alice')).toStrictEqual({ deck: 12, discard: 2 })
    expect(personalPileCounts(state(), 'bob')).toStrictEqual({ deck: 0, discard: 0 })
    expect(sharedPileCounts(state())).toStrictEqual({ deck: 18, discard: 1 })
  })

  test('keeps Personal history per actor and Shared history on the immediately completed turn', () => {
    expect(lastCompletedTurnDiscardForPlayer(state(), 'alice')?.card).toStrictEqual(fireThree)
    expect(latestSharedTurnDiscard(state())?.card).toStrictEqual(waterTwo)

    const skippedLatestTurn = state({
      lastCompletedTurnDiscards: [{ player: 'alice', card: fireThree, turnNumber: 4 }],
    })
    expect(latestSharedTurnDiscard(skippedLatestTurn)).toBeNull()
  })
})
