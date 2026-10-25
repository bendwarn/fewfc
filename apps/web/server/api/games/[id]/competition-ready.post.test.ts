import { expect, mock, test } from 'bun:test'
import { createApp, createError, createRouter, eventHandler, toWebHandler } from 'h3'
import type { CompetitionGameReadyRequest } from '../../../../shared/competition-game-room'

mock.module('../../../utils/auth', () => ({
  requireSession: async (event: { headers: Headers }) => {
    const userId = event.headers.get('x-test-session')
    if (!userId) throw createError({ statusCode: 401, statusMessage: 'Authentication required.' })
    return { user: { id: userId } }
  },
}))

const { default: competitionReadyPost } = await import('./competition-ready.post')

function routeHarness(
  rpc: (request: CompetitionGameReadyRequest) => Promise<unknown> = async request => ({
    ok: true, ready: request.ready,
  }),
) {
  const roomNames: string[] = []
  const roomIds: unknown[] = []
  const requests: CompetitionGameReadyRequest[] = []
  let fetchCalls = 0
  const app = createApp()
  app.use(eventHandler((event) => {
    event.context._platform = {
      cloudflare: {
        env: {
          APP_ENV: 'development',
          GAME_ROOM: {
            idFromName(name: string) {
              roomNames.push(name)
              return `room:${name}`
            },
            get(id: unknown) {
              roomIds.push(id)
              return {
                async setCompetitionReady(request: CompetitionGameReadyRequest) {
                  requests.push(request)
                  return await rpc(request)
                },
                async fetch() {
                  fetchCalls += 1
                  throw new Error('legacy fetch must not be called')
                },
              }
            },
          },
        },
      },
    }
  }))
  const router = createRouter()
  router.post('/api/games/:id/competition-ready', competitionReadyPost)
  // 同一真 handler 的缺少 path 參數情境，不替換其驗證或 helper。
  router.post('/api/games/competition-ready', competitionReadyPost)
  app.use(router.handler)
  const handle = toWebHandler(app)
  const postRaw = (body: string | undefined, userId = 'session-user', path = '/api/games/game-1/competition-ready') => {
    const headers = new Headers({ 'content-type': 'application/json' })
    if (userId) headers.set('x-test-session', userId)
    return handle(new Request(`http://fewfc.test${path}`, { method: 'POST', headers, body }))
  }
  return {
    roomNames, roomIds, requests,
    fetchCalls: () => fetchCalls,
    postRaw,
    post: (body: unknown, userId?: string, path?: string) => postRaw(JSON.stringify(body), userId, path),
  }
}

test('authenticated H3 ready route derives the session actor and normalizes the game namespace', async () => {
  const harness = routeHarness()
  const response = await harness.post({ ready: true }, 'session-user', '/api/games/%20game-1%20/competition-ready')
  expect(response.status).toBe(200)
  const value: unknown = await response.json()
  expect(value).toEqual({ ready: true })
  expect(harness.roomNames).toEqual(['game-1'])
  expect(harness.roomIds).toEqual(['room:game-1'])
  expect(harness.requests).toEqual([{ actorUserId: 'session-user', ready: true }])
  expect(harness.fetchCalls()).toBe(0)
})

test('same desired ready state retries preserve true or false instead of toggling', async () => {
  const harness = routeHarness()
  for (const ready of [true, true, false, false]) {
    const response = await harness.post({ ready })
    expect(response.status).toBe(200)
    const value: unknown = await response.json()
    expect(value).toEqual({ ready })
  }
  expect(harness.requests).toEqual([true, true, false, false].map(ready => ({ actorUserId: 'session-user', ready })))
  expect(harness.fetchCalls()).toBe(0)
})

test('unauthenticated ready requests never resolve or dispatch to a room', async () => {
  const harness = routeHarness()
  const response = await harness.post({ ready: true }, '')
  expect(response.status).toBe(401)
  expect(harness.roomNames).toEqual([])
  expect(harness.requests).toEqual([])
})

test('empty or missing normalized game path fails before dispatch', async () => {
  const harness = routeHarness()
  for (const path of ['/api/games/%20%20/competition-ready', '/api/games/competition-ready']) {
    const response = await harness.post({ ready: true }, 'session-user', path)
    expect(response.status).toBe(400)
    expect(await response.json()).toMatchObject({ data: { code: 'invalidRequest' } })
  }
  expect(harness.roomNames).toEqual([])
  expect(harness.requests).toEqual([])
})

test('forged identity, deck, time and extra ready payloads never resolve or dispatch a room', async () => {
  const harness = routeHarness()
  for (const key of ['actorUserId', 'callerUserId', 'userId', 'gameId', 'competitionId', 'matchId',
    'team', 'player', 'serverActor', 'deck', 'deckList', 'deckSnapshot', 'deckSeed',
    'time', 'trustedTimeMs', 'commandId', 'binding', 'turnOrder', 'extra', '__proto__']) {
    const response = await harness.post({ ready: true, [key]: 'private-forged-secret' })
    expect(response.status).toBe(400)
    const text = await response.text()
    expect(JSON.parse(text)).toMatchObject({ data: { code: 'invalidRequest' } })
    expect(text).not.toContain('private-forged-secret')
  }
  expect(harness.roomNames).toEqual([])
  expect(harness.roomIds).toEqual([])
  expect(harness.requests).toEqual([])
  expect(harness.fetchCalls()).toBe(0)
})

test('missing or non-boolean ready JSON bodies are not coerced and never dispatch', async () => {
  const harness = routeHarness()
  for (const body of [undefined, null, [], true, false, 'true', 1, {}, { ready: 1 }, { ready: 0 },
    { ready: 'true' }, { ready: 'false' }, { ready: null }, { ready: [] }, { ready: {} }]) {
    const response = await harness.post(body)
    expect(response.status).toBe(400)
    expect(await response.json()).toMatchObject({ data: { code: 'invalidRequest' } })
  }
  expect(harness.roomNames).toEqual([])
  expect(harness.requests).toEqual([])
})

test('malformed JSON does not reflect parser errors or payload fragments', async () => {
  const harness = routeHarness()
  const response = await harness.postRaw('{"ready":true,"private-input-secret":')
  expect(response.status).toBe(400)
  const text = await response.text()
  expect(JSON.parse(text)).toMatchObject({ data: { code: 'invalidRequest' } })
  expect(text).not.toMatch(/private-input-secret|JSON|SyntaxError|Unexpected/)
  expect(harness.roomNames).toEqual([])
  expect(harness.requests).toEqual([])
})

test('query identity or desired state cannot override session and exact ready body', async () => {
  const harness = routeHarness()
  const response = await harness.post({ ready: false }, 'session-user',
    '/api/games/game-1/competition-ready?actorUserId=attacker&ready=true&deck=private-query-secret')
  expect(response.status).toBe(200)
  const value: unknown = await response.json()
  expect(value).toEqual({ ready: false })
  expect(harness.requests).toEqual([{ actorUserId: 'session-user', ready: false }])
  expect(harness.roomNames).toEqual(['game-1'])
})

test.each([
  ['invalidRequest', 400], ['notCompetitionRoom', 404], ['invalidCompetitionRoom', 409],
  ['fixedMemberRequired', 403], ['playerNotConnected', 409], ['lockedDeckMissing', 409],
  ['gameAlreadyStarted', 409], ['gameFinished', 409], ['roomNotWaiting', 409],
])('H3 ready route maps dedicated %s to fixed status %i', async (code, status) => {
  const harness = routeHarness(async () => ({ ok: false, code }))
  const response = await harness.post({ ready: true })
  expect(response.status).toBe(status)
  expect(await response.json()).toMatchObject({ data: { code } })
  expect(harness.requests).toEqual([{ actorUserId: 'session-user', ready: true }])
  expect(harness.fetchCalls()).toBe(0)
})

test('malformed or mismatched ready RPC values return safe 502 instead of public success', async () => {
  const malformed = [undefined, null, [], true, 'private-result-secret', {},
    { ok: 'true', ready: true }, { ok: true }, { ok: true, ready: 'true' },
    { ok: true, ready: false }, { ok: false }, { ok: false, code: 1 },
    { ok: false, code: 'engine:private-result-secret' }, { ok: false, code: 'constructor' }]
  for (const result of malformed) {
    const harness = routeHarness(async () => result)
    const response = await harness.post({ ready: true })
    expect(response.status).toBe(502)
    const text = await response.text()
    expect(JSON.parse(text)).toMatchObject({ data: { code: 'competitionGameProtocolError' } })
    expect(text).not.toMatch(/private-result-secret|engine:/)
    expect(harness.requests).toHaveLength(1)
  }
})

test('extra or private fields in dedicated RPC success and failure are rejected without disclosure', async () => {
  for (const envelope of [{ ok: true, ready: true }, { ok: false, code: 'fixedMemberRequired' }]) {
    for (const key of ['binding', 'players', 'deckSnapshot', 'deckSeed', 'turnOrder', 'serverState',
      'private', 'metadata', 'message', 'extra']) {
      const harness = routeHarness(async () => ({ ...envelope, [key]: { secret: 'private-rpc-secret' } }))
      const response = await harness.post({ ready: true })
      expect(response.status).toBe(502)
      const text = await response.text()
      expect(JSON.parse(text)).toMatchObject({ data: { code: 'competitionGameProtocolError' } })
      expect(text).not.toMatch(/private-rpc-secret|deckSnapshot|deckSeed|binding|players|turnOrder|serverState/)
      expect(harness.requests).toHaveLength(1)
    }
  }
})

test('RPC thrown messages, H3 statuses and private objects are not reflected by the route', async () => {
  for (const failure of [new Error('engine:private-transient-secret'), createError({
    statusCode: 418, statusMessage: 'private-h3-secret', data: { binding: 'private-binding-secret' },
  }), { code: 'fixedMemberRequired', message: 'private-object-secret' }]) {
    const harness = routeHarness(async () => { throw failure })
    const response = await harness.post({ ready: true })
    expect(response.status).toBe(503)
    const text = await response.text()
    expect(JSON.parse(text)).toMatchObject({ data: { code: 'competitionGameRoomUnavailable' } })
    expect(text).not.toMatch(/engine:|private-transient-secret|private-h3-secret|private-binding-secret|private-object-secret|418/)
    expect(harness.requests).toHaveLength(1)
    expect(harness.fetchCalls()).toBe(0)
  }
})

test.each([true, false])('response-loss retry preserves the same explicit desired ready=%s', async (desiredReady) => {
  let storedReady = !desiredReady
  let firstCall = true
  const harness = routeHarness(async (request) => {
    storedReady = request.ready
    if (firstCall) {
      firstCall = false
      throw new Error('private-response-loss-secret')
    }
    return { ok: true, ready: storedReady }
  })
  const lost = await harness.post({ ready: desiredReady })
  expect(lost.status).toBe(503)
  expect(await lost.text()).not.toContain('private-response-loss-secret')
  expect(harness.requests).toHaveLength(1)

  const retried = await harness.post({ ready: desiredReady })
  expect(retried.status).toBe(200)
  const value: unknown = await retried.json()
  expect(value).toEqual({ ready: desiredReady })
  expect(harness.requests).toEqual([
    { actorUserId: 'session-user', ready: desiredReady },
    { actorUserId: 'session-user', ready: desiredReady },
  ])
  expect(storedReady).toBe(desiredReady)
  expect(harness.fetchCalls()).toBe(0)
})

test('real H3 ready route consumes native disposer and keeps desired-state retries minimal', async () => {
  let disposed = 0
  const harness = routeHarness(async request => Object.defineProperty({ ok: true, ready: request.ready }, Symbol.dispose, {
    value() { disposed += 1 },
  }))
  for (const ready of [true, true, false, false]) {
    const response = await harness.post({ ready })
    expect(response.status).toBe(200)
    const value: unknown = await response.json()
    expect(value).toEqual({ ready })
  }
  expect(disposed).toBe(4)
  expect(harness.requests).toEqual([true, true, false, false].map(ready => ({ actorUserId: 'session-user', ready })))
  expect(harness.fetchCalls()).toBe(0)
})

test('real H3 ready route rejects private fields and unknown symbols but still disposes once', async () => {
  for (const key of ['binding', 'deckSeed', 'extra', Symbol('private'), Symbol.asyncDispose]) {
    let disposed = 0
    const result = { ok: true, ready: true }
    Object.defineProperty(result, key, { value: 'private-native-secret' })
    Object.defineProperty(result, Symbol.dispose, { value() { disposed += 1 } })
    const harness = routeHarness(async () => result)
    const response = await harness.post({ ready: true })
    expect(response.status).toBe(502)
    const text = await response.text()
    expect(JSON.parse(text)).toMatchObject({ data: { code: 'competitionGameProtocolError' } })
    expect(text).not.toContain('private-native-secret')
    expect(disposed).toBe(1)
    expect(harness.requests).toHaveLength(1)
  }
})

test('real H3 ready route maps native failures and cleanup throws without leaking internals', async () => {
  const cases: Array<[Record<string, unknown>, boolean, number, string]> = [
    [{ ok: false, code: 'fixedMemberRequired' }, false, 403, 'fixedMemberRequired'],
    [{ ok: true, ready: false }, false, 502, 'competitionGameProtocolError'],
    [{ ok: true, ready: true }, true, 502, 'competitionGameProtocolError'],
  ]
  for (const [envelope, failCleanup, status, code] of cases) {
    let disposed = 0
    const result = Object.defineProperty(envelope, Symbol.dispose, {
      value() {
        disposed += 1
        if (failCleanup) throw new Error('engine:private-native-cleanup-secret')
      },
    })
    const harness = routeHarness(async () => result)
    const response = await harness.post({ ready: true })
    expect(response.status).toBe(status)
    const text = await response.text()
    expect(JSON.parse(text)).toMatchObject({ data: { code } })
    expect(text).not.toMatch(/engine:|private-native-cleanup-secret/)
    expect(disposed).toBe(1)
    expect(harness.requests).toHaveLength(1)
  }
})

test('real H3 errors expose fixed ASCII reasons and safe Chinese data without RPC messages', async () => {
  const cases: Array<[() => Promise<unknown>, number, string, string, string]> = [
    [async () => ({ ok: false, code: 'fixedMemberRequired' }), 403, 'Forbidden', 'fixedMemberRequired', '僅固定參賽者可設定準備狀態。'],
    [async () => ({ ok: true, ready: true, message: 'private-reason-secret' }), 502, 'Bad Gateway', 'competitionGameProtocolError', '競賽房間回應格式不正確。'],
    [async () => { throw new Error('engine:private-reason-secret') }, 503, 'Service Unavailable', 'competitionGameRoomUnavailable', '競賽房間暫時無法使用，請稍後重試。'],
  ]
  for (const [rpc, statusCode, statusMessage, code, message] of cases) {
    const harness = routeHarness(rpc)
    const response = await harness.post({ ready: true })
    expect(response.status).toBe(statusCode)
    expect(response.statusText).toBe(statusMessage)
    const text = await response.text()
    const value: unknown = JSON.parse(text)
    expect(value).toMatchObject({ status: statusCode, statusText: statusMessage, data: { code, message } })
    expect(text).not.toMatch(/private-reason-secret|engine:/)
    expect(harness.requests).toHaveLength(1)
  }
})
