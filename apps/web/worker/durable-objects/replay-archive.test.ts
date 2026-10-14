import { expect, mock, test } from 'bun:test'

const rulesRequests: Array<{ action: { type: string }, viewer: string }> = []

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

mock.module('../rules-engine', () => ({
  callRulesEngine: async (request: { action: { type: string }, viewer: string }) => {
    rulesRequests.push(structuredClone(request))
    return {
      currentStep: 0,
      totalSteps: 0,
      state: { hands: [{ cards: { kind: request.viewer === 'observer' ? 'hidden' : 'known' } }] },
      battleRecord: { preparation: { entries: [] }, turns: [] },
      interaction: { canChooseInitialPouch: false },
    }
  },
}))

const { ReplayArchive } = await import('./replay-archive')

class MemoryStorage {
  readonly values = new Map<string, unknown>()
  deleteAllCalls = 0

  async get<T>(key: string): Promise<T | undefined> {
    return this.values.get(key) as T | undefined
  }

  async put(key: string, value: unknown): Promise<void> {
    this.values.set(key, structuredClone(value))
  }

  async deleteAll(): Promise<void> {
    this.deleteAllCalls += 1
    this.values.clear()
  }
}

function replayArchive(storage: MemoryStorage) {
  return new ReplayArchive(
    { storage } as unknown as DurableObjectState,
    {} as never,
  )
}

test('ReplayArchive 清理僅接受其自身 sourceGameId 證明的關聯，且可安全重試', async () => {
  const storage = new MemoryStorage()
  storage.values.set('archive', { replayId: 'replay-1', sourceGameId: 'deleted-room' })
  storage.values.set('lifecycle', { referenceCount: 1, version: 1 })
  const archive = replayArchive(storage)

  const probe = await archive.fetch(new Request('https://replay.internal/manage/probe-legacy', {
    method: 'POST',
  }))
  expect(probe.status).toBe(200)
  expect(await probe.json()).toEqual({
    status: 'preserved',
    replayId: 'replay-1',
    sourceGameId: 'deleted-room',
  })

  const mismatch = await archive.fetch(new Request('https://replay.internal/manage/purge-legacy', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ epoch: 'cutover-75', sourceGameId: 'other-room' }),
  }))
  expect(mismatch.status).toBe(200)
  expect(await mismatch.json()).toEqual({ status: 'preserved', purged: false, epoch: 'cutover-75' })
  expect(storage.deleteAllCalls).toBe(0)
  expect(storage.values.has('archive')).toBe(true)

  const deleted = await archive.fetch(new Request('https://replay.internal/manage/purge-legacy', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ epoch: 'cutover-75', sourceGameId: 'deleted-room' }),
  }))
  expect(deleted.status).toBe(200)
  expect(await deleted.json()).toEqual({ status: 'deleted', purged: true, epoch: 'cutover-75' })
  expect(storage.deleteAllCalls).toBe(1)

  const retry = await archive.fetch(new Request('https://replay.internal/manage/purge-legacy', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ epoch: 'cutover-75', sourceGameId: 'deleted-room' }),
  }))
  expect(retry.status).toBe(404)
  expect(await retry.json()).toEqual({ status: 'absent', purged: false, epoch: 'cutover-75' })
  expect(storage.deleteAllCalls).toBe(1)
})

test('公開競賽重播使用 observer projection，私有重播維持 Replay projection', async () => {
  const storage = new MemoryStorage()
  const archive = replayArchive(storage)
  const draft = {
    schemaVersion: 1,
    replayId: 'competition:competition-1:game-1',
    sourceGameId: 'game-1',
    finishedAt: '2026-10-14T00:00:00.000Z',
    setup: {},
    record: [],
    players: [{ player: 'player-a', displayName: '甲' }],
    originalUserIds: ['user-a'],
    roomName: 'game-1',
    result: {},
    firstPlayer: 'player-a',
  }
  const created = await archive.fetch(new Request('https://replay.internal/create', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ archive: draft, lifecycle: { referenceCount: 1, version: 1 } }),
  }))
  expect(created.status).toBe(201)

  const publicFrameResponse = await archive.fetch(new Request(
    'https://replay.internal/?step=0&public=1',
  ))
  expect(publicFrameResponse.status).toBe(200)
  const publicFrame = await publicFrameResponse.json() as { state: { hands: Array<{ cards: { kind: string } }> } }
  expect(publicFrame.state.hands[0]?.cards.kind).toBe('hidden')
  expect(rulesRequests[0]).toMatchObject({
    action: { type: 'publicReplayFrame', step: 0 }, viewer: 'observer',
  })

  const privateFrameResponse = await archive.fetch(new Request('https://replay.internal/?step=0'))
  expect(privateFrameResponse.status).toBe(200)
  const privateFrame = await privateFrameResponse.json() as { state: { hands: Array<{ cards: { kind: string } }> } }
  expect(privateFrame.state.hands[0]?.cards.kind).toBe('known')
  expect(rulesRequests[1]).toMatchObject({ action: { type: 'replayFrame', step: 0 }, viewer: 'replay' })
})
