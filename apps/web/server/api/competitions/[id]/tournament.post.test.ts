import { createError, createApp, createRouter, eventHandler, toWebHandler } from 'h3'
import { afterEach, expect, mock, test } from 'bun:test'
import type { CompetitionRoomCreateTournamentRequest } from '../../../../shared/competition-room'

let sessionCalls: string[] = []
let roomNames: string[] = []
let tournamentRequests: CompetitionRoomCreateTournamentRequest[] = []
let executeCalls = 0
let rpcResult: unknown

mock.module('../../../utils/auth', () => ({
  requireSession: async (event: { headers: Headers }) => {
    const sessionUserId = event.headers.get('x-test-session')
    sessionCalls.push(sessionUserId ?? '')
    if (!sessionUserId) {
      throw createError({ statusCode: 401, statusMessage: 'Authentication required.' })
    }
    return { user: { id: sessionUserId } }
  },
}))

const { default: tournamentPost } = await import('./tournament.post')

const stage = {
  stageId: 'stage-1',
  format: 'swiss',
  teams: [{ teamId: 'team-1', seed: 1 }, { teamId: 'team-2', seed: 2 }],
  randomSeed: 8,
  matchFormat: { type: 'fixedGames', value: 1 },
  clockConfig: { mainTimeMs: null, actionTimeMs: null, outOfTurnChoiceTimeMs: null },
  swissRounds: 3,
  swissPolicy: null,
}

function resetState(result?: unknown) {
  sessionCalls = []
  roomNames = []
  tournamentRequests = []
  executeCalls = 0
  rpcResult = result ?? {
    ok: true,
    value: {
      competitionId: 'path-competition',
      revision: 2,
      commandId: 'client-command',
      idempotent: false,
      projection: { ok: true, operation: 'tournamentCreate', public: { tournamentId: 'path-competition' } },
    },
  }
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
              createTournamentFromLockedRoster: async (request: CompetitionRoomCreateTournamentRequest) => {
                tournamentRequests.push(request)
                return rpcResult
              },
              execute: async () => {
                executeCalls += 1
                throw new Error('generic execute must not be called')
              },
            }),
          },
        },
      },
    }
  }))
  const router = createRouter()
  router.post('/api/competitions/:id/tournament', tournamentPost)
  app.use(router.handler)
  return toWebHandler(app)
}

async function post(body: unknown, sessionUserId?: string, path = '/api/competitions/path-competition/tournament') {
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

test('POST route derives session, path, trusted time, and command for the dedicated RPC and projects its response', async () => {
  resetState({
    ok: true,
    value: {
      competitionId: 'path-competition',
      revision: 2,
      commandId: 'client-command',
      idempotent: false,
      serverState: { secret: 'server-state-secret' },
      private: { secret: 'private-secret' },
      extra: 'top-level-secret',
      projection: {
        ok: true,
        operation: 'tournamentCreate',
        public: { tournamentId: 'path-competition' },
        serverState: { secret: 'projection-state-secret' },
        private: { secret: 'projection-private-secret' },
        extra: 'projection-extra-secret',
      },
    },
  })
  const before = Date.now()
  const response = await post({ stage, expectedRevision: 1, commandId: 'client-command' }, 'session-user')
  const after = Date.now()
  const result = await response.json() as Record<string, unknown>

  expect(response.status).toBe(200)
  expect(sessionCalls).toEqual(['session-user'])
  expect(roomNames).toEqual(['path-competition'])
  expect(tournamentRequests).toHaveLength(1)
  expect(tournamentRequests[0]).toMatchObject({
    competitionId: 'path-competition',
    callerUserId: 'session-user',
    commandId: 'client-command',
    expectedRevision: 1,
    stage,
  })
  expect(tournamentRequests[0]?.trustedTimeMs).toBeGreaterThanOrEqual(before)
  expect(tournamentRequests[0]?.trustedTimeMs).toBeLessThanOrEqual(after)
  expect(executeCalls).toBe(0)
  expect(result).toEqual({
    competitionId: 'path-competition',
    revision: 2,
    commandId: 'client-command',
    idempotent: false,
    projection: {
      ok: true,
      operation: 'tournamentCreate',
      public: { tournamentId: 'path-competition' },
    },
  })
  expect(JSON.stringify(result)).not.toMatch(/server-state-secret|private-secret|top-level-secret|projection-state-secret|projection-private-secret|projection-extra-secret/)
})

test('empty route id, invalid body, and unauthenticated request never dispatch the RPC', async () => {
  resetState()
  const emptyPathResponse = await post({ stage, expectedRevision: 1 }, 'session-user', '/api/competitions//tournament')
  expect(emptyPathResponse.status).toBeGreaterThanOrEqual(400)
  expect(tournamentRequests).toHaveLength(0)

  const emptyBodyResponse = await post(undefined, 'session-user')
  expect(emptyBodyResponse.status).toBe(400)
  expect(tournamentRequests).toHaveLength(0)

  const invalidBodyResponse = await post({ stage: {}, expectedRevision: 1 }, 'session-user')
  expect(invalidBodyResponse.status).toBe(400)
  expect(tournamentRequests).toHaveLength(0)

  const unauthenticatedResponse = await post({ stage, expectedRevision: 1 })
  expect(unauthenticatedResponse.status).toBe(401)
  expect(tournamentRequests).toHaveLength(0)
  expect(executeCalls).toBe(0)
})

test('malformed RPC success fails closed as a safe 502', async () => {
  resetState({
    ok: true,
    value: {
      competitionId: 'path-competition',
      revision: 'invalid',
      commandId: 'client-command',
      idempotent: false,
      projection: { ok: true, operation: 'tournamentCreate', public: {}, private: 'private-secret' },
      extra: 'secretInternalMessage',
    },
  })
  const response = await post({ stage, expectedRevision: 1 }, 'session-user')
  const responseText = await response.text()
  expect(response.status).toBe(502)
  expect(responseText).not.toContain('private-secret')
  expect(responseText).not.toContain('secretInternalMessage')
  expect(responseText).toContain('賽事服務暫時無法處理要求。')
  expect(executeCalls).toBe(0)
})
