export const DECK_ELEMENTS = ['metal', 'wood', 'water', 'fire', 'earth'] as const
export const DECK_LEVELS = [1, 2, 3, 4, 5] as const
export const PRECONSTRUCTED_DECK_NAME = '五行均衡預組'

export interface PlayerDeckList {
  name: string
  cards: string[]
}

export interface DeckValidation {
  valid: boolean
  cardCount: number
  levelTotal: number
  errors: string[]
}

export function preconstructedDeck(): PlayerDeckList {
  const copies = [3, 2, 3, 2, 2]
  return {
    name: PRECONSTRUCTED_DECK_NAME,
    cards: DECK_ELEMENTS.flatMap(element => DECK_LEVELS.flatMap(
      (level, index) => Array.from({ length: copies[index] ?? 0 }, () => `${element}-${level}`),
    )),
  }
}

export function validateDeck(deck: PlayerDeckList): DeckValidation {
  const errors: string[] = []
  const counts = new Map<string, number>()
  let levelTotal = 0

  for (const card of deck.cards) {
    const match = /^(metal|wood|water|fire|earth)-([1-5])$/.exec(card)
    if (!match) {
      errors.push(`未知卡牌：${card}`)
      continue
    }
    const level = Number(match[2])
    levelTotal += level
    counts.set(card, (counts.get(card) ?? 0) + 1)
  }

  if (deck.cards.length !== 60) {
    errors.push(`牌組必須正好 60 張，目前為 ${deck.cards.length} 張`)
  }
  if (levelTotal > 170) {
    errors.push(`等級總和不得超過 170，目前為 ${levelTotal}`)
  }
  for (const [card, count] of counts) {
    const level = Number(card.at(-1))
    const maximum = level <= 3 ? 4 : 3
    if (count > maximum) {
      errors.push(`${card} 最多 ${maximum} 張，目前為 ${count} 張`)
    }
  }

  return {
    valid: errors.length === 0,
    cardCount: deck.cards.length,
    levelTotal,
    errors,
  }
}

export function effectiveDeck(deck: PlayerDeckList | undefined): {
  deck: PlayerDeckList
  source: 'custom' | 'preconstructed'
} {
  if (deck && validateDeck(deck).valid) {
    return { deck, source: 'custom' }
  }
  return { deck: preconstructedDeck(), source: 'preconstructed' }
}
