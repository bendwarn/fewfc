import { createError, createApp, createRouter, eventHandler, toWebHandler } from 'h3'
import { afterEach, expect, mock, test } from 'bun:test'
import type { CompetitionRoomMatchExecuteRequest } from '../../../../../../shared/competition-room'
import { emptyCompetitionServerState, isJsonObject } from '../../../../../../shared/competition'
import type { CompetitionRequest } from '../../../../../../shared/competition'
import { callCompetitionEngine, projectCompetitionResponse } from '../../../../../../worker/competition-bridge'
import type { ClockControlPublicValue } from '../../../../../utils/competition-room'

let sessionCalls: string[] = []
let roomNames: string[] = []
let requests: CompetitionRoomMatchExecuteRequest[] = []
let rpcResult: unknown
let rpcHandler: ((request: CompetitionRoomMatchExecuteRequest) => Promise<unknown>) | undefined

mock.module('../../../../../utils/auth', () => ({
  requireSession: async (event: { headers: Headers }) => {
    const userId = event.headers.get('x-test-session')
    sessionCalls.push(userId ?? '')
    if (!userId) throw createError({ statusCode: 401, statusMessage: 'Authentication required.' })
    return { user: { id: userId } }
  },
}))

const { default: clockControlPost } = await import('./clock-control.post')

function validResult(request = {
  competitionId: 'path-competition', matchId: 'path-match', callerUserId: 'referee',
  commandId: 'strict-command', expectedMatchRevision: 3, trustedTimeMs: 123,
  operation: { type: 'clockPause', matchId: 'path-match', reason: '裁判處理' },
} as CompetitionRoomMatchExecuteRequest, overrides: Record<string, unknown> = {}) {
  if (request.operation.type !== 'clockPause' && request.operation.type !== 'clockResume') {
    throw new Error('expected clock-control request')
  }
  return {
    ok: true,
    value: {
      competitionId: request.competitionId,
      matchId: request.matchId,
      matchRevision: request.expectedMatchRevision + 1,
      commandId: request.commandId,
      idempotent: false,
      timeoutEvidence: null,
      nextClockDeadlineMs: request.operation.type === 'clockPause' ? null : 456789,
      matchProjection: null,
      clockProjection: {
        ok: true,
        operation: request.operation.type,
        public: { paused: request.operation.type === 'clockPause', display: { safe: true } },
      },
      ...overrides,
    },
  }
}

function resetState(result?: unknown) {
  sessionCalls = []
  roomNames = []
  requests = []
  rpcResult = result
  rpcHandler = undefined
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
              executeMatch: async (request: CompetitionRoomMatchExecuteRequest) => {
                requests.push(request)
                if (rpcHandler) return rpcHandler(request)
                return rpcResult ?? validResult(request)
              },
              execute: async () => { throw new Error('generic execute must never be called') },
            }),
          },
        },
      },
    }
  }))
  const router = createRouter()
  router.post('/api/competitions/:id/matches/:matchId/clock-control', clockControlPost)
  app.use(router.handler)
  return toWebHandler(app)
}

async function post(
  body: unknown,
  sessionUserId?: string,
  path = '/api/competitions/path-competition/matches/path-match/clock-control',
) {
  const headers = new Headers({ 'content-type': 'application/json' })
  if (sessionUserId) headers.set('x-test-session', sessionUserId)
  return makeWebHandler()(new Request(`http://fewfc.test${path}`, {
    method: 'POST', headers, body: body === undefined ? undefined : JSON.stringify(body),
  }))
}

async function readAcknowledgement(response: Response): Promise<ClockControlPublicValue> {
  const value: unknown = await response.json()
  const keys = ['competitionId', 'matchId', 'matchRevision', 'commandId', 'action', 'idempotent', 'nextClockDeadlineMs']
  if (!isJsonObject(value) || Reflect.ownKeys(value).length !== keys.length
    || !keys.every(key => Object.hasOwn(value, key))
    || typeof value.competitionId !== 'string' || typeof value.matchId !== 'string'
    || typeof value.matchRevision !== 'number' || !Number.isSafeInteger(value.matchRevision)
    || typeof value.commandId !== 'string' || (value.action !== 'pause' && value.action !== 'resume')
    || typeof value.idempotent !== 'boolean'
    || !(value.nextClockDeadlineMs === null || (typeof value.nextClockDeadlineMs === 'number'
      && Number.isSafeInteger(value.nextClockDeadlineMs) && value.nextClockDeadlineMs >= 0))) {
    throw new Error('H3 acknowledgement must have the exact minimal public shape')
  }
  return { competitionId: value.competitionId, matchId: value.matchId, matchRevision: value.matchRevision,
    commandId: value.commandId, action: value.action, idempotent: value.idempotent, nextClockDeadlineMs: value.nextClockDeadlineMs }
}

afterEach(() => {
  mock.restore()
})

test('authenticated H3 route maps pause and resume to the exact executeMatch request and safe DTO', async () => {
  resetState()
  const realNow = Date.now
  let nowCalls = 0
  Date.now = () => { nowCalls += 1; return 123456 }
  try {
    const pause = await post({
      action: 'pause', reason: '  處理斷線  ', expectedMatchRevision: 4, commandId: 'pause-command',
    }, 'session-referee', '/api/competitions/%20path-competition%20/matches/%20path-match%20/clock-control')
    const pauseValue = await readAcknowledgement(pause)
    expect(pause.status).toBe(200)
    expect(requests[0]).toEqual({
      competitionId: 'path-competition', matchId: 'path-match', callerUserId: 'session-referee',
      commandId: 'pause-command', expectedMatchRevision: 4, trustedTimeMs: 123456,
      operation: { type: 'clockPause', matchId: 'path-match', reason: '處理斷線' },
    })
    expect(pauseValue).toEqual({
      competitionId: 'path-competition', matchId: 'path-match', matchRevision: 5,
      commandId: 'pause-command', action: 'pause', idempotent: false, nextClockDeadlineMs: null,
    })

    const resume = await post({
      action: 'resume', reason: '恢復計時', expectedMatchRevision: 5, commandId: 'resume-command',
    }, 'session-referee')
    const resumeValue = await readAcknowledgement(resume)
    expect(resume.status).toBe(200)
    expect(requests[1]).toEqual({
      competitionId: 'path-competition', matchId: 'path-match', callerUserId: 'session-referee',
      commandId: 'resume-command', expectedMatchRevision: 5, trustedTimeMs: 123456,
      operation: { type: 'clockResume', matchId: 'path-match', reason: '恢復計時' },
    })
    expect(resumeValue).toEqual({
      competitionId: 'path-competition', matchId: 'path-match', matchRevision: 6,
      commandId: 'resume-command', action: 'resume', idempotent: false, nextClockDeadlineMs: 456789,
    })
    expect(sessionCalls).toEqual(['session-referee', 'session-referee'])
    expect(roomNames).toEqual(['path-competition', 'path-competition'])
    expect(nowCalls).toBe(2)
    expect(JSON.stringify({ pauseValue, resumeValue })).not.toMatch(/處理斷線|恢復計時|projection|timeoutEvidence|private|serverState/)
  } finally {
    Date.now = realNow
  }
})

test('unauthenticated, empty path, and every extra or spoofed body field fail before dispatch', async () => {
  resetState()
  const valid = { action: 'pause', reason: '裁判處理', expectedMatchRevision: 1, commandId: 'command' }
  expect((await post(valid)).status).toBe(401)
  expect((await post(valid, 'referee', '/api/competitions/%20/matches/path-match/clock-control')).status).toBe(400)
  expect((await post(valid, 'referee', '/api/competitions/path-competition/matches/%20/clock-control')).status).toBe(400)
  const invalidBodies = [
    null,
    { ...valid, callerUserId: 'attacker' },
    { ...valid, refereeId: 'attacker' },
    { ...valid, trustedTimeMs: 0 },
    { ...valid, competitionId: 'other' },
    { ...valid, matchId: 'other' },
    { ...valid, operation: { type: 'clockResume' } },
    { ...valid, team: 'attacker' },
    { ...valid, turn: 7 },
    { ...valid, choiceId: 'attacker' },
    { ...valid, serverState: {} },
    { action: 'pause', reason: '裁判處理', expectedMatchRevision: 1 },
    { ...valid, action: 'clockPause' },
    { ...valid, reason: '  ' },
    { ...valid, expectedMatchRevision: 1.5 },
    { ...valid, commandId: '' },
  ]
  for (const body of invalidBodies) expect((await post(body, 'referee')).status).toBe(400)
  expect(requests).toHaveLength(0)
  expect(roomNames).toHaveLength(0)
})

test('DO authorization, conflict, and provisioning errors map to fixed safe responses', async () => {
  for (const [code, expectedStatus] of [
    ['forbidden', 403], ['matchRevisionConflict', 409], ['gameRoomNotProvisioned', 409],
  ] as const) {
    resetState({ ok: false, error: { code, message: 'private internal explanation' } })
    const response = await post({
      action: 'pause', reason: '暫停', expectedMatchRevision: 2, commandId: `cmd-${code}`,
    }, 'session-referee')
    const text = await response.text()
    expect(response.status).toBe(expectedStatus)
    expect(text).not.toContain('private internal explanation')
    expect(text).toContain(code)
  }
  resetState({ ok: false, error: { code: 'engine:private-engine-detail', message: 'private engine text' } })
  const engine = await post({ action: 'resume', reason: '繼續', expectedMatchRevision: 2, commandId: 'engine' }, 'referee')
  const engineText = await engine.text()
  expect(engine.status).toBe(422)
  expect(engineText).toContain('engineRejected')
  expect(engineText).not.toMatch(/private-engine-detail|private engine text/)
})

test('malformed RPC envelopes, values, and clock projections fail closed with 502', async () => {
  const body = { action: 'pause', reason: '裁判處理', expectedMatchRevision: 3, commandId: 'strict-command' }
  for (const malformed of [
    { ok: true, value: {}, private: { secret: true } },
    { ok: true, value: {} },
    validResult(undefined, { private: { secret: true } }),
    validResult(undefined, { serverState: { secret: true } }),
    validResult(undefined, { extra: 'secret' }),
    validResult(undefined, { matchRevision: 5 }),
    validResult(undefined, { commandId: 'other' }),
    validResult(undefined, { timeoutEvidence: { type: 'timeout' } }),
    validResult(undefined, { matchProjection: { ok: true } }),
    validResult(undefined, { nextClockDeadlineMs: -1 }),
    validResult(undefined, { nextClockDeadlineMs: Number.MAX_SAFE_INTEGER + 1 }),
    validResult(undefined, { clockProjection: { ok: true, operation: 'clockResume', public: {} } }),
    validResult(undefined, { clockProjection: { ok: true, operation: 'clockPause', public: { private: { leak: true } } } }),
    validResult(undefined, { clockProjection: { ok: true, operation: 'clockPause', public: { serverState: {} } } }),
  ]) {
    resetState(malformed)
    const response = await post(body, 'referee')
    const text = await response.text()
    expect(response.status).toBe(502)
    expect(text).not.toMatch(/secret|leak|serverState/)
  }
  resetState({ ok: false, error: { code: 'gameRoomNotProvisioned', message: 'secret' }, value: {} })
  expect((await post(body, 'referee')).status).toBe(502)
})

// 直接載入正式 ABI，不替換全程序 rules-engine mock；明示路徑優先，否則用既有 bundled WASM。
async function formalClockControlFixture(submitted: boolean) {
  const path = process.env.FEWC_COMPETITION_TEST_WASM
    ?? new URL('../../../../../../worker/wasm/fewfc.wasm', import.meta.url)
  const instance = new WebAssembly.Instance(new WebAssembly.Module(await Bun.file(path).arrayBuffer()), {})
  const { memory, fewfc_alloc: alloc, fewfc_dealloc: dealloc, fewfc_handle_competition_request: invoke } = instance.exports
  if (!(memory instanceof WebAssembly.Memory)
    || typeof alloc !== 'function' || typeof dealloc !== 'function' || typeof invoke !== 'function') {
    throw new Error('formal competition WASM exports are missing')
  }
  const wasm = async (input: CompetitionRequest): Promise<unknown> => {
    const bytes = new TextEncoder().encode(JSON.stringify(input))
    const ptr: unknown = alloc(bytes.length)
    if (typeof ptr !== 'number' || !Number.isSafeInteger(ptr) || ptr < 0) throw new Error('invalid WASM allocation')
    let packed: unknown
    try {
      new Uint8Array(memory.buffer, ptr, bytes.length).set(bytes)
      packed = invoke(ptr, bytes.length)
    } finally {
      dealloc(ptr, bytes.length)
    }
    if (typeof packed !== 'bigint') throw new Error('invalid WASM JSON result')
    const outputPtr = Number(packed >> 32n)
    const outputLength = Number(packed & 0xffffffffn)
    try {
      const raw: unknown = JSON.parse(new TextDecoder().decode(new Uint8Array(memory.buffer, outputPtr, outputLength)))
      return raw
    } finally {
      dealloc(outputPtr, outputLength)
    }
  }
  let state = emptyCompetitionServerState()
  let controlCalls = 0
  const run = async (operation: CompetitionRequest['operation'], time: number, actor: CompetitionRequest['serverActor'] = {
    type: 'organizer', playerId: 'organizer',
  }) => {
    const raw = await callCompetitionEngine({ serverActor: actor, trustedTimeMs: time, operation, serverState: state }, wasm)
    if (!raw.ok) throw new Error(`formal prefix failed: ${raw.error.code}`)
    state = raw.serverState
    return raw
  }
  let match = await run({ type: 'matchCreate', config: {
    matchId: 'path-match', teams: ['a', 'b'], format: { type: 'fixedGames', value: 3 },
    firstGame: { type: 'assigned', team: 'a' }, referees: [{ refereeId: 'session-referee', conflictedTeam: null }],
  } }, 0)
  const projections = [projectCompetitionResponse(match)]
  if (submitted) {
    const submittedMatch = await run({ type: 'matchSubmitGame', matchId: 'path-match', submission: {
      submissionId: 'submission-1', gameRecordId: 'record-1', rulingRecordId: 'ruling-1',
      startKind: { type: 'ordinaryNewGame' }, firstPlayer: 'a',
      terminalEvidence: { type: 'canonicalGameEnded', conclusion: {
        outcome: { type: 'winner', team: 'a' }, causes: [{ type: 'teamHpDepleted', teams: ['b'] }],
      } }, ruling: { type: 'gameAccepted', outcome: { winner: 'a' } },
    } }, 0)
    projections.push(projectCompetitionResponse(submittedMatch))
    match = await run({ type: 'matchAssignFirstPlayer', matchId: 'path-match', team: 'b', reason: '敗方指定' }, 0)
    projections.push(projectCompetitionResponse(match))
    match = await run({ type: 'matchCorrectRuling', matchId: 'path-match', correctionId: 'correction-1',
      refereeId: 'session-referee', gameRecordId: 'record-1', replacementRulingRecordId: 'replacement-1',
      replacementRuling: { type: 'gameAccepted', outcome: { winner: 'a' } }, reason: '確認原判定',
    }, 0, { type: 'referee', playerId: 'session-referee' })
    projections.push(projectCompetitionResponse(match))
  }
  const matchProjection = projectCompetitionResponse(match)
  await run({ type: 'clockCreate', teams: ['a', 'b'], config: {
    mainTimeMs: 300000, actionTimeMs: 30000, outOfTurnChoiceTimeMs: 30000,
  } }, 1000)
  await run({ type: 'clockBeginAction', team: 'a', turn: 1 }, 1000)
  const receipts = new Map<string, ReturnType<typeof validResult>>()
  return {
    matchProjection,
    projections,
    checkpoint: () => structuredClone(state),
    controlCalls: () => controlCalls,
    query: async (time: number) => {
      const raw = await callCompetitionEngine({ serverActor: { type: 'player', playerId: 'reader' },
        trustedTimeMs: time, operation: { type: 'clockView' }, serverState: state }, wasm)
      if (!raw.ok) throw new Error('formal clockView failed')
      expect(raw.serverState).toStrictEqual(state)
      return raw.public
    },
    execute: async (request: CompetitionRoomMatchExecuteRequest) => {
      const receipt = receipts.get(request.commandId)
      if (receipt) return { ok: true, value: { ...receipt.value, idempotent: true } }
      if (request.operation.type !== 'clockPause' && request.operation.type !== 'clockResume') throw new Error('unexpected operation')
      controlCalls += 1
      const raw = await run(request.operation, request.trustedTimeMs, { type: 'referee', playerId: request.callerUserId })
      if (!isJsonObject(raw.public) || !isJsonObject(raw.public.clock)) throw new Error('formal clock public is missing')
      const result = validResult(request, { matchProjection,
        clockProjection: projectCompetitionResponse(raw), nextClockDeadlineMs: raw.public.clock.nextTimeoutDeadlineMs })
      receipts.set(request.commandId, result)
      return result
    },
  }
}

test('E4d3c-r2 formal WASM non-null match projection crosses true H3 pause/resume and receipt retry', async () => {
  const realNow = Date.now
  let now = 1010
  Date.now = () => now
  try {
    for (const submitted of [false, true]) {
      resetState()
      const fixture = await formalClockControlFixture(submitted)
      let disposals = 0
      rpcHandler = async request => {
        const result = await fixture.execute(request)
        return Object.assign(result, { [Symbol.dispose]: function () {
          expect(Object.is(this, result)).toBe(true)
          disposals += 1
        } })
      }
      now = 1010
      const pauseBody = { action: 'pause', reason: '裁判暫停', expectedMatchRevision: 4, commandId: 'formal-pause' }
      const pause = await post(pauseBody, 'session-referee')
      expect(pause.status).toBe(200)
      expect(await readAcknowledgement(pause)).toStrictEqual({ competitionId: 'path-competition', matchId: 'path-match',
        matchRevision: 5, commandId: 'formal-pause', action: 'pause', idempotent: false, nextClockDeadlineMs: null })
      const pausedCheckpoint = fixture.checkpoint()
      expect(await fixture.query(10000)).toMatchObject({ running: null, paused: { type: 'action', team: 'a', turn: 1 },
        teams: [{ mainTimeRemainingMs: 299990 }, { mainTimeRemainingMs: 300000 }] })
      now = 1050
      const retry = await post(pauseBody, 'session-referee')
      expect(retry.status).toBe(200)
      expect(await readAcknowledgement(retry)).toMatchObject({ matchRevision: 5, idempotent: true, action: 'pause' })
      expect(fixture.checkpoint()).toStrictEqual(pausedCheckpoint)
      expect(fixture.controlCalls()).toBe(1)
      now = 1100
      const resume = await post({ action: 'resume', reason: '裁判恢復', expectedMatchRevision: 5, commandId: 'formal-resume' }, 'session-referee')
      expect(resume.status).toBe(200)
      expect(await readAcknowledgement(resume)).toStrictEqual({ competitionId: 'path-competition', matchId: 'path-match',
        matchRevision: 6, commandId: 'formal-resume', action: 'resume', idempotent: false, nextClockDeadlineMs: 331090 })
      expect(await fixture.query(1200)).toMatchObject({ running: { type: 'action', team: 'a', turn: 1 }, paused: null,
        teams: [{ mainTimeRemainingMs: 299890 }, { mainTimeRemainingMs: 300000 }] })
      expect(fixture.controlCalls()).toBe(2)
      expect(requests).toHaveLength(3)
      expect(disposals).toBe(3)
    }
  } finally {
    Date.now = realNow
  }
})

test('E4d3c-r2 true H3 accepts only exact formal public match DTOs and rejects corrupt/private projections', async () => {
  const fixture = await formalClockControlFixture(true)
  const body = { action: 'pause', reason: '暫停', expectedMatchRevision: 3, commandId: 'strict-command' }
  for (const projection of fixture.projections) {
    resetState(validResult(undefined, { matchProjection: projection }))
    const response = await post(body, 'session-referee')
    expect(response.status).toBe(200)
    const acknowledgement = await readAcknowledgement(response)
    expect(acknowledgement).toStrictEqual({ competitionId: 'path-competition', matchId: 'path-match',
      matchRevision: 4, commandId: 'strict-command', action: 'pause', idempotent: false, nextClockDeadlineMs: null })
    expect(JSON.stringify(acknowledgement)).not.toMatch(/projection|config|record-1|replacement-1|session-referee/)
    expect(requests).toHaveLength(1)
  }
  const malformedProjection = (path: string[], value: unknown): unknown => {
    const root = structuredClone(fixture.matchProjection)
    let cursor: unknown = root
    for (const key of path.slice(0, -1)) {
      if (!isJsonObject(cursor)) throw new Error('fixture path is not an object')
      cursor = cursor[key]
    }
    const key = path.at(-1)
    if (!isJsonObject(cursor) || !key) throw new Error('invalid fixture leaf')
    Object.defineProperty(cursor, key, { value, enumerable: true, writable: true, configurable: true })
    return root
  }
  const malformed = [
    malformedProjection(['operation'], 'matchView'),
    malformedProjection(['operation'], ['matchCreate']),
    malformedProjection(['public', 'match', 'config', 'matchId'], 'another-match'),
    malformedProjection(['public', 'match', 'config', 'format'], { type: 'unknown', value: 3 }),
    malformedProjection(['public', 'match', 'config', 'teams'], ['a', 'a']),
    malformedProjection(['public', 'match', 'config', 'referees'], [{ refereeId: 'ref' }]),
    malformedProjection(['public', 'match', 'status'], ['open']),
    malformedProjection(['public', 'match', 'score'], [0]),
    malformedProjection(['public', 'match', 'nextGame'], { gameNumber: 2, kind: { type: 'ordinaryNewGame' }, firstPlayer: { type: 'assigned', team: 'outsider' } }),
    malformedProjection(['public', 'match', 'games'], [{ gameNumber: 1 }]),
    malformedProjection(['public', 'match', 'rulings'], [{ rulingRecordId: 'r', gameRecordId: 'g', ruling: { type: 'unknown' } }]),
    malformedProjection(['public', 'match', 'corrections'], [{ correctionId: 'c' }]),
    malformedProjection(['public', 'match', 'outcome'], { type: 'winner', winnerId: 'a', loserId: 'b', score: [1, 0], private: {} }),
    ...['serverState', 'private', 'binding', 'deckSeed', 'deckSnapshot', 'announcement', 'captain', 'members', 'decks', 'rosterVersion', 'lockedRoster', 'extra']
      .map(key => malformedProjection(['public', 'match', 'config', key], 'private-marker')),
    malformedProjection(['public', 'match', 'games'], Object.assign([], { [Symbol.dispose]: () => {} })),
    Object.assign(structuredClone(fixture.matchProjection), { [Symbol.dispose]: () => {} }),
    Object.defineProperty(structuredClone(fixture.matchProjection), 'hidden', { value: 'private-marker' }),
  ]
  for (const projection of malformed) {
    let disposals = 0
    resetState(Object.assign(validResult(undefined, { matchProjection: projection }), {
      [Symbol.dispose]: () => { disposals += 1 },
    }))
    const response = await post(body, 'session-referee')
    expect(response.status).toBe(502)
    const error: unknown = await response.json()
    expect(error).toMatchObject({ data: { code: 'engineProtocolError' } })
    expect(JSON.stringify(error)).not.toMatch(/private-marker|another-match|deckSnapshot|outsider/)
    expect(disposals).toBe(1)
    expect(requests).toHaveLength(1)
  }
})

test('E4d3c-r2 H3 native-style root disposer is receiver-bound and once-only on success or denial', async () => {
  const body = { action: 'pause', reason: '暫停', expectedMatchRevision: 3, commandId: 'strict-command' }
  for (const [result, status] of [
    [validResult(), 200], [{ ok: false, error: { code: 'forbidden', message: 'internal-private-message' } }, 403],
    [{ ok: true, value: {}, extra: 'secret' }, 502],
  ] as const) {
    let disposals = 0
    const root = Object.assign(result, { [Symbol.dispose]: function () {
      expect(this).toBe(root)
      disposals += 1
    } })
    resetState(root)
    const response = await post(body, 'session-referee')
    expect(response.status).toBe(status)
    expect(await response.text()).not.toMatch(/internal-private-message|secret/)
    expect(disposals).toBe(1)
    expect(requests).toHaveLength(1)
  }
})

test('E4d3c-r2 H3 unknown root symbols and nested resource symbols are protocol errors', async () => {
  const body = { action: 'pause', reason: '暫停', expectedMatchRevision: 3, commandId: 'strict-command' }
  for (const root of [
    Object.assign(validResult(), { [Symbol('unknown')]: 'secret' }),
    validResult(undefined, { clockProjection: { ok: true, operation: 'clockPause', public: {
      display: { [Symbol.dispose]: () => {} },
    } } }),
    validResult(undefined, { [Symbol('semantic-extra')]: 'secret' }),
  ]) {
    resetState(root)
    const response = await post(body, 'session-referee')
    expect(response.status).toBe(502)
    expect(await response.text()).not.toContain('secret')
    expect(requests).toHaveLength(1)
  }
})

test('E4d3c-r2 H3 root lifecycle rejects async/accessor disposers and safely closes cleanup failures', async () => {
  const body = { action: 'pause', reason: '暫停', expectedMatchRevision: 3, commandId: 'strict-command' }
  for (const kind of ['async', 'accessor', 'throws', 'returns-value', 'unknown-symbol'] as const) {
    let calls = 0
    const root = validResult()
    if (kind === 'async') Object.defineProperty(root, Symbol.dispose, { value: async () => { calls += 1 } })
    else if (kind === 'accessor') Object.defineProperty(root, Symbol.dispose, { get() { calls += 1; return () => {} } })
    else Object.defineProperty(root, Symbol.dispose, { value: function () {
      expect(this).toBe(root)
      calls += 1
      if (kind === 'throws') throw new Error('private-cleanup-marker')
      if (kind === 'returns-value') return 'private-cleanup-marker'
    } })
    if (kind === 'unknown-symbol') Object.assign(root, { [Symbol('unknown')]: 'private-cleanup-marker' })
    resetState(root)
    const response = await post(body, 'session-referee')
    expect(response.status).toBe(502)
    const error: unknown = await response.json()
    expect(error).toMatchObject({ data: { code: kind === 'throws' || kind === 'returns-value'
      ? 'competitionRoomFailure' : 'engineProtocolError' } })
    expect(JSON.stringify(error)).not.toContain('private-cleanup-marker')
    expect(calls).toBe(kind === 'async' || kind === 'accessor' ? 0 : 1)
    expect(requests).toHaveLength(1)
  }
})
