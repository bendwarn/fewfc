import type {
  CardInstanceId,
  ChoiceAnswer,
  Element,
  PendingChoice,
  PlayerId,
  PublicPendingChoice,
  PublicCard,
  VisiblePendingChoice,
} from '../types/fewfc'

export type CardPendingChoice = Extract<PendingChoice, { type: 'card' }>
export type VisibleCardPendingChoice = VisiblePendingChoice & { choice: CardPendingChoice }
export type CardsChoiceAnswer = Extract<ChoiceAnswer, { type: 'cards' }>

export function visiblePendingChoice(
  choice: PublicPendingChoice | null,
): VisiblePendingChoice | null {
  return choice?.visibility === 'visible' ? choice : null
}

export function visibleCardPendingChoice(
  choice: PublicPendingChoice | null,
): VisibleCardPendingChoice | null {
  const visible = visiblePendingChoice(choice)
  return visible?.choice.type === 'card' ? visible as VisibleCardPendingChoice : null
}

export function pendingChoiceKey(choice: PublicPendingChoice | null): string {
  if (!choice) return ''
  if (choice.visibility === 'hidden') return `hidden:${choice.player}:${choice.reason.type}`
  return `${choice.choiceId}:${choice.player}:${choice.reason.type}:${choice.choice.type}`
}

/**
 * A choice draft belongs to the choice instance, not merely to its shape.  A
 * reconnect intentionally starts a fresh local draft even when the server is
 * still waiting for the same choice.
 */
export function shouldResetPendingChoiceDraft(
  previous: PublicPendingChoice | null,
  next: PublicPendingChoice | null,
  reconnected = false,
): boolean {
  return reconnected || pendingChoiceKey(previous) !== pendingChoiceKey(next)
}

export function toggleChoiceCard(
  selectedCards: CardInstanceId[],
  card: CardInstanceId,
  maximum: number,
): CardInstanceId[] {
  if (selectedCards.includes(card)) return selectedCards.filter(selected => selected !== card)
  return selectedCards.length < maximum ? [...selectedCards, card] : selectedCards
}

/**
 * Card choices with one maximum result are answers, rather than local drafts.
 * Clear Wind retains its canonical zero-or-one Cards encoding, but presents
 * the revealed Card as the keep-on-deck outcome.
 */
export function isImmediateCardChoice(choice: CardPendingChoice): boolean {
  return choice.maximum === 1
}

export function cardChoiceDraftCount(
  choice: CardPendingChoice,
  selectedCount: number,
): string {
  return `已選 ${selectedCount}（${choice.minimum}~${choice.maximum}）`
}

export function immediateCardChoiceAnswer(
  choice: VisibleCardPendingChoice,
  card: PublicCard,
): CardsChoiceAnswer | undefined {
  if (
    !isImmediateCardChoice(choice.choice)
    || !choice.choice.cards.some(candidate => candidate.id === card.id)
  ) {
    return undefined
  }

  return {
    type: 'cards',
    cards: choice.reason.type === 'clearWind' ? [] : [card.id],
  }
}

export function clearWindDiscardAnswer(
  choice: VisibleCardPendingChoice,
): CardsChoiceAnswer | undefined {
  const card = choice.choice.cards[0]
  if (choice.reason.type !== 'clearWind' || !isImmediateCardChoice(choice.choice) || !card) {
    return undefined
  }

  return { type: 'cards', cards: [card.id] }
}

export function cardChoiceIsComplete(
  choice: Extract<PendingChoice, { type: 'card' }>,
  selectedCards: CardInstanceId[],
): boolean {
  return selectedCards.length >= choice.minimum && selectedCards.length <= choice.maximum
}

export function cardChoiceAnswer(
  choice: Extract<PendingChoice, { type: 'card' }>,
  selectedCards: CardInstanceId[],
): Extract<ChoiceAnswer, { type: 'cards' }> | undefined {
  return cardChoiceIsComplete(choice, selectedCards)
    ? { type: 'cards', cards: selectedCards }
    : undefined
}

export function playerChoiceAnswer(
  player: PlayerId,
): Extract<ChoiceAnswer, { type: 'player' }> {
  return { type: 'player', player }
}

export function formationChoiceAnswer(
  formationId: string,
): Extract<ChoiceAnswer, { type: 'formation' }> {
  return { type: 'formation', formationId }
}

export function environmentChoiceAnswer(
  environment: Element,
): Extract<ChoiceAnswer, { type: 'environment' }> {
  return { type: 'environment', environment }
}

export function declineChoiceAnswer(): Extract<ChoiceAnswer, { type: 'decline' }> {
  return { type: 'decline' }
}

export function chainChoiceAnswer(
  answer: Omit<Extract<ChoiceAnswer, { type: 'chain' }>, 'type'>,
): Extract<ChoiceAnswer, { type: 'chain' }> {
  return { type: 'chain', ...answer }
}

export function sheepStealingChoiceAnswer(
  deckCards: CardInstanceId[],
  discardCards: CardInstanceId[],
): Extract<ChoiceAnswer, { type: 'sheepStealing' }> {
  return { type: 'sheepStealing', deckCards, discardCards }
}
