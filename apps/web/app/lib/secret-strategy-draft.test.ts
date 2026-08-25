import { describe, expect, test } from 'bun:test'
import type { PlayerFacingActionDetail, SecretStrategyOption } from '../types/fewfc'
import { secretStrategyDraftAction } from './secret-strategy-draft'

const detail: PlayerFacingActionDetail = { consequences: [] }
const targetDraft: SecretStrategyOption = {
  sourceCard: 10,
  targetPlayers: ['alice', 'bob'],
  type: 'targetPlayer',
  detail,
}

describe('secretStrategyDraftAction', () => {
  test('does not produce a Decision until an answer-shaped input is complete', () => {
    for (const draft of [
      targetDraft,
      { type: 'star', sourceCard: 10, gainStars: ['Metal'], breakStars: ['Fire'], detail },
      { type: 'environment', sourceCard: 10, handCards: [41], detail },
    ] satisfies SecretStrategyOption[]) {
      expect(secretStrategyDraftAction(draft, {})).toBeUndefined()
    }
  })

  test('maps every answer family to its closed Decision', () => {
    expect(secretStrategyDraftAction(
      targetDraft,
      { targetPlayer: 'bob' },
    )).toEqual({ type: 'targetPlayer', sourceCard: 10, targetPlayer: 'bob' })
    expect(secretStrategyDraftAction(
      { type: 'star', sourceCard: 10, gainStars: ['Metal'], breakStars: ['Fire'], detail },
      { star: 'Fire', breakStar: true },
    )).toEqual({ type: 'star', sourceCard: 10, operation: { type: 'break', star: 'Fire' } })
    expect(secretStrategyDraftAction(
      { type: 'environment', sourceCard: 10, handCards: [41], detail },
      { retreat: 41 },
    )).toEqual({
      type: 'environment',
      sourceCard: 10,
      operation: { type: 'transferByDiscard', card: 41 },
    })
    expect(secretStrategyDraftAction(
      { type: 'environment', sourceCard: 10, handCards: [41], detail },
      { retreat: 'clearEnvironment' },
    )).toEqual({ type: 'environment', sourceCard: 10, operation: { type: 'clear' } })
    expect(secretStrategyDraftAction(
      { type: 'noInput', sourceCard: 10, strategy: 'GoldenCicada', detail },
      {},
    )).toEqual({ type: 'noInput', sourceCard: 10, strategy: 'GoldenCicada' })
    expect(secretStrategyDraftAction(
      { type: 'sheepStealing', sourceCard: 10, detail },
      {},
    )).toEqual({ type: 'sheepStealing', sourceCard: 10 })
  })
})
