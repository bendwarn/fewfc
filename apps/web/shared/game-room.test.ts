import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { normalizeGameRoomMetadata } from './game-room'

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

    assert.equal(metadata.schemaVersion, 2)
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
})
