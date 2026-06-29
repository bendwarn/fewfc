import type { CardInstanceId } from '~/types/fewfc'

export function togglePendingChoiceSelection(
  selectedCards: CardInstanceId[],
  card: CardInstanceId,
  requiredCount: number,
): CardInstanceId[] {
  if (selectedCards.includes(card)) {
    return selectedCards.filter(selected => selected !== card)
  }

  if (selectedCards.length >= requiredCount) {
    return selectedCards
  }

  return [...selectedCards, card]
}

export function isPendingChoiceComplete(
  selectedCards: CardInstanceId[],
  requiredCount: number,
): boolean {
  return requiredCount > 0 && selectedCards.length === requiredCount
}

export function completedPendingChoiceSelection(
  selectedCards: CardInstanceId[],
  requiredCount: number,
): CardInstanceId[] | undefined {
  return isPendingChoiceComplete(selectedCards, requiredCount)
    ? [...selectedCards]
    : undefined
}
