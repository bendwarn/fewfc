import type { CardInstanceId, PublicGameState } from '../types/fewfc'

export function toggleActionDraftCard(
  selectedCards: readonly CardInstanceId[],
  card: CardInstanceId,
): CardInstanceId[] {
  return selectedCards.includes(card)
    ? selectedCards.filter(selected => selected !== card)
    : [...selectedCards, card]
}

export function actionDraftContextKey(state: PublicGameState): string {
  const hand = state.currentPlayer
    ? state.hands.find(candidate => candidate.player === state.currentPlayer)?.cards
    : undefined
  const visibleHand = hand?.kind === 'known'
    ? hand.cards.map(card => card.id).join(',')
    : hand?.kind === 'partiallyKnown'
      ? hand.cards.map(card => card?.id ?? '?').join(',')
      : String(hand?.count ?? 0)

  return [
    state.status,
    state.turnNumber,
    state.phase,
    state.currentPlayer ?? '',
    state.pendingChoice?.purpose ?? '',
    visibleHand,
  ].join(':')
}

export function reconcileActionDraft(
  selectedCards: readonly CardInstanceId[],
  previous: PublicGameState,
  next: PublicGameState,
): CardInstanceId[] {
  return actionDraftContextKey(previous) === actionDraftContextKey(next)
    ? [...selectedCards]
    : []
}
