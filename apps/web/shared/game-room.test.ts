import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import {
  continuesPendingCommandDraft,
  invitationCredentialMatches,
  normalizeGameRoomMetadata,
  normalizeRuleModules,
  requiresPendingCommandDraft,
} from './game-room'

test('all released advanced rules are available default Rule Modules', () => {
  assert.deepEqual(normalizeRuleModules(undefined), [
    'star',
    'hero-schools',
    'discard-retrieval',
    'personal-deck',
    'five-directions-legend',
    'spirit',
  ])
  assert.deepEqual(normalizeRuleModules(['star', 'five-directions-legend', 'unknown']), [
    'star',
    'five-directions-legend',
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

    assert.deepEqual(metadata.members, [
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

    assert.equal(metadata.schemaVersion, 3)
    assert.deepEqual(metadata.enabledRuleModules, [])
    assert.equal(metadata.name, 'version-one-room')
    assert.equal(metadata.capacity, 2)
    assert.deepEqual(metadata.members, [{
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

    assert.equal(metadata.enabledRuleModules.includes('spirit'), false)
  })
})

describe('invitationCredentialMatches', () => {
  const invitation = {
    roomCode: 'ABCD234',
    inviteToken: 'token-value',
  }

  test('accepts the matching token or case-insensitive room code', () => {
    assert.equal(invitationCredentialMatches(invitation, {
      type: 'token',
      value: 'token-value',
    }), true)
    assert.equal(invitationCredentialMatches(invitation, {
      type: 'code',
      value: 'abcd234',
    }), true)
  })

  test('rejects absent and incorrect credentials', () => {
    assert.equal(invitationCredentialMatches(invitation, undefined), false)
    assert.equal(invitationCredentialMatches(invitation, {
      type: 'token',
      value: 'wrong',
    }), false)
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
    assert.equal(requiresPendingCommandDraft(formation, 'EffectGenerated'), true)
    assert.equal(requiresPendingCommandDraft(formation, 'TurnDrawDiscard'), false)
    assert.equal(requiresPendingCommandDraft({ type: 'passAction' }, 'EffectGenerated'), false)
  })
})

describe('continuesPendingCommandDraft', () => {
  test('continues only for another effect-generated choice', () => {
    assert.equal(continuesPendingCommandDraft('EffectGenerated'), true)
    assert.equal(continuesPendingCommandDraft('TurnDrawDiscard'), false)
    assert.equal(continuesPendingCommandDraft(undefined), false)
  })
})
