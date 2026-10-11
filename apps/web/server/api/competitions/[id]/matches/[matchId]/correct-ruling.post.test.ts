import { createError, createApp, createRouter, eventHandler, toWebHandler } from 'h3'
import { afterEach, expect, mock, test } from 'bun:test'
import type { CompetitionRoomMatchExecuteRequest } from '../../../../../../shared/competition-room'

let sessionCalls: string[] = []
let roomNames: string[] = []
let requests: CompetitionRoomMatchExecuteRequest[] = []
let rpcResult: unknown
let rpcThrows = false
let bindingEnabled = true

mock.module('../../../../../utils/auth', () => ({
  requireSession: async (event: { headers: Headers }) => {
    const userId = event.headers.get('x-test-session')
    sessionCalls.push(userId ?? '')
    if (!userId) throw createError({ statusCode: 401, statusMessage: 'Authentication required.' })
    return { user: { id: userId } }
  },
}))

const { default: correctRulingPost } = await import('./correct-ruling.post')

const body = {
  commandId: 'correction-command', expectedMatchRevision: 4,
  correctionId: 'correction-1', gameRecordId: 'game-1',
  replacementRulingRecordId: 'ruling-2',
  replacementRuling: { type: 'gameAccepted', outcome: { winner: 'team-b' } },
  reason: '裁判覆核',
}

function validResult(request = requests.at(-1)!, overrides: Record<string, unknown> = {}) {
  return {
    ok: true,
    value: {
      competitionId: request.competitionId,
      matchId: request.matchId,
      matchRevision: request.expectedMatchRevision + 1,
      commandId: request.commandId,
      idempotent: false,
      timeoutEvidence: null,
      nextClockDeadlineMs: null,
      matchProjection: { ok: true, operation: 'matchCorrectRuling', public: { match: { status: 'open' } } },
      clockProjection: null,
      ...overrides,
    },
  }
}

function resetState(result?: unknown, throws = false) {
  sessionCalls = []
  roomNames = []
  requests = []
  rpcResult = result
  rpcThrows = throws
  bindingEnabled = true
}

function capturedRequest(index: number): CompetitionRoomMatchExecuteRequest {
  const request = requests[index]
  if (!request) throw new Error(`expected captured executeMatch request at index ${index}`)
  return request
}

function isResponseRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function makeWebHandler(roomOverride?: unknown) {
  const app = createApp()
  app.use(eventHandler((event) => {
    ;(event.context as { _platform?: unknown })._platform = {
      cloudflare: {
        env: {
          APP_ENV: 'development',
          COMPETITION_ROOM: roomOverride !== undefined ? roomOverride : bindingEnabled ? {
            idFromName: (name: string) => { roomNames.push(name); return name },
            get: () => ({
              executeMatch: async (request: CompetitionRoomMatchExecuteRequest) => {
                requests.push(request)
                if (rpcThrows) throw new Error('private transport detail')
                return rpcResult ?? validResult(request)
              },
              execute: async () => { throw new Error('generic execute must never be called') },
              read: async () => { throw new Error('private read must never be called') },
            }),
          } : undefined,
        },
      },
    }
  }))
  const router = createRouter()
  router.post('/api/competitions/:id/matches/:matchId/correct-ruling', correctRulingPost)
  app.use(router.handler)
  return toWebHandler(app)
}

async function post(
  input: unknown = body,
  sessionUserId?: string,
  path = '/api/competitions/comp-1/matches/match-1/correct-ruling',
  roomOverride?: unknown,
) {
  const headers = new Headers({ 'content-type': 'application/json' })
  if (sessionUserId) headers.set('x-test-session', sessionUserId)
  return makeWebHandler(roomOverride)(new Request(`http://fewfc.test${path}`, {
    method: 'POST', headers, body: input === undefined ? undefined : JSON.stringify(input),
  }))
}

async function postRaw(
  rawBody: string,
  sessionUserId?: string,
  path = '/api/competitions/comp-1/matches/match-1/correct-ruling',
) {
  const headers = new Headers({ 'content-type': 'application/json' })
  if (sessionUserId) headers.set('x-test-session', sessionUserId)
  return makeWebHandler()(new Request(`http://fewfc.test${path}`, {
    method: 'POST', headers, body: rawBody,
  }))
}

afterEach(() => {
  mock.restore()
})

test('H3 handler derives referee and server time, executes once, and exposes only a minimal ACK', async () => {
  resetState()
  const realNow = Date.now
  let now = 700
  let nowCalls = 0
  Date.now = () => { nowCalls += 1; return now++ }
  try {
    const response = await post({ ...body, reason: '  裁判覆核  ' }, 'session-referee',
      '/api/competitions/%20comp-1%20/matches/%20match-1%20/correct-ruling')
    const value = await response.json()
    expect(response.status).toBe(200)
    expect(requests).toHaveLength(1)
    expect(requests[0]).toEqual({
      competitionId: 'comp-1', matchId: 'match-1', callerUserId: 'session-referee',
      commandId: 'correction-command', expectedMatchRevision: 4, trustedTimeMs: 700,
      operation: {
        type: 'matchCorrectRuling', matchId: 'match-1', correctionId: 'correction-1',
        refereeId: 'session-referee', gameRecordId: 'game-1',
        replacementRulingRecordId: 'ruling-2',
        replacementRuling: { type: 'gameAccepted', outcome: { winner: 'team-b' } },
        reason: '裁判覆核',
      },
    })
    expect(value).toEqual({
      competitionId: 'comp-1', matchId: 'match-1', matchRevision: 5,
      commandId: 'correction-command', idempotent: false,
    })
    if (!isResponseRecord(value)) throw new Error('expected public correction ACK object')
    expect(Object.keys(value).sort()).toEqual(['commandId', 'competitionId', 'idempotent', 'matchId', 'matchRevision'])
    expect(roomNames).toEqual(['comp-1'])
    expect(nowCalls).toBe(1)
    expect(JSON.stringify(value)).not.toMatch(/projection|private|reason|ruling|serverState/)
  } finally {
    Date.now = realNow
  }
})

test('same command retry preserves payload/referee identity while server time remains server-derived', async () => {
  resetState()
  const realNow = Date.now
  let now = 1000
  Date.now = () => now++
  try {
    const handler = makeWebHandler()
    const first = await handler(new Request('http://fewfc.test/api/competitions/comp-1/matches/match-1/correct-ruling', {
      method: 'POST', headers: { 'content-type': 'application/json', 'x-test-session': 'referee-a' }, body: JSON.stringify(body),
    }))
    expect(first.status).toBe(200)
    rpcResult = validResult(capturedRequest(0), { idempotent: true })
    const second = await handler(new Request('http://fewfc.test/api/competitions/comp-1/matches/match-1/correct-ruling', {
      method: 'POST', headers: { 'content-type': 'application/json', 'x-test-session': 'referee-a' }, body: JSON.stringify(body),
    }))
    expect(second.status).toBe(200)
    expect(await second.json()).toMatchObject({ idempotent: true, matchRevision: 5 })
    expect(requests).toHaveLength(2)
    const { trustedTimeMs: _firstTime, ...firstRequest } = capturedRequest(0)
    const { trustedTimeMs: _secondTime, ...secondRequest } = capturedRequest(1)
    expect(firstRequest).toEqual(secondRequest)
    expect(requests.map(request => request.trustedTimeMs)).toEqual([1000, 1001])
  } finally {
    Date.now = realNow
  }
})

test('authentication, trimmed path, and all spoofed or extra body fields reject before RPC/time', async () => {
  resetState()
  const realNow = Date.now
  let nowCalls = 0
  Date.now = () => { nowCalls += 1; return 10 }
  try {
    expect((await post(body)).status).toBe(401)
    expect((await post(body, 'referee', '/api/competitions/%20/matches/match-1/correct-ruling')).status).toBe(400)
    expect((await post(body, 'referee', '/api/competitions/comp-1/matches/%20/correct-ruling')).status).toBe(400)
    const invalidBodies = [
      null,
      { ...body, callerUserId: 'attacker' }, { ...body, refereeId: 'attacker' },
      { ...body, trustedTimeMs: 0 }, { ...body, serverActor: { type: 'referee' } },
      { ...body, binding: {} }, { ...body, decks: [] }, { ...body, operation: { type: 'matchCorrectRuling' } },
      { ...body, reason: '  ' }, { ...body, unknown: true },
    ]
    for (const input of invalidBodies) expect((await post(input, 'referee')).status).toBe(400)
    expect(requests).toHaveLength(0)
    expect(roomNames).toHaveLength(0)
    expect(nowCalls).toBe(0)
  } finally {
    Date.now = realNow
  }
})

test('authenticated H3 route maps raw malformed JSON to the fixed invalidRequest envelope', async () => {
  resetState()
  const response = await postRaw('{', 'session-referee')
  const text = await response.text()
  expect(response.status).toBe(400)
  expect(response.statusText).toBe('InvalidCompetitionRulingCorrectionRequest')
  expect(text).toContain('invalidRequest')
  expect(text).toContain('更正要求格式不正確。')
  expect(text).not.toMatch(/JSON\.parse|Unexpected|Malformed|private|session-referee/i)
  expect(sessionCalls).toEqual(['session-referee'])
  expect(requests).toHaveLength(0)
  expect(roomNames).toHaveLength(0)
})

test('known typed refusals map to fixed safe status and unknown codes fail closed', async () => {
  const refusals = [
    { code: 'forbidden', status: 403, publicCode: 'forbidden', message: '你沒有更正這場交手判定的權限。' },
    { code: 'notInitialized', status: 404, publicCode: 'notInitialized', message: '找不到這個賽事。' },
    { code: 'matchNotFound', status: 404, publicCode: 'matchNotFound', message: '找不到這場交手。' },
    { code: 'matchRevisionConflict', status: 409, publicCode: 'matchRevisionConflict', message: '交手狀態已更新，請重新讀取後再試。' },
    { code: 'engine:downstreamStarted', status: 409, publicCode: 'downstreamConflict', message: '更正會影響已開始的後續賽程。' },
    { code: 'engine:scheduleError', status: 409, publicCode: 'scheduleConflict', message: '賽程狀態不允許此判定更正。' },
    { code: 'engine:matchError', status: 422, publicCode: 'rulingRejected', message: '賽事核心拒絕此判定更正。' },
  ] as const
  for (const { code, status, publicCode, message } of refusals) {
    resetState({ ok: false, error: { code, message: 'private internal explanation' } })
    const response = await post(body, 'referee')
    const text = await response.text()
    expect(response.status).toBe(status)
    expect(response.statusText).toMatch(/^[A-Za-z][A-Za-z0-9]*$/)
    expect(text).toContain(publicCode)
    expect(text).toContain(message)
    expect(text).not.toContain('private internal explanation')
  }
  for (const code of ['privateUnknownCode', 'engine:unmappedError']) {
    resetState({ ok: false, error: { code, message: 'private detail' } })
    const unknown = await post(body, 'referee')
    expect(unknown.status).toBe(502)
    expect(await unknown.text()).not.toMatch(/privateUnknownCode|unmappedError|private detail/)
  }
})

test('malformed identity, internal fields, operation, or nested symbols never become a public ACK', async () => {
  const request = {
    competitionId: 'comp-1', matchId: 'match-1', callerUserId: 'referee', commandId: body.commandId,
    expectedMatchRevision: body.expectedMatchRevision, trustedTimeMs: 123,
    operation: { type: 'matchCorrectRuling', matchId: 'match-1', correctionId: body.correctionId,
      refereeId: 'referee', gameRecordId: 'game-1', replacementRulingRecordId: 'ruling-2',
      replacementRuling: body.replacementRuling, reason: body.reason },
  } as CompetitionRoomMatchExecuteRequest
  const ok = validResult(request)
  const value = (ok as { value: Record<string, unknown> }).value
  const malformed: unknown[] = [
    { ...ok, extra: 'secret' },
    { ok: true, value: { ...value, private: { secret: true } } },
    { ok: true, value: { ...value, serverState: {} } },
    { ok: true, value: { ...value, competitionId: 'other' } },
    { ok: true, value: { ...value, matchId: 'other' } },
    { ok: true, value: { ...value, matchRevision: 7 } },
    { ok: true, value: { ...value, matchProjection: { ok: true, operation: 'matchCorrectRuling', public: { binding: {} } } } },
    { ok: true, value: { ...value, timeoutEvidence: { type: 'timeout' } } },
  ]
  const nestedSymbol = { ...value.matchProjection as object, [Symbol.for('unknown')]: true }
  malformed.push({ ok: true, value: { ...value, matchProjection: nestedSymbol } })
  for (const result of malformed) {
    resetState(result)
    const response = await post(body, 'referee')
    expect(response.status).toBe(502)
    expect(await response.text()).not.toMatch(/secret|binding/)
  }
})

test('native-style root synchronous disposer runs exactly once with its receiver', async () => {
  resetState()
  const root = validResult({
    competitionId: 'comp-1', matchId: 'match-1', callerUserId: 'referee', commandId: body.commandId,
    expectedMatchRevision: body.expectedMatchRevision, trustedTimeMs: 123,
    operation: { type: 'matchCorrectRuling', matchId: 'match-1', correctionId: body.correctionId,
      refereeId: 'referee', gameRecordId: 'game-1', replacementRulingRecordId: 'ruling-2',
      replacementRuling: body.replacementRuling, reason: body.reason },
  } as CompetitionRoomMatchExecuteRequest)
  let disposed = 0
  Object.defineProperty(root, Symbol.dispose, { value: function () { expect(this).toBe(root); disposed += 1 } })
  rpcResult = root
  const response = await post(body, 'referee')
  expect(response.status).toBe(200)
  expect(disposed).toBe(1)

  resetState()
  const rejected = { ok: false, error: { code: 'forbidden', message: 'private refusal detail' } }
  let rejectedDisposed = 0
  Object.defineProperty(rejected, Symbol.dispose, { value: function () { rejectedDisposed += 1 } })
  rpcResult = rejected
  const refusal = await post(body, 'referee')
  expect(refusal.status).toBe(403)
  expect(rejectedDisposed).toBe(1)
})

test('class-based platform namespace and stub preserve capability receivers and typed refusal', async () => {
  resetState()
  let idFromNameCalls = 0
  let getCalls = 0
  let executeCalls = 0
  let namespaceReceiverPreserved = false
  let stubReceiverPreserved = false
  const stub = new class NativeLikeStub {
    async executeMatch() {
      executeCalls += 1
      stubReceiverPreserved = this === stub
      return { ok: false, error: { code: 'forbidden', message: 'private native refusal' } }
    }
  }()
  const namespace = new class NativeLikeNamespace {
    idFromName(name: string) {
      idFromNameCalls += 1
      namespaceReceiverPreserved = this === namespace
      expect(name).toBe('comp-1')
      return 'native-like-id'
    }

    get(id: unknown) {
      getCalls += 1
      expect(id).toBe('native-like-id')
      return stub
    }
  }()

  const response = await post(body, 'organizer', undefined, namespace)
  const text = await response.text()
  expect(response.status).toBe(403)
  expect(text).toContain('forbidden')
  expect(text).not.toContain('private native refusal')
  expect(idFromNameCalls).toBe(1)
  expect(getCalls).toBe(1)
  expect(executeCalls).toBe(1)
  expect(namespaceReceiverPreserved).toBe(true)
  expect(stubReceiverPreserved).toBe(true)
})

test('transport failure, missing binding, async disposer, and hidden symbols fail safely', async () => {
  resetState(undefined, true)
  const transport = await post(body, 'referee')
  expect(transport.status).toBe(503)
  expect(await transport.text()).not.toContain('private transport detail')

  resetState()
  bindingEnabled = false
  expect((await post(body, 'referee')).status).toBe(503)

  resetState()
  const badDispose = validResult({
    competitionId: 'comp-1', matchId: 'match-1', callerUserId: 'referee', commandId: body.commandId,
    expectedMatchRevision: body.expectedMatchRevision, trustedTimeMs: 123,
    operation: { type: 'matchCorrectRuling', matchId: 'match-1', correctionId: body.correctionId,
      refereeId: 'referee', gameRecordId: 'game-1', replacementRulingRecordId: 'ruling-2',
      replacementRuling: body.replacementRuling, reason: body.reason },
  } as CompetitionRoomMatchExecuteRequest)
  let asyncCalls = 0
  Object.defineProperty(badDispose, Symbol.dispose, { value: async function () { asyncCalls += 1 } })
  rpcResult = badDispose
  expect((await post(body, 'referee')).status).toBe(502)
  expect(asyncCalls).toBe(0)

  const hidden = validResult()
  Object.defineProperty(hidden, Symbol.for('hidden-root'), { value: true })
  rpcResult = hidden
  expect((await post(body, 'referee')).status).toBe(502)
})
