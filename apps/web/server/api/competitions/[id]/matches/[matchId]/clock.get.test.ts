import { createApp, createError, createRouter, eventHandler, toWebHandler } from 'h3'
import { afterEach, expect, mock, spyOn, test } from 'bun:test'
import type { CompetitionRoomMatchClockReadRequest, CompetitionRoomMatchClockReadValue } from '../../../../../../shared/competition-room'

mock.module('../../../../../utils/auth', () => ({
  requireSession: async (event: { headers: Headers }) => {
    const id = event.headers.get('x-test-session')
    if (!id) throw createError({ statusCode: 401, statusMessage: 'Unauthorized' })
    return { user: { id } }
  },
}))

const { default: clockGet } = await import('./clock.get')

function currentValue(request: CompetitionRoomMatchClockReadRequest): CompetitionRoomMatchClockReadValue {
  return {
    competitionId: request.competitionId, matchId: request.matchId, matchRevision: 4, matchStatus: 'open',
    game: { gameId: 'game-1', gameNumber: 1 },
    clock: {
      sampledAtMs: request.trustedTimeMs,
      teams: [
        { team: 'team-a', mainTimeRemainingMs: 299000, byoyomiTimeRemainingMs: 30000, byoyomiActive: false },
        { team: 'team-b', mainTimeRemainingMs: 300000, byoyomiTimeRemainingMs: 30000, byoyomiActive: false },
      ],
      running: { type: 'action', team: 'team-a', turn: 1 }, paused: null, awaitingAuthority: false,
    },
  }
}

type Options = {
  result?: unknown
  namespace?: unknown
  throws?: boolean
  resultFor?: (request: CompetitionRoomMatchClockReadRequest) => unknown
}

function endpoint(options: Options = {}) {
  const requests: CompetitionRoomMatchClockReadRequest[] = []
  const calls: string[] = []
  const receivers: boolean[] = []
  class ClockStub {
    async readMatchClock(request: CompetitionRoomMatchClockReadRequest) {
      calls.push('readMatchClock')
      receivers.push(this === stub)
      requests.push(request)
      if (options.throws) throw new Error('private transport sentinel')
      if (options.resultFor) return options.resultFor(request)
      return Object.hasOwn(options, 'result') ? options.result : { ok: true, value: currentValue(request) }
    }
    async execute() { calls.push('execute'); throw new Error('mutation forbidden') }
    async executeMatch() { calls.push('executeMatch'); throw new Error('mutation forbidden') }
    async readMatch() { calls.push('readMatch'); throw new Error('fallback forbidden') }
    async read() { calls.push('read'); throw new Error('fallback forbidden') }
  }
  const stub = new ClockStub()
  class ClockNamespace {
    idFromName(name: string) { calls.push(`id:${name}`); receivers.push(this === namespace); return name }
    get(id: unknown) { calls.push(`get:${id}`); receivers.push(this === namespace); return stub }
  }
  const namespace = new ClockNamespace()
  const app = createApp()
  app.use(eventHandler(event => {
    Object.assign(event.context, { _platform: { cloudflare: { env: {
      APP_ENV: 'development',
      COMPETITION_ROOM: Object.hasOwn(options, 'namespace') ? options.namespace : namespace,
    } } } })
    if (event.headers.get('x-test-cached-body')) event._requestBody = '{}'
  }))
  const router = createRouter()
  router.get('/api/competitions/:id/matches/:matchId/clock', clockGet)
  app.use(router.handler)
  const run = toWebHandler(app)
  return {
    requests, calls, receivers,
    get: (path = '/api/competitions/competition-1/matches/match-1/clock', session: string | null = 'canonical-user', extras?: HeadersInit) => {
      const headers = new Headers(extras)
      if (session !== null) headers.set('x-test-session', session)
      return run(new Request(`http://fewfc.test${path}`, { method: 'GET', headers }))
    },
  }
}

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

afterEach(() => { mock.restore() })

test('真 H3 GET 使用 class capability、canonical session、trim path 與單次時間，只派 dedicated query', async () => {
  const time = spyOn(Date, 'now').mockReturnValue(4000)
  const api = endpoint()
  const response = await api.get('/api/competitions/%20competition-1%20/matches/%20match-1%20/clock')
  expect(response.status).toBe(200)
  const body: unknown = await response.json()
  const request = api.requests[0]
  if (!request || !record(body)) throw new Error('缺少正式 handler request／public DTO。')
  expect(body).toEqual({ ...currentValue(request) })
  expect(Object.keys(body).sort()).toEqual(['clock', 'competitionId', 'game', 'matchId', 'matchRevision', 'matchStatus'])
  expect(api.requests).toEqual([{ competitionId: 'competition-1', matchId: 'match-1', callerUserId: 'canonical-user', trustedTimeMs: 4000 }])
  expect(api.calls).toEqual(['id:competition-1', 'get:competition-1', 'readMatchClock'])
  expect(api.receivers).toEqual([true, true, true])
  expect(time).toHaveBeenCalledTimes(1)
  expect(JSON.stringify(body)).not.toMatch(/private|serverState|binding|deck|callerUserId|trustedTimeMs|config/)
})

test('session 最先驗證；空路徑、任意 query/body 在 binding 與時間取樣前拒絕', async () => {
  const time = spyOn(Date, 'now').mockReturnValue(4000)
  const api = endpoint()
  expect((await api.get('/api/competitions/%20/matches/%20/clock?callerUserId=attacker', null)).status).toBe(401)
  for (const path of ['/api/competitions/%20/matches/match-1/clock', '/api/competitions/competition-1/matches/%20/clock']) {
    expect((await api.get(path)).status).toBe(400)
  }
  for (const query of ['callerUserId=attacker', 'trustedTimeMs=1', 'operation=clockObserve', 'revision=9', 'role=organizer', 'gameId=old', 'x=']) {
    const response = await api.get(`/api/competitions/competition-1/matches/match-1/clock?${query}`)
    expect(response.status).toBe(400)
    const body: unknown = await response.json()
    if (!record(body) || !record(body.data)) throw new Error('缺少安全 error data。')
    expect(body.data).toEqual({ code: 'invalidRequest', message: '賽事、交手識別碼或查詢格式不正確。' })
  }
  const bodyHeaders: Record<string, string>[] = [
    { 'content-length': '1' }, { 'transfer-encoding': 'chunked' }, { 'x-test-cached-body': '1' },
  ]
  for (const headers of bodyHeaders) {
    expect((await api.get(undefined, undefined, headers)).status).toBe(400)
  }
  expect(api.calls).toEqual([])
  expect(api.requests).toEqual([])
  expect(time).not.toHaveBeenCalled()
})

test('非安全或丟錯的 server clock 固定503且不派 RPC，不能使用 query時間替代', async () => {
  for (const bad of [-1, NaN, Infinity, 1.5, Number.MAX_SAFE_INTEGER + 1]) {
    const time = spyOn(Date, 'now').mockReturnValue(bad)
    const api = endpoint()
    const response = await api.get()
    expect(response.status).toBe(503)
    expect(await response.text()).toContain('伺服器時間暫時無法使用。')
    expect(api.calls).toEqual([])
    expect(time).toHaveBeenCalledTimes(1)
    time.mockRestore()
  }
  spyOn(Date, 'now').mockImplementation(() => { throw new Error('private time detail') })
  const response = await endpoint().get()
  expect(response.status).toBe(503)
  expect(await response.text()).not.toContain('private time detail')
})

test('真 H3 回完整六欄四status與null game，completed 不回非法舊局', async () => {
  spyOn(Date, 'now').mockReturnValue(4000)
  for (const matchStatus of ['open', 'awaitingFirstPlayer', 'awaitingReferee', 'completed'] as const) {
    const api = endpoint({ resultFor: request => {
      const current = currentValue(request)
      current.matchStatus = matchStatus
      current.game = null
      current.clock.running = null
      current.clock.awaitingAuthority = matchStatus === 'awaitingReferee'
      return { ok: true, value: current }
    } })
    const response = await api.get()
    expect(response.status).toBe(200)
    const body: unknown = await response.json()
    if (!record(body)) throw new Error('缺少 public DTO。')
    expect(body.matchStatus).toBe(matchStatus)
    expect(body.game).toBeNull()
    expect(api.requests).toHaveLength(1)
  }
  for (const matchStatus of ['awaitingFirstPlayer', 'awaitingReferee', 'completed'] as const) {
    const api = endpoint({ resultFor: request => ({ ok: true, value: { ...currentValue(request), matchStatus } }) })
    expect((await api.get()).status).toBe(502)
    expect(api.requests).toHaveLength(1)
  }
})

test('actual DO typed failures 對應safe400/404/403/409/502/503，未知code不反射', async () => {
  spyOn(Date, 'now').mockReturnValue(4000)
  for (const [code, status] of [
    ['invalidRequest', 400], ['notInitialized', 404], ['competitionIdMismatch', 404], ['matchNotFound', 404],
    ['forbidden', 403], ['notParticipant', 403], ['notReady', 409], ['trustedTimeRegression', 409],
    ['invalidMatchCheckpoint', 409], ['invalidClockCheckpoint', 409], ['invalidLockedRoster', 409],
    ['invalidGameProvision', 409], ['engineProtocolError', 502], ['internalFailure', 503], ['unknown-code-sentinel', 502],
  ] as const) {
    const api = endpoint({ result: { ok: false, error: { code, message: 'private error sentinel' } } })
    const response = await api.get()
    expect(response.status).toBe(status)
    const body: unknown = await response.json()
    if (!record(body) || !record(body.data) || typeof body.data.message !== 'string' || typeof body.data.code !== 'string') {
      throw new Error('缺少固定安全 message/code。')
    }
    expect(body.data.code).toMatch(/^[A-Za-z]+$/)
    expect(body.data.message).toMatch(/[\u3400-\u9fff]/)
    expect(JSON.stringify(body)).not.toMatch(/private error sentinel|unknown-code-sentinel/)
    expect(api.requests).toHaveLength(1)
  }
})

test('missing/throwing capability 與 transport 只固定503，不走generic/mutation fallback', async () => {
  spyOn(Date, 'now').mockReturnValue(4000)
  for (const namespace of [undefined, null, {}, { idFromName: () => 'id', get: () => ({ read: async () => 'legacy' }) },
    { idFromName: () => { throw new Error('private namespace') }, get: () => ({}) },
    Object.defineProperty({}, 'idFromName', { get: () => { throw new Error('private getter') } })]) {
    const api = endpoint({ namespace })
    const response = await api.get()
    expect(response.status).toBe(503)
    expect(await response.text()).not.toMatch(/private namespace|private getter|legacy/)
    expect(api.requests).toEqual([])
  }
  const api = endpoint({ throws: true })
  const response = await api.get()
  expect(response.status).toBe(503)
  expect(await response.text()).not.toContain('private transport sentinel')
  expect(api.calls).toEqual(['id:competition-1', 'get:competition-1', 'readMatchClock'])
  expect(api.requests).toHaveLength(1)
})

test('root同步native-style disposer mock在成功/拒絕/malformed中finally receiver一次，不稱native證據', async () => {
  spyOn(Date, 'now').mockReturnValue(4000)
  for (const kind of ['success', 'denied', 'malformed', 'unknown-symbol']) {
    let raw: unknown
    let disposed = 0
    let receiver = false
    const api = endpoint({ resultFor: request => {
      const result = kind === 'denied'
        ? { ok: false, error: { code: 'forbidden', message: 'private refusal' } }
        : { ok: true, value: kind === 'malformed' ? { ...currentValue(request), private: 'secret' } : currentValue(request) }
      Object.defineProperty(result, Symbol.dispose, { value: function (this: unknown) { disposed += 1; receiver = this === raw } })
      if (kind === 'unknown-symbol') Object.defineProperty(result, Symbol('unknown'), { value: true })
      raw = result
      return result
    } })
    const response = await api.get()
    expect(response.status).toBe(kind === 'success' ? 200 : kind === 'denied' ? 403 : 502)
    expect(disposed).toBe(1)
    expect(receiver).toBe(true)
    expect(api.requests).toHaveLength(1)
    expect(await response.text()).not.toMatch(/private refusal|secret|Symbol/)
  }
})

test('未知/nested/async/accessor disposer拒絕；同步cleanup throw/nonvoid固定503且不重試', async () => {
  spyOn(Date, 'now').mockReturnValue(4000)
  for (const kind of ['root-symbol', 'nested-symbol', 'nested-disposer', 'async', 'generator', 'accessor', 'throw', 'nonvoid']) {
    let called = 0
    const api = endpoint({ resultFor: request => {
      const result = { ok: true, value: currentValue(request) }
      if (kind === 'root-symbol') Object.defineProperty(result, Symbol('extra'), { value: 1 })
      if (kind === 'nested-symbol') Object.defineProperty(result.value.clock, Symbol('extra'), { value: 1 })
      if (kind === 'nested-disposer') Object.defineProperty(result.value, Symbol.dispose, { value: () => { called += 1 } })
      if (kind === 'async') Object.defineProperty(result, Symbol.dispose, { value: async () => { called += 1 } })
      if (kind === 'generator') Object.defineProperty(result, Symbol.dispose, { value: function* () { called += 1 } })
      if (kind === 'accessor') Object.defineProperty(result, Symbol.dispose, { get: () => { called += 1; return () => undefined } })
      if (kind === 'throw') Object.defineProperty(result, Symbol.dispose, { value: () => { called += 1; throw new Error('private cleanup') } })
      if (kind === 'nonvoid') Object.defineProperty(result, Symbol.dispose, { value: () => { called += 1; return 1 } })
      return result
    } })
    const response = await api.get()
    expect(response.status).toBe(kind === 'throw' || kind === 'nonvoid' ? 503 : 502)
    expect(called).toBe(kind === 'throw' || kind === 'nonvoid' ? 1 : 0)
    expect(api.requests).toHaveLength(1)
    expect(await response.text()).not.toMatch(/private cleanup|Symbol/)
  }
})

test('identity/time mismatch、額外private與非JSON/accessor回應在真H3固定502且不執行getter', async () => {
  spyOn(Date, 'now').mockReturnValue(4000)
  for (const kind of ['identity', 'time', 'private', 'binding', 'nonJSON', 'accessor']) {
    let getters = 0
    const api = endpoint({ resultFor: request => {
      const current = currentValue(request)
      if (kind === 'identity') current.matchId = 'other-match'
      if (kind === 'time') current.clock.sampledAtMs = 4100
      if (kind === 'private') Object.assign(current, { private: 'private payload sentinel' })
      if (kind === 'binding') Object.assign(current.game ?? {}, { binding: 'private payload sentinel' })
      if (kind === 'nonJSON') Object.assign(current.clock, { sampledAtMs: new Date() })
      if (kind === 'accessor') Object.defineProperty(current.clock, 'running', { enumerable: true, get: () => { getters += 1; throw new Error('private getter') } })
      return { ok: true, value: current }
    } })
    const response = await api.get()
    expect(response.status).toBe(502)
    expect(getters).toBe(0)
    expect(api.requests).toHaveLength(1)
    expect(await response.text()).not.toMatch(/private payload sentinel|private getter|other-match/)
  }
})

test('function capability仍receiver綁定；重複GET保持same revision但每request只sample一次', async () => {
  const time = spyOn(Date, 'now').mockReturnValue(4000)
  const rawRequests: CompetitionRoomMatchClockReadRequest[] = []
  let correctReceiver = false
  const stub = Object.assign(() => undefined, {
    readMatchClock: async function (this: unknown, request: CompetitionRoomMatchClockReadRequest) {
      correctReceiver = this === stub
      rawRequests.push(request)
      return { ok: true, value: currentValue(request) }
    },
  })
  const namespace = Object.assign(() => undefined, { idFromName: (name: string) => name, get: () => stub })
  const api = endpoint({ namespace })
  const first: unknown = await (await api.get()).json()
  const second: unknown = await (await api.get()).json()
  expect(first).toEqual(second)
  expect(correctReceiver).toBe(true)
  expect(rawRequests).toHaveLength(2)
  expect(time).toHaveBeenCalledTimes(2)
})
