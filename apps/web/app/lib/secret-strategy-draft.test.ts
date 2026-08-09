import { describe, expect, test } from 'bun:test'
import type { SecretStrategyOption } from '../types/fewfc'
import { secretStrategyDraftAction } from './secret-strategy-draft'

const baseDraft: Omit<SecretStrategyOption, 'strategy' | 'input'> = {
  sourceCard: 10,
  targetPlayers: ['alice', 'bob'],
  stars: ['Metal', 'Wood'],
  breakStars: ['Fire'],
  deckCards: [],
  discardCards: [],
  handCards: [41, 42],
  requiredCardCount: 0,
  detail: { consequences: [] },
}

describe('secretStrategyDraftAction', () => {
  test('does not produce a command until a legal local input is selected', () => {
    for (const draft of [
      { ...baseDraft, strategy: 'LureTheTigerAway' as const, input: 'targetPlayer' as const },
      { ...baseDraft, strategy: 'DeceiveHeaven' as const, input: 'star' as const },
      { ...baseDraft, strategy: 'Retreat' as const, input: 'retreat' as const },
    ]) {
      expect(secretStrategyDraftAction(draft, {})).toBeUndefined()
    }
  })

  test('produces the exact typed action inputs for target, star, and retreat drafts', () => {
    expect(secretStrategyDraftAction(
      { ...baseDraft, strategy: 'LureTheTigerAway', input: 'targetPlayer' },
      { targetPlayer: 'bob' },
    )).toEqual({ strategy: 'LureTheTigerAway', options: { targetPlayer: 'bob' } })
    expect(secretStrategyDraftAction(
      { ...baseDraft, strategy: 'DeceiveHeaven', input: 'star' },
      { star: 'Fire', breakStar: true },
    )).toEqual({ strategy: 'DeceiveHeaven', options: { star: 'Fire', breakStar: true } })
    expect(secretStrategyDraftAction(
      { ...baseDraft, strategy: 'Retreat', input: 'retreat' },
      { retreat: 41 },
    )).toEqual({ strategy: 'Retreat', options: { discardCard: 41 } })
    expect(secretStrategyDraftAction(
      { ...baseDraft, strategy: 'Retreat', input: 'retreat' },
      { retreat: 'clearEnvironment' },
    )).toEqual({ strategy: 'Retreat', options: {} })
  })

  test('rejects stale or forbidden local selections instead of creating a command', () => {
    expect(secretStrategyDraftAction(
      { ...baseDraft, strategy: 'LureTheTigerAway', input: 'targetPlayer' },
      { targetPlayer: 'mallory' },
    )).toBeUndefined()
    expect(secretStrategyDraftAction(
      { ...baseDraft, strategy: 'DeceiveHeaven', input: 'star' },
      { star: 'Metal', breakStar: true },
    )).toBeUndefined()
    expect(secretStrategyDraftAction(
      { ...baseDraft, strategy: 'Retreat', input: 'retreat' },
      { retreat: 99 },
    )).toBeUndefined()
  })
})
