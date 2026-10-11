import { createApp, createError, createRouter, eventHandler, toWebHandler } from 'h3'
import { afterEach, describe, expect, mock, spyOn, test } from 'bun:test'
import type { CompetitionRoomRoundPublicationReadRequest } from '../../../../../../../../shared/competition-room'

let sessionCalls: string[] = []
let roomNames: string[] = []
let readRequests: CompetitionRoomRoundPublicationReadRequest[] = []
let readResults: unknown[] = []
let missingBinding = false
let throwFromRead = false
let markRequestBody = false

mock.module('../../../../../../../utils/auth', () => ({
  requireSession: async (event: { headers: Headers }) => {
    const userId = event.headers.get('x-test-session')
    sessionCalls.push(userId ?? '')
    if (!userId) throw createError({ statusCode: 401, statusMessage: 'Authentication required.' })
    return { user: { id: userId } }
  },
}))

const { default: publicationGet } = await import('./publication.get')

const roundStartMs = 1_800_000_000_000

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

function readSuccess(valueOverrides: Record<string, unknown> = {}) {
  return {
    ok: true,
    value: {
      competitionId: 'competition-1', revision: 9, publication: publication(),
      ...valueOverrides,
    },
  }
}

function resetState(results: unknown[] = [readSuccess()]) {
  sessionCalls = []
  roomNames = []
  readRequests = []
  readResults = [...results]
  missingBinding = false
  throwFromRead = false
  markRequestBody = false
}

function makeWebHandler() {
  const app = createApp()
  app.use(eventHandler((event) => {
    if (markRequestBody) event._requestBody = 'unexpected request body'
    const env: Record<string, unknown> = { APP_ENV: 'development' }
    if (!missingBinding) {
      env.COMPETITION_ROOM = {
        idFromName: (name: string) => {
          roomNames.push(name)
          return name
        },
        get: () => ({
          readRoundPublication: async (request: CompetitionRoomRoundPublicationReadRequest) => {
            readRequests.push(request)
            if (throwFromRead) throw new Error('private transport details')
            return readResults.shift()
          },
          announceRound: async () => { throw new Error('announceRound must not be called by GET') },
        }),
      }
    }
    ;(event.context as { _platform?: unknown })._platform = { cloudflare: { env } }
  }))
  const router = createRouter()
  router.get('/api/competitions/:id/stages/:stageId/rounds/:roundNumber/publication', publicationGet)
  app.use(router.handler)
  return toWebHandler(app)
}

async function get(
  path = '/api/competitions/competition-1/stages/stage-1/rounds/3/publication',
  sessionUserId?: string,
  headers: Record<string, string> = {},
) {
  const requestHeaders = new Headers(headers)
  if (sessionUserId) requestHeaders.set('x-test-session', sessionUserId)
  return makeWebHandler()(new Request(`http://fewfc.test${path}`, { headers: requestHeaders }))
}

afterEach(() => {
  mock.restore()
})

describe('round-publication HTTP module GET', () => {
  test('round-publication HTTP module authenticates first and sends only path selectors plus session actor', async () => {
    resetState()
    const now = spyOn(Date, 'now').mockImplementation(() => { throw new Error('GET must not sample time') })
    const response = await get(
      '/api/competitions/%20competition-1%20/stages/%20stage-1%20/rounds/3/publication',
      'session-user',
    )
    expect(response.status).toBe(200)
    const body: unknown = await response.json()
    expect(body).toEqual({
      competitionId: 'competition-1', revision: 9, publication: publication(),
    })
    expect(sessionCalls).toEqual(['session-user'])
    expect(roomNames).toEqual(['competition-1'])
    expect(readRequests).toEqual([{
      competitionId: 'competition-1', stageId: 'stage-1', roundNumber: 3, callerUserId: 'session-user',
    }])
    expect(now).not.toHaveBeenCalled()
  })

  test('round-publication HTTP module returns an exact read value when publication is null', async () => {
    resetState([readSuccess({ publication: null })])
    const response = await get(undefined, 'session-user')
    expect(response.status).toBe(200)
    const body: unknown = await response.json()
    expect(body).toEqual({ competitionId: 'competition-1', revision: 9, publication: null })
  })

  test('round-publication HTTP module reads a schema-valid publication time from its response snapshot', async () => {
    resetState([readSuccess({ publication: publication({ roundStartMs: roundStartMs + 1 }) })])
    const response = await get(undefined, 'session-user')
    expect(response.status).toBe(200)
    const body: unknown = await response.json()
    expect(body).toEqual({
      competitionId: 'competition-1', revision: 9,
      publication: publication({ roundStartMs: roundStartMs + 1 }),
    })
  })

  test('round-publication HTTP module rejects unauthenticated access before path, query, or room lookup', async () => {
    resetState()
    const response = await get('/api/competitions/%20/stages/stage-1/rounds/0/publication?callerUserId=attacker')
    expect(response.status).toBe(401)
    expect(sessionCalls).toEqual([''])
    expect(roomNames).toHaveLength(0)
    expect(readRequests).toHaveLength(0)
  })

  test('round-publication HTTP module rejects every query marker and GET body before dispatch', async () => {
    resetState()
    for (const path of [
      '/api/competitions/competition-1/stages/stage-1/rounds/3/publication?callerUserId=attacker',
      '/api/competitions/competition-1/stages/stage-1/rounds/3/publication?',
    ]) {
      expect((await get(path, 'session-user')).status).toBe(400)
    }
    expect((await get(undefined, 'session-user', { 'content-length': '1' })).status).toBe(400)
    markRequestBody = true
    expect((await get(undefined, 'session-user')).status).toBe(400)
    expect(roomNames).toHaveLength(0)
    expect(readRequests).toHaveLength(0)
  })

  test('round-publication HTTP module accepts only canonical positive round paths within uint32', async () => {
    resetState()
    for (const value of ['0', '00', '03', '-3', '3.0', '3e0', 'NaN', '4294967296']) {
      const response = await get(
        `/api/competitions/competition-1/stages/stage-1/rounds/${encodeURIComponent(value)}/publication`,
        'session-user',
      )
      expect(response.status).toBe(400)
    }
    expect(roomNames).toHaveLength(0)
    expect(readRequests).toHaveLength(0)
  })

  test('round-publication HTTP module maps room ACL failures without returning room messages', async () => {
    resetState([{ ok: false, error: { code: 'forbidden', message: 'private ACL and organizer details' } }])
    const response = await get(undefined, 'session-user')
    const text = await response.text()
    expect(response.status).toBe(403)
    expect(text).not.toMatch(/private ACL|organizer details/)
    expect(text).toContain('你沒有執行此操作的權限。')
    expect(readRequests).toHaveLength(1)
  })

  test('round-publication HTTP module rejects mismatched identity, schema, and private response fields', async () => {
    const malformedValues = [
      { competitionId: 'other-competition' },
      { revision: 0 },
      { private: { deckSeed: 'private-seed' } },
      { publication: publication({ roundRef: {
        competitionId: 'competition-1', stageId: 'other-stage', roundNumber: 3,
        roundGenerationId: 'generation-1',
      } }) },
      { publication: publication({ roundRef: {
        competitionId: 'competition-1', stageId: 'stage-1', roundNumber: 4,
        roundGenerationId: 'generation-1',
      } }) },
      { publication: publication({ roundStartMs: 299_999 }) },
      { publication: publication({ privateManifest: 'private-manifest' }) },
      { publication: publication({ rooms: [{ matchId: 'match-1', gameId: 'game-1', ackConfirmedAtMs: null, binding: {} }] }) },
    ]
    for (const overrides of malformedValues) {
      resetState([readSuccess(overrides)])
      const response = await get(undefined, 'session-user')
      const text = await response.text()
      expect(response.status).toBe(502)
      expect(text).not.toMatch(/private-seed|private-manifest|other-stage|binding/)
    }
    expect(readRequests).toHaveLength(1)
  })

  test('round-publication HTTP module rejects nested accessors without executing them and releases the root once', async () => {
    let getterCalls = 0
    let disposalCalls = 0
    let receiverCorrect = false
    const valuePublication = publication()
    const value = { competitionId: 'competition-1', revision: 9, publication: valuePublication }
    const rooms = valuePublication.rooms
    Object.defineProperty(rooms[0]!, 'gameId', {
      enumerable: true,
      get() { getterCalls += 1; throw new Error('private nested getter') },
    })
    const raw = { ok: true, value }
    Object.defineProperty(raw, Symbol.dispose, {
      value: function (this: unknown) {
        receiverCorrect = this === raw
        disposalCalls += 1
      },
    })
    resetState([raw])
    const response = await get(undefined, 'session-user')
    const text = await response.text()
    expect(response.status).toBe(502)
    expect(getterCalls).toBe(0)
    expect(disposalCalls).toBe(1)
    expect(receiverCorrect).toBe(true)
    expect(text).not.toMatch(/private nested getter|Symbol/)
  })

  test('round-publication HTTP cleanup snapshot preserves validated value after releasing root once', async () => {
    const raw = readSuccess()
    let disposalCalls = 0
    Object.defineProperty(raw, Symbol.dispose, {
      value: function (this: unknown) {
        expect(this).toBe(raw)
        disposalCalls += 1
        raw.value.revision = 0
        raw.value.publication.rooms[0]!.gameId = 'cleanup-mutated-game'
        Object.assign(raw.value, { cleanupPrivate: 'cleanup-private-value' })
      },
    })
    resetState([raw])

    const response = await get(undefined, 'session-user')
    const body = await response.text()
    expect(response.status).toBe(200)
    expect(JSON.parse(body)).toEqual({
      competitionId: 'competition-1', revision: 9, publication: publication(),
    })
    expect(body).not.toContain('cleanup-mutated-game')
    expect(body).not.toContain('cleanup-private-value')
    expect(disposalCalls).toBe(1)
  })

  test('round-publication HTTP module releases error and invalid-envelope resources once with their original receiver', async () => {
    let failureDisposals = 0
    let invalidDisposals = 0
    let failureReceiverCorrect = false
    let invalidReceiverCorrect = false
    const failure = { ok: false, error: { code: 'revisionConflict', message: 'private revision detail' } }
    Object.defineProperty(failure, Symbol.dispose, {
      value: function (this: unknown) { failureReceiverCorrect = this === failure; failureDisposals += 1 },
    })
    const invalid = { ok: true, value: readSuccess().value, private: 'secret' }
    Object.defineProperty(invalid, Symbol.dispose, {
      value: function (this: unknown) { invalidReceiverCorrect = this === invalid; invalidDisposals += 1 },
    })

    resetState([failure])
    expect((await get(undefined, 'session-user')).status).toBe(409)
    resetState([invalid])
    const invalidResponse = await get(undefined, 'session-user')
    expect(invalidResponse.status).toBe(502)
    expect(await invalidResponse.text()).not.toContain('secret')
    expect(failureDisposals).toBe(1)
    expect(invalidDisposals).toBe(1)
    expect(failureReceiverCorrect).toBe(true)
    expect(invalidReceiverCorrect).toBe(true)
  })

  test('round-publication HTTP module maps missing binding and thrown RPC to fixed safe errors', async () => {
    resetState()
    missingBinding = true
    const missing = await get(undefined, 'session-user')
    expect(missing.status).toBe(501)
    expect(await missing.text()).not.toMatch(/workerEnv|COMPETITION_ROOM/)

    resetState()
    throwFromRead = true
    const thrown = await get(undefined, 'session-user')
    const text = await thrown.text()
    expect(thrown.status).toBe(502)
    expect(text).not.toContain('private transport details')
    expect(text).toContain('賽事服務暫時無法處理要求。')
  })
})
