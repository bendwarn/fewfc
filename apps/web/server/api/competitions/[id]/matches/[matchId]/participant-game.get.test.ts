import { createError, createApp, createRouter, eventHandler, toWebHandler } from 'h3'
import { afterEach, expect, mock, test } from 'bun:test'
import type { CompetitionRoomParticipantGameReadRequest } from '../../../../../../shared/competition-room'

let sessionCalls: string[] = []
let roomNames: string[] = []
let rpcRequests: CompetitionRoomParticipantGameReadRequest[] = []
let rpcResult: unknown = null
let rpcThrows = false
let namespaceAvailable = true

mock.module('../../../../../utils/auth', () => ({
  requireSession: async (event: { headers: Headers }) => {
    const userId = event.headers.get('x-test-session')
    sessionCalls.push(userId ?? '')
    if (!userId) throw createError({ statusCode: 401, statusMessage: 'Authentication required.' })
    return { user: { id: userId } }
  },
}))

const { default: participantGameGet } = await import('./participant-game.get')

function success(game: unknown = null, overrides: Record<string, unknown> = {}) {
  return {
    ok: true,
    value: {
      competitionId: 'path-competition',
      matchId: 'path-match',
      matchRevision: 9,
      game,
      ...overrides,
    },
  }
}

function resetState(result: unknown = success()) {
  sessionCalls = []
  roomNames = []
  rpcRequests = []
  rpcResult = result
  rpcThrows = false
  namespaceAvailable = true
}

function makeWebHandler() {
  const app = createApp()
  app.use(eventHandler((event) => {
    const env: Record<string, unknown> = { APP_ENV: 'development' }
    if (namespaceAvailable) {
      env.COMPETITION_ROOM = {
        idFromName: (name: string) => {
          roomNames.push(name)
          return name
        },
        get: () => ({
          readParticipantGame: async (request: CompetitionRoomParticipantGameReadRequest) => {
            rpcRequests.push(request)
            if (rpcThrows) throw new Error('private transport failure')
            return rpcResult
          },
          read: async () => { throw new Error('generic read must never be called') },
          readMatch: async () => { throw new Error('organizer match read must never be called') },
          readProvisionedGame: async () => { throw new Error('organizer provision read must never be called') },
        }),
      }
    }
    ;(event.context as { _platform?: unknown })._platform = { cloudflare: { env } }
  }))
  const router = createRouter()
  router.get('/api/competitions/:id/matches/:matchId/participant-game', participantGameGet)
  app.use(router.handler)
  return toWebHandler(app)
}

async function get(
  path = '/api/competitions/path-competition/matches/path-match/participant-game',
  sessionUserId?: string,
  extraHeaders?: HeadersInit,
) {
  const headers = new Headers(extraHeaders)
  if (sessionUserId) headers.set('x-test-session', sessionUserId)
  return makeWebHandler()(new Request(`http://fewfc.test${path}`, { method: 'GET', headers }))
}

afterEach(() => {
  mock.restore()
})

test('authenticated GET trims route identity, uses the session actor, and returns only the current-game projection', async () => {
  resetState(success({ gameId: 'competition-game-1', gameNumber: 1 }))
  const response = await get(
    '/api/competitions/%20path-competition%20/matches/%20path-match%20/participant-game',
    'session-player',
  )
  expect(response.status).toBe(200)
  const responseValue = await response.json()
  expect(responseValue).toEqual({
    competitionId: 'path-competition',
    matchId: 'path-match',
    matchRevision: 9,
    game: { gameId: 'competition-game-1', gameNumber: 1 },
  })
  expect(sessionCalls).toEqual(['session-player'])
  expect(roomNames).toEqual(['path-competition'])
  expect(rpcRequests).toEqual([{
    competitionId: 'path-competition',
    matchId: 'path-match',
    callerUserId: 'session-player',
  }])
  expect(JSON.stringify(responseValue)).not.toMatch(/callerUserId|trustedTimeMs|expectedRevision|binding|roster|deck|private/)
})

test('supports fixed 1v1 and 2v2 session members with the same minimal response shape', async () => {
  for (const [gameNumber, participantIds] of [
    [1, ['solo-a', 'solo-b']],
    [2, ['team-a-player-1', 'team-a-player-2', 'team-b-player-1', 'team-b-player-2']],
  ] as const) {
    resetState(success({ gameId: `competition-game-${gameNumber}`, gameNumber }))
    for (const participantId of participantIds) {
      const response = await get(undefined, participantId)
      expect(response.status).toBe(200)
      const actual: unknown = await response.json()
      expect(actual).toEqual({
        competitionId: 'path-competition',
        matchId: 'path-match',
        matchRevision: 9,
        game: { gameId: `competition-game-${gameNumber}`, gameNumber },
      })
    }
    expect(rpcRequests.map(request => request.callerUserId)).toEqual([...participantIds])
    expect(rpcRequests.every(request => Object.keys(request).sort().join(',')
      === 'callerUserId,competitionId,matchId')).toBe(true)
  }
})

test('returns a successful null game when the current game is not materialized', async () => {
  resetState(success(null))
  const response = await get(undefined, 'session-player')
  expect(response.status).toBe(200)
  const actual: unknown = await response.json()
  expect(actual).toEqual({
    competitionId: 'path-competition', matchId: 'path-match', matchRevision: 9, game: null,
  })
  expect(rpcRequests).toHaveLength(1)
})

test('accepts only the mock RPC envelope Symbol.dispose and always cleans the resource in finally (not native proof)', async () => {
  let disposeCalls = 0
  const disposableSuccess = success({ gameId: 'competition-game-1', gameNumber: 1 })
  Object.defineProperty(disposableSuccess, Symbol.dispose, {
    value: () => { disposeCalls += 1 },
  })
  resetState(disposableSuccess)
  const successfulResponse = await get(undefined, 'session-player')
  expect(successfulResponse.status).toBe(200)
  expect(disposeCalls).toBe(1)

  const privateExtra = success(null)
  Object.defineProperty(privateExtra, Symbol.dispose, {
    value: () => { disposeCalls += 1 },
  })
  Object.defineProperty(privateExtra, 'privateMarker', { value: 'private-root-marker' })
  resetState(privateExtra)
  const privateResponse = await get(undefined, 'session-player')
  const privateBody = await privateResponse.text()
  expect(privateResponse.status).toBe(502)
  expect(privateBody).not.toContain('private-root-marker')
  expect(disposeCalls).toBe(2)

  const symbolGame = { gameId: 'competition-game-1', gameNumber: 1 }
  Object.defineProperty(symbolGame, Symbol.for('private-symbol'), { value: true })
  const nestedSymbol = success(symbolGame)
  Object.defineProperty(nestedSymbol, Symbol.dispose, {
    value: () => { disposeCalls += 1 },
  })
  resetState(nestedSymbol)
  expect((await get(undefined, 'session-player')).status).toBe(502)
  expect(disposeCalls).toBe(3)

  const extraRootSymbol = success(null)
  Object.defineProperty(extraRootSymbol, Symbol.dispose, {
    value: () => { disposeCalls += 1 },
  })
  Object.defineProperty(extraRootSymbol, Symbol.for('private-symbol'), { value: true })
  resetState(extraRootSymbol)
  expect((await get(undefined, 'session-player')).status).toBe(502)
  expect(disposeCalls).toBe(4)
})

test('authentication, spoofed query fields, invalid paths, and declared bodies fail before RPC dispatch', async () => {
  resetState()
  expect((await get()).status).toBe(401)
  for (const query of [
    'callerUserId=attacker',
    'gameId=forged-game',
    'matchRevision=999',
    'revision=999',
    'trustedTimeMs=1',
  ]) {
    expect((await get(`/api/competitions/path-competition/matches/path-match/participant-game?${query}`,
      'session-player')).status).toBe(400)
  }
  expect((await get('/api/competitions/%20/matches/path-match/participant-game', 'session-player')).status).toBe(400)
  expect((await get('/api/competitions/path-competition/matches/%20/participant-game', 'session-player')).status).toBe(400)
  expect((await get(undefined, 'session-player', { 'content-length': '1' })).status).toBe(400)
  expect(rpcRequests).toHaveLength(0)
  expect(roomNames).toHaveLength(0)
})

test('maps ACL, checkpoint, missing, and linkage failures to fixed safe HTTP responses', async () => {
  for (const [code, statusCode] of [
    ['forbidden', 403],
    ['invalidMatchCheckpoint', 409],
    ['invalidLockedRoster', 409],
    ['invalidGameProvision', 409],
    ['matchNotFound', 404],
  ] as const) {
    resetState({ ok: false, error: { code, message: 'private roster checkpoint detail' } })
    const response = await get(undefined, 'session-player')
    const body = await response.text()
    expect(response.status).toBe(statusCode)
    expect(body).not.toContain('private roster checkpoint detail')
    expect(body).not.toMatch(/binding|serverState|private roster/)
    expect(rpcRequests).toHaveLength(1)
  }
})

test('transient namespace and RPC failures return a safe service error without automatic retry', async () => {
  resetState()
  namespaceAvailable = false
  const unavailable = await get(undefined, 'session-player')
  expect(unavailable.status).toBe(503)
  expect((await unavailable.text())).not.toMatch(/binding|namespace|private/)
  expect(rpcRequests).toHaveLength(0)

  resetState()
  rpcThrows = true
  const failed = await get(undefined, 'session-player')
  const body = await failed.text()
  expect(failed.status).toBe(503)
  expect(body).not.toContain('private transport failure')
  expect(rpcRequests).toHaveLength(1)
})

test('rejects forged envelopes, private extras, and invalid revision or game fields without reflection', async () => {
  const malformed: unknown[] = [
    { ...success(null), private: { secret: 'root-private-marker' } },
    success(null, { serverState: { secret: 'value-private-marker' } }),
    success({ gameId: 'game-1', gameNumber: 1, binding: { secret: 'game-private-marker' } }),
    success(null, { competitionId: 'other-competition' }),
    success(null, { matchId: 'other-match' }),
    success(null, { matchRevision: 0 }),
    success(null, { matchRevision: 1.5 }),
    success(null, { matchRevision: Number.MAX_SAFE_INTEGER + 1 }),
    success({ gameId: '', gameNumber: 1 }),
    success({ gameId: ' game-1 ', gameNumber: 1 }),
    success({ gameId: 'game-1', gameNumber: 0 }),
    success({ gameId: 'game-1', gameNumber: 1.5 }),
    success({ gameId: 'game-1', gameNumber: Number.MAX_SAFE_INTEGER + 1 }),
    { ok: false, error: { code: 'private-error-code', message: 'private error message' }, extra: true },
    {
      ok: false,
      error: { code: 'forbidden', message: 'private forbidden detail', private: 'nested-private-marker' },
    },
  ]

  for (const responseValue of malformed) {
    resetState(responseValue)
    const response = await get(undefined, 'session-player')
    const body = await response.text()
    expect(response.status).toBe(502)
    expect(body).not.toMatch(/root-private-marker|value-private-marker|game-private-marker|private-error-code|private error message|private forbidden detail|nested-private-marker/)
    expect(rpcRequests).toHaveLength(1)
  }
})
