import { createError, createApp, createRouter, eventHandler, toWebHandler } from 'h3'
import { afterEach, expect, mock, test } from 'bun:test'
import type {
  CompetitionRoomAssignNextFirstPlayerByCaptainRequest,
  CompetitionRoomProvisionCaptainAssignedScheduledGameRequest,
} from '../../../../../../shared/competition-room'

let sessionCalls: string[] = []
let roomNames: string[] = []
let calls: string[] = []
let assignmentRequests: CompetitionRoomAssignNextFirstPlayerByCaptainRequest[] = []
let provisionRequests: CompetitionRoomProvisionCaptainAssignedScheduledGameRequest[] = []
let assignmentResults: unknown[] = []
let provisionResults: unknown[] = []
let forbiddenRpcCalls = 0

mock.module('../../../../../utils/auth', () => ({
  requireSession: async (event: { headers: Headers }) => {
    const userId = event.headers.get('x-test-session')
    sessionCalls.push(userId ?? '')
    if (!userId) throw createError({ statusCode: 401, statusMessage: 'Authentication required.' })
    return { user: { id: userId } }
  },
}))

const { default: nextGamePost } = await import('./next-game.post')

function assignmentValue(overrides: Record<string, unknown> = {}) {
  return {
    competitionId: 'competition-1', matchId: 'match-1', matchRevision: 8,
    commandId: 'root:assign-first-player', idempotent: false,
    matchProjection: {
      ok: true, operation: 'matchAssignFirstPlayer',
      public: { match: {
        config: {
          matchId: 'match-1', teams: ['team-a', 'team-b'],
          format: { type: 'fixedGames', value: 3 },
          firstGame: { type: 'assigned', team: 'team-a' }, referees: [],
        },
        status: 'open', score: [0, 0],
        games: [{
          gameNumber: 1, gameRecordId: 'game-record-1', startKind: { type: 'ordinaryNewGame' },
          firstPlayer: 'team-b',
          terminalEvidence: {
            type: 'canonicalGameEnded',
            conclusion: { outcome: { type: 'draw' }, causes: [] },
          },
          rulingRecordId: 'ruling-1',
        }],
        rulings: [{
          rulingRecordId: 'ruling-1', gameRecordId: 'game-record-1',
          ruling: { type: 'gameAccepted', outcome: 'draw' },
        }],
        corrections: [], outcome: null, nextFirstPlayer: 'team-a',
        nextGame: {
          gameNumber: 2, kind: { type: 'ordinaryNewGame' },
          firstPlayer: { type: 'assigned', team: 'team-a' },
        },
      } },
    },
    ...overrides,
  }
}

function assignmentEnvelope(overrides: Record<string, unknown> = {}) {
  return { ok: true, value: { ...assignmentValue(), ...overrides } }
}

function provisionEnvelope(overrides: Record<string, unknown> = {}) {
  return {
    ok: true,
    value: {
      competitionId: 'competition-1', matchId: 'match-1', matchRevision: 8,
      gameNumber: 2, gameId: 'game-2', commandId: 'root:captain-game',
      idempotent: false, status: 'pending', ...overrides,
    },
  }
}

function resetState(assignments: unknown[] = [assignmentEnvelope()], provisions: unknown[] = [provisionEnvelope()]) {
  sessionCalls = []
  roomNames = []
  calls = []
  assignmentRequests = []
  provisionRequests = []
  assignmentResults = [...assignments]
  provisionResults = [...provisions]
  forbiddenRpcCalls = 0
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
              assignNextFirstPlayerByCaptain: async (request: CompetitionRoomAssignNextFirstPlayerByCaptainRequest) => {
                calls.push('assignNextFirstPlayerByCaptain')
                assignmentRequests.push(request)
                return assignmentResults.shift()
              },
              provisionCaptainAssignedScheduledGame: async (request: CompetitionRoomProvisionCaptainAssignedScheduledGameRequest) => {
                calls.push('provisionCaptainAssignedScheduledGame')
                provisionRequests.push(request)
                return provisionResults.shift()
              },
              execute: async () => { forbiddenRpcCalls += 1; throw new Error('generic execute forbidden') },
              executeMatch: async () => { forbiddenRpcCalls += 1; throw new Error('generic match execute forbidden') },
              assignFirstPlayer: async () => { forbiddenRpcCalls += 1; throw new Error('organizer RPC forbidden') },
              assignNextFirstPlayerByOrganizer: async () => { forbiddenRpcCalls += 1; throw new Error('organizer RPC forbidden') },
              provisionInitialScheduledGame: async () => { forbiddenRpcCalls += 1; throw new Error('initial RPC forbidden') },
            }),
          },
        },
      },
    }
  }))
  const router = createRouter()
  router.post('/api/competitions/:id/matches/:matchId/next-game', nextGamePost)
  app.use(router.handler)
  return toWebHandler(app)
}

async function post(
  body: unknown,
  sessionUserId?: string,
  path = '/api/competitions/competition-1/matches/match-1/next-game',
) {
  const headers = new Headers({ 'content-type': 'application/json' })
  if (sessionUserId) headers.set('x-test-session', sessionUserId)
  return makeWebHandler()(new Request(`http://fewfc.test${path}`, {
    method: 'POST', headers, body: body === undefined ? undefined : JSON.stringify(body),
  }))
}

const validBody = { selectedFirstPlayerTeam: 'team-a', expectedMatchRevision: 7, commandId: 'root' }

afterEach(() => {
  mock.restore()
})

test('authenticated H3 route runs dedicated assignment then provision with normalized identity and one shared server time', async () => {
  resetState([
    assignmentEnvelope(),
    assignmentEnvelope({ idempotent: true }),
  ], [
    provisionEnvelope(),
    provisionEnvelope({ idempotent: true, status: 'provisioned' }),
  ])
  const realNow = Date.now
  let nowCalls = 0
  Date.now = () => { nowCalls += 1; return 987654 }
  try {
    const first = await post({ ...validBody, selectedFirstPlayerTeam: ' team-a ', commandId: ' root ' }, 'captain-user',
      '/api/competitions/%20competition-1%20/matches/%20match-1%20/next-game')
    const firstValue = await first.json()
    expect(first.status).toBe(200)
    expect(firstValue).toEqual({
      competitionId: 'competition-1', matchId: 'match-1', matchRevision: 8,
      gameNumber: 2, gameId: 'game-2', commandId: 'root', status: 'pending',
    })
    expect(JSON.stringify(firstValue)).not.toMatch(/assign-first-player|captain-game|binding|private|secret|serverState|roster|deck|captain/)

    const retry = await post(validBody, 'captain-user')
    expect(retry.status).toBe(200)
    expect(await retry.json()).toMatchObject({ commandId: 'root', status: 'provisioned' })
    expect(sessionCalls).toEqual(['captain-user', 'captain-user'])
    expect(roomNames).toEqual(['competition-1', 'competition-1', 'competition-1', 'competition-1'])
    expect(calls).toEqual([
      'assignNextFirstPlayerByCaptain', 'provisionCaptainAssignedScheduledGame',
      'assignNextFirstPlayerByCaptain', 'provisionCaptainAssignedScheduledGame',
    ])
    expect(assignmentRequests[0]).toEqual({
      competitionId: 'competition-1', matchId: 'match-1', callerUserId: 'captain-user',
      commandId: 'root:assign-first-player', expectedMatchRevision: 7, trustedTimeMs: 987654,
      selectedFirstPlayerTeam: 'team-a',
    })
    expect(provisionRequests[0]).toEqual({
      competitionId: 'competition-1', matchId: 'match-1', callerUserId: 'captain-user',
      commandId: 'root:captain-game', assignmentCommandId: 'root:assign-first-player',
      expectedMatchRevision: 8, trustedTimeMs: 987654,
    })
    expect(assignmentRequests[0]?.trustedTimeMs).toBe(provisionRequests[0]?.trustedTimeMs)
    expect(assignmentRequests[0]?.commandId).toBe(assignmentRequests[1]?.commandId)
    expect(provisionRequests[0]?.commandId).toBe(provisionRequests[1]?.commandId)
    expect(nowCalls).toBe(2)
    expect(forbiddenRpcCalls).toBe(0)
  } finally {
    Date.now = realNow
  }
})

test('second-step failure can retry with the same root and command ids without repeating a different policy', async () => {
  resetState([
    assignmentEnvelope(),
    assignmentEnvelope({ idempotent: true }),
  ], [
    { ok: false, error: { code: 'gameRoomNotProvisioned', message: 'private delivery detail' } },
    provisionEnvelope({ idempotent: true }),
  ])
  const realNow = Date.now
  let nowCalls = 0
  Date.now = () => { nowCalls += 1; return 1000 + nowCalls }
  try {
    const failed = await post(validBody, 'captain-user')
    const failedBody = await failed.json()
    expect(failed.status).toBe(409)
    expect(failedBody).toMatchObject({ data: { code: 'gameRoomNotProvisioned' } })
    expect(JSON.stringify(failedBody)).not.toContain('private delivery detail')
    expect(calls).toEqual(['assignNextFirstPlayerByCaptain', 'provisionCaptainAssignedScheduledGame'])

    const retried = await post(validBody, 'captain-user')
    expect(retried.status).toBe(200)
    expect(calls.slice(2)).toEqual(['assignNextFirstPlayerByCaptain', 'provisionCaptainAssignedScheduledGame'])
    expect(assignmentRequests[0]?.commandId).toBe(assignmentRequests[1]?.commandId)
    expect(provisionRequests[0]?.commandId).toBe(provisionRequests[1]?.commandId)
    expect(assignmentRequests[0]?.expectedMatchRevision).toBe(assignmentRequests[1]?.expectedMatchRevision)
    expect(provisionRequests[0]?.expectedMatchRevision).toBe(provisionRequests[1]?.expectedMatchRevision)
    expect(assignmentRequests[0]?.trustedTimeMs).toBe(provisionRequests[0]?.trustedTimeMs)
    expect(assignmentRequests[1]?.trustedTimeMs).toBe(provisionRequests[1]?.trustedTimeMs)
    expect(nowCalls).toBe(2)
  } finally {
    Date.now = realNow
  }
})

test('second-step trusted-time regression exposes its exact H3 code and can retry the same provision command', async () => {
  resetState([
    assignmentEnvelope(),
    assignmentEnvelope({ idempotent: true }),
  ], [
    { ok: false, error: { code: 'trustedTimeRegression', message: 'private time detail' } },
    provisionEnvelope({ idempotent: true, status: 'provisioned' }),
  ])
  const realNow = Date.now
  let nowCalls = 0
  Date.now = () => { nowCalls += 1; return 1000 + nowCalls }
  try {
    const failed = await post(validBody, 'captain-user')
    expect(failed.status).toBe(409)
    const failedBody = await failed.json()
    expect(failedBody).toMatchObject({ data: { code: 'trustedTimeRegression' } })
    expect(JSON.stringify(failedBody)).not.toContain('private time detail')

    const retried = await post(validBody, 'captain-user')
    expect(retried.status).toBe(200)
    expect(await retried.json()).toMatchObject({ commandId: 'root', status: 'provisioned' })
    expect(assignmentRequests[0]?.commandId).toBe(assignmentRequests[1]?.commandId)
    expect(provisionRequests[0]?.commandId).toBe(provisionRequests[1]?.commandId)
    expect(assignmentRequests[0]?.expectedMatchRevision).toBe(assignmentRequests[1]?.expectedMatchRevision)
    expect(provisionRequests[0]?.expectedMatchRevision).toBe(provisionRequests[1]?.expectedMatchRevision)
    expect(assignmentRequests[0]?.trustedTimeMs).toBe(provisionRequests[0]?.trustedTimeMs)
    expect(assignmentRequests[1]?.trustedTimeMs).toBe(provisionRequests[1]?.trustedTimeMs)
    expect(nowCalls).toBe(2)
  } finally {
    Date.now = realNow
  }
})

test('unauthenticated, empty normalized path and any missing, extra, or spoofed field do not dispatch', async () => {
  resetState()
  expect((await post(validBody)).status).toBe(401)
  expect((await post(validBody, 'captain-user', '/api/competitions/%20/matches/match-1/next-game')).status).toBe(400)
  expect((await post(validBody, 'captain-user', '/api/competitions/competition-1/matches/%20/next-game')).status).toBe(400)
  const invalidBodies = [
    null,
    { selectedFirstPlayerTeam: 'team-a', expectedMatchRevision: 7 },
    { selectedFirstPlayerTeam: 'team-a', expectedMatchRevision: 7, commandId: '' },
    { ...validBody, selectedFirstPlayerTeam: ' ' },
    { ...validBody, selectedFirstPlayerTeam: 7 },
    { ...validBody, expectedMatchRevision: 0 },
    { ...validBody, expectedMatchRevision: 1.5 },
    { ...validBody, expectedMatchRevision: Number.MAX_SAFE_INTEGER },
    ...['callerUserId', 'competitionId', 'matchId', 'trustedTimeMs', 'assignmentCommandId', 'firstPlayer', 'turnOrder', 'binding', 'deckSeed', 'gameId', 'serverState']
      .map(key => ({ ...validBody, [key]: 'spoof' })),
  ]
  for (const body of invalidBodies) expect((await post(body, 'captain-user')).status).toBe(400)
  expect(sessionCalls).toHaveLength(1 + 1 + 1 + invalidBodies.length)
  expect(roomNames).toHaveLength(0)
  expect(calls).toHaveLength(0)
  expect(forbiddenRpcCalls).toBe(0)
})

test('assignment room errors stop before provisioning and policy codes map to safe conflicts', async () => {
  for (const [code, status] of [
    ['assignmentNotCurrent', 409], ['firstPlayerPolicyUnresolved', 409], ['eliminationDrawLossUnresolved', 409],
  ] as const) {
    resetState([{ ok: false, error: { code, message: 'private policy explanation' } }], [provisionEnvelope()])
    const response = await post(validBody, 'captain-user')
    const text = await response.text()
    expect(response.status).toBe(status)
    expect(text).toContain(code)
    expect(text).not.toContain('private policy explanation')
    expect(calls).toEqual(['assignNextFirstPlayerByCaptain'])
    expect(provisionRequests).toHaveLength(0)
  }
})

test('malformed assignment envelopes, identity, revisions, projection, and prohibited public data fail closed', async () => {
  const base = assignmentEnvelope()
  const value = base.value as Record<string, unknown>
  const projection = value.matchProjection as Record<string, unknown>
  const publicValue = projection.public as { match: Record<string, unknown> }
  const assignedNextGame = publicValue.match.nextGame as Record<string, unknown>
  const malformed = [
    { ...base, private: { secret: true } },
    { ok: true, value, extra: true },
    { ok: true, value: { ...value, extra: true } },
    { ok: true, value: { ...value, competitionId: 'wrong' } },
    { ok: true, value: { ...value, matchId: 'wrong' } },
    { ok: true, value: { ...value, matchRevision: 9 } },
    { ok: true, value: { ...value, commandId: 'wrong' } },
    { ok: true, value: { ...value, matchProjection: { ...projection, operation: 'matchView' } } },
    { ok: true, value: { ...value, matchProjection: { ...projection, serverState: {} } } },
    {
      ok: true,
      value: {
        ...value,
        matchProjection: {
          ...projection,
          public: { match: { ...publicValue.match, extra: 'not-allowed' } },
        },
      },
    },
    ...['roster', 'deck', 'captain', 'members', 'private', 'serverState', 'authorizedPrivate', 'announcement', 'decks', 'rosterVersion', 'lockedRoster'].map(key => ({
      ok: true,
      value: { ...value, matchProjection: { ...projection, public: { match: { ...publicValue.match, [key]: 'secret' } } } },
    })),
    { ok: true, value: { ...value, matchProjection: { ...projection, public: { match: { ...publicValue.match, status: 'completed' } } } } },
    { ok: true, value: { ...value, matchProjection: { ...projection, public: { match: { ...publicValue.match, outcome: { winner: 'team-a' } } } } } },
    {
      ok: true,
      value: {
        ...value,
        matchProjection: {
          ...projection,
          public: { match: { ...publicValue.match, nextGame: {
            ...assignedNextGame, kind: { type: 'eliminationContinuation' },
          } } },
        },
      },
    },
    {
      ok: true,
      value: {
        ...value,
        matchProjection: {
          ...projection,
          public: { match: { ...publicValue.match, nextGame: {
            gameNumber: 2, kind: { type: 'ordinaryNewGame' }, firstPlayer: { type: 'pending' },
          } } },
        },
      },
    },
  ]
  for (const result of malformed) {
    resetState([result], [provisionEnvelope()])
    const response = await post(validBody, 'captain-user')
    expect(response.status).toBe(502)
    expect(await response.text()).not.toMatch(/secret|wrong|eliminationContinuation/)
    expect(calls).toEqual(['assignNextFirstPlayerByCaptain'])
  }
})

test('malformed provision envelopes and values are not returned to the client', async () => {
  const malformed = [
    { ok: true, value: {}, private: { secret: true } },
    { ok: true, value: {} },
    provisionEnvelope({ extra: true }),
    provisionEnvelope({ binding: {} }),
    provisionEnvelope({ serverState: { secret: true } }),
    provisionEnvelope({ competitionId: 'wrong' }),
    provisionEnvelope({ matchId: 'wrong' }),
    provisionEnvelope({ matchRevision: 9 }),
    provisionEnvelope({ gameNumber: 1 }),
    provisionEnvelope({ gameNumber: 3 }),
    provisionEnvelope({ gameId: '' }),
    provisionEnvelope({ commandId: 'wrong' }),
    provisionEnvelope({ idempotent: 'false' }),
    provisionEnvelope({ status: 'unknown' }),
  ]
  for (const result of malformed) {
    resetState([assignmentEnvelope()], [result])
    const response = await post(validBody, 'captain-user')
    expect(response.status).toBe(502)
    expect(await response.text()).not.toMatch(/secret|binding|serverState/)
    expect(calls).toEqual(['assignNextFirstPlayerByCaptain', 'provisionCaptainAssignedScheduledGame'])
  }
})
