import { expect, test } from 'bun:test'
import { createApp, createError, eventHandler, toWebHandler } from 'h3'
import type { CompetitionGameReadyRequest } from '../../shared/competition-game-room'
import {
  buildCompetitionGameReadyRequest,
  callCompetitionGameReady,
  competitionGameReadyError,
  normalizeCompetitionGameId,
} from './competition-game-room'

function readyHarness(stub: unknown, options: {
  namespace?: unknown
  environment?: unknown
  gameId?: string
  readyRequest?: CompetitionGameReadyRequest
} = {}) {
  const roomNames: string[] = []
  const roomIds: unknown[] = []
  const namespace = {
    idFromName(name: string) {
      roomNames.push(name)
      return `room:${name}`
    },
    get(id: unknown) {
      roomIds.push(id)
      return stub
    },
  }
  const app = createApp()
  app.use(eventHandler(async (event) => {
    event.context._platform = {
      cloudflare: {
        env: Object.hasOwn(options, 'environment') ? options.environment : {
          APP_ENV: 'development',
          GAME_ROOM: Object.hasOwn(options, 'namespace') ? options.namespace : namespace,
        },
      },
    }
    return await callCompetitionGameReady(event, options.gameId ?? ' game-1 ', options.readyRequest ?? {
      actorUserId: 'session-user', ready: true,
    })
  }))
  return {
    roomNames,
    roomIds,
    request: () => toWebHandler(app)(new Request('http://fewfc.test/ready')),
  }
}

test('ready request preserves each explicit boolean and binds only the session actor', () => {
  expect(buildCompetitionGameReadyRequest({ ready: true }, 'session-user')).toEqual({
    actorUserId: 'session-user', ready: true,
  })
  expect(buildCompetitionGameReadyRequest({ ready: false }, 'session-user')).toEqual({
    actorUserId: 'session-user', ready: false,
  })
})

test('fixed ready errors use ASCII reasons and retain safe Chinese message and data', () => {
  const cases: Array<[Parameters<typeof competitionGameReadyError>[0], number, string, string]> = [
    ['invalidRequest', 400, 'Bad Request', '競賽準備要求格式不正確。'],
    ['notCompetitionRoom', 404, 'Not Found', '找不到競賽對局。'],
    ['invalidCompetitionRoom', 409, 'Conflict', '競賽房間狀態無法驗證。'],
    ['fixedMemberRequired', 403, 'Forbidden', '僅固定參賽者可設定準備狀態。'],
    ['playerNotConnected', 409, 'Conflict', '請先連線至競賽房間。'],
    ['lockedDeckMissing', 409, 'Conflict', '競賽鎖定牌組尚未就緒。'],
    ['gameAlreadyStarted', 409, 'Conflict', '對局已開始，無法變更準備狀態。'],
    ['gameFinished', 409, 'Conflict', '對局已結束，無法變更準備狀態。'],
    ['checkInDeadlinePassed', 409, 'Conflict', '比賽開始時間已到，無法變更房間報到狀態。'],
    ['roomNotWaiting', 409, 'Conflict', '房間目前不在準備階段。'],
    ['competitionGameProtocolError', 502, 'Bad Gateway', '競賽房間回應格式不正確。'],
    ['competitionGameRoomUnavailable', 503, 'Service Unavailable', '競賽房間暫時無法使用，請稍後重試。'],
  ]
  for (const [code, statusCode, statusMessage, message] of cases) {
    const error = competitionGameReadyError(code)
    expect(error.statusCode).toBe(statusCode)
    expect(error.statusMessage).toBe(statusMessage)
    expect(error.message).toBe(message)
    expect(error.data).toEqual({ code, message })
    expect(error.toJSON()).toEqual({ statusCode, statusMessage, message, data: { code, message } })
  }
})

test('RPC adapter uses the normalized namespace and a bound dedicated method', async () => {
  const requests: CompetitionGameReadyRequest[] = []
  const stub = {
    marker: 'bound-method',
    async setCompetitionReady(request: CompetitionGameReadyRequest) {
      expect(this.marker).toBe('bound-method')
      requests.push(request)
      return { ok: true, ready: request.ready }
    },
    async fetch() { throw new Error('legacy fetch must not be used') },
  }
  const harness = readyHarness(stub)
  const response = await harness.request()
  expect(response.status).toBe(200)
  const value: unknown = await response.json()
  expect(value).toEqual({ ready: true })
  expect(harness.roomNames).toEqual(['game-1'])
  expect(harness.roomIds).toEqual(['room:game-1'])
  expect(requests).toEqual([{ actorUserId: 'session-user', ready: true }])
})

test('ready body rejects missing, coerced or non-object booleans', () => {
  for (const body of [undefined, null, [], true, false, 1, 'true', {}, { ready: 1 },
    { ready: 0 }, { ready: 'true' }, { ready: null }, { ready: [] }, { ready: {} }]) {
    expect(() => buildCompetitionGameReadyRequest(body, 'session-user')).toThrow('競賽準備要求格式不正確。')
  }
})

test('ready body rejects identity, deck, time and every extra key', () => {
  for (const key of ['actorUserId', 'callerUserId', 'userId', 'gameId', 'competitionId', 'matchId',
    'team', 'player', 'serverActor', 'deck', 'deckList', 'deckSnapshot', 'deckSeed',
    'trustedTimeMs', 'time', 'commandId', 'binding', 'turnOrder', 'extra', '__proto__']) {
    expect(() => buildCompetitionGameReadyRequest({ ready: true, [key]: 'forged-secret' }, 'session-user'))
      .toThrow('競賽準備要求格式不正確。')
  }
  expect(() => buildCompetitionGameReadyRequest({ ready: true, [Symbol('private')]: 'secret' }, 'session-user'))
    .toThrow('競賽準備要求格式不正確。')
  expect(() => buildCompetitionGameReadyRequest(Object.defineProperty({ ready: true }, 'private', { value: 'secret' }), 'session-user'))
    .toThrow('競賽準備要求格式不正確。')
})

test('trusted session actor must be a nonempty normalized identity', () => {
  for (const actor of ['', ' ', ' session-user', 'session-user ']) {
    expect(() => buildCompetitionGameReadyRequest({ ready: false }, actor)).toThrow('競賽準備要求格式不正確。')
  }
})

test('game path normalization rejects absent or non-string identity', () => {
  expect(normalizeCompetitionGameId(' \tgame-1\n ')).toBe('game-1')
  for (const id of [undefined, null, '', ' \t ', 1, {}, ['game-1']]) {
    expect(() => normalizeCompetitionGameId(id)).toThrow('競賽準備要求格式不正確。')
  }
})

test('invalid trusted RPC input never resolves a namespace', async () => {
  const harness = readyHarness({}, {
    readyRequest: { actorUserId: ' session-user', ready: true },
  })
  const response = await harness.request()
  expect(response.status).toBe(400)
  expect(await response.json()).toMatchObject({ data: { code: 'invalidRequest' } })
  expect(harness.roomNames).toEqual([])
  expect(harness.roomIds).toEqual([])
})

test('fetch-only and malformed capabilities fail closed without legacy fallback', async () => {
  let fetchCalls = 0
  const fetchOnly = { async fetch() { fetchCalls += 1; throw new Error('private-legacy-secret') } }
  for (const stub of [undefined, null, [], {}, fetchOnly, { setCompetitionReady: true }, { setCompetitionReady: null }]) {
    const response = await readyHarness(stub).request()
    expect(response.status).toBe(503)
    const text = await response.text()
    expect(JSON.parse(text)).toMatchObject({ data: { code: 'competitionGameRoomUnavailable' } })
    expect(text).not.toContain('private-legacy-secret')
  }
  expect(fetchCalls).toBe(0)
})

test('missing or malformed namespace and environment return a fixed availability error', async () => {
  const options = [
    { namespace: undefined }, { namespace: null }, { namespace: {} },
    { namespace: { idFromName: true, get: () => ({}) } },
    { namespace: { idFromName: () => 'game-1', get: false } },
    { environment: { APP_ENV: 'private-environment-secret' } },
    { environment: { APP_ENV: 'development' } },
  ]
  for (const option of options) {
    const response = await readyHarness({}, option).request()
    expect(response.status).toBe(503)
    const text = await response.text()
    expect(JSON.parse(text)).toMatchObject({ data: { code: 'competitionGameRoomUnavailable' } })
    expect(text).not.toMatch(/private-environment-secret|APP_ENV|Cloudflare bindings/)
  }
})

test('namespace lookup exceptions do not reflect internal messages or statuses', async () => {
  for (const namespace of [
    { idFromName() { throw new Error('private-id-secret') }, get() { throw new Error('unreachable') } },
    { idFromName: () => 'game-1', get() { throw createError({ statusCode: 418, statusMessage: 'private-get-secret' }) } },
  ]) {
    const response = await readyHarness({}, { namespace }).request()
    expect(response.status).toBe(503)
    const text = await response.text()
    expect(JSON.parse(text)).toMatchObject({ data: { code: 'competitionGameRoomUnavailable' } })
    expect(text).not.toMatch(/private-id-secret|private-get-secret|418/)
  }
})

test('RPC exceptions map to a fixed transient error without retrying or reflecting private data', async () => {
  let calls = 0
  for (const failure of [new Error('engine:private-rpc-secret'), createError({
    statusCode: 403, statusMessage: 'private-acl-secret', data: { deckSnapshot: 'secret-deck' },
  })]) {
    const response = await readyHarness({
      async setCompetitionReady() { calls += 1; throw failure },
    }).request()
    expect(response.status).toBe(503)
    const text = await response.text()
    expect(JSON.parse(text)).toMatchObject({ data: { code: 'competitionGameRoomUnavailable' } })
    expect(text).not.toMatch(/engine:|private-rpc-secret|private-acl-secret|secret-deck|deckSnapshot/)
  }
  expect(calls).toBe(2)
})

test.each([
  ['invalidRequest', 400], ['notCompetitionRoom', 404], ['invalidCompetitionRoom', 409],
  ['fixedMemberRequired', 403], ['playerNotConnected', 409], ['lockedDeckMissing', 409],
  ['gameAlreadyStarted', 409], ['gameFinished', 409], ['roomNotWaiting', 409],
])('dedicated ready failure %s has fixed HTTP status %i', async (code, status) => {
  const response = await readyHarness({ async setCompetitionReady() { return { ok: false, code } } }).request()
  expect(response.status).toBe(status)
  expect(await response.json()).toMatchObject({ data: { code } })
})

test('malformed RPC envelopes and acknowledgments fail with a fixed protocol error', async () => {
  for (const result of [undefined, null, [], true, 1, 'private-result-secret', {}, { ok: 'true', ready: true },
    { ok: true }, { ok: true, ready: 'true' }, { ok: true, ready: false }, { ok: false },
    { ok: false, code: 1 }, { ok: false, code: 'engine:private-result-secret' },
    { ok: false, code: 'constructor' }, { ok: false, code: 'competitionGameRoomUnavailable' }]) {
    const response = await readyHarness({ async setCompetitionReady() { return result } }).request()
    expect(response.status).toBe(502)
    const text = await response.text()
    expect(JSON.parse(text)).toMatchObject({ data: { code: 'competitionGameProtocolError' } })
    expect(text).not.toMatch(/private-result-secret|engine:/)
  }
})

test('RPC success and failure reject extra public or private fields instead of returning them', async () => {
  for (const envelope of [{ ok: true, ready: true }, { ok: false, code: 'fixedMemberRequired' }]) {
    for (const key of ['private', 'binding', 'players', 'deckSnapshot', 'deckSeed', 'turnOrder',
      'serverState', 'message', 'error', 'metadata', 'extra']) {
      const response = await readyHarness({
        async setCompetitionReady() { return { ...envelope, [key]: 'private-projection-secret' } },
      }).request()
      expect(response.status).toBe(502)
      const text = await response.text()
      expect(JSON.parse(text)).toMatchObject({ data: { code: 'competitionGameProtocolError' } })
      expect(text).not.toContain('private-projection-secret')
    }
  }
})

test('RPC success may confirm an explicit false desired state', async () => {
  const response = await readyHarness({
    async setCompetitionReady(request: CompetitionGameReadyRequest) { return { ok: true, ready: request.ready } },
  }, { readyRequest: { actorUserId: 'session-user', ready: false } }).request()
  expect(response.status).toBe(200)
  const value: unknown = await response.json()
  expect(value).toEqual({ ready: false })
})

test('native RPC disposer is released once and never becomes part of the public ready DTO', async () => {
  for (const ready of [true, false]) {
    let calls = 0
    let disposed = 0
    const result = { ok: true, ready }
    Object.defineProperty(result, Symbol.dispose, {
      value() {
        expect(this).toBe(result)
        disposed += 1
      },
    })
    const response = await readyHarness({
      async setCompetitionReady() { calls += 1; return result },
    }, { readyRequest: { actorUserId: 'session-user', ready } }).request()
    expect(response.status).toBe(200)
    const value: unknown = await response.json()
    expect(value).toEqual({ ready })
    expect(disposed).toBe(1)
    expect(calls).toBe(1)
  }
})

test('native transport never permits the disposer in an HTTP request body', () => {
  let disposed = 0
  const body = Object.defineProperty({ ready: true }, Symbol.dispose, {
    value() { disposed += 1 },
  })
  expect(() => buildCompetitionGameReadyRequest(body, 'session-user'))
    .toThrow('競賽準備要求格式不正確。')
  expect(disposed).toBe(0)
})

test('native RPC lifecycle does not weaken exact string or unknown symbol validation', async () => {
  for (const key of ['extra', 'private', 'binding', 'deckSeed', Symbol('private'),
    Symbol('Symbol.dispose'), Symbol.for('Symbol.dispose'), Symbol.asyncDispose]) {
    let disposed = 0
    let calls = 0
    const result = { ok: true, ready: true }
    Object.defineProperty(result, key, { value: 'private-transport-secret' })
    Object.defineProperty(result, Symbol.dispose, { value() { disposed += 1 } })
    const response = await readyHarness({
      async setCompetitionReady() { calls += 1; return result },
    }).request()
    expect(response.status).toBe(502)
    const text = await response.text()
    expect(JSON.parse(text)).toMatchObject({ data: { code: 'competitionGameProtocolError' } })
    expect(text).not.toContain('private-transport-secret')
    expect(disposed).toBe(1)
    expect(calls).toBe(1)
  }
})

test('native RPC known failures retain safe mapping and release their resource once', async () => {
  const cases: Array<[string, number]> = [
    ['invalidRequest', 400], ['notCompetitionRoom', 404], ['invalidCompetitionRoom', 409],
    ['fixedMemberRequired', 403], ['playerNotConnected', 409], ['lockedDeckMissing', 409],
    ['gameAlreadyStarted', 409], ['gameFinished', 409], ['roomNotWaiting', 409],
  ]
  for (const [code, status] of cases) {
    let disposed = 0
    const result = Object.defineProperty({ ok: false, code }, Symbol.dispose, {
      value() { disposed += 1 },
    })
    const response = await readyHarness({ async setCompetitionReady() { return result } }).request()
    expect(response.status).toBe(status)
    expect(await response.json()).toMatchObject({ data: { code } })
    expect(disposed).toBe(1)
  }
})

test('native malformed ACKs are disposed even when semantic validation fails', async () => {
  for (const envelope of [{ ok: true, ready: false }, { ok: true, ready: 'private-ready-secret' },
    { ok: false, code: 'engine:private-ready-secret' }, { ok: 'true', ready: true }, {}]) {
    let disposed = 0
    const result = Object.defineProperty(envelope, Symbol.dispose, { value() { disposed += 1 } })
    const response = await readyHarness({ async setCompetitionReady() { return result } }).request()
    expect(response.status).toBe(502)
    const text = await response.text()
    expect(JSON.parse(text)).toMatchObject({ data: { code: 'competitionGameProtocolError' } })
    expect(text).not.toMatch(/private-ready-secret|engine:/)
    expect(disposed).toBe(1)
  }
})

test('native malformed or throwing disposers fail with a fixed protocol error and no RPC retry', async () => {
  for (const disposer of [undefined, null, true, 1, 'private-disposer-secret', {}]) {
    const result = Object.defineProperty({ ok: true, ready: true }, Symbol.dispose, { value: disposer })
    const response = await readyHarness({ async setCompetitionReady() { return result } }).request()
    expect(response.status).toBe(502)
    const text = await response.text()
    expect(JSON.parse(text)).toMatchObject({ data: { code: 'competitionGameProtocolError' } })
    expect(text).not.toContain('private-disposer-secret')
  }
  let calls = 0
  let disposed = 0
  const result = Object.defineProperty({ ok: true, ready: true }, Symbol.dispose, {
    value() { disposed += 1; throw new Error('engine:private-disposal-secret') },
  })
  const response = await readyHarness({ async setCompetitionReady() { calls += 1; return result } }).request()
  expect(response.status).toBe(502)
  const text = await response.text()
  expect(JSON.parse(text)).toMatchObject({ data: { code: 'competitionGameProtocolError' } })
  expect(text).not.toMatch(/engine:|private-disposal-secret/)
  expect(disposed).toBe(1)
  expect(calls).toBe(1)
})

test('native ACK property access errors are safe and still release a captured disposer', async () => {
  let disposed = 0
  const result = Object.defineProperties({ ok: true }, {
    ready: { get() { throw new Error('private-accessor-secret') } },
    [Symbol.dispose]: { value() { disposed += 1 } },
  })
  const response = await readyHarness({ async setCompetitionReady() { return result } }).request()
  expect(response.status).toBe(502)
  const text = await response.text()
  expect(JSON.parse(text)).toMatchObject({ data: { code: 'competitionGameProtocolError' } })
  expect(text).not.toContain('private-accessor-secret')
  expect(disposed).toBe(1)
})
