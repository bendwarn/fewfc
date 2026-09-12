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

const { ReplayArchive } = await import('./replay-archive')

class MemoryStorage {
  readonly values = new Map<string, unknown>()
  deleteAllCalls = 0

  async get<T>(key: string): Promise<T | undefined> {
    return this.values.get(key) as T | undefined
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
