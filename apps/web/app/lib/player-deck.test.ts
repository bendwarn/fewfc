import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import {
  effectiveDeck,
  preconstructedDeck,
  validateDeck,
} from './player-deck'

describe('player deck', () => {
  test('preconstructed deck uses 3/2/3/2/2 for every element', () => {
    const deck = preconstructedDeck()
    const validation = validateDeck(deck)

    assert.deepEqual(validation, {
      valid: true,
      cardCount: 60,
      levelTotal: 170,
      errors: [],
    })
  })

  test('invalid custom deck falls back to the preconstructed deck', () => {
    const result = effectiveDeck({ name: '無效牌組', cards: [] })

    assert.equal(result.source, 'preconstructed')
    assert.equal(result.deck.name, '五行均衡預組')
  })
})
