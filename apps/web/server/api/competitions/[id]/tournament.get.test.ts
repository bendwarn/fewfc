import { createApp, createError, createRouter, eventHandler, toWebHandler } from 'h3'
import { afterEach, expect, mock, test } from 'bun:test'
import type { CompetitionRoomTournamentReadRequest } from '../../../../shared/competition-room'

let sessionCalls: string[] = []
let roomNames: string[] = []
let rpcRequests: CompetitionRoomTournamentReadRequest[] = []
let rpcResult: unknown = null
let rpcError: unknown = null
let namespaceAvailable = true

mock.module('../../../utils/auth', () => ({
  requireSession: async (event: { headers: Headers }) => {
    const userId = event.headers.get('x-test-session')
    sessionCalls.push(userId ?? '')
    if (!userId) throw createError({ statusCode: 401, statusMessage: 'Unauthorized' })
    return { user: { id: userId } }
  },
}))

const { default: tournamentGet } = await import('./tournament.get')

function tournament(format: 'singleElimination' | 'roundRobin' | 'doubleElimination' | 'swiss') {
  const rounds = format === 'swiss'
    ? [1, 2, 3]
    : format === 'doubleElimination'
      ? [1, 2, 3]
      : [1, 2]
  return {
    tournamentId: 'competition-1',
    stages: [{
      config: {
        format,
        matchFormat: { type: 'fixedGames', value: 1 },
        clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
        swissRounds: format === 'swiss' ? 3 : null,
      },
      status: 'finished',
      standings: [
        { teamId: 'team-a', rank: 1, matchPoints: '3' },
        { teamId: 'team-b', rank: 2, matchPoints: '0' },
      ],
      rounds: rounds.map((roundNumber, index) => ({
        roundNumber,
        kind: format === 'doubleElimination'
          ? ['winners', 'grandFinal', 'grandFinalReset'][index]
          : 'main',
        matches: [{
          matchId: `${format}-round-${roundNumber}`,
          participants: ['team-a', 'team-b'],
          status: 'completed',
          started: true,
          result: { type: 'win', winnerId: 'team-a', loserId: 'team-b' },
          cancellationReason: null,
        }],
      })),
    }],
  }
}

function success(publicValue: unknown, overrides: Record<string, unknown> = {}) {
  return {
    ok: true,
    value: {
      competitionId: 'competition-1',
      revision: 8,
      projection: { ok: true, operation: 'tournamentView', public: publicValue },
      ...overrides,
    },
  }
}

function reset(result: unknown = success(tournament('singleElimination'))) {
  sessionCalls = []
  roomNames = []
  rpcRequests = []
  rpcResult = result
  rpcError = null
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
          readTournament: async (request: CompetitionRoomTournamentReadRequest) => {
            rpcRequests.push(request)
            if (rpcError) throw rpcError
            return rpcResult
          },
          read: async () => { throw new Error('generic global projection read must never be called') },
          readMatch: async () => { throw new Error('match projection fallback must never be called') },
          audit: async () => { throw new Error('audit fallback must never be called') },
        }),
      }
    }
    ;(event.context as { _platform?: unknown })._platform = { cloudflare: { env } }
  }))
  const router = createRouter()
  router.get('/api/competitions/:id/tournament', tournamentGet)
  app.use(router.handler)
  return toWebHandler(app)
}

async function get(
  path = '/api/competitions/competition-1/tournament',
  sessionUserId?: string,
  headersInit?: HeadersInit,
) {
  const headers = new Headers(headersInit)
  if (sessionUserId) headers.set('x-test-session', sessionUserId)
  return makeWebHandler()(new Request(`http://fewfc.test${path}`, { method: 'GET', headers }))
}

afterEach(() => {
  mock.restore()
})

test('authenticated GET trims path, uses canonical session actor, and returns the exact complete DTO', async () => {
  const current = tournament('singleElimination')
  reset(success(current))
  const response = await get('/api/competitions/%20competition-1%20/tournament', 'session-organizer')
  expect(response.status).toBe(200)
  const body: unknown = await response.json()
  expect(body).toEqual({
    competitionId: 'competition-1',
    revision: 8,
    projection: { ok: true, operation: 'tournamentView', public: current },
  })
  expect(Object.keys(body as object).sort()).toEqual(['competitionId', 'projection', 'revision'])
  expect(sessionCalls).toEqual(['session-organizer'])
  expect(roomNames).toEqual(['competition-1'])
  expect(rpcRequests).toEqual([{ competitionId: 'competition-1', callerUserId: 'session-organizer' }])
  expect(JSON.stringify(body)).not.toMatch(/callerUserId|expectedRevision|trustedTimeMs|serverState|private|binding|deckSeed/)
  expect(rpcRequests).toHaveLength(1)
})

test('returns full current public DTOs for all four formats including completed schedules', async () => {
  for (const format of ['singleElimination', 'roundRobin', 'doubleElimination', 'swiss'] as const) {
    const current = tournament(format)
    reset(success(current))
    const response = await get(undefined, 'session-player')
    expect(response.status).toBe(200)
    const body: unknown = await response.json()
    expect(body).toEqual({
      competitionId: 'competition-1',
      revision: 8,
      projection: { ok: true, operation: 'tournamentView', public: current },
    })
    expect(rpcRequests).toEqual([{ competitionId: 'competition-1', callerUserId: 'session-player' }])
    expect(rpcRequests).toHaveLength(1)
  }
})

test('requires session first and rejects invalid path, spoofed query, and body before RPC dispatch', async () => {
  reset()
  expect((await get('/api/competitions/%20/tournament')).status).toBe(401)
  expect(sessionCalls).toEqual([''])
  expect((await get('/api/competitions/%20/tournament', 'session-user')).status).toBe(400)
  for (const query of [
    'callerUserId=attacker',
    'revision=999',
    'expectedRevision=999',
    'trustedTimeMs=1',
    'projection=latest',
  ]) {
    expect((await get(`/api/competitions/competition-1/tournament?${query}`, 'session-user')).status).toBe(400)
  }
  expect((await get(undefined, 'session-user', { 'content-length': '1' })).status).toBe(400)
  expect(roomNames).toHaveLength(0)
  expect(rpcRequests).toHaveLength(0)
})

test('maps not-ready, missing, ACL and malformed RPC failures to safe fixed H3 responses', async () => {
  for (const [code, statusCode, statusMessage, safeMessage] of [
    ['notReady', 409, 'Conflict', '賽事目前尚未準備就緒。'],
    ['notInitialized', 404, 'Not Found', '找不到這場賽事。'],
    ['competitionIdMismatch', 404, 'Not Found', '找不到這場賽事。'],
    ['notParticipant', 403, 'Forbidden', '你沒有這場賽事的存取權。'],
    ['invalidTournamentCheckpoint', 502, 'Bad Gateway', '賽事房間回應格式不正確。'],
  ] as const) {
    reset({ ok: false, error: { code, message: 'private checkpoint or session details' } })
    const response = await get(undefined, 'session-user')
    expect(response.status).toBe(statusCode)
    expect(response.statusText).toBe(statusMessage)
    const body = await response.text()
    expect(body).toContain(safeMessage)
    expect(body).not.toMatch(/private checkpoint or session details|private|session details/)
    expect(rpcRequests).toHaveLength(1)
  }

  reset({ ok: false, error: { code: 'unknown-private-error', message: 'private raw reason' } })
  const malformed = await get(undefined, 'session-user')
  expect(malformed.status).toBe(502)
  expect(await malformed.text()).not.toMatch(/unknown-private-error|private raw reason/)
  expect(rpcRequests).toHaveLength(1)
})

test('accepts only a root native-style mock disposer and always calls it once in finally (not platform proof)', async () => {
  let disposeCalls = 0
  let receiverCorrect = false
  const current = success(tournament('doubleElimination'))
  Object.defineProperty(current, Symbol.dispose, {
    value: function dispose(this: unknown) {
      disposeCalls += 1
      receiverCorrect = this === current
    },
  })
  reset(current)
  const response = await get(undefined, 'session-user')
  expect(response.status).toBe(200)
  expect(disposeCalls).toBe(1)
  expect(receiverCorrect).toBe(true)
  expect(rpcRequests).toHaveLength(1)

  const privateExtra = success(tournament('roundRobin'))
  Object.defineProperty(privateExtra, Symbol.dispose, { value: () => { disposeCalls += 1 } })
  Object.defineProperty(privateExtra, 'private', { value: 'never-return-this' })
  reset(privateExtra)
  const rejected = await get(undefined, 'session-user')
  expect(rejected.status).toBe(502)
  expect(await rejected.text()).not.toContain('never-return-this')
  expect(disposeCalls).toBe(2)
  expect(rpcRequests).toHaveLength(1)

  const nestedSymbolPublic = tournament('swiss')
  Object.defineProperty(nestedSymbolPublic.stages[0], Symbol.for('unknown-nested'), { value: true })
  const nestedSymbol = success(nestedSymbolPublic)
  Object.defineProperty(nestedSymbol, Symbol.dispose, { value: () => { disposeCalls += 1 } })
  reset(nestedSymbol)
  expect((await get(undefined, 'session-user')).status).toBe(502)
  expect(disposeCalls).toBe(3)

  const unknownRootSymbol = success(tournament('singleElimination'))
  Object.defineProperty(unknownRootSymbol, Symbol.dispose, { value: () => { disposeCalls += 1 } })
  Object.defineProperty(unknownRootSymbol, Symbol.for('unknown-envelope'), { value: true })
  reset(unknownRootSymbol)
  expect((await get(undefined, 'session-user')).status).toBe(502)
  expect(disposeCalls).toBe(4)
  expect(rpcRequests).toHaveLength(1)

  let asyncDisposeCalls = 0
  const asyncDisposer = success(tournament('singleElimination'))
  Object.defineProperty(asyncDisposer, Symbol.dispose, {
    value: async function disposeAsync() {
      asyncDisposeCalls += 1
    },
  })
  reset(asyncDisposer)
  expect((await get(undefined, 'session-user')).status).toBe(502)
  expect(asyncDisposeCalls).toBe(0)
})

test('transport failures and missing RPC capability are safe 503 responses without retries or fallbacks', async () => {
  reset()
  rpcError = new Error('private transport address')
  const failed = await get(undefined, 'session-user')
  expect(failed.status).toBe(503)
  expect(await failed.text()).not.toContain('private transport address')
  expect(rpcRequests).toHaveLength(1)

  reset()
  namespaceAvailable = false
  const unavailable = await get(undefined, 'session-user')
  expect(unavailable.status).toBe(503)
  expect(rpcRequests).toHaveLength(0)
  expect(roomNames).toHaveLength(0)
})
