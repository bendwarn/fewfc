import { expect, test } from 'bun:test'
import type { RulesCatalog } from '../types/fewfc'
import { createDeckCompositionPolicy } from './player-deck'

const composition: RulesCatalog['deckComposition'] = {
  cardDefinitions: [
    {
      id: 'metal-1',
      name: '金',
      element: 'Metal',
      level: 1,
      sharedDeckCopies: 4,
      personalDeckCopyLimit: 2,
      preconstructedCopies: 1,
    },
    {
      id: 'fire-2',
      name: '火',
      element: 'Fire',
      level: 2,
      sharedDeckCopies: 3,
      personalDeckCopyLimit: 1,
      preconstructedCopies: 1,
    },
  ],
  sharedDeck: { exactCardCount: 7 },
  personalDeck: {
    exactCardCount: 2,
    maximumLevelTotal: 3,
    preconstructed: { name: '測試預組', cards: ['metal-1', 'fire-2'] },
  },
}

test('deck policy interprets the supplied composition instead of hard-coding limits', () => {
  const policy = createDeckCompositionPolicy(composition)
  const deck = policy.preconstructedDeck()

  expect(deck).toEqual({ name: '測試預組', cards: ['metal-1', 'fire-2'] })
  expect(policy.validate(deck)).toEqual({
    valid: true,
    cardCount: 2,
    levelTotal: 3,
    errors: [],
  })
  expect(policy.validate({ name: 'too many', cards: ['fire-2', 'fire-2'] }).valid).toBe(false)
})

test('invalid custom decks fall back to the catalog preconstructed deck', () => {
  const policy = createDeckCompositionPolicy(composition)
  const result = policy.effectiveDeck({ name: '無效牌組', cards: [] })

  expect(result.source).toBe('preconstructed')
  expect(result.deck.name).toBe('測試預組')
})
