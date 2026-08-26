import { describe, expect, test } from 'bun:test'
import {
  isOnlineGameAction,
  invitationCredentialMatches,
  normalizeGameRoomMetadata,
  systemBattleRecord,
} from './game-room'
import type { OnlineGameAction, TrustedRandomnessRequest } from './game-room'
import { isDevelopmentScenario } from './development-scenarios'

test('development fixtures expose only the closed named scenario catalog', () => {
  expect(isDevelopmentScenario({ name: 'star-endgame' })).toBe(true)
  expect(isDevelopmentScenario({ name: 'echo-ringing-metal' })).toBe(true)
  expect(isDevelopmentScenario({ name: 'echo-split-earth' })).toBe(true)
  expect(isDevelopmentScenario({ name: 'tribulation-earth-rending' })).toBe(true)
  expect(isDevelopmentScenario({ name: 'tribulation-rusted-forest' })).toBe(true)
  expect(isDevelopmentScenario({ name: 'pouch-chain-sheep' })).toBe(true)
  expect(isDevelopmentScenario({ name: 'star-endgame', state: {} })).toBe(false)
  expect(isDevelopmentScenario({ name: 'arbitrary-state', state: {} })).toBe(false)
  expect(isDevelopmentScenario({ record: [] })).toBe(false)
})

test('waiting room retains visible legacy system notices in its preparation record', () => {
  const record = systemBattleRecord([{
    sequence: 7,
    type: 'LegacyGamePurged',
    payload: {},
    createdAt: '2026-08-24T00:00:00Z',
  }], () => ({
    title: '房間更新',
    summary: '系統版本更新，上一局已清除，請重新準備。',
  }))

  expect(record.preparation.entries).toEqual([{
    id: 'room-event-7',
    title: '房間更新',
    summary: '系統版本更新，上一局已清除，請重新準備。',
  }])
  expect(record.turns).toEqual([])
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

test('Secret Strategy uses the closed direct and Chain Decision wire shapes', () => {
  const direct: OnlineGameAction = {
    type: 'triggerSecretStrategy',
    player: 'alice',
    decision: {
      type: 'environment',
      sourceCard: 7,
      operation: { type: 'transferByDiscard', card: 8 },
    },
  }
  const chain: OnlineGameAction = {
    type: 'answerChoice',
    player: 'alice',
    choiceId: 9,
    answer: {
      type: 'chain',
      decision: {
        type: 'placeAndTrigger',
        pouchOwner: 'alice',
        pouchCard: 10,
        decision: { type: 'sheepStealing', sourceCard: 11 },
      },
    },
  }

  expect(direct).toStrictEqual({
    type: 'triggerSecretStrategy',
    player: 'alice',
    decision: {
      type: 'environment',
      sourceCard: 7,
      operation: { type: 'transferByDiscard', card: 8 },
    },
  })
  expect(chain).toStrictEqual({
    type: 'answerChoice',
    player: 'alice',
    choiceId: 9,
    answer: {
      type: 'chain',
      decision: {
        type: 'placeAndTrigger',
        pouchOwner: 'alice',
        pouchCard: 10,
        decision: { type: 'sheepStealing', sourceCard: 11 },
      },
    },
  })
})

test('pending randomness exposes only the input required to resolve it', () => {
  const request: TrustedRandomnessRequest = {
    requestId: 'spirit:death-omen:1:p1',
    operation: { type: 'discardShuffle', pile: 'Shared', placement: 'Bottom' },
    currentOrder: [4, 3, 2],
  }

  expect('continuation' in request).toBe(false)
})

test('online command transaction fields keep the exact camelCase wire contract', () => {
  const receipt = {
    gameInstanceId: 'game-1',
    transactionId: 'transaction-1',
    committedSequence: 8,
    transactionStatus: 'awaitingChoice',
  }
  const wire = JSON.stringify(receipt)

  expect(wire).toContain('gameInstanceId')
  expect(wire).toContain('transactionId')
  expect(wire).toContain('committedSequence')
  expect(wire).toContain('transactionStatus')
  expect(wire).not.toContain('game_instance_id')
  expect(wire).not.toContain('transaction_id')
})

test('active Game Record versions keep the exact camelCase delivery contract', () => {
  const wire = JSON.stringify({
    activeGameVersion: { gameInstanceId: 'game-1', recordSequence: 8 },
  })

  expect(wire).toContain('activeGameVersion')
  expect(wire).toContain('gameInstanceId')
  expect(wire).toContain('recordSequence')
  expect(wire).not.toContain('record_sequence')
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

    expect(metadata.schemaVersion).toBe(5)
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
    expect(metadata.observers).toStrictEqual([])
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
