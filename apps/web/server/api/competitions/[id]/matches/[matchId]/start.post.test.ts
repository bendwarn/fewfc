import { createError, createApp, createRouter, eventHandler, toWebHandler } from 'h3'
import { afterEach, expect, mock, test } from 'bun:test'
import type {
  CompetitionRoomProvisionInitialScheduledGameRequest,
  CompetitionRoomStartScheduledMatchRequest,
} from '../../../../../../shared/competition-room'
import { validateCompetitionGameBinding, type CompetitionGameBinding } from '../../../../../../shared/competition-game-room'

let sessionCalls: string[] = []
let roomNames: string[] = []
let startRequests: CompetitionRoomStartScheduledMatchRequest[] = []
let initialGameRequests: CompetitionRoomProvisionInitialScheduledGameRequest[] = []
let rpcResults: unknown[] = []

mock.module('../../../../../utils/auth', () => ({
  requireSession: async (event: { headers: Headers }) => {
    const userId = event.headers.get('x-test-session')
    sessionCalls.push(userId ?? '')
    if (!userId) throw createError({ statusCode: 401, statusMessage: 'Authentication required.' })
    return { user: { id: userId } }
  },
}))

const { default: startPost } = await import('./start.post')

// 僅驗正式 H3 adapter／fixture 契約；RPC stub 不構成整輪物化、release 或實際 start 證據。
const roundBindingEpochMs = 1_791_158_400_000
const binding = {
  roundRef: {
    competitionId: 'path-competition', stageId: 'stage-1', roundNumber: 1,
    roundGenerationId: 'round-generation-1',
  },
  roundStartMs: roundBindingEpochMs + 600_000,
  competitionId: 'path-competition',
  matchId: 'path-match',
  gameNumber: 1,
  organizerUserId: 'organizer-user',
  expectedMatchRevision: 2,
  startKind: { type: 'ordinaryNewGame' },
  ruleVersion: '5.17',
  ruleset: 'fewfc-base',
  enabledRuleModules: [],
  initialHp: [{ team: 'secret-team-a', hp: 20 }, { team: 'secret-team-b', hp: 20 }],
  players: [
    { userId: 'secret-user-a', player: 'secret-player-a', team: 'secret-team-a' },
    { userId: 'secret-user-b', player: 'secret-player-b', team: 'secret-team-b' },
  ],
  deckSnapshot: [],
  firstPlayer: 'secret-player-a',
  turnOrder: ['secret-player-a', 'secret-player-b'],
  deckSeed: 'secret-deck-seed',
} satisfies CompetitionGameBinding

function startedResult(overrides: Record<string, unknown> = {}) {
  return {
    ok: true,
    value: {
      competitionId: 'path-competition', stageId: 'stage-1', matchId: 'path-match',
      revision: 6, matchRevision: 1, commandId: 'root-command:start-schedule', idempotent: false,
      tournamentProjection: { ok: true, operation: 'tournamentStartMatch', public: { visible: true } },
      matchProjection: { ok: true, operation: 'matchCreate', public: { visible: true } },
      clockProjection: { ok: true, operation: 'clockCreate', public: { visible: true } },
      ...overrides,
    },
  }
}

function provisionResult(overrides: Record<string, unknown> = {}) {
  return {
    ok: true,
    value: {
      competitionId: 'path-competition', matchId: 'path-match', matchRevision: 2,
      gameNumber: 1, gameId: 'generated-game-id', commandId: 'root-command:initial-game',
      idempotent: false, status: 'pending', binding, ...overrides,
    },
  }
}

function resetState(results: unknown[] = [startedResult(), provisionResult()]) {
  sessionCalls = []
  roomNames = []
  startRequests = []
  initialGameRequests = []
  rpcResults = [...results]
}

function makeWebHandler() {
  const app = createApp()
  app.use(eventHandler((event) => {
    ;(event.context as { _platform?: unknown })._platform = {
      cloudflare: {
        env: {
          APP_ENV: 'development',
          COMPETITION_ROOM: {
            idFromName: (name: string) => {
              roomNames.push(name)
              return name
            },
            get: () => ({
              startScheduledMatch: async (request: CompetitionRoomStartScheduledMatchRequest) => {
                startRequests.push(request)
                return rpcResults.shift()
              },
              provisionInitialScheduledGame: async (request: CompetitionRoomProvisionInitialScheduledGameRequest) => {
                initialGameRequests.push(request)
                return rpcResults.shift()
              },
              execute: async () => { throw new Error('generic execute must not be called') },
              provisionScheduledGame: async () => { throw new Error('later-game RPC must not be called') },
            }),
          },
        },
      },
    }
  }))
  const router = createRouter()
  router.post('/api/competitions/:id/matches/:matchId/start', startPost)
  app.use(router.handler)
  return toWebHandler(app)
}

async function post(body: unknown, sessionUserId?: string, path = '/api/competitions/path-competition/matches/path-match/start') {
  const headers = new Headers({ 'content-type': 'application/json' })
  if (sessionUserId) headers.set('x-test-session', sessionUserId)
  return makeWebHandler()(new Request(`http://fewfc.test${path}`, {
    method: 'POST',
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
  }))
}

afterEach(() => {
  mock.restore()
})

test('authenticated H3 route executes the two dedicated RPCs with one trusted time and stable child ids', async () => {
  resetState([
    startedResult({
      tournamentProjection: { ok: true, operation: 'tournamentStartMatch', public: { publicOnly: true } },
    }),
    provisionResult({
      private: { secret: 'second-private' }, serverState: { secret: 'second-server-state' },
      extra: 'extra-secret',
    }),
    startedResult({ idempotent: true }),
    provisionResult({ idempotent: true, status: 'provisioned' }),
  ])
  const realNow = Date.now
  let nowCalls = 0
  Date.now = () => { nowCalls += 1; return roundBindingEpochMs }
  try {
    const first = await post({
      stageId: 'stage-1', expectedRevision: 5, commandId: 'root-command',
      referees: [{ refereeId: 'referee-1', conflictedTeam: null }],
    }, 'session-user')
    const firstValue = await first.json()
    const retry = await post({ stageId: 'stage-1', expectedRevision: 5, commandId: 'root-command' }, 'session-user')
    const retryValue = await retry.json()

    expect(first.status).toBe(200)
    expect(firstValue).toEqual({
      competitionId: 'path-competition', matchId: 'path-match', competitionRevision: 6,
      matchRevision: 2, gameNumber: 1, gameId: 'generated-game-id', commandId: 'root-command', status: 'pending',
    })
    expect(retry.status).toBe(200)
    expect(retryValue).toMatchObject({ commandId: 'root-command', status: 'provisioned' })
    expect(JSON.stringify(firstValue)).not.toMatch(/binding|private|serverState|secret|players|deck|turnOrder|initial-game|start-schedule|roundRef|roundStartMs|round-generation/)
    expect(sessionCalls).toEqual(['session-user', 'session-user'])
    expect(roomNames).toEqual(['path-competition', 'path-competition', 'path-competition', 'path-competition'])
    expect(startRequests).toHaveLength(2)
    expect(initialGameRequests).toHaveLength(2)
    expect(startRequests[0]).toEqual({
      competitionId: 'path-competition', stageId: 'stage-1', matchId: 'path-match',
      callerUserId: 'session-user', commandId: 'root-command:start-schedule', expectedRevision: 5,
      trustedTimeMs: roundBindingEpochMs, referees: [{ refereeId: 'referee-1', conflictedTeam: null }],
    })
    expect(initialGameRequests[0]).toEqual({
      competitionId: 'path-competition', matchId: 'path-match', callerUserId: 'session-user',
      commandId: 'root-command:initial-game', expectedMatchRevision: 1, trustedTimeMs: roundBindingEpochMs,
    })
    expect(startRequests[0]?.trustedTimeMs).toBe(initialGameRequests[0]?.trustedTimeMs)
    expect(startRequests[0]?.commandId).toBe(startRequests[1]?.commandId)
    expect(initialGameRequests[0]?.commandId).toBe(initialGameRequests[1]?.commandId)
    expect(nowCalls).toBe(2)
  } finally {
    Date.now = realNow
  }
})

test('H3 route trims URL-encoded path identities before selecting the room or building either RPC', async () => {
  resetState()
  const response = await post(
    { stageId: 'stage-1', expectedRevision: 5, commandId: 'root-command' },
    'session-user',
    '/api/competitions/%20path-competition%20/matches/%20path-match%20/start',
  )
  expect(response.status).toBe(200)
  expect(await response.json()).toMatchObject({
    competitionId: 'path-competition', matchId: 'path-match', commandId: 'root-command',
  })
  expect(roomNames).toEqual(['path-competition', 'path-competition'])
  expect(startRequests[0]?.competitionId).toBe('path-competition')
  expect(startRequests[0]?.matchId).toBe('path-match')
  expect(initialGameRequests[0]?.competitionId).toBe('path-competition')
  expect(initialGameRequests[0]?.matchId).toBe('path-match')
})

test('unauthenticated, empty-path, missing command id, and spoofed bodies never dispatch', async () => {
  resetState()
  const unauthenticated = await post({ stageId: 'stage-1', expectedRevision: 1, commandId: 'root' })
  expect(unauthenticated.status).toBe(401)
  expect(startRequests).toHaveLength(0)

  const emptyCompetitionPath = await post({ stageId: 'stage-1', expectedRevision: 1, commandId: 'root' }, 'session-user',
    '/api/competitions/%20/matches/path-match/start')
  expect(emptyCompetitionPath.status).toBe(400)
  const emptyMatchPath = await post({ stageId: 'stage-1', expectedRevision: 1, commandId: 'root' }, 'session-user',
    '/api/competitions/path-competition/matches/%20/start')
  expect(emptyMatchPath.status).toBe(400)
  expect(startRequests).toHaveLength(0)

  const invalidBodies = [
    { stageId: 'stage-1', expectedRevision: 1 },
    { stageId: 'stage-1', expectedRevision: 1, commandId: '' },
    { stageId: 'stage-1', expectedRevision: 1, commandId: 'root', callerUserId: 'attacker' },
    { stageId: 'stage-1', expectedRevision: 1, commandId: 'root', trustedTimeMs: 1 },
    { stageId: 'stage-1', expectedRevision: 1, commandId: 'root', firstPlayer: 'team-a' },
    { stageId: 'stage-1', expectedRevision: 1, commandId: 'root', turnOrder: ['attacker'] },
    { stageId: 'stage-1', expectedRevision: 1, commandId: 'root', binding: {} },
    { stageId: 'stage-1', expectedRevision: 1, commandId: 'root', deckSeed: 'attacker' },
    { stageId: 'stage-1', expectedRevision: 1, commandId: 'root', expectedMatchRevision: 2 },
    { stageId: 'stage-1', expectedRevision: 1, commandId: 'root', referees: [{ refereeId: 'r', conflictedTeam: null, extra: true }] },
  ]
  for (const body of invalidBodies) {
    const response = await post(body, 'session-user')
    expect(response.status).toBe(400)
  }
  expect(startRequests).toHaveLength(0)
  expect(initialGameRequests).toHaveLength(0)
})

test('first-step errors or malformed success stop before initial-game RPC', async () => {
  resetState([{ ok: false, error: { code: 'matchNotFound', message: 'secret room detail' } }, provisionResult()])
  const roomFailure = await post({ stageId: 'stage-1', expectedRevision: 5, commandId: 'root-command' }, 'session-user')
  const roomFailureText = await roomFailure.text()
  expect(roomFailure.status).toBe(404)
  expect(roomFailureText).not.toContain('secret room detail')
  expect(startRequests).toHaveLength(1)
  expect(initialGameRequests).toHaveLength(0)

  resetState([startedResult({ matchRevision: 2 }), provisionResult()])
  const malformed = await post({ stageId: 'stage-1', expectedRevision: 5, commandId: 'root-command' }, 'session-user')
  expect(malformed.status).toBe(502)
  expect(initialGameRequests).toHaveLength(0)

  resetState([{ ...startedResult(), internalSecret: 'must-not-be-ignored' }, provisionResult()])
  const malformedEnvelope = await post({ stageId: 'stage-1', expectedRevision: 5, commandId: 'root-command' }, 'session-user')
  const envelopeText = await malformedEnvelope.text()
  expect(malformedEnvelope.status).toBe(502)
  expect(envelopeText).not.toContain('must-not-be-ignored')
  expect(initialGameRequests).toHaveLength(0)
})

test('second-step room failure is safely mapped and does not expose room details', async () => {
  resetState([startedResult(), { ok: false, error: { code: 'randomSourceFailure', message: 'private entropy detail' } }])
  const response = await post({ stageId: 'stage-1', expectedRevision: 5, commandId: 'root-command' }, 'session-user')
  const text = await response.text()
  expect(response.status).toBe(502)
  expect(text).not.toContain('private entropy detail')
  expect(text).toContain('賽事服務暫時無法處理要求。')
  expect(startRequests).toHaveLength(1)
  expect(initialGameRequests).toHaveLength(1)
})

test('malformed second-step success is not returned to the client', async () => {
  expect(() => validateCompetitionGameBinding(binding)).not.toThrow()
  resetState([startedResult(), provisionResult({
    binding: { ...binding, turnOrder: ['secret-player-a', 'secret-player-a'] },
  })])
  const response = await post({ stageId: 'stage-1', expectedRevision: 5, commandId: 'root-command' }, 'session-user')
  const text = await response.text()
  expect(response.status).toBe(502)
  expect(text).not.toContain('secret-deck-seed')
  expect(text).toContain('賽事服務暫時無法處理要求。')
})

test('round-release H3 adapter rejects missing required17 and conflicting round data with safe protocol errors', async () => {
  const { roundRef, roundStartMs, ...missingBoth } = binding
  const malformed = [
    missingBoth, { ...missingBoth, roundRef }, { ...missingBoth, roundStartMs },
    { ...binding, round_start_ms: roundStartMs },
    { ...binding, bindingDigest: 'secret-not-a-public-field' },
    { ...binding, roundRef: { ...roundRef, competitionId: 'other-competition' } },
    { ...binding, roundRef: { ...roundRef, private: 'secret-round-data' } },
    { ...binding, roundStartMs: 299_999 },
  ]
  for (const malformedBinding of malformed) {
    resetState([startedResult(), provisionResult({ binding: malformedBinding })])
    const response = await post({ stageId: 'stage-1', expectedRevision: 5, commandId: 'root-command' }, 'session-user')
    const text = await response.text()
    expect(response.status).toBe(502)
    expect(text).toContain('engineProtocolError')
    expect(text).toContain('賽事服務暫時無法處理要求。')
    expect(text).not.toMatch(/secret|roundRef|roundStartMs|bindingDigest|other-competition/)
    expect(startRequests).toHaveLength(1)
    expect(initialGameRequests).toHaveLength(1)
  }
})

test('round-release H3 adapter rejects raw nested getters and symbols without evaluating or exposing them', async () => {
  let getterCalls = 0
  const malformed = [
    { ...binding, roundRef: Object.defineProperty({ ...binding.roundRef }, 'competitionId', {
      enumerable: true, get() { getterCalls += 1; return 'secret' },
    }) },
    Object.assign({ ...binding }, { [Symbol.dispose]: () => {} }),
    { ...binding, turnOrder: Object.assign([...binding.turnOrder], { [Symbol('unknown')]: 'secret' }) },
  ]
  for (const malformedBinding of malformed) {
    resetState([startedResult(), provisionResult({ binding: malformedBinding })])
    const response = await post({ stageId: 'stage-1', expectedRevision: 5, commandId: 'root-command' }, 'session-user')
    const text = await response.text()
    expect(response.status).toBe(502)
    expect(text).toContain('engineProtocolError')
    expect(text).not.toMatch(/secret|roundRef|binding|dispose/)
    expect(startRequests).toHaveLength(1)
    expect(initialGameRequests).toHaveLength(1)
  }
  expect(getterCalls).toBe(0)
})

test('round-release H3 adapter preserves root-only receiver-bound disposer cleanup for complete bindings', async () => {
  const started = startedResult()
  const provisioned = provisionResult()
  let startedDisposals = 0
  let provisionedDisposals = 0
  const startedResource = Object.assign(started, {
    [Symbol.dispose]: function (this: unknown) {
      expect(this === startedResource).toBe(true)
      startedDisposals += 1
    },
  })
  const provisionedResource = Object.assign(provisioned, {
    [Symbol.dispose]: function (this: unknown) {
      expect(this === provisionedResource).toBe(true)
      provisionedDisposals += 1
    },
  })
  resetState([startedResource, provisionedResource])
  const response = await post({ stageId: 'stage-1', expectedRevision: 5, commandId: 'root-command' }, 'session-user')
  expect(response.status).toBe(200)
  const body: unknown = await response.json()
  expect(body).toEqual({
    competitionId: 'path-competition', matchId: 'path-match', competitionRevision: 6,
    matchRevision: 2, gameNumber: 1, gameId: 'generated-game-id', commandId: 'root-command', status: 'pending',
  })
  expect(startedDisposals).toBe(1)
  expect(provisionedDisposals).toBe(1)

  let rejectedDisposals = 0
  const rejectedResource = Object.assign(provisionResult({
    binding: { ...binding, roundRef: Object.assign({ ...binding.roundRef }, { [Symbol.dispose]: () => {} }) },
  }), {
    [Symbol.dispose]: function (this: unknown) {
      expect(this === rejectedResource).toBe(true)
      rejectedDisposals += 1
    },
  })
  resetState([startedResource, rejectedResource])
  const rejected = await post({ stageId: 'stage-1', expectedRevision: 5, commandId: 'root-command' }, 'session-user')
  expect(rejected.status).toBe(502)
  expect(await rejected.text()).not.toMatch(/secret|roundRef|binding|dispose/)
  expect(startedDisposals).toBe(2)
  expect(provisionedDisposals).toBe(1)
  expect(rejectedDisposals).toBe(1)
})
