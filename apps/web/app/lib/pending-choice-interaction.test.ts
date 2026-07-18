import { describe, expect, test as it } from 'bun:test'
import {
  cardChoiceAnswer,
  chainChoiceAnswer,
  declineChoiceAnswer,
  environmentChoiceAnswer,
  formationChoiceAnswer,
  pendingChoiceKey,
  playerChoiceAnswer,
  sheepStealingChoiceAnswer,
  shouldResetPendingChoiceDraft,
  toggleChoiceCard,
  visiblePendingChoice,
} from './pending-choice-interaction'

describe('Pending Choice interaction', () => {
  const cardChoice = {
    visibility: 'visible' as const,
    choiceId: 7,
    player: 'p1',
    reason: { type: 'chaos' as const },
    choice: {
      type: 'card' as const,
      cards: [],
      minimum: 2,
      maximum: 2,
      canDecline: false,
    },
  }

  it('keys choices by the canonical choice id and hides owner-only payloads', () => {
    expect(pendingChoiceKey(cardChoice)).toBe('7:p1:chaos:card')
    expect(visiblePendingChoice({ visibility: 'hidden', player: 'p1', reason: { type: 'chaos' } })).toBeNull()
  })

  it('builds a card answer only when the declared bounds are met', () => {
    expect(toggleChoiceCard([1], 2, 2)).toEqual([1, 2])
    expect(cardChoiceAnswer(cardChoice.choice, [1])).toBeUndefined()
    expect(cardChoiceAnswer(cardChoice.choice, [1, 2])).toEqual({ type: 'cards', cards: [1, 2] })
  })

  it('resets local drafts for a new choice or reconnect', () => {
    expect(shouldResetPendingChoiceDraft(cardChoice, cardChoice)).toBeFalse()
    expect(shouldResetPendingChoiceDraft(cardChoice, { ...cardChoice, choiceId: 8 })).toBeTrue()
    expect(shouldResetPendingChoiceDraft(cardChoice, cardChoice, true)).toBeTrue()
  })

  it('builds every non-card answer as its discriminated transport shape', () => {
    expect(playerChoiceAnswer('p2')).toEqual({ type: 'player', player: 'p2' })
    expect(formationChoiceAnswer('split-earth')).toEqual({ type: 'formation', formationId: 'split-earth' })
    expect(environmentChoiceAnswer('Fire')).toEqual({ type: 'environment', environment: 'Fire' })
    expect(declineChoiceAnswer()).toEqual({ type: 'decline' })
    expect(chainChoiceAnswer({ pouchOwner: 'p1', pouchCard: 1 })).toEqual({
      type: 'chain',
      pouchOwner: 'p1',
      pouchCard: 1,
    })
    expect(sheepStealingChoiceAnswer([1, 2], [3, 4])).toEqual({
      type: 'sheepStealing',
      deckCards: [1, 2],
      discardCards: [3, 4],
    })
  })
})
