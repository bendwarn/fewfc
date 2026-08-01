import { describe, expect, test } from 'bun:test'
import {
  continuesPendingCommandDraft,
  isOnlineGameAction,
  invitationCredentialMatches,
  normalizeGameRoomMetadata,
  resolvePendingRandomnessSequence,
  requiresPendingCommandDraft,
} from './game-room'
import { isDevelopmentScenario } from './development-scenarios'
import type { TrustedRandomnessAction } from './game-room'

test('development fixtures expose only the closed named scenario catalog', () => {
  expect(isDevelopmentScenario({ name: 'star-endgame' })).toBe(true)
  expect(isDevelopmentScenario({ name: 'echo-split-earth' })).toBe(true)
  expect(isDevelopmentScenario({ name: 'tribulation-earth-rending' })).toBe(true)
  expect(isDevelopmentScenario({ name: 'tribulation-rusted-forest' })).toBe(true)
  expect(isDevelopmentScenario({ name: 'pouch-chain-sheep' })).toBe(true)
  expect(isDevelopmentScenario({ name: 'star-endgame', state: {} })).toBe(false)
  expect(isDevelopmentScenario({ name: 'arbitrary-state', state: {} })).toBe(false)
  expect(isDevelopmentScenario({ record: [] })).toBe(false)
})

test('trusted randomness actions are not player-submittable', () => {
  expect(isOnlineGameAction({
    type: 'resolveRandomness',
    requestId: 'request',
    shuffledOrder: [3, 1, 2],
  })).toBe(false)
  expect(isOnlineGameAction({
    type: 'answerChoice',
    player: 'alice',
    choiceId: 7,
    answer: { type: 'decline' },
  })).toBe(true)
})

test('trusted randomness resolves sequential requests as distinct persisted decisions', async () => {
  type Ready = {
    type: 'ready'
    marker: string
  }
  type NeedsRandomness = {
    type: 'needsRandomness'
    marker: string
    record: string[]
    request: {
      requestId: string
      operation: { type: 'deckShuffle'; deck: 'Shared' }
      continuation: { type: 'echo'; kind: 'ringingMetalRecycleDiscard' | 'ringingMetalPostSearch' }
      currentOrder: number[]
    }
  }
  type Result = Ready | NeedsRandomness
  const persisted: string[] = []
  const actions: TrustedRandomnessAction[] = []
  const responses: Result[] = [
    {
      type: 'needsRandomness',
      marker: 'after-first',
      record: ['command', 'first-shuffle'],
      request: {
        requestId: 'post-search',
        operation: { type: 'deckShuffle', deck: 'Shared' },
        continuation: { type: 'echo', kind: 'ringingMetalPostSearch' },
        currentOrder: [4, 5],
      },
    },
    { type: 'ready', marker: 'complete' },
  ]

  const result = await resolvePendingRandomnessSequence<Ready, NeedsRandomness>(
    {
      type: 'needsRandomness',
      marker: 'initial',
      record: ['command'],
      request: {
        requestId: 'discard-recycle',
        operation: { type: 'deckShuffle', deck: 'Shared' },
        continuation: { type: 'echo', kind: 'ringingMetalRecycleDiscard' },
        currentOrder: [1, 2, 3],
      },
    },
    cards => [...cards].reverse(),
    async (current) => {
      persisted.push(current.request.requestId)
    },
    async (action) => {
      actions.push(action)
      return responses.shift() as Result
    },
  )

  expect(result.type).toBe('ready')
  expect(result.marker).toBe('complete')
  expect(persisted).toStrictEqual(['discard-recycle', 'post-search'])
  expect(actions).toStrictEqual([
    { type: 'resolveRandomness', requestId: 'discard-recycle', shuffledOrder: [3, 2, 1] },
    { type: 'resolveRandomness', requestId: 'post-search', shuffledOrder: [5, 4] },
  ])
})

test('trusted randomness persists one continuation before returning ready', async () => {
  type Ready = { type: 'ready'; marker: 'complete' }
  type NeedsRandomness = {
    type: 'needsRandomness'
    record: string[]
    request: {
      requestId: string
      operation: { type: 'deckShuffle'; deck: 'Shared' }
      continuation: { type: 'tribulation'; kind: 'rustedForestShuffle' }
      currentOrder: number[]
    }
  }
  const sequence: string[] = []

  const result = await resolvePendingRandomnessSequence<Ready, NeedsRandomness>(
    {
      type: 'needsRandomness',
      record: ['rusted-forest-command'],
      request: {
        requestId: 'rusted-forest-shuffle',
        operation: { type: 'deckShuffle', deck: 'Shared' },
        continuation: { type: 'tribulation', kind: 'rustedForestShuffle' },
        currentOrder: [7, 8, 9],
      },
    },
    cards => [cards[1] as number, cards[2] as number, cards[0] as number],
    async (current) => {
      sequence.push(`persist:${current.record.join(',')}`)
    },
    async (action) => {
      sequence.push(`resolve:${action.requestId}:${action.shuffledOrder.join(',')}`)
      return { type: 'ready', marker: 'complete' }
    },
  )

  expect(result).toStrictEqual({ type: 'ready', marker: 'complete' })
  expect(sequence).toStrictEqual([
    'persist:rusted-forest-command',
    'resolve:rusted-forest-shuffle:8,9,7',
  ])
})

describe('normalizeGameRoomMetadata', () => {
  test('restores the first member as owner when persisted owner flags are false', () => {
    const metadata = normalizeGameRoomMetadata({
      schemaVersion: 2,
      gameId: 'legacy-room',
      name: 'Legacy room',
      access: 'public',
      capacity: 2,
      ruleset: 'fewfc-base',
      players: ['bob', 'alice'],
      members: [
        {
          userId: 'bob-user',
          displayName: 'Bob',
          player: 'bob',
          ready: true,
          connected: true,
          owner: false,
        },
        {
          userId: 'alice-user',
          displayName: 'Alice',
          player: 'alice',
          ready: true,
          connected: true,
          owner: false,
        },
      ],
      status: 'Waiting',
      createdAt: '2026-06-28T00:00:00.000Z',
      updatedAt: '2026-06-28T00:00:00.000Z',
    })

    expect(metadata.members).toStrictEqual([
      {
        userId: 'bob-user',
        displayName: 'Bob',
        player: 'bob',
        ready: false,
        connected: true,
        owner: true,
      },
      {
        userId: 'alice-user',
        displayName: 'Alice',
        player: 'alice',
        ready: true,
        connected: true,
        owner: false,
      },
    ])
  })

  test('upgrades schema version one metadata', () => {
    const metadata = normalizeGameRoomMetadata({
      schemaVersion: 1,
      gameId: 'version-one-room',
      access: 'private',
      ruleset: 'fewfc-base',
      players: ['alice', 'bob'],
      members: [
        {
          userId: 'alice-user',
          player: 'alice',
          ready: true,
        },
      ],
      status: 'Waiting',
      createdAt: '2026-06-28T00:00:00.000Z',
      updatedAt: '2026-06-28T00:00:00.000Z',
    })

    expect(metadata.schemaVersion).toBe(3)
    expect(metadata.enabledRuleModules).toStrictEqual([])
    expect(metadata.name).toBe('version-one-room')
    expect(metadata.capacity).toBe(2)
    expect(metadata.members).toStrictEqual([{
      userId: 'alice-user',
      displayName: 'alice',
      player: 'alice',
      ready: false,
      connected: false,
      owner: true,
    }])
  })

  test('preserves stored rooms without newly released Spirit', () => {
    const metadata = normalizeGameRoomMetadata({
      schemaVersion: 3,
      gameId: 'pre-spirit-room',
      name: 'Pre-Spirit room',
      access: 'public',
      capacity: 2,
      ruleset: 'fewfc-base',
      enabledRuleModules: ['star', 'hero-schools', 'five-directions-legend'],
      players: ['alice', 'bob'],
      members: [{
        userId: 'alice-user',
        displayName: 'Alice',
        player: 'alice',
        ready: false,
        owner: true,
      }],
      status: 'Waiting',
      createdAt: '2026-06-28T00:00:00.000Z',
      updatedAt: '2026-06-28T00:00:00.000Z',
    })

    expect(metadata.enabledRuleModules.includes('spirit')).toBe(false)
  })
})

describe('invitationCredentialMatches', () => {
  const invitation = {
    roomCode: 'ABCD234',
    inviteToken: 'token-value',
  }

  test('accepts the matching token or case-insensitive room code', () => {
    expect(invitationCredentialMatches(invitation, {
      type: 'token',
      value: 'token-value',
    })).toBe(true)
    expect(invitationCredentialMatches(invitation, {
      type: 'code',
      value: 'abcd234',
    })).toBe(true)
  })

  test('rejects absent and incorrect credentials', () => {
    expect(invitationCredentialMatches(invitation, undefined)).toBe(false)
    expect(invitationCredentialMatches(invitation, {
      type: 'token',
      value: 'wrong',
    })).toBe(false)
  })
})

describe('requiresPendingCommandDraft', () => {
  const formation = {
    type: 'performFormation' as const,
    player: 'alice',
    formationId: 'fire-strike',
    cards: [1],
  }
  const stagedChoice = {
    visibility: 'visible' as const,
    choiceId: 7,
    player: 'alice',
    reason: { type: 'chaos' as const },
    choice: { type: 'card' as const, cards: [], minimum: 2, maximum: 2, canDecline: false },
  }
  const turnDrawChoice = {
    ...stagedChoice,
    reason: { type: 'turnDrawDiscard' as const },
  }

  test('keeps the originating command through staged typed choices', () => {
    expect(requiresPendingCommandDraft(formation, stagedChoice)).toBe(true)
    expect(requiresPendingCommandDraft({
      type: 'triggerSecretStrategy',
      player: 'alice',
      strategy: 'SheepStealing',
    }, stagedChoice)).toBe(true)
    expect(requiresPendingCommandDraft(formation, turnDrawChoice)).toBe(false)
    expect(requiresPendingCommandDraft({
      type: 'passAction',
      reason: 'CannotActByStatus',
    }, stagedChoice)).toBe(false)
  })
})

describe('continuesPendingCommandDraft', () => {
  test('continues across visible effect choices but not turn draw choices', () => {
    const stagedChoice = {
      visibility: 'visible' as const,
      choiceId: 7,
      player: 'alice',
      reason: { type: 'sheepStealing' as const },
      choice: { type: 'sheepStealing' as const, deckCards: [], discardCards: [] },
    }
    expect(continuesPendingCommandDraft(stagedChoice)).toBe(true)
    expect(continuesPendingCommandDraft({
      ...stagedChoice,
      reason: { type: 'turnDrawDiscard' as const },
    })).toBe(false)
    expect(continuesPendingCommandDraft(null)).toBe(false)
  })
})
