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

test('development fixtures expose only the closed named scenario catalog', () => {
  expect(isDevelopmentScenario({ name: 'star-endgame' })).toBe(true)
  expect(isDevelopmentScenario({ name: 'tribulation-earth-rending' })).toBe(true)
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
    type: 'answerEffectChoiceTyped',
    player: 'alice',
    answer: { type: 'decline' },
  })).toBe(true)
})

test('trusted randomness resolves sequential requests as distinct persisted decisions', async () => {
  type Result = {
    marker: string
    pendingRandomnessRequest?: {
      requestId: string
      currentOrder: number[]
    }
  }
  const persisted: string[] = []
  const actions: Array<{ requestId: string; shuffledOrder: number[] }> = []
  const responses: Result[] = [
    {
      marker: 'after-first',
      pendingRandomnessRequest: {
        requestId: 'post-search',
        currentOrder: [4, 5],
      },
    },
    { marker: 'complete' },
  ]

  const result = await resolvePendingRandomnessSequence<Result>(
    {
      marker: 'initial',
      pendingRandomnessRequest: {
        requestId: 'discard-recycle',
        currentOrder: [1, 2, 3],
      },
    },
    cards => [...cards].reverse(),
    async (current) => {
      persisted.push(current.pendingRandomnessRequest?.requestId ?? '')
    },
    async (action) => {
      actions.push(action)
      return responses.shift() as Result
    },
  )

  expect(result.marker).toBe('complete')
  expect(persisted).toEqual(['discard-recycle', 'post-search'])
  expect(actions).toEqual([
    { type: 'resolveRandomness', requestId: 'discard-recycle', shuffledOrder: [3, 2, 1] },
    { type: 'resolveRandomness', requestId: 'post-search', shuffledOrder: [5, 4] },
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

    expect(metadata.members).toEqual([
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
    expect(metadata.enabledRuleModules).toEqual([])
    expect(metadata.name).toBe('version-one-room')
    expect(metadata.capacity).toBe(2)
    expect(metadata.members).toEqual([{
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

  test('creates drafts only for effect-generated formation choices', () => {
    expect(requiresPendingCommandDraft(formation, 'EffectGenerated')).toBe(true)
    expect(requiresPendingCommandDraft(formation, 'TurnDrawDiscard')).toBe(false)
    expect(requiresPendingCommandDraft({ type: 'passAction' }, 'EffectGenerated')).toBe(false)
  })
})

describe('continuesPendingCommandDraft', () => {
  test('continues only for another effect-generated choice', () => {
    expect(continuesPendingCommandDraft('EffectGenerated')).toBe(true)
    expect(continuesPendingCommandDraft('TurnDrawDiscard')).toBe(false)
    expect(continuesPendingCommandDraft(undefined)).toBe(false)
  })
})
