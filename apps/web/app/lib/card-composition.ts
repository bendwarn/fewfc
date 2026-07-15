import type { CardInstanceId, PublicCard } from '../types/fewfc'
import { cardElementGlyph } from './card-face-presentation'

export const CARD_ELEMENTS = ['金', '木', '水', '火', '土'] as const
export const CARD_LEVELS = [1, 2, 3, 4, 5] as const

export type CardElement = typeof CARD_ELEMENTS[number]
export type CardLevel = typeof CARD_LEVELS[number]

export interface CardCompositionCell {
  element: CardElement
  level: CardLevel
  count: number
  cardIds: CardInstanceId[]
}

export interface CardCompositionRow {
  element: CardElement
  cells: CardCompositionCell[]
}

export type CardCompositionSelectionMode = 'replace' | 'toggle'

export function buildCardComposition(cards: PublicCard[]): CardCompositionRow[] {
  const cardIds = new Map<string, CardInstanceId[]>()

  for (const card of cards) {
    const element = card.element ? cardElementGlyph(card.element) : undefined
    const level = card.level

    if (!element || !CARD_LEVELS.includes(level as CardLevel)) continue

    const key = compositionKey(element, level as CardLevel)
    const matchingCards = cardIds.get(key) ?? []
    matchingCards.push(card.id)
    cardIds.set(key, matchingCards)
  }

  return CARD_ELEMENTS.map(element => ({
    element,
    cells: CARD_LEVELS.map((level) => {
      const matchingCards = cardIds.get(compositionKey(element, level)) ?? []
      return {
        element,
        level,
        count: matchingCards.length,
        cardIds: matchingCards,
      }
    }),
  }))
}

export function cardForCompositionSelection(
  cardIds: CardInstanceId[],
  selectedCards: CardInstanceId[],
  maximum: number,
  mode: CardCompositionSelectionMode,
): CardInstanceId | undefined {
  const selectedInCell = cardIds.filter(card => selectedCards.includes(card))

  if (mode === 'replace') {
    return selectedInCell[0] ?? cardIds[0]
  }

  if (selectedCards.length < maximum) {
    return cardIds.find(card => !selectedCards.includes(card))
  }

  return selectedInCell.at(-1)
}

function compositionKey(element: CardElement, level: CardLevel): string {
  return `${element}-${level}`
}
