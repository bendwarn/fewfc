import { describe, expect, test as it } from 'bun:test'
import {
  cardChoiceAnswer,
  cardChoiceDraftCount,
  chainChoiceAnswer,
  clearWindDiscardAnswer,
  declineChoiceAnswer,
  environmentChoiceAnswer,
  formationChoiceAnswer,
  immediateCardChoiceAnswer,
  isImmediateCardChoice,
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

  it('submits one-card choices directly but keeps larger choices as drafts', () => {
    const oneCardChoice = {
      visibility: 'visible' as const,
      choiceId: 8,
      player: 'p1',
      reason: { type: 'turnDrawDiscard' as const },
      choice: {
        type: 'card' as const,
        cards: [{ id: 41, label: '金 1', element: 'Metal' as const, level: 1, secretStrategies: [] }],
        minimum: 1,
        maximum: 1,
        canDecline: false,
      },
    }

    expect(isImmediateCardChoice(oneCardChoice.choice)).toBeTrue()
    expect(immediateCardChoiceAnswer(oneCardChoice, oneCardChoice.choice.cards[0]!)).toEqual({
      type: 'cards',
      cards: [41],
    })
    expect(isImmediateCardChoice(cardChoice.choice)).toBeFalse()
    expect(cardChoiceDraftCount({ ...cardChoice.choice, minimum: 0, maximum: 4 }, 1)).toBe('已選 1（0~4）')
    expect(immediateCardChoiceAnswer(cardChoice, { id: 1, label: '金 1', element: 'Metal', level: 1, secretStrategies: [] })).toBeUndefined()
  })

  it('maps Clear Wind keep and discard controls to its existing Cards answers', () => {
    const clearWindChoice = {
      visibility: 'visible' as const,
      choiceId: 9,
      player: 'p1',
      reason: { type: 'clearWind' as const },
      choice: {
        type: 'card' as const,
        cards: [{ id: 42, label: '木 2', element: 'Wood' as const, level: 2, secretStrategies: [] }],
        minimum: 0,
        maximum: 1,
        canDecline: false,
      },
    }

    expect(immediateCardChoiceAnswer(clearWindChoice, clearWindChoice.choice.cards[0]!)).toEqual({
      type: 'cards',
      cards: [],
    })
    expect(clearWindDiscardAnswer(clearWindChoice)).toEqual({
      type: 'cards',
      cards: [42],
    })
  })

  it('treats Echo payment and decline as explicit immediate answers', () => {
    const echoCostChoice = {
      visibility: 'visible' as const,
      choiceId: 10,
      player: 'p1',
      reason: { type: 'echoCost' as const, melody: 'ringingMetal' as const },
      choice: {
        type: 'card' as const,
        cards: [{ id: 43, label: '金 3', element: 'Metal' as const, level: 3, secretStrategies: [] }],
        minimum: 1,
        maximum: 1,
        canDecline: true,
      },
    }

    expect(immediateCardChoiceAnswer(echoCostChoice, echoCostChoice.choice.cards[0]!)).toEqual({
      type: 'cards',
      cards: [43],
    })
    expect(declineChoiceAnswer()).toEqual({ type: 'decline' })
  })

  it('does not create an immediate answer while a view mounts or reconnects', () => {
    const oneCardChoice = {
      visibility: 'visible' as const,
      choiceId: 11,
      player: 'p1',
      reason: { type: 'turnDrawDiscard' as const },
      choice: {
        type: 'card' as const,
        cards: [{ id: 44, label: '水 4', element: 'Water' as const, level: 4, secretStrategies: [] }],
        minimum: 1,
        maximum: 1,
        canDecline: false,
      },
    }

    expect(shouldResetPendingChoiceDraft(oneCardChoice, oneCardChoice, true)).toBeTrue()
    expect(cardChoiceAnswer(oneCardChoice.choice, [])).toBeUndefined()
    expect(immediateCardChoiceAnswer(oneCardChoice, oneCardChoice.choice.cards[0]!)).toEqual({
      type: 'cards',
      cards: [44],
    })
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
