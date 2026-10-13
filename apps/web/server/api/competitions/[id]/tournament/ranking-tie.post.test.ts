import { createError, createApp, createRouter, eventHandler, toWebHandler } from 'h3'
import { afterEach, expect, mock, test } from 'bun:test'
import type { CompetitionRoomExecuteRequest } from '../../../../../shared/competition-room'

let requests: CompetitionRoomExecuteRequest[] = []
let rpcResult: unknown

mock.module('../../../../utils/auth', () => ({
  requireSession: async (event: { headers: Headers }) => {
    const userId = event.headers.get('x-test-session')
    if (!userId) throw createError({ statusCode: 401, statusMessage: 'Authentication required.' })
    return { user: { id: userId } }
  },
}))

const { default: rankingTiePost } = await import('./ranking-tie.post')

function resetState(result?: unknown) {
  requests = []
  rpcResult = result
}

function makeWebHandler() {
  const app = createApp()
  app.use(eventHandler((event) => {
    ;(event.context as { _platform?: unknown })._platform = {
      cloudflare: {
        env: {
          APP_ENV: 'development',
          COMPETITION_ROOM: {
            idFromName: (name: string) => name,
            get: () => ({
              execute: async (request: CompetitionRoomExecuteRequest) => {
                requests.push(request)
                return rpcResult ?? {
                  ok: true,
                  value: {
                    competitionId: request.competitionId,
                    revision: request.expectedRevision + 1,
                    commandId: request.commandId,
                    idempotent: false,
                    projection: {
                      ok: true,
                      operation: 'tournamentAdjudicateRankingTie',
                      public: { decided: true },
                    },
                  },
                }
              },
            }),
          },
        },
      },
    }
  }))
  const router = createRouter()
  router.post('/api/competitions/:id/tournament/ranking-tie', rankingTiePost)
  app.use(router.handler)
  return toWebHandler(app)
}

async function post(body: unknown, sessionUserId?: string, path = '/api/competitions/event-1/tournament/ranking-tie') {
  const headers = new Headers({ 'content-type': 'application/json' })
  if (sessionUserId) headers.set('x-test-session', sessionUserId)
  return makeWebHandler()(new Request(`http://fewfc.test${path}`, {
    method: 'POST', headers, body: body === undefined ? undefined : JSON.stringify(body),
  }))
}

afterEach(() => mock.restore())

test('authenticated route derives caller and trusted time, then sends only the exact tie decision', async () => {
  resetState()
  const before = Date.now()
  const response = await post({
    commandId: 'decision-1', expectedRevision: 7, stageId: 'stage-1',
    orderedTeamIds: ['team-b', 'team-a'], reason: '公開排序理由',
  }, 'signed-in-user')
  const after = Date.now()
  expect(response.status).toBe(200)
  expect(requests).toHaveLength(1)
  expect(requests[0]).toMatchObject({
    competitionId: 'event-1', callerUserId: 'signed-in-user', commandId: 'decision-1',
    expectedRevision: 7,
    operation: {
      type: 'tournamentAdjudicateRankingTie', stageId: 'stage-1',
      orderedTeamIds: ['team-b', 'team-a'], reason: '公開排序理由',
    },
  })
  expect(requests[0]?.trustedTimeMs).toBeGreaterThanOrEqual(before)
  expect(requests[0]?.trustedTimeMs).toBeLessThanOrEqual(after)
  const result = await response.json() as Record<string, unknown>
  expect(result).toMatchObject({
    revision: 8, commandId: 'decision-1',
    projection: { ok: true, operation: 'tournamentAdjudicateRankingTie', public: { decided: true } },
  })
  expect(JSON.stringify(requests[0])).not.toMatch(/callerUserId.*attacker|private|serverState/)
})

test('unauthenticated, stale, and malformed decisions do not reach the room', async () => {
  resetState()
  const valid = {
    commandId: 'decision-1', expectedRevision: 7, stageId: 'stage-1',
    orderedTeamIds: ['team-a', 'team-b'], reason: '公開理由',
  }
  expect((await post(valid)).status).toBe(401)
  for (const body of [
    null,
    { ...valid, trustedTimeMs: 1 },
    { ...valid, callerUserId: 'spoofed-user' },
    { ...valid, operation: { type: 'tournamentAdjudicateRankingTie' } },
    { ...valid, orderedTeamIds: ['team-a', 'team-a'] },
    { ...valid, reason: ' ' },
  ]) expect((await post(body, 'signed-in-user')).status).toBe(400)
  expect(requests).toHaveLength(0)

  resetState({ ok: false, error: { code: 'revisionConflict', message: 'private state detail' } })
  const stale = await post(valid, 'signed-in-user')
  expect(stale.status).toBe(409)
  expect(await stale.text()).not.toContain('private state detail')
  expect(requests).toHaveLength(1)
})
