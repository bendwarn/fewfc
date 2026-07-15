import { expect, test } from 'bun:test'
import type { RulesCatalog } from '../types/fewfc'
import {
  createDeckCompositionPolicy,
  expandPlayerDeckCounts,
  parsePlayerDeckCounts,
  playerDeckCounts,
  serializePlayerDeckCounts,
} from './player-deck'

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

  expect(deck).toStrictEqual({ name: '測試預組', cards: ['metal-1', 'fire-2'] })
  expect(policy.validate(deck)).toStrictEqual({
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

const fullComposition: RulesCatalog['deckComposition'] = {
  cardDefinitions: (
    [
      ['Metal', '金'],
      ['Wood', '木'],
      ['Water', '水'],
      ['Fire', '火'],
      ['Earth', '土'],
    ] as const
  ).flatMap(([element, name]) => [1, 2, 3, 4, 5].map(level => ({
    id: `${element}-${level}`,
    name,
    element,
    level,
    sharedDeckCopies: 4,
    personalDeckCopyLimit: level <= 3 ? 4 : 3,
    preconstructedCopies: 0,
  }))),
  sharedDeck: { exactCardCount: 90 },
  personalDeck: {
    exactCardCount: 60,
    maximumLevelTotal: 170,
    preconstructed: { name: '測試預組', cards: [] },
  },
}

const exampleCounts = [
  4, 1, 1, 3, 1,
  4, 1, 1, 2, 2,
  4, 4, 4, 3, 3,
  4, 1, 1, 3, 3,
  4, 1, 1, 1, 3,
]

test('parses tab/newline, space-separated, and compact personal-deck imports', () => {
  const tabAndNewline = `4\t1\t1\t3\t1
4\t1\t1\t2\t2
4\t4\t4\t3\t3
4\t1\t1\t3\t3
4\t1\t1\t1\t3`

  expect(parsePlayerDeckCounts(tabAndNewline)).toStrictEqual({ counts: exampleCounts, error: null })
  expect(parsePlayerDeckCounts(exampleCounts.join(' '))).toStrictEqual({ counts: exampleCounts, error: null })
  expect(parsePlayerDeckCounts('4113141122444334113341113')).toStrictEqual({ counts: exampleCounts, error: null })
})

test('keeps gold, wood, water, fire, earth and levels one through five in a fixed order', () => {
  const counts = Array.from({ length: 25 }, (_, index) => index % 10)
  const shuffledDefinitions = [...fullComposition.cardDefinitions].reverse()
  const cards = expandPlayerDeckCounts(counts, shuffledDefinitions)
  const policy = createDeckCompositionPolicy({
    ...fullComposition,
    cardDefinitions: shuffledDefinitions,
  })

  expect(policy.elements).toStrictEqual(['Metal', 'Wood', 'Water', 'Fire', 'Earth'])
  expect(policy.levels).toStrictEqual([1, 2, 3, 4, 5])
  expect(cards).toStrictEqual(counts.flatMap((count, index) => {
    const element = ['Metal', 'Wood', 'Water', 'Fire', 'Earth'][Math.floor(index / 5)]
    const level = (index % 5) + 1
    return Array.from({ length: count }, () => `${element}-${level}`)
  }))
})

test('imports and exports a draft without changing its matrix order', () => {
  const imported = parsePlayerDeckCounts('4113141122444334113341113')
  if (!imported.counts) throw new Error(imported.error)
  const draft = { name: '保留名稱', cards: expandPlayerDeckCounts(imported.counts, fullComposition.cardDefinitions) }
  const exported = serializePlayerDeckCounts(playerDeckCounts(draft, fullComposition.cardDefinitions))

  expect(parsePlayerDeckCounts(exported)).toStrictEqual({ counts: exampleCounts, error: null })
})

test('rejects wrong count and illegal import formats', () => {
  for (const input of [
    Array(24).fill('1').join(' '),
    Array(26).fill('1').join(' '),
    '111111111111111111111111',
    '11111111111111111111111111',
    '1, 2, 3',
    '1 2 x',
    '1 2 10',
  ]) {
    expect(parsePlayerDeckCounts(input).counts).toBeNull()
  }
})

test('a syntactically valid import still creates a draft when deck rules reject it', () => {
  const imported = parsePlayerDeckCounts('9'.repeat(25))
  if (!imported.counts) throw new Error(imported.error)
  const policy = createDeckCompositionPolicy(fullComposition)
  const draft = policy.deckFromCounts('超量草稿', imported.counts)

  expect(policy.validate(draft).valid).toBe(false)
  expect(draft.cards).toHaveLength(225)
})
