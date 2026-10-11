import { createError, createApp, createRouter, eventHandler, toWebHandler } from 'h3'
import { afterEach, describe, expect, mock, spyOn, test } from 'bun:test'
import type { CompetitionRoomAnnounceRoundRequest } from '../../../../../../../../shared/competition-room'

let sessionCalls: string[] = []
let roomNames: string[] = []
let announceRequests: CompetitionRoomAnnounceRoundRequest[] = []
let readCalls = 0
let announceResults: unknown[] = []
let missingBinding = false
let throwFromAnnounce = false

mock.module('../../../../../../../utils/auth', () => ({
  requireSession: async (event: { headers: Headers }) => {
    const userId = event.headers.get('x-test-session')
    sessionCalls.push(userId ?? '')
    if (!userId) throw createError({ statusCode: 401, statusMessage: 'Authentication required.' })
    return { user: { id: userId } }
  },
}))

const { default: publicationPost } = await import('./publication.post')

const roundStartMs = 1_800_000_000_000
const initialTrustedTimeMs = roundStartMs - 300_000

function validBody(overrides: Record<string, unknown> = {}) {
  return {
    commandId: 'round-command-1',
    expectedRevision: 8,
    roundStartMs,
    refereeAssignments: [{
      matchId: 'match-1',
      referees: [{ refereeId: 'referee-1', conflictedTeam: null }],
    }],
    ...overrides,
  }
}

function publication(overrides: Record<string, unknown> = {}) {
  return {
    roundRef: {
      competitionId: 'competition-1', stageId: 'stage-1', roundNumber: 3,
      roundGenerationId: 'generation-1',
    },
    roundStartMs,
    publicationVersion: 1,
    status: 'materializing',
    releaseDecisionId: null,
    requiresReferee: false,
    rooms: [{ matchId: 'match-1', gameId: 'game-1', ackConfirmedAtMs: null }],
    ...overrides,
  }
}

function announceSuccess(request: CompetitionRoomAnnounceRoundRequest, overrides: Record<string, unknown> = {}) {
  return {
    ok: true,
    value: {
      competitionId: request.competitionId,
      revision: request.expectedRevision + 1,
      commandId: request.commandId,
      idempotent: false,
      publication: publication({ roundStartMs: request.roundStartMs }),
      ...overrides,
    },
  }
}

function resetState(results: unknown[] = []) {
  sessionCalls = []
  roomNames = []
  announceRequests = []
  readCalls = 0
  announceResults = [...results]
  missingBinding = false
  throwFromAnnounce = false
}

function makeWebHandler() {
  const app = createApp()
  app.use(eventHandler((event) => {
    const env: Record<string, unknown> = { APP_ENV: 'development' }
    if (!missingBinding) {
      env.COMPETITION_ROOM = {
        idFromName: (name: string) => {
          roomNames.push(name)
          return name
        },
        get: () => ({
          announceRound: async (request: CompetitionRoomAnnounceRoundRequest) => {
            announceRequests.push(request)
            if (throwFromAnnounce) throw new Error('private SQL and storage details')
            return announceResults.shift()
          },
          readRoundPublication: async () => {
            readCalls += 1
            throw new Error('read RPC must not be called by POST')
          },
        }),
      }
    }
    ;(event.context as { _platform?: unknown })._platform = { cloudflare: { env } }
  }))
  const router = createRouter()
  router.post('/api/competitions/:id/stages/:stageId/rounds/:roundNumber/publication', publicationPost)
  app.use(router.handler)
  return toWebHandler(app)
}

async function post(
  body: unknown,
  sessionUserId = 'session-user',
  path = '/api/competitions/competition-1/stages/stage-1/rounds/3/publication',
) {
  const headers = new Headers({ 'content-type': 'application/json' })
  if (sessionUserId) headers.set('x-test-session', sessionUserId)
  return makeWebHandler()(new Request(`http://fewfc.test${path}`, {
    method: 'POST', headers, body: JSON.stringify(body),
  }))
}

async function postRaw(
  body: string,
  path = '/api/competitions/competition-1/stages/stage-1/rounds/3/publication',
) {
  return makeWebHandler()(new Request(`http://fewfc.test${path}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', 'x-test-session': 'session-user' },
    body,
  }))
}

afterEach(() => {
  mock.restore()
})

describe('round-publication HTTP module POST', () => {
  test('round-publication HTTP module authenticates first and sends an exact nine-field server request', async () => {
    const time = spyOn(Date, 'now').mockReturnValue(initialTrustedTimeMs)
    const expectedRequest = {
      competitionId: 'competition-1', stageId: 'stage-1', roundNumber: 3,
      callerUserId: 'session-user', commandId: 'round-command-1', expectedRevision: 8,
      trustedTimeMs: initialTrustedTimeMs, roundStartMs,
      refereeAssignments: [{ matchId: 'match-1', referees: [{ refereeId: 'referee-1', conflictedTeam: null }] }],
    }
    resetState([announceSuccess(expectedRequest)])
    const response = await post(validBody(), 'session-user',
      '/api/competitions/%20competition-1%20/stages/%20stage-1%20/rounds/3/publication')
    const value = await response.json()
    expect(response.status).toBe(200)
    expect(value).toEqual({
      competitionId: 'competition-1', revision: 9, commandId: 'round-command-1', idempotent: false,
      publication: publication(),
    })
    expect(Object.keys(announceRequests[0]!)).toEqual([
      'competitionId', 'stageId', 'roundNumber', 'callerUserId', 'commandId',
      'expectedRevision', 'trustedTimeMs', 'roundStartMs', 'refereeAssignments',
    ])
    expect(announceRequests).toEqual([expectedRequest])
    expect(sessionCalls).toEqual(['session-user'])
    expect(roomNames).toEqual(['competition-1'])
    expect(readCalls).toBe(0)
    expect(time).toHaveBeenCalledTimes(1)
    expect(JSON.stringify(value)).not.toMatch(/private|manifest|binding|seed|serverState/)
  })

  test('round-publication HTTP module rejects unauthenticated and invalid path requests before dispatch', async () => {
    resetState()
    const noSession = await post(validBody(), '', '/api/competitions/%20/stages/stage-1/rounds/3/publication')
    expect(noSession.status).toBe(401)
    expect(sessionCalls).toEqual([''])
    expect(roomNames).toHaveLength(0)

    sessionCalls = []
    for (const value of ['0', '03', '-3', '3.0', '3e0', 'NaN', '4294967296']) {
      expect((await post(validBody(), 'session-user',
        `/api/competitions/competition-1/stages/stage-1/rounds/${encodeURIComponent(value)}/publication`)).status).toBe(400)
    }
    expect(roomNames).toHaveLength(0)
    expect(announceRequests).toHaveLength(0)
  })

  test('round-publication HTTP module rejects query strings and malformed JSON without sampling time', async () => {
    resetState()
    const time = spyOn(Date, 'now').mockReturnValue(initialTrustedTimeMs)
    for (const path of [
      '/api/competitions/competition-1/stages/stage-1/rounds/3/publication?callerUserId=attacker',
      '/api/competitions/competition-1/stages/stage-1/rounds/3/publication?',
    ]) {
      expect((await post(validBody(), 'session-user', path)).status).toBe(400)
    }
    const malformed = await postRaw('{')
    const text = await malformed.text()
    expect(malformed.status).toBe(400)
    expect(text).toContain('malformedJSON')
    expect(text).not.toMatch(/Unexpected|JSON.parse|position/)
    expect(time).not.toHaveBeenCalled()
    expect(announceRequests).toHaveLength(0)
  })

  test('round-publication HTTP module rejects spoofed identities, extra fields, and duplicate assignment ids', async () => {
    resetState()
    const time = spyOn(Date, 'now').mockReturnValue(initialTrustedTimeMs)
    const invalidBodies = [
      validBody({ callerUserId: 'attacker' }),
      validBody({ trustedTimeMs: 1 }),
      validBody({ serverActor: 'organizer' }),
      validBody({ competitionId: 'other-competition' }),
      validBody({ stageId: 'other-stage' }),
      validBody({ roundNumber: 4 }),
      validBody({ expected_revision: 8 }),
      validBody({ round_start_ms: roundStartMs }),
      validBody({ referee_assignments: [] }),
      validBody({ binding: { deckSeed: 'private-seed' } }),
      validBody({ manifest: { private: true } }),
      validBody({ deckSeed: 'private-seed' }),
      validBody({ refereeAssignments: [
        { matchId: 'match-1', referees: [] }, { matchId: 'match-1', referees: [] },
      ] }),
      validBody({ refereeAssignments: [{
        matchId: 'match-1', referees: [
          { refereeId: 'referee-1', conflictedTeam: null },
          { refereeId: 'referee-1', conflictedTeam: null },
        ],
      }] }),
      validBody({ commandId: ' round-command-1 ' }),
    ]
    for (const body of invalidBodies) {
      const response = await post(body)
      expect(response.status).toBe(400)
      expect(await response.text()).not.toMatch(/private-seed|attacker|manifest/)
    }
    expect(time).toHaveBeenCalledTimes(3)
    expect(announceRequests).toHaveLength(0)
  })

  test('round-publication HTTP module rejects sparse assignment arrays in parsed JSON', async () => {
    resetState()
    const time = spyOn(Date, 'now').mockReturnValue(initialTrustedTimeMs)
    const sparseAssignments = Array(1)
    const sparseBody = validBody({ refereeAssignments: sparseAssignments })
    const sparseResponse = await post(sparseBody)
    expect(sparseResponse.status).toBe(400)
    expect(time).toHaveBeenCalledTimes(1)
    expect(announceRequests).toHaveLength(0)
  })

  test('round-publication HTTP module preserves original command intent across idempotent retry', async () => {
    const firstRequest = {
      competitionId: 'competition-1', stageId: 'stage-1', roundNumber: 3,
      callerUserId: 'session-user', commandId: 'round-command-1', expectedRevision: 8,
      trustedTimeMs: initialTrustedTimeMs, roundStartMs,
      refereeAssignments: [{ matchId: 'match-1', referees: [{ refereeId: 'referee-1', conflictedTeam: null }] }],
    }
    const first = announceSuccess(firstRequest)
    const retry = announceSuccess(firstRequest, { idempotent: true })
    const retryTimes = [initialTrustedTimeMs, initialTrustedTimeMs + 1]
    const time = spyOn(Date, 'now').mockImplementation(() => retryTimes.shift() ?? initialTrustedTimeMs + 1)
    resetState([first, retry])
    const body = validBody()
    const initial = await post(body)
    const repeated = await post(body)
    expect(initial.status).toBe(200)
    expect(repeated.status).toBe(200)
    expect(await initial.json()).toMatchObject({ commandId: 'round-command-1', idempotent: false })
    expect(await repeated.json()).toMatchObject({ commandId: 'round-command-1', idempotent: true })
    expect(announceRequests).toHaveLength(2)
    expect(announceRequests[0]).toEqual(firstRequest)
    expect(announceRequests[1]).toEqual({ ...firstRequest, trustedTimeMs: initialTrustedTimeMs + 1 })
    expect(body).toEqual(validBody())
    expect(time).toHaveBeenCalledTimes(2)
  })

  test('round-publication HTTP module forwards late server time and never adjusts the announced start', async () => {
    resetState([{
      ok: false,
      error: { code: 'roundNotPublishable', message: 'private cutoff details' },
    }])
    const lateTrustedTimeMs = initialTrustedTimeMs + 1
    const time = spyOn(Date, 'now').mockReturnValue(lateTrustedTimeMs)
    const response = await post(validBody())
    const text = await response.text()
    expect(response.status).toBe(409)
    expect(text).not.toContain('private cutoff details')
    expect(announceRequests).toHaveLength(1)
    expect(announceRequests[0]?.trustedTimeMs).toBe(lateTrustedTimeMs)
    expect(announceRequests[0]?.roundStartMs).toBe(roundStartMs)
    expect(announceRequests[0]?.commandId).toBe('round-command-1')
    expect(time).toHaveBeenCalledTimes(1)
  })

  test('round-publication HTTP module maps current room failure codes to fixed finite responses', async () => {
    const cases: Array<[string, number]> = [
      ['invalidRequest', 400], ['notInitialized', 404], ['competitionIdMismatch', 404],
      ['forbidden', 403], ['notParticipant', 403], ['revisionConflict', 409],
      ['trustedTimeRegression', 409], ['commandIdConflict', 409],
      ['invalidTournamentCheckpoint', 409], ['invalidLockedRoster', 409], ['invalidRefereeAcl', 403],
      ['invalidRoundPublication', 409], ['roundNotPublishable', 409], ['roundAlreadyPublished', 409],
      ['invalidRefereeAssignment', 400], ['engine:private-engine-detail', 422],
      ['unknownPrivateFailure', 502], ['toString', 502], ['__proto__', 502],
    ]
    const time = spyOn(Date, 'now').mockReturnValue(initialTrustedTimeMs)
    for (const [code, status] of cases) {
      resetState([{ ok: false, error: { code, message: 'private SQL and engine message' } }])
      const response = await post(validBody())
      const text = await response.text()
      expect(response.status).toBe(status)
      expect(text).not.toMatch(/private SQL|engine message|private-engine-detail/)
      expect(announceRequests).toHaveLength(1)
    }
    expect(time).toHaveBeenCalledTimes(cases.length)
  })

  test('round-publication HTTP module validates competition, stage, round, time, command, revision, and room schema', async () => {
    const request = {
      competitionId: 'competition-1', stageId: 'stage-1', roundNumber: 3,
      callerUserId: 'session-user', commandId: 'round-command-1', expectedRevision: 8,
      trustedTimeMs: initialTrustedTimeMs, roundStartMs,
      refereeAssignments: [{ matchId: 'match-1', referees: [{ refereeId: 'referee-1', conflictedTeam: null }] }],
    }
    const malformedValues = [
      { competitionId: 'other-competition' },
      { commandId: 'other-command' },
      { revision: 10 },
      { publication: publication({ roundRef: {
        competitionId: 'competition-1', stageId: 'other-stage', roundNumber: 3,
        roundGenerationId: 'generation-1',
      } }) },
      { publication: publication({ roundRef: {
        competitionId: 'competition-1', stageId: 'stage-1', roundNumber: 4,
        roundGenerationId: 'generation-1',
      } }) },
      { publication: publication({ roundStartMs: roundStartMs + 1 }) },
      { publication: publication({ publicationVersion: 2 }) },
      { publication: publication({ privateManifest: 'private-manifest' }) },
      { publication: publication({ rooms: [{ matchId: 'match-1', gameId: 'game-1', ackConfirmedAtMs: 1 }] }) },
    ]
    for (const overrides of malformedValues) {
      resetState([announceSuccess(request, overrides)])
      const response = await post(validBody())
      const text = await response.text()
      expect(response.status).toBe(502)
      expect(text).not.toMatch(/private-manifest|other-stage|other-command/)
    }
    expect(announceRequests).toHaveLength(1)
  })

  test('round-publication HTTP module releases success, failure, and invalid response roots once with correct receiver', async () => {
    let successDisposals = 0
    let failureDisposals = 0
    let invalidDisposals = 0
    let successReceiverCorrect = false
    let failureReceiverCorrect = false
    let invalidReceiverCorrect = false
    const success = announceSuccess({
      competitionId: 'competition-1', stageId: 'stage-1', roundNumber: 3,
      callerUserId: 'session-user', commandId: 'round-command-1', expectedRevision: 8,
      trustedTimeMs: initialTrustedTimeMs, roundStartMs,
      refereeAssignments: validBody().refereeAssignments,
    })
    Object.defineProperty(success, Symbol.dispose, {
      value: function (this: unknown) { successReceiverCorrect = this === success; successDisposals += 1 },
    })
    const failure = { ok: false, error: { code: 'revisionConflict', message: 'private revision detail' } }
    Object.defineProperty(failure, Symbol.dispose, {
      value: function (this: unknown) { failureReceiverCorrect = this === failure; failureDisposals += 1 },
    })
    const invalid = { ...success, private: 'private response' }
    Object.defineProperty(invalid, Symbol.dispose, {
      value: function (this: unknown) { invalidReceiverCorrect = this === invalid; invalidDisposals += 1 },
    })

    spyOn(Date, 'now').mockReturnValue(initialTrustedTimeMs)
    resetState([success])
    expect((await post(validBody())).status).toBe(200)
    resetState([failure])
    expect((await post(validBody())).status).toBe(409)
    resetState([invalid])
    const invalidResponse = await post(validBody())
    expect(invalidResponse.status).toBe(502)
    expect(await invalidResponse.text()).not.toContain('private response')
    expect([successDisposals, failureDisposals, invalidDisposals]).toEqual([1, 1, 1])
    expect([successReceiverCorrect, failureReceiverCorrect, invalidReceiverCorrect]).toEqual([true, true, true])
  })

  test('round-publication HTTP module rejects nested symbols and disposer accessors without invoking them', async () => {
    let rootDisposals = 0
    let nestedDisposals = 0
    let symbolGetterCalls = 0
    const request = {
      competitionId: 'competition-1', stageId: 'stage-1', roundNumber: 3,
      callerUserId: 'session-user', commandId: 'round-command-1', expectedRevision: 8,
      trustedTimeMs: initialTrustedTimeMs, roundStartMs,
      refereeAssignments: validBody().refereeAssignments,
    }
    const nested = announceSuccess(request)
    Object.defineProperty(nested.value.publication, Symbol.dispose, {
      value: () => { nestedDisposals += 1 },
    })
    Object.defineProperty(nested, Symbol.dispose, {
      value: function (this: unknown) { expect(this === nested).toBe(true); rootDisposals += 1 },
    })
    const accessor = announceSuccess(request)
    Object.defineProperty(accessor, Symbol.dispose, {
      get() { symbolGetterCalls += 1; throw new Error('private disposer getter') },
    })

    spyOn(Date, 'now').mockReturnValue(initialTrustedTimeMs)
    resetState([nested, accessor])
    expect((await post(validBody())).status).toBe(502)
    const accessorResponse = await post(validBody())
    expect(accessorResponse.status).toBe(502)
    expect(await accessorResponse.text()).not.toMatch(/private disposer getter|Symbol/)
    expect(rootDisposals).toBe(1)
    expect(nestedDisposals).toBe(0)
    expect(symbolGetterCalls).toBe(0)
  })

  test('round-publication HTTP module rejects unsafe server time and never falls back to caller time', async () => {
    const invalidTimes = [-1, 1.5, Number.NaN, Number.POSITIVE_INFINITY, 8_640_000_000_000_001]
    const attemptCount = invalidTimes.length
    const time = spyOn(Date, 'now').mockImplementation(() => invalidTimes.shift() ?? -1)
    for (let attempt = 0; attempt < attemptCount; attempt += 1) {
      resetState()
      const response = await post(validBody())
      const text = await response.text()
      expect(response.status).toBe(503)
      expect(text).toContain('serverTimeUnavailable')
      expect(text).not.toMatch(/caller time|trustedTimeMs/)
      expect(announceRequests).toHaveLength(0)
    }
    expect(time).toHaveBeenCalledTimes(5)
  })

  test('round-publication HTTP module safely handles missing binding and thrown RPC', async () => {
    resetState()
    const time = spyOn(Date, 'now').mockReturnValue(initialTrustedTimeMs)
    missingBinding = true
    const missing = await post(validBody())
    expect(missing.status).toBe(501)
    expect(await missing.text()).not.toMatch(/COMPETITION_ROOM|workerEnv/)
    expect(announceRequests).toHaveLength(0)

    resetState()
    throwFromAnnounce = true
    const thrown = await post(validBody())
    const text = await thrown.text()
    expect(thrown.status).toBe(502)
    expect(text).not.toMatch(/private SQL|storage details/)
    expect(text).toContain('賽事服務暫時無法處理要求。')
    expect(time).toHaveBeenCalledTimes(2)
  })
})
