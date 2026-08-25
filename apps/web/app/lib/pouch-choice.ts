import type { PublicCard } from '../types/fewfc'

export function sheepReturnCards(
  allowedCardIds: number[],
  discardCards: PublicCard[],
  deckCards: PublicCard[],
  selectedDeckCardIds: number[],
): PublicCard[] {
  const allowed = new Set(allowedCardIds)
  const selectedDeckCards = new Set(selectedDeckCardIds)
  const cardsById = new Map<number, PublicCard>()

  for (const card of discardCards) {
    if (allowed.has(card.id)) cardsById.set(card.id, card)
  }
  for (const card of deckCards) {
    if (allowed.has(card.id) && selectedDeckCards.has(card.id)) {
      cardsById.set(card.id, card)
    }
  }

  return [...cardsById.values()]
}
