import { expect, mock, test } from 'bun:test'

mock.module('cloudflare:workers', () => ({
  DurableObject: class {
    protected ctx: unknown
    protected env: unknown

    constructor(ctx: unknown, env: unknown) {
      this.ctx = ctx
      this.env = env
    }
  },
}))

const { GameRoom } = await import('./game-room')

class MemoryStorage {
  readonly values = new Map<string, unknown>([['orphan-marker', 'keep']])
  deleteAllCalls = 0

  async get<T>(key: string): Promise<T | undefined> {
    return this.values.get(key) as T | undefined
  }

  async put(key: string, value: unknown): Promise<void> {
    this.values.set(key, value)
  }

  async delete(key: string | string[]): Promise<boolean> {
    if (Array.isArray(key)) {
      let deleted = false
      for (const entry of key) deleted = this.values.delete(entry) || deleted
      return deleted
    }
    return this.values.delete(key)
  }

  async deleteAll(): Promise<void> {
    this.deleteAllCalls += 1
    this.values.clear()
  }

  async list<T>(): Promise<Map<string, T>> {
    return new Map(this.values as Map<string, T>)
  }
}

function gameRoom(storage: MemoryStorage) {
  const state = {
    storage,
    waitUntil() {},
  }
  const env = { PLAYER_NOTIFICATIONS: {} }
  return new GameRoom(
    state as unknown as DurableObjectState,
    env as never,
  )
}

function activeMetadata() {
  const timestamp = '2026-09-09T00:00:00.000Z'
  return {
    schemaVersion: 5,
    gameId: 'active-legacy-room',
    name: '既有對局',
    access: 'public',
    capacity: 2,
    ruleset: 'fewfc-base',
    ruleVersion: '5.17',
    enabledRuleModules: [],
    players: ['player-1', 'player-2'],
    members: [{
      userId: 'traveler',
      displayName: '旅人',
      player: 'player-1',
      ready: true,
      connected: false,
      owner: true,
    }],
    observers: [],
    status: 'Active',
    createdAt: timestamp,
    updatedAt: timestamp,
  }
}

test('空的 GameRoom 對外查詢回傳 404，且不會因讀取清除 storage', async () => {
  const storage = new MemoryStorage()
  const room = gameRoom(storage)

  const publicResponse = await room.fetch(new Request('https://game-room.internal/'))
  expect(publicResponse.status).toBe(404)
  expect(await publicResponse.json()).toEqual({ error: 'room not found' })

  const stateResponse = await room.fetch(new Request('https://game-room.internal/', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ type: 'getState', actorUserId: 'traveler' }),
  }))
  expect(stateResponse.status).toBe(404)
  expect(await stateResponse.json()).toEqual({ error: 'room not found' })
  expect(storage.deleteAllCalls).toBe(0)
  expect(storage.values.get('orphan-marker')).toBe('keep')
})

test('Waiting 與 Dissolved 房間沒有 Game Record 仍會被 management probe 保留', async () => {
  for (const status of ['Waiting', 'Dissolved']) {
    const storage = new MemoryStorage()
    storage.values.set('metadata', { ...activeMetadata(), status })
    const room = gameRoom(storage)

    const probeResponse = await room.fetch(new Request('https://game-room.internal/manage/probe-legacy', {
      method: 'POST',
    }))
    expect(probeResponse.status).toBe(200)
    expect(await probeResponse.json()).toEqual({
      status: 'preserved',
      gameId: 'active-legacy-room',
    })

    const purgeResponse = await room.fetch(new Request('https://game-room.internal/manage/purge-legacy', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ epoch: 'cutover-75' }),
    }))
    expect(purgeResponse.status).toBe(200)
    expect(await purgeResponse.json()).toEqual({
      status: 'preserved',
      gameId: 'active-legacy-room',
      epoch: 'cutover-75',
    })
    expect(storage.deleteAllCalls).toBe(0)
    expect(storage.values.has('metadata')).toBe(true)
  }
})

test('Active 房間缺少舊版 Game Record 時會被 management probe 判定為 broken 並清除', async () => {
  const storage = new MemoryStorage()
  storage.values.set('metadata', activeMetadata())
  const room = gameRoom(storage)

  const stateResponse = await room.fetch(new Request('https://game-room.internal/', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ type: 'getState', actorUserId: 'traveler' }),
  }))
  expect(stateResponse.status).toBe(500)
  expect(await stateResponse.json()).toEqual({
    error: '房間服務暫時無法處理要求，請稍後再試。',
    code: 'gameRoomFailure',
  })

  const probeResponse = await room.fetch(new Request('https://game-room.internal/manage/probe-legacy', {
    method: 'POST',
  }))
  expect(probeResponse.status).toBe(500)
  expect(await probeResponse.json()).toEqual({
    status: 'broken',
    gameId: 'active-legacy-room',
  })

  const purgeResponse = await room.fetch(new Request('https://game-room.internal/manage/purge-legacy', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ epoch: 'cutover-75' }),
  }))
  expect(purgeResponse.status).toBe(500)
  expect(await purgeResponse.json()).toEqual({
    status: 'broken',
    gameId: 'active-legacy-room',
    epoch: 'cutover-75',
  })
  expect(storage.deleteAllCalls).toBe(1)
  expect(storage.values.has('metadata')).toBe(false)
})

test('Finished 房間缺少 Game Record 或仍是舊 schema 時會被判定為 broken 並清除', async () => {
  for (const gameRecord of [undefined, { schemaVersion: 6 }]) {
    const storage = new MemoryStorage()
    storage.values.set('metadata', { ...activeMetadata(), status: 'Finished' })
    if (gameRecord) storage.values.set('gameRecord', gameRecord)
    const room = gameRoom(storage)

    const response = await room.fetch(new Request('https://game-room.internal/', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ type: 'getState', actorUserId: 'traveler' }),
    }))
    expect(response.status).toBe(500)
    expect(await response.json()).toEqual({
      error: '房間服務暫時無法處理要求，請稍後再試。',
      code: 'gameRoomFailure',
    })

    const purgeResponse = await room.fetch(new Request('https://game-room.internal/manage/purge-legacy', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ epoch: 'cutover-75' }),
    }))
    expect(purgeResponse.status).toBe(500)
    expect(await purgeResponse.json()).toEqual({
      status: 'broken',
      gameId: 'active-legacy-room',
      epoch: 'cutover-75',
    })
    expect(storage.deleteAllCalls).toBe(1)
    expect(storage.values.has('metadata')).toBe(false)
  }
})
