import { expect, test } from 'bun:test'
import {
  callCompetitionEngine,
  executeCompetitionOperation,
  parseCompetitionEngineResponse,
  projectCompetitionResponse,
} from './competition-bridge'
import type {
  CompetitionEngineResponse,
  CompetitionRequest,
  JsonObject,
  QualifiedStageTemplate,
  StageQualificationSlot,
  TournamentFormat,
} from '../shared/competition'
import { emptyCompetitionServerState, isExactCompetitionJson, isJsonObject } from '../shared/competition'

const request: CompetitionRequest = {
  serverActor: { type: 'referee', playerId: 'referee-1' },
  trustedTimeMs: 1000,
  operation: {
    type: 'tournamentSubmitResult',
    stageId: 'stage-1',
    matchId: 'stage-1-g0-r1-m1',
    result: { type: 'doubleLoss' },
  },
  serverState: {
    registration: null,
    clock: null,
    tournament: {
      tournamentId: 'event-1',
      stages: [],
    },
    matches: {},
  },
}

function partialScheduleSuccess(): CompetitionEngineResponse {
  return {
    ok: true,
    operation: 'tournamentSubmitResult',
    public: {
      result: {
        type: 'appliedWithScheduleError',
        scheduleError: 'double loss needs referee',
      },
      tournament: {
        tournamentId: 'event-1',
        stages: [{ status: 'active', lastScheduleError: 'double loss needs referee' }],
      },
    },
    serverState: request.serverState,
  }
}

test('E6e1 clockView recognizes a minimal display and strips private checkpoints', () => {
  const display = {
    sampledAtMs: 110,
    teams: [
      { team: 'a', mainTimeRemainingMs: 0, byoyomiTimeRemainingMs: 20, byoyomiActive: true },
      { team: 'b', mainTimeRemainingMs: 100, byoyomiTimeRemainingMs: 30, byoyomiActive: false },
    ],
    running: { type: 'action', team: 'a', turn: 1 },
    paused: null,
    awaitingAuthority: false,
  }
  const raw = parseCompetitionEngineResponse({
    ok: true, operation: 'clockView', public: display,
    private: { cards: ['private-card'] },
    serverState: {
      ...request.serverState,
      registration: { commands: [{ action: { cards: ['private-card'] } }] },
      clock: { lastObservedMs: 0, nextByoyomiEntrySequence: 0 },
    },
  })
  expect(projectCompetitionResponse(raw)).toStrictEqual({ ok: true, operation: 'clockView', public: display })
  expect(JSON.stringify(projectCompetitionResponse(raw))).not.toMatch(/private-card|serverState|nextByoyomiEntrySequence/)
})

test('E6e1 clockView rejects malformed display and virtual adjudication fields', () => {
  const valid = {
    sampledAtMs: 110,
    teams: [
      { team: 'a', mainTimeRemainingMs: 0, byoyomiTimeRemainingMs: 20, byoyomiActive: true },
      { team: 'b', mainTimeRemainingMs: 100, byoyomiTimeRemainingMs: 30, byoyomiActive: false },
    ],
    running: { type: 'action', team: 'a', turn: 1 }, paused: null, awaitingAuthority: false,
  }
  const invalid: unknown[] = [
    { ...valid, winner: 'a' }, { ...valid, timeoutRuling: { type: 'timeoutLoss', loser: 'a' } },
    { ...valid, needsDecision: 'zeroTimeControl' }, { ...valid, private: { cards: ['secret'] } },
    { ...valid, sampledAtMs: -1 }, { ...valid, sampledAtMs: 0.5 },
    { ...valid, teams: valid.teams.slice(0, 1) },
    { ...valid, teams: [valid.teams[0], valid.teams[0]] },
    { ...valid, teams: [{ ...valid.teams[0], byoyomiEntrySequence: 0 }, valid.teams[1]] },
    { ...valid, teams: [{ ...valid.teams[0], mainTimeRemainingMs: -1 }, valid.teams[1]] },
    { ...valid, running: { type: 'action', team: 'outsider', turn: 1 } },
    { ...valid, running: { ...valid.running, private: 'secret' } },
    { ...valid, paused: valid.running },
    { ...valid, running: { type: 'outOfTurnChoice', team: 'b', choiceId: '', remainingMs: 1 } },
    { ...valid, awaitingAuthority: 'false' },
  ]
  for (const publicValue of invalid) {
    expect(() => parseCompetitionEngineResponse({
      ok: true, operation: 'clockView', public: publicValue, serverState: request.serverState,
    })).toThrow('clockView display is invalid')
  }
})

test('E6e1 clockView keeps null, zero and independent running or paused choice budgets', () => {
  const teams = [
    { team: 'a', mainTimeRemainingMs: null, byoyomiTimeRemainingMs: null, byoyomiActive: false },
    { team: 'b', mainTimeRemainingMs: 0, byoyomiTimeRemainingMs: 0, byoyomiActive: false },
  ]
  for (const window of [
    null,
    { type: 'preparation', teams: ['a', 'b'] },
    { type: 'action', team: 'a', turn: 2 },
    { type: 'outOfTurnChoice', team: 'b', choiceId: 'choice', remainingMs: 7 },
    { type: 'outOfTurnChoice', team: 'a', choiceId: 'unlimited', remainingMs: null },
  ]) {
    for (const paused of [false, true]) {
      const display = {
        sampledAtMs: 125, teams,
        running: paused ? null : window, paused: paused ? window : null, awaitingAuthority: false,
      }
      const raw = parseCompetitionEngineResponse({
        ok: true, operation: 'clockView', public: display, serverState: request.serverState,
      })
      expect(projectCompetitionResponse(raw)).toStrictEqual({ ok: true, operation: 'clockView', public: display })
    }
  }
})

test('E6e1 clockView does not synthesize an omitted legacy matches checkpoint', async () => {
  const state = { registration: null, clock: { lastObservedMs: 0 }, tournament: null }
  const display = {
    sampledAtMs: 1,
    teams: ['a', 'b'].map(team => ({ team, mainTimeRemainingMs: 100, byoyomiTimeRemainingMs: 30, byoyomiActive: false })),
    running: null, paused: null, awaitingAuthority: false,
  }
  const raw = await callCompetitionEngine({
    ...request, operation: { type: 'clockView' }, serverState: state,
  }, async () => ({ ok: true, operation: 'clockView', public: display, serverState: state }))
  expect(raw.ok).toBe(true)
  if (raw.ok) expect(raw.serverState).toStrictEqual(state)
})

test('E6e1 clockView has a closed operation name and preserves caller input across repeated query', async () => {
  for (const operation of ['clock_view', 'clockview', 'clockView ', 'clockDisplayRaw',
    'clock_pause', 'clock_resume', 'clockPause ', 'clockResumeUnexpected']) {
    expect(() => parseCompetitionEngineResponse({
      ok: true, operation, public: {}, serverState: request.serverState,
    })).toThrow('operation is invalid')
  }
  const display = {
    sampledAtMs: 110,
    teams: [
      { team: 'a', mainTimeRemainingMs: 0, byoyomiTimeRemainingMs: 0, byoyomiActive: true },
      { team: 'b', mainTimeRemainingMs: 100, byoyomiTimeRemainingMs: 30, byoyomiActive: false },
    ],
    running: { type: 'action', team: 'a', turn: 1 }, paused: null, awaitingAuthority: true,
  }
  for (const type of ['player', 'referee', 'organizer'] as const) {
    const query: CompetitionRequest = {
      ...structuredClone(request), serverActor: { type, playerId: `${type}-1` },
      trustedTimeMs: 110, operation: { type: 'clockView' },
    }
    const before = structuredClone(query)
    for (let retry = 0; retry < 2; retry++) {
      const raw = await callCompetitionEngine(query, async input => {
        expect(input).toStrictEqual(before)
        return { ok: true, operation: 'clockView', public: display, serverState: input.serverState }
      })
      expect(raw.ok).toBe(true)
      if (raw.ok) expect(raw.serverState).toStrictEqual(before.serverState)
      expect(projectCompetitionResponse(raw)).toStrictEqual({ ok: true, operation: 'clockView', public: display })
      expect(query).toStrictEqual(before)
    }
  }
})

const clockDisplayWasmPath = process.env.FEWC_COMPETITION_TEST_WASM

test('E6e2a metadata parser accepts only the exact typed deadline and preserves legacy state', () => {
  const state = { registration: { commands: [] }, clock: { lastObservedMs: 3000 }, tournament: null }
  for (const nextTimeoutDeadlineMs of [null, 0, 4100, Number.MAX_SAFE_INTEGER]) {
    const raw = parseCompetitionEngineResponse({
      ok: true, operation: 'clockCheckpointMetadata', public: { nextTimeoutDeadlineMs }, serverState: state,
    })
    expect(raw.ok).toBe(true)
    if (!raw.ok) throw new Error('metadata parse 失敗')
    expect(raw.serverState).toStrictEqual(state)
    expect(projectCompetitionResponse(raw, { type: 'authorizedPrivate' })).toStrictEqual({
      ok: true, operation: 'clockCheckpointMetadata', public: { nextTimeoutDeadlineMs },
    })
    expect(projectCompetitionResponse(raw)).not.toHaveProperty('serverState')
    expect(projectCompetitionResponse(raw)).not.toHaveProperty('private')
  }
  for (const name of ['clock_checkpoint_metadata', 'clockCheckpointMetadata ', 'clockcheckpointmetadata', 'clockMetadataRaw']) {
    expect(() => parseCompetitionEngineResponse({
      ok: true, operation: name, public: {}, serverState: state,
    })).toThrow('operation is invalid')
  }
})

test('E6e2a metadata rejects unknown, private, malformed and symbol response fields', () => {
  const valid = {
    ok: true, operation: 'clockCheckpointMetadata', public: { nextTimeoutDeadlineMs: 4100 },
    serverState: emptyCompetitionServerState(),
  }
  const publicValues: unknown[] = [
    {}, { next_timeout_deadline_ms: 4100 },
    ...[-1, 0.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1, '4100', false, {}, []]
      .map(nextTimeoutDeadlineMs => ({ nextTimeoutDeadlineMs })),
    ...['private', 'ruling', 'winner', 'sampledAtMs', 'clock', 'binding', 'serverState']
      .map(field => ({ nextTimeoutDeadlineMs: 4100, [field]: 'private-sentinel' })),
    Object.defineProperty({ nextTimeoutDeadlineMs: 4100 }, Symbol('unknown'), { value: true }),
  ]
  for (const publicValue of publicValues) {
    expect(() => parseCompetitionEngineResponse({ ...valid, public: publicValue })).toThrow()
  }
  for (const field of ['private', 'binding', 'deckSeed', 'unknown']) {
    expect(() => parseCompetitionEngineResponse({ ...valid, [field]: 'private-sentinel' })).toThrow()
  }
  expect(() => parseCompetitionEngineResponse(Object.defineProperty(
    { ...valid }, Symbol.dispose, { value: () => {} },
  ))).toThrow()
  for (const serverState of [
    { ...valid.serverState, unknown: null },
    { ...valid.serverState, matches: [] },
    { ...valid.serverState, clock: 'invalid' },
    Object.defineProperty({ ...valid.serverState }, Symbol('unknown'), { value: true }),
    { ...valid.serverState, clock: Object.defineProperty({ lastObservedMs: 3000 }, Symbol('unknown'), { value: true }) },
  ]) {
    expect(() => parseCompetitionEngineResponse({ ...valid, serverState })).toThrow()
  }
})

test('E6e2a metadata request extras are rejected before any WASM dispatch', async () => {
  let calls = 0
  for (const operation of [
    ...['trustedTimeMs', 'nextTimeoutDeadlineMs', 'clock', 'config', 'serverState', 'private', 'next_timeout_deadline_ms']
      .map(field => ({ type: 'clockCheckpointMetadata', [field]: 1 })),
    Object.defineProperty({ type: 'clockCheckpointMetadata' }, Symbol('unknown'), { value: true }),
  ]) {
    const query: CompetitionRequest = {
      ...request, operation: { type: 'clockCheckpointMetadata' },
    }
    Reflect.set(query, 'operation', operation)
    await expect(callCompetitionEngine(query, async () => { calls++; return null })).rejects.toThrow('metadata request')
  }
  const query: CompetitionRequest = { ...structuredClone(request), operation: { type: 'clockCheckpointMetadata' } }
  Reflect.set(query.serverState, 'unknown', null)
  await expect(callCompetitionEngine(query, async () => { calls++; return null })).rejects.toThrow('metadata request')
  expect(calls).toBe(0)
})

test('E6e2a metadata rejects wrong operation, checkpoint changes and malformed failures safely', async () => {
  const query: CompetitionRequest = {
    ...structuredClone(request), operation: { type: 'clockCheckpointMetadata' },
  }
  const before = structuredClone(query)
  for (const response of [
    { ok: true, operation: 'clockObserve', public: {}, serverState: query.serverState },
    { ok: true, operation: 'clockCheckpointMetadata', public: { nextTimeoutDeadlineMs: 4100 },
      serverState: { ...query.serverState, clock: { lastObservedMs: 9999 } } },
    { ok: false, error: { code: 'clockError', layer: 'validation', message: 'secret', private: 'secret' } },
    { ok: false, error: { code: 'clockError', layer: 'validation', message: 'secret' }, private: 'secret' },
    Object.defineProperty({ ok: false, error: { code: 'clockError', layer: 'validation', message: 'secret' } }, Symbol('unknown'), { value: true }),
  ]) {
    await expect(callCompetitionEngine(query, async () => response)).rejects.toThrow()
    expect(query).toStrictEqual(before)
  }
  await expect(callCompetitionEngine(query, async input => {
    input.serverState.clock = { lastObservedMs: 9999 }
    return { ok: true, operation: 'clockCheckpointMetadata', public: { nextTimeoutDeadlineMs: 4100 }, serverState: input.serverState }
  })).rejects.toThrow('changed checkpoint')
  expect(query).toStrictEqual(before)
  expect(await callCompetitionEngine(query, async () => ({
    ok: false, error: { code: 'clockError', layer: 'validation', message: 'checkpoint rejected' },
  }))).toStrictEqual({ ok: false, error: { code: 'clockError', layer: 'validation', message: 'checkpoint rejected' } })
})

async function actualClockDisplayWasm() {
  if (!clockDisplayWasmPath) throw new Error('FEWC_COMPETITION_TEST_WASM is required')
  const module = new WebAssembly.Module(await Bun.file(clockDisplayWasmPath).arrayBuffer())
  const instance = new WebAssembly.Instance(module, {})
  const { memory, fewfc_alloc: alloc, fewfc_dealloc: dealloc, fewfc_handle_competition_request: invoke } = instance.exports
  if (!(memory instanceof WebAssembly.Memory)
    || typeof alloc !== 'function' || typeof dealloc !== 'function' || typeof invoke !== 'function') {
    throw new Error('formal competition WASM exports are missing')
  }
  return async (input: unknown): Promise<unknown> => {
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
      const json = new TextDecoder().decode(new Uint8Array(memory.buffer, outputPtr, outputLength))
      const value: unknown = JSON.parse(json)
      return value
    } finally {
      dealloc(outputPtr, outputLength)
    }
  }
}

test.skipIf(!clockDisplayWasmPath)('E6a3a formal WASM rules settings cross the closed bridge without checkpoint writes', async () => {
  const wasm = await actualClockDisplayWasm()
  // 先使用既有 ABI 的 unknown JSON seam，不以尚未存在的新 TS type 製造編譯失敗。
  const input = {
    serverActor: { type: 'organizer', playerId: 'organizer' }, trustedTimeMs: 123,
    operation: { type: 'registrationResolveSettings', ruleVersion: '5.17', ruleset: 'base',
      enabledRuleModules: [], teamSize: 1 },
    serverState: emptyCompetitionServerState(),
  }
  const before = structuredClone(input)
  const raw = await wasm(input)
  expect(raw).toMatchObject({ ok: true, operation: 'registrationResolveSettings' })
  const response = parseCompetitionEngineResponse(raw)
  expect(response.ok).toBe(true)
  if (!response.ok || !isJsonObject(response.public)) throw new Error('正式 settings query 失敗')
  expect(Object.keys(response.public)).toEqual(['initialHp'])
  expect(response.public.initialHp).toBeGreaterThan(0)
  expect(response.serverState).toStrictEqual(input.serverState)
  expect(input).toStrictEqual(before)
  expect(projectCompetitionResponse(response)).toStrictEqual({
    ok: true, operation: 'registrationResolveSettings', public: response.public,
  })
})

test('E6a3a settings response rejects invalid HP, extra private fields and non-JSON keys', () => {
  const valid = { ok: true, operation: 'registrationResolveSettings', public: { initialHp: 1 },
    serverState: emptyCompetitionServerState() }
  for (const initialHp of [0, -1, 0.5, 2147483648, NaN, Infinity, '100', null]) {
    expect(() => parseCompetitionEngineResponse({ ...valid, public: { initialHp } })).toThrow()
  }
  for (const invalid of [
    { ...valid, public: { initialHp: 1, private: 'secret' } },
    { ...valid, public: { initial_hp: 1 } },
    { ...valid, private: { secret: 'cards' } }, { ...valid, extra: true },
    { ...valid, serverState: { ...valid.serverState, extra: true } },
    Object.defineProperty({ ...valid }, Symbol('unknown'), { value: true }),
    { ...valid, public: Object.defineProperty({ initialHp: 1 }, Symbol('unknown'), { value: true }) },
    { ...valid, public: Object.defineProperty({}, 'initialHp', { enumerable: true, get: () => 1 }) },
  ]) {
    expect(() => parseCompetitionEngineResponse(invalid)).toThrow()
  }
  expect(parseCompetitionEngineResponse(valid)).toStrictEqual(valid)
})

test('E6a3a settings query rejects request extras before WASM and rejects checkpoint mutation', async () => {
  const query: CompetitionRequest = {
    serverActor: { type: 'organizer', playerId: 'organizer' }, trustedTimeMs: 0,
    operation: { type: 'registrationResolveSettings', ruleVersion: '5.17', ruleset: 'base',
      enabledRuleModules: [], teamSize: 1 },
    serverState: emptyCompetitionServerState(),
  }
  const before = structuredClone(query)
  for (const invalid of [
    { ...query, operation: { ...query.operation, initialHp: 100 } },
    { ...query, operation: { ...query.operation, organizer: 'spoof' } },
    Object.defineProperty({ ...query }, Symbol('unknown'), { value: true }),
    { ...query, serverState: { ...query.serverState, extra: true } },
    { ...query, trustedTimeMs: -1 },
  ]) {
    let dispatched = false
    await expect(callCompetitionEngine(invalid, async () => {
      dispatched = true
      return { ok: true, operation: 'registrationResolveSettings', public: { initialHp: 1 }, serverState: query.serverState }
    })).rejects.toThrow()
    expect(dispatched).toBe(false)
  }
  for (const invalid of [
    { ok: true, operation: 'registrationView', public: {}, serverState: query.serverState },
    { ok: true, operation: 'registrationResolveSettings', public: { initialHp: 1 },
      serverState: { ...query.serverState, registration: { secret: true } } },
    { ok: false, error: { code: 'registrationError', layer: 'registration', message: 'private', extra: true } },
    { ok: false, error: { code: 'registrationError', layer: 'registration', message: 'private' }, extra: true },
  ]) await expect(callCompetitionEngine(query, async () => invalid)).rejects.toThrow()
  await expect(callCompetitionEngine(query, async input => {
    input.serverState.registration = { secret: true }
    return { ok: true, operation: 'registrationResolveSettings', public: { initialHp: 1 }, serverState: input.serverState }
  })).rejects.toThrow()
  expect(query).toStrictEqual(before)
})

test.skipIf(!clockDisplayWasmPath)('E6e2a formal metadata reads the checkpoint deadline without changing other domains', async () => {
  const wasm = await actualClockDisplayWasm()
  const created = await callCompetitionEngine({
    serverActor: { type: 'organizer', playerId: 'organizer' }, trustedTimeMs: 3000,
    operation: { type: 'clockCreate', teams: ['a', 'b'], config: {
      mainTimeMs: 1000, actionTimeMs: 100, outOfTurnChoiceTimeMs: 20,
    } }, serverState: emptyCompetitionServerState(),
  }, wasm)
  if (!created.ok) throw new Error('正式 metadata clockCreate 失敗')
  const started = await callCompetitionEngine({
    serverActor: { type: 'organizer', playerId: 'organizer' }, trustedTimeMs: 3000,
    operation: { type: 'clockBeginAction', team: 'a', turn: 1 }, serverState: created.serverState,
  }, wasm)
  if (!started.ok) throw new Error('正式 metadata clockBeginAction 失敗')
  expect(started.public).toMatchObject({ clock: { nextTimeoutDeadlineMs: 4100 } })
  const state = structuredClone(started.serverState)
  for (const type of ['organizer', 'player', 'referee'] as const) {
    for (const time of [3000, 4000, 10000]) {
      const query: CompetitionRequest = {
        serverActor: { type, playerId: `${type}-reader` }, trustedTimeMs: time,
        operation: { type: 'clockCheckpointMetadata' }, serverState: structuredClone(state),
      }
      for (const engine of [wasm, await actualClockDisplayWasm()]) {
        const response = await callCompetitionEngine(query, engine)
        expect(response.ok).toBe(true)
        if (!response.ok) throw new Error('正式 metadata query 失敗')
        expect(response.public).toStrictEqual({ nextTimeoutDeadlineMs: 4100 })
        expect(response.serverState).toStrictEqual(state)
        expect(query.serverState).toStrictEqual(state)
        expect(projectCompetitionResponse(response)).toStrictEqual({
          ok: true, operation: 'clockCheckpointMetadata', public: { nextTimeoutDeadlineMs: 4100 },
        })
        expect(Object.keys(response).sort()).toEqual(['ok', 'operation', 'public', 'serverState'])
      }
    }
  }
})

test.skipIf(!clockDisplayWasmPath)('E6e2a formal metadata covers windows pause reset carry terminal zero and null', async () => {
  const wasm = await actualClockDisplayWasm()
  const official = { type: 'organizer', playerId: 'organizer' } as const
  async function execute(operation: CompetitionRequest['operation'], time: number,
    state: CompetitionRequest['serverState']) {
    const response = await callCompetitionEngine({ serverActor: official, trustedTimeMs: time,
      operation, serverState: structuredClone(state) }, wasm)
    if (!response.ok) throw new Error(`正式 metadata prefix ${operation.type} 失敗`)
    return response.serverState
  }
  async function check(state: CompetitionRequest['serverState'], deadline: number | null) {
    const before = structuredClone(state)
    for (const engine of [wasm, await actualClockDisplayWasm()]) {
      for (const time of [0, 4000, 10000]) {
        const response = await callCompetitionEngine({ serverActor: official, trustedTimeMs: time,
          operation: { type: 'clockCheckpointMetadata' }, serverState: state }, engine)
        if (!response.ok) throw new Error('正式 metadata query 失敗')
        expect(response.public).toStrictEqual({ nextTimeoutDeadlineMs: deadline })
        expect(response.serverState).toStrictEqual(before)
        expect(state).toStrictEqual(before)
      }
    }
  }
  const registration = await execute({ type: 'registrationCreate', announcement: {
    organizer: 'organizer', ruleVersion: '5.17', ruleset: 'base', enabledRuleModules: [],
    teamSize: 1, initialHp: 30, registrationMode: 'public', personalDecks: 'disabled',
    registrationDeadline: 60_000, checkInDeadline: 120_000,
    plan: {
      initialSeedPolicy: 'randomDrawAfterRegistrationDeadline',
      swissPairingPolicy: 'publishedRulesWithPersistedRandomSeed',
      rankingPolicy: 'matchPointsThenOpponentWinsThenOpponentLosses',
      exactRankingTiePolicy: 'organizerOrUnconflictedRefereeWithPublicReason',
      stages: [{ stageId: 'stage-1', format: 'roundRobin', matchFormat: { type: 'fixedGames', value: 1 },
        clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
        swissRounds: null, swissPolicy: null, qualification: { type: 'allEligibleTeams' } }],
    },
  } }, 0, emptyCompetitionServerState())
  const idle = await execute({ type: 'clockCreate', teams: ['a', 'b'], config: {
    mainTimeMs: 1000, actionTimeMs: 100, outOfTurnChoiceTimeMs: 20,
  } }, 3000, registration)
  await check(idle, null)
  const prep = await execute({ type: 'clockBeginPreparation' }, 3000, idle)
  await check(prep, 4100)
  await check(await execute({ type: 'clockFinishPreparation', team: 'a' }, 3100, prep), 4100)
  const action = await execute({ type: 'clockBeginAction', team: 'a', turn: 1 }, 3000, idle)
  await check(action, 4100)
  await check(await execute({ type: 'clockBeginOutOfTurnChoice', team: 'b', choiceId: 'choice' }, 3000, idle), 3020)
  const paused = await execute({ type: 'clockPause', reason: 'metadata 暫停' }, 3100, action)
  await check(paused, null)
  await check(await execute({ type: 'clockResume', reason: 'metadata 恢復' }, 4000, paused), 5000)
  const ended = await execute({ type: 'clockEndWindow' }, 3100, action)
  await check(ended, null)
  for (const time of ['reset', 'carryForward'] as const) {
    await check(await execute({ type: 'clockNewGame', time }, 5000, ended), null)
  }
  await check(await execute({ type: 'clockObserve' }, 4100, action), null)
  for (const config of [
    { mainTimeMs: 1000, actionTimeMs: 0, outOfTurnChoiceTimeMs: 20 },
    { mainTimeMs: null, actionTimeMs: null, outOfTurnChoiceTimeMs: null },
  ]) {
    const created = await execute({ type: 'clockCreate', teams: ['a', 'b'], config }, 3000, registration)
    await check(await execute({ type: 'clockBeginAction', team: 'a', turn: 1 }, 3000, created),
      config.mainTimeMs === null ? null : 4000)
  }
  const missing = await callCompetitionEngine({ serverActor: official, trustedTimeMs: 0,
    operation: { type: 'clockCheckpointMetadata' }, serverState: emptyCompetitionServerState() }, wasm)
  expect(missing).toMatchObject({ ok: false, error: { code: 'missingState' } })
  expect(missing).not.toHaveProperty('serverState')
  const malformed = structuredClone(idle)
  if (!malformed.clock) throw new Error('正式 clock checkpoint 缺失')
  malformed.clock.version = 999
  const rejected = await callCompetitionEngine({ serverActor: official, trustedTimeMs: 0,
    operation: { type: 'clockCheckpointMetadata' }, serverState: malformed }, wasm)
  expect(rejected).toMatchObject({ ok: false, error: { code: 'clockError' } })
  expect(rejected).not.toHaveProperty('public')
})

test.skipIf(!clockDisplayWasmPath)('E4d3c formal WASM pause and resume clock windows cross the production bridge', async () => {
  const wasm = await actualClockDisplayWasm()
  const actor = { type: 'organizer', playerId: 'organizer' } as const
  async function executeWithFormalWasm(
    engine: (input: unknown) => Promise<unknown>,
    input: CompetitionRequest,
  ): Promise<{ raw: unknown; response: CompetitionEngineResponse }> {
    let raw: unknown
    const response = await callCompetitionEngine(input, async request => {
      // 每次都把請求交給實際 release WASM；此包裝只保留 ABI 原值供斷言，不替代時計結果。
      raw = await engine(request)
      return raw
    })
    return { raw, response }
  }
  function expectRawOperation(raw: unknown, operation: string): void {
    if (!isJsonObject(raw)) throw new Error('正式 WASM 回應不是 JSON object')
    expect(raw).toMatchObject({ ok: true, operation })
    expect(Object.keys(raw).sort()).toEqual(['ok', 'operation', 'public', 'serverState'])
    expect(Object.hasOwn(raw, 'private')).toBe(false)
  }
  function expectOtherDomainsUnchanged(
    before: CompetitionRequest['serverState'],
    after: CompetitionRequest['serverState'],
  ): void {
    expect(after.registration).toStrictEqual(before.registration)
    expect(after.tournament).toStrictEqual(before.tournament)
    expect(after.matches).toStrictEqual(before.matches)
  }

  const created = await executeWithFormalWasm(wasm, {
    serverActor: actor, trustedTimeMs: 0, serverState: emptyCompetitionServerState(),
    operation: { type: 'clockCreate', teams: ['a', 'b'], config: {
      mainTimeMs: 1000, actionTimeMs: 30, outOfTurnChoiceTimeMs: 20,
    } },
  })
  expectRawOperation(created.raw, 'clockCreate')
  expect(created.response.ok).toBe(true)
  if (!created.response.ok) throw new Error('正式 clockCreate 經 bridge 失敗')

  const begun = await executeWithFormalWasm(wasm, {
    serverActor: actor, trustedTimeMs: 0, serverState: structuredClone(created.response.serverState),
    operation: { type: 'clockBeginAction', team: 'a', turn: 1 },
  })
  expectRawOperation(begun.raw, 'clockBeginAction')
  expect(begun.response.ok).toBe(true)
  if (!begun.response.ok) throw new Error('正式 clockBeginAction 經 bridge 失敗')
  expectOtherDomainsUnchanged(created.response.serverState, begun.response.serverState)

  const pauseRequest: CompetitionRequest = {
    serverActor: actor, trustedTimeMs: 40, serverState: structuredClone(begun.response.serverState),
    operation: { type: 'clockPause', reason: '主審橋接回歸' },
  }
  const paused = await executeWithFormalWasm(wasm, pauseRequest)
  expectRawOperation(paused.raw, 'clockPause')
  expect(paused.response.ok).toBe(true)
  if (!paused.response.ok) throw new Error('正式 clockPause 經 bridge 失敗')
  expect(paused.response.operation).toBe('clockPause')
  expect(paused.response.public).toMatchObject({
    result: { type: 'paused' },
    clock: {
      activeWindow: null,
      pausedWindow: { type: 'action', team: 'a', turn: 1 },
      lastObservedMs: 40,
      nextTimeoutDeadlineMs: null,
    },
  })
  expect(paused.response.serverState.clock).toMatchObject({
    running: null,
    paused: { type: 'action', team: 'a', turn: 1 },
    lastObservedMs: 40,
    teams: [
      { team: 'a', mainTimeRemainingMs: 960 },
      { team: 'b', mainTimeRemainingMs: 1000 },
    ],
  })
  expectOtherDomainsUnchanged(begun.response.serverState, paused.response.serverState)
  expect(projectCompetitionResponse(paused.response)).toStrictEqual({
    ok: true, operation: 'clockPause', public: paused.response.public,
  })

  const pausedCheckpoint = structuredClone(paused.response.serverState)
  const pausedQuery: CompetitionRequest = {
    serverActor: actor, trustedTimeMs: 1000, serverState: structuredClone(pausedCheckpoint),
    operation: { type: 'clockView' },
  }
  const pausedRead = await executeWithFormalWasm(wasm, pausedQuery)
  expectRawOperation(pausedRead.raw, 'clockView')
  expect(pausedRead.response.ok).toBe(true)
  if (!pausedRead.response.ok) throw new Error('paused clockView 經 bridge 失敗')
  expect(pausedRead.response.public).toStrictEqual({
    sampledAtMs: 1000,
    teams: [
      { team: 'a', mainTimeRemainingMs: 960, byoyomiTimeRemainingMs: 30, byoyomiActive: false },
      { team: 'b', mainTimeRemainingMs: 1000, byoyomiTimeRemainingMs: 30, byoyomiActive: false },
    ],
    running: null,
    paused: { type: 'action', team: 'a', turn: 1 },
    awaitingAuthority: false,
  })
  expect(pausedRead.response.serverState).toStrictEqual(pausedCheckpoint)
  expect(pausedQuery.serverState).toStrictEqual(pausedCheckpoint)
  const restartedPausedRead = await executeWithFormalWasm(await actualClockDisplayWasm(), pausedQuery)
  expect(restartedPausedRead.response).toStrictEqual(pausedRead.response)

  const resumeRequest: CompetitionRequest = {
    serverActor: actor, trustedTimeMs: 2000, serverState: structuredClone(pausedCheckpoint),
    operation: { type: 'clockResume', reason: '主審橋接回歸完成' },
  }
  const resumed = await executeWithFormalWasm(await actualClockDisplayWasm(), resumeRequest)
  expectRawOperation(resumed.raw, 'clockResume')
  expect(resumed.response.ok).toBe(true)
  if (!resumed.response.ok) throw new Error('正式 clockResume 經 bridge 失敗')
  expect(resumed.response.operation).toBe('clockResume')
  expect(resumed.response.public).toMatchObject({
    result: { type: 'resumed' },
    clock: {
      activeWindow: { type: 'action', team: 'a', turn: 1 },
      pausedWindow: null,
      lastObservedMs: 2000,
      nextTimeoutDeadlineMs: 2990,
    },
  })
  expect(resumed.response.serverState.clock).toMatchObject({
    running: { type: 'action', team: 'a', turn: 1 },
    paused: null,
    lastObservedMs: 2000,
    teams: [
      { team: 'a', mainTimeRemainingMs: 960 },
      { team: 'b', mainTimeRemainingMs: 1000 },
    ],
  })
  expectOtherDomainsUnchanged(pausedCheckpoint, resumed.response.serverState)
  expect(projectCompetitionResponse(resumed.response)).toStrictEqual({
    ok: true, operation: 'clockResume', public: resumed.response.public,
  })

  const resumedCheckpoint = structuredClone(resumed.response.serverState)
  const resumedQuery: CompetitionRequest = {
    serverActor: actor, trustedTimeMs: 2040, serverState: structuredClone(resumedCheckpoint),
    operation: { type: 'clockView' },
  }
  const resumedRead = await executeWithFormalWasm(await actualClockDisplayWasm(), resumedQuery)
  expectRawOperation(resumedRead.raw, 'clockView')
  expect(resumedRead.response.ok).toBe(true)
  if (!resumedRead.response.ok) throw new Error('resumed clockView 經 bridge 失敗')
  expect(resumedRead.response.public).toStrictEqual({
    sampledAtMs: 2040,
    teams: [
      { team: 'a', mainTimeRemainingMs: 920, byoyomiTimeRemainingMs: 30, byoyomiActive: false },
      { team: 'b', mainTimeRemainingMs: 1000, byoyomiTimeRemainingMs: 30, byoyomiActive: false },
    ],
    running: { type: 'action', team: 'a', turn: 1 },
    paused: null,
    awaitingAuthority: false,
  })
  expect(resumedRead.response.serverState).toStrictEqual(resumedCheckpoint)
  expect(resumedQuery.serverState).toStrictEqual(resumedCheckpoint)
})

test.skipIf(!clockDisplayWasmPath)('E6e1 formal WASM clockView is pure through deadline, repeated queries and restart', async () => {
  const wasm = await actualClockDisplayWasm()
  const created = await callCompetitionEngine({
    serverActor: { type: 'organizer', playerId: 'organizer' }, trustedTimeMs: 0,
    operation: { type: 'clockCreate', teams: ['a', 'b'], config: {
      mainTimeMs: 100, actionTimeMs: 30, outOfTurnChoiceTimeMs: 20,
    } },
    serverState: emptyCompetitionServerState(),
  }, wasm)
  expect(created.ok).toBe(true)
  if (!created.ok) throw new Error('formal clockCreate failed')
  const prepared = await callCompetitionEngine({
    serverActor: { type: 'organizer', playerId: 'organizer' }, trustedTimeMs: 0,
    operation: { type: 'clockBeginPreparation' }, serverState: created.serverState,
  }, wasm)
  expect(prepared.ok).toBe(true)
  if (!prepared.ok) throw new Error('formal clockBeginPreparation failed')
  const checkpoint = structuredClone(prepared.serverState)
  for (const sampledAtMs of [67, 110, 130, 1000]) {
    const expectedMain = sampledAtMs === 67 ? 33 : 0
    const expectedByoyomi = sampledAtMs === 67 ? 30 : sampledAtMs === 110 ? 20 : 0
    const query: CompetitionRequest = {
      serverActor: { type: 'player', playerId: 'reader' }, trustedTimeMs: sampledAtMs,
      operation: { type: 'clockView' }, serverState: structuredClone(checkpoint),
    }
    const expected = {
      ok: true, operation: 'clockView', public: {
        sampledAtMs,
        teams: ['a', 'b'].map(team => ({ team, mainTimeRemainingMs: expectedMain,
          byoyomiTimeRemainingMs: expectedByoyomi, byoyomiActive: sampledAtMs >= 100 })),
        running: { type: 'preparation', teams: ['a', 'b'] }, paused: null,
        awaitingAuthority: sampledAtMs >= 130,
      },
    }
    for (const engine of [wasm, await actualClockDisplayWasm()]) {
      const raw = await callCompetitionEngine(query, engine)
      expect(raw.ok).toBe(true)
      if (raw.ok) expect(raw.serverState).toStrictEqual(checkpoint)
      expect(projectCompetitionResponse(raw)).toStrictEqual(expected)
      expect(query.serverState).toStrictEqual(checkpoint)
    }
  }
})

test.skipIf(!clockDisplayWasmPath)('E6e1 formal WASM positive main zero action waits at exact deadline only while running', async () => {
  const wasm = await actualClockDisplayWasm()
  const created = await callCompetitionEngine({
    serverActor: { type: 'organizer', playerId: 'organizer' }, trustedTimeMs: 0,
    operation: { type: 'clockCreate', teams: ['a', 'b'], config: {
      mainTimeMs: 100, actionTimeMs: 0, outOfTurnChoiceTimeMs: 20,
    } }, serverState: emptyCompetitionServerState(),
  }, wasm)
  expect(created.ok).toBe(true)
  if (!created.ok) throw new Error('formal zero-action clockCreate failed')
  const started = await callCompetitionEngine({
    serverActor: { type: 'organizer', playerId: 'organizer' }, trustedTimeMs: 0,
    operation: { type: 'clockBeginAction', team: 'a', turn: 1 }, serverState: created.serverState,
  }, wasm)
  expect(started.ok).toBe(true)
  if (!started.ok) throw new Error('formal zero-action clockBeginAction failed')
  expect(started.public).toMatchObject({ clock: { nextTimeoutDeadlineMs: 100 } })
  const checkpoint = structuredClone(started.serverState)
  for (const [sampledAtMs, remaining, active, waiting] of [
    [99, 1, false, false], [100, 0, true, true], [101, 0, true, true],
  ] as const) {
    const query: CompetitionRequest = {
      serverActor: { type: 'player', playerId: 'reader' }, trustedTimeMs: sampledAtMs,
      operation: { type: 'clockView' }, serverState: structuredClone(checkpoint),
    }
    for (const engine of [wasm, await actualClockDisplayWasm()]) {
      for (let retry = 0; retry < 2; retry++) {
        const raw = await callCompetitionEngine(query, engine)
        expect(raw.ok).toBe(true)
        if (raw.ok) expect(raw.serverState).toStrictEqual(checkpoint)
        expect(projectCompetitionResponse(raw)).toStrictEqual({ ok: true, operation: 'clockView', public: {
          sampledAtMs,
          teams: [
            { team: 'a', mainTimeRemainingMs: remaining, byoyomiTimeRemainingMs: 0, byoyomiActive: active },
            { team: 'b', mainTimeRemainingMs: 100, byoyomiTimeRemainingMs: 0, byoyomiActive: false },
          ],
          running: { type: 'action', team: 'a', turn: 1 }, paused: null, awaitingAuthority: waiting,
        } })
        expect(query.serverState).toStrictEqual(checkpoint)
      }
    }
  }
  for (const operation of [
    { type: 'clockPause', reason: '裁判暫停' }, { type: 'clockEndWindow' },
  ] satisfies CompetitionRequest['operation'][]) {
    // 只用正式 mutation 建立 paused／idle，不手造 checkpoint 或虛構結果。
    const stopped = await callCompetitionEngine({
      serverActor: { type: 'organizer', playerId: 'organizer' }, trustedTimeMs: 99,
      operation, serverState: structuredClone(checkpoint),
    }, wasm)
    if (!stopped.ok) {
      throw new Error('formal zero-action pause/end failed')
    }
    // paused／idle 前綴亦經 production bridge；query 只取樣而不改寫正式 checkpoint。
    const stoppedState = stopped.serverState
    for (const sampledAtMs of [100, 101, 10_000]) {
      const raw = parseCompetitionEngineResponse(await wasm({
        serverActor: { type: 'player', playerId: 'reader' }, trustedTimeMs: sampledAtMs,
        operation: { type: 'clockView' }, serverState: stoppedState,
      }))
      expect(raw.ok).toBe(true)
      if (raw.ok) {
        expect(raw.serverState).toStrictEqual(stoppedState)
        expect(raw.public).toMatchObject({ awaitingAuthority: false, teams: [
          { team: 'a', mainTimeRemainingMs: 1, byoyomiTimeRemainingMs: 0 },
          { team: 'b', mainTimeRemainingMs: 100, byoyomiTimeRemainingMs: 0 },
        ] })
      }
    }
  }
})

test('E6d1a tournamentView recognizes the pure query and strips private checkpoints from its public projection', () => {
  const tournament = {
    tournamentId: 'current-event',
    stages: [{
      config: {
        stageId: 'stage-1', format: 'roundRobin', randomSeed: 7,
        teams: [{ teamId: 'team-a', seed: 1 }, { teamId: 'team-b', seed: 2 }],
        matchFormat: { type: 'fixedGames', value: 1 },
        clockConfig: { mainTimeMs: 300000, actionTimeMs: 30000, outOfTurnChoiceTimeMs: 30000 },
        swissRounds: null, swissPolicy: null,
      },
      status: 'finished',
      rounds: [{ roundNumber: 1, kind: 'main', matches: [{
        matchId: 'stage-1-g0-r1-m1', stageId: 'stage-1', roundNumber: 1,
        sequence: 1, kind: 'main', participants: ['team-a', 'team-b'],
        status: 'completed', started: true, cancellationReason: null,
        result: { type: 'win', winnerId: 'team-a', loserId: 'team-b', winnerGames: 1, loserGames: 0 },
      }] }],
      standings: [
        { teamId: 'team-a', matchPoints: 3, wins: 1, draws: 0, losses: 0,
          gameWins: 1, gameLosses: 0, netGameWins: 1, opponentWinRateBasisPoints: null, dropped: false, rank: 1 },
        { teamId: 'team-b', matchPoints: 0, wins: 0, draws: 0, losses: 1,
          gameWins: 0, gameLosses: 1, netGameWins: -1, opponentWinRateBasisPoints: null, dropped: false, rank: 2 },
      ],
      corrections: [], lastScheduleError: null, scheduleGeneration: 0, supersededRounds: [],
      teams: { 'team-a': { seed: 1, dropped: false, hadBye: false }, 'team-b': { seed: 2, dropped: false, hadBye: false } },
      doubleState: null,
    }],
  }
  const raw = parseCompetitionEngineResponse({
    ok: true,
    operation: 'tournamentView',
    public: tournament,
    private: { cards: ['private-card'] },
    serverState: {
      registration: { commands: [{ action: { cards: ['private-card'] } }] },
      clock: { lastObservedMs: 456 },
      tournament,
      matches: { 'match-1': { history: [] } },
    },
    deckSeed: 123,
  })
  const projected = projectCompetitionResponse(raw)
  expect(projected).toStrictEqual({ ok: true, operation: 'tournamentView', public: tournament })
  expect(JSON.stringify(projected)).not.toContain('private-card')
  expect(JSON.stringify(projected)).not.toMatch(/"(?:private|serverState|registration|clock|deckSeed)"\s*:/)
  expect(raw.ok).toBe(true)
  if (raw.ok) expect(raw.serverState.tournament).toStrictEqual(tournament)
})

test('E6d1a tournamentView remains a closed operation name rather than an arbitrary query string', () => {
  for (const operation of ['tournament_view', 'tournamentview', 'tournamentView ', 'tournamentRawView']) {
    expect(() => parseCompetitionEngineResponse({
      ok: true, operation, public: {}, serverState: request.serverState,
    })).toThrow('operation is invalid')
  }
})

test('E6d1a tournamentView uses the same fieldless request for every trusted server actor', async () => {
  for (const type of ['player', 'referee', 'organizer'] as const) {
    const query: CompetitionRequest = {
      ...request,
      serverActor: { type, playerId: `${type}-1` },
      operation: { type: 'tournamentView' },
    }
    const before = structuredClone(query)
    const projected = await executeCompetitionOperation(query, { type: 'public' }, async input => {
      expect(input).toStrictEqual(before)
      expect(input.operation).toStrictEqual({ type: 'tournamentView' })
      return {
        ok: true, operation: 'tournamentView',
        public: { tournamentId: 'event-1', stages: [] },
        serverState: input.serverState,
      }
    })
    expect(projected).toStrictEqual({
      ok: true, operation: 'tournamentView', public: { tournamentId: 'event-1', stages: [] },
    })
    expect(query).toStrictEqual(before)
    expect(projected).not.toHaveProperty('serverState')
  }
})

test('callCompetitionEngine parses a successful WASM envelope without exposing it', async () => {
  let received: CompetitionRequest | undefined
  const response = await callCompetitionEngine(request, async input => {
    received = input
    return {
      ok: true,
      operation: 'tournamentSubmitResult',
      public: { result: { type: 'applied' } },
      private: { audit: { accepted: true } },
      serverState: input.serverState,
    }
  })

  expect(received).toStrictEqual(request)
  expect(response.ok).toBe(true)
  if (response.ok) {
    expect(response.serverState).toStrictEqual(request.serverState)
    expect(response.private).toStrictEqual({ audit: { accepted: true } })
  }
})

test('callCompetitionEngine preserves machine-readable Rust errors', async () => {
  const response = await callCompetitionEngine(request, async () => ({
    ok: false,
    error: {
      code: 'doubleLossNeedsReferee',
      layer: 'validation',
      message: '雙方判負已保留，淘汰賽後續需裁判處理',
    },
  }))

  expect(response).toStrictEqual({
    ok: false,
    error: {
      code: 'doubleLossNeedsReferee',
      layer: 'validation',
      message: '雙方判負已保留，淘汰賽後續需裁判處理',
    },
  })
})

test('partial schedule errors remain public while serverState stays private', async () => {
  const raw = partialScheduleSuccess()
  const publicProjection = projectCompetitionResponse(raw)
  const privateProjection = projectCompetitionResponse(raw, { type: 'authorizedPrivate' })

  expect(publicProjection).toStrictEqual({
    ok: true,
    operation: 'tournamentSubmitResult',
    public: raw.ok ? raw.public : null,
  })
  expect(JSON.stringify(publicProjection)).not.toContain('serverState')
  expect(JSON.stringify(publicProjection)).not.toContain('audit')
  expect(privateProjection).toStrictEqual(publicProjection)
})

test('authorized private projection includes private data but never the checkpoint', async () => {
  const raw = parseCompetitionEngineResponse({
    ok: true,
    operation: 'registrationView',
    public: { lifecycle: 'open', teams: [] },
    private: { ownTeam: { teamId: 'team-a', ownDeck: { cards: ['card-a'] } } },
    serverState: {
      registration: { commands: [] },
      clock: null,
      tournament: null,
    },
  })
  const projected = await executeCompetitionOperation(
    request,
    { type: 'authorizedPrivate' },
    async () => raw,
  )

  expect(projected).toStrictEqual({
    ok: true,
    operation: 'registrationView',
    public: { lifecycle: 'open', teams: [] },
    private: { ownTeam: { teamId: 'team-a', ownDeck: { cards: ['card-a'] } } },
  })
  expect(JSON.stringify(projected)).not.toContain('serverState')
  expect(JSON.stringify(projected)).not.toContain('commands')
})

test('invalid WASM response schema fails closed', () => {
  expect(() => parseCompetitionEngineResponse({
    ok: true,
    operation: 'tournamentSubmitResult',
    public: {},
    serverState: { registration: null, clock: null },
  })).toThrow('serverState')
})

test('actual match public JSON uses camelCase ruling and terminal evidence encodings', () => {
  const response = parseCompetitionEngineResponse({
    ok: true,
    operation: 'matchView',
    public: {
      match: {
        games: [{
          terminalEvidence: {
            type: 'canonicalGameEnded',
            conclusion: {
              outcome: { type: 'winner', team: 'team-a' },
              causes: [{ type: 'teamHpDepleted', teams: ['team-b'] }],
            },
          },
        }],
        rulings: [{
          ruling: { type: 'gameAccepted', outcome: { winner: 'team-a' } },
        }],
        corrections: [{
          replacementRuling: { type: 'gameAccepted', outcome: { winner: 'team-a' } },
        }],
      },
    },
    serverState: {
      registration: null,
      clock: null,
      tournament: null,
      matches: {
        'match-1': {
          config: { matchId: 'match-1' },
          history: [{ type: 'submitGame' }],
        },
      },
    },
  })

  expect(response.ok).toBe(true)
  if (response.ok) {
    expect(response.public).toMatchObject({
      match: {
        rulings: [{ ruling: { type: 'gameAccepted', outcome: { winner: 'team-a' } } }],
      },
    })
    expect(JSON.stringify(response.public)).not.toContain('Winner')
    expect(JSON.stringify(response.public)).not.toContain('TeamHpDepleted')
  }
})

test('clock pause and resume bridge requests use explicit camelCase operations and reason', () => {
  const pauseRequest: CompetitionRequest = {
    ...request,
    operation: { type: 'clockPause', reason: '裁判處理斷線' },
  }
  const resumeRequest: CompetitionRequest = {
    ...request,
    operation: { type: 'clockResume', reason: '裁判處理完成' },
  }
  const encoded = JSON.stringify({ pauseRequest, resumeRequest })
  expect(encoded).toContain('clockPause')
  expect(encoded).toContain('clockResume')
  expect(encoded).toContain('reason')
  expect(encoded).not.toContain('clock_pause')
})

const qualifiedStageBridgeSlots: StageQualificationSlot[] = [
  { sourceStageId: 'source-a', source: { type: 'finishedStageWinner' }, destinationSeed: 7 },
  { sourceStageId: 'source-b', source: { type: 'uniqueStandingRank', rank: 2 }, destinationSeed: 3 },
]

const qualifiedStageBridgeTemplates: QualifiedStageTemplate[] = [
  {
    stageId: 'next-single', format: 'singleElimination', randomSeed: 19,
    matchFormat: { type: 'firstToWins', value: 1 },
    clockConfig: { mainTimeMs: 300000, actionTimeMs: 30000, outOfTurnChoiceTimeMs: 30000 },
    swissRounds: null, swissPolicy: null, sourceSlots: qualifiedStageBridgeSlots,
  },
  {
    stageId: 'next-double', format: 'doubleElimination', randomSeed: 19,
    matchFormat: { type: 'firstToWins', value: 1 },
    clockConfig: { mainTimeMs: 300000, actionTimeMs: 30000, outOfTurnChoiceTimeMs: 30000 },
    swissRounds: null, swissPolicy: null, sourceSlots: qualifiedStageBridgeSlots,
  },
  {
    stageId: 'next-round-robin', format: 'roundRobin', randomSeed: 19,
    matchFormat: { type: 'fixedGames', value: 1 },
    clockConfig: { mainTimeMs: 300000, actionTimeMs: 30000, outOfTurnChoiceTimeMs: 30000 },
    swissRounds: null, swissPolicy: null, sourceSlots: qualifiedStageBridgeSlots,
  },
  {
    stageId: 'next-swiss', format: 'swiss', randomSeed: 19,
    matchFormat: { type: 'fixedGames', value: 1 },
    clockConfig: { mainTimeMs: 300000, actionTimeMs: 30000, outOfTurnChoiceTimeMs: 30000 },
    swissRounds: 1,
    swissPolicy: {
      matchPoints: { win: 3, draw: 1, loss: 0, bye: 3 },
      drawCredit: { numerator: 1, denominator: 2 },
      opponentWinRateFloorPercent: null,
      opponentWinRateCapPercent: null,
      excludeByesFromOpponentWinRate: false,
      tieBreakers: ['opponentWinRate'],
    },
    sourceSlots: qualifiedStageBridgeSlots,
  },
]

const qualifiedStageBridgeTournament: JsonObject = {
  tournamentId: 'event-1',
  stages: [{ config: { stageId: 'finished-stage' }, status: 'finished' }],
}

const qualifiedStageBridgeSeededTeams = [
  { teamId: 'team-a', seed: 7 }, { teamId: 'team-b', seed: 3 },
]

function qualifiedStageBridgeJsonObject(value: unknown): JsonObject {
  if (!isExactCompetitionJson(value) || !isJsonObject(value)) {
    throw new Error('qualified-stage fixture is not exact JSON')
  }
  return structuredClone(value)
}

const qualifiedStageBridgeQualification = qualifiedStageBridgeJsonObject({
  seededTeams: qualifiedStageBridgeSeededTeams,
  sourceMappings: [
    {
      sourceStageId: 'source-a', source: { type: 'finishedStageWinner' },
      teamId: 'team-a', destinationSeed: 7,
    },
    {
      sourceStageId: 'source-b', source: { type: 'uniqueStandingRank', rank: 2 },
      teamId: 'team-b', destinationSeed: 3,
    },
  ],
})

function qualifiedStageBridgeState(includeMatches = true): CompetitionRequest['serverState'] {
  return {
    registration: { untouched: { version: 4 } },
    clock: { untouched: { version: 9 } },
    tournament: structuredClone(qualifiedStageBridgeTournament),
    ...(includeMatches ? { matches: { 'match-a': { history: [{ type: 'untouched' }] } } } : {}),
  }
}

function qualifiedStageBridgeRequest(
  operationType: 'tournamentPreviewQualifiedStage' | 'tournamentCreateQualifiedStage',
  template = qualifiedStageBridgeTemplates[0]!,
  serverState = qualifiedStageBridgeState(),
): CompetitionRequest {
  return {
    serverActor: { type: 'organizer', playerId: 'organizer-1' },
    trustedTimeMs: 42,
    operation: { type: operationType, template: structuredClone(template) },
    serverState,
  }
}

function qualifiedStageBridgePlannedStage(template: QualifiedStageTemplate): JsonObject {
  return qualifiedStageBridgeJsonObject({
    config: {
      stageId: template.stageId,
      format: template.format,
      teams: structuredClone(qualifiedStageBridgeSeededTeams),
      randomSeed: template.randomSeed,
      matchFormat: structuredClone(template.matchFormat),
      clockConfig: structuredClone(template.clockConfig),
      swissRounds: template.swissRounds,
      swissPolicy: structuredClone(template.swissPolicy),
    },
    status: 'active', rounds: [], standings: [], corrections: [], lastScheduleError: null,
    scheduleGeneration: 0, supersededRounds: [],
    teams: {
      'team-a': { seed: 7, dropped: false, hadBye: false },
      'team-b': { seed: 3, dropped: false, hadBye: false },
    },
    doubleState: template.format === 'doubleElimination'
      ? {
          phase: 'winners', winnersPool: ['team-a', 'team-b'], losersWaiting: [], lowerPool: [],
          winnersChampion: null, losersChampion: null, grandFinalWinner: null,
        }
      : null,
  })
}

function qualifiedStageBridgeRawResponse(request: CompetitionRequest): {
  ok: true
  operation: string
  public: JsonObject
  serverState: CompetitionRequest['serverState']
} {
  const operation = request.operation
  if (operation.type !== 'tournamentPreviewQualifiedStage'
    && operation.type !== 'tournamentCreateQualifiedStage') {
    throw new Error('qualified-stage fixture received a different operation')
  }
  const template = operation.template
  const plannedStage = qualifiedStageBridgePlannedStage(template)
  const existingTournament = request.serverState.tournament
  if (!existingTournament || !Array.isArray(existingTournament.stages)) {
    throw new Error('qualified-stage fixture requires a tournament snapshot')
  }
  const currentTournament = qualifiedStageBridgeJsonObject({
    ...structuredClone(existingTournament),
    stages: [...existingTournament.stages, plannedStage],
  })
  const isCreate = operation.type === 'tournamentCreateQualifiedStage'
  const publicValue = qualifiedStageBridgeJsonObject(isCreate
    ? { qualification: structuredClone(qualifiedStageBridgeQualification), currentTournament }
    : { qualification: structuredClone(qualifiedStageBridgeQualification), plannedStage })
  return {
    ok: true,
    operation: operation.type,
    public: publicValue,
    serverState: isCreate
      ? { ...structuredClone(request.serverState), tournament: currentTournament }
      : structuredClone(request.serverState),
  }
}

test('qualified-stage TS bridge accepts exact preview and create envelopes for all four formats', async () => {
  const formats: TournamentFormat[] = [
    'singleElimination', 'doubleElimination', 'roundRobin', 'swiss',
  ]
  expect(qualifiedStageBridgeTemplates.map(template => template.format)).toEqual(formats)

  for (const template of qualifiedStageBridgeTemplates) {
    for (const operationType of ['tournamentPreviewQualifiedStage', 'tournamentCreateQualifiedStage'] as const) {
      const query = qualifiedStageBridgeRequest(operationType, template)
      const before = structuredClone(query)
      let dispatched = false
      const response = await callCompetitionEngine(query, async outbound => {
        dispatched = true
        expect(outbound).toStrictEqual(before)
        return qualifiedStageBridgeRawResponse(outbound)
      })

      expect(dispatched).toBe(true)
      expect(query).toStrictEqual(before)
      expect(response.ok).toBe(true)
      if (!response.ok) throw new Error('qualified-stage success fixture was rejected')
      expect(response.operation).toBe(operationType)
      expect(Object.keys(response).sort()).toEqual(['ok', 'operation', 'public', 'serverState'])
      expect(Object.hasOwn(response, 'private')).toBe(false)
      expect(projectCompetitionResponse(response)).toStrictEqual({
        ok: true, operation: operationType, public: response.public,
      })

      if (!isJsonObject(response.public)) throw new Error('qualified-stage public projection is not an object')
      expect(Object.keys(response.public).sort()).toEqual(
        operationType === 'tournamentPreviewQualifiedStage'
          ? ['plannedStage', 'qualification'] : ['currentTournament', 'qualification'],
      )
      expect(response.public.qualification).toStrictEqual(qualifiedStageBridgeQualification)
      const mappings = response.public.qualification
      if (!isJsonObject(mappings)) throw new Error('qualified-stage qualification is not an object')
      expect(mappings.seededTeams).toStrictEqual([
        { teamId: 'team-a', seed: 7 }, { teamId: 'team-b', seed: 3 },
      ])
      expect(mappings.sourceMappings).toStrictEqual([
        {
          sourceStageId: template.sourceSlots[0]!.sourceStageId,
          source: template.sourceSlots[0]!.source,
          teamId: 'team-a', destinationSeed: template.sourceSlots[0]!.destinationSeed,
        },
        {
          sourceStageId: template.sourceSlots[1]!.sourceStageId,
          source: template.sourceSlots[1]!.source,
          teamId: 'team-b', destinationSeed: template.sourceSlots[1]!.destinationSeed,
        },
      ])

      if (operationType === 'tournamentPreviewQualifiedStage') {
        expect(response.serverState).toStrictEqual(before.serverState)
        expect(response.public.plannedStage).toStrictEqual(qualifiedStageBridgePlannedStage(template))
        expect(response.public.plannedStage).toHaveProperty('teams')
        expect(response.public.plannedStage).toHaveProperty('doubleState')
      } else {
        expect(response.public.currentTournament).toStrictEqual(response.serverState.tournament)
        expect(response.serverState.registration).toStrictEqual(before.serverState.registration)
        expect(response.serverState.clock).toStrictEqual(before.serverState.clock)
        expect(response.serverState.matches).toStrictEqual(before.serverState.matches)
      }
    }
  }
})

test('qualified-stage TS bridge parser recognizes both exact Rust operation names', () => {
  for (const operationType of ['tournamentPreviewQualifiedStage', 'tournamentCreateQualifiedStage'] as const) {
    const request = qualifiedStageBridgeRequest(operationType)
    const raw = qualifiedStageBridgeRawResponse(request)
    const parsed = parseCompetitionEngineResponse(raw)
    expect(parsed.ok).toBe(true)
    if (!parsed.ok) throw new Error('qualified-stage success fixture was parsed as an error')
    expect(parsed.operation).toBe(operationType)
  }
})

test('qualified-stage TS bridge preserves preview and create checkpoint shape, including omitted matches', async () => {
  for (const operationType of ['tournamentPreviewQualifiedStage', 'tournamentCreateQualifiedStage'] as const) {
    for (const includeMatches of [true, false]) {
      const state = qualifiedStageBridgeState(includeMatches)
      const request = qualifiedStageBridgeRequest(operationType, qualifiedStageBridgeTemplates[0]!, state)
      const before = structuredClone(state)
      const response = await callCompetitionEngine(request, async outbound => qualifiedStageBridgeRawResponse(outbound))
      expect(response.ok).toBe(true)
      if (!response.ok) throw new Error('qualified-stage checkpoint fixture was rejected')
      expect(Object.hasOwn(response.serverState, 'matches')).toBe(includeMatches)
      expect(response.serverState.registration).toStrictEqual(before.registration)
      expect(response.serverState.clock).toStrictEqual(before.clock)
      expect(response.serverState.matches).toStrictEqual(before.matches)
      if (operationType === 'tournamentPreviewQualifiedStage') {
        expect(response.serverState).toStrictEqual(before)
      } else if (isJsonObject(response.public)) {
        expect(response.public.currentTournament).toStrictEqual(response.serverState.tournament)
      }
    }
  }
})

test('qualified-stage TS bridge rejects caller teams, proofs, snake case and unknown source before dispatch', async () => {
  const invalidRequests = [
    (() => {
      const value = qualifiedStageBridgeRequest('tournamentPreviewQualifiedStage')
      Reflect.set(value.operation, 'teams', [{ teamId: 'forged', seed: 1 }])
      return value
    })(),
    (() => {
      const value = qualifiedStageBridgeRequest('tournamentCreateQualifiedStage')
      Reflect.set(value.operation, 'rawPlan', {})
      return value
    })(),
    (() => {
      const value = qualifiedStageBridgeRequest('tournamentPreviewQualifiedStage')
      Reflect.set(value.operation, 'sourceProof', {})
      return value
    })(),
    (() => {
      const value = qualifiedStageBridgeRequest('tournamentCreateQualifiedStage')
      Reflect.set(value.operation.template, 'unrecognized', true)
      return value
    })(),
    (() => {
      const value = qualifiedStageBridgeRequest('tournamentCreateQualifiedStage')
      Reflect.set(value.operation.template, 'random_seed', 19)
      return value
    })(),
    (() => {
      const value = qualifiedStageBridgeRequest('tournamentPreviewQualifiedStage')
      Reflect.deleteProperty(value.operation.template, 'swissPolicy')
      return value
    })(),
    (() => {
      const value = qualifiedStageBridgeRequest('tournamentCreateQualifiedStage')
      Reflect.set(value.operation.template.sourceSlots[0]!, 'source', { type: 'callerTeam', teamId: 'forged' })
      return value
    })(),
    (() => {
      const value = qualifiedStageBridgeRequest('tournamentPreviewQualifiedStage')
      Reflect.set(value.operation.template.sourceSlots[0]!, 'source', { type: 'finishedStageWinner', extra: true })
      return value
    })(),
    (() => {
      const value = qualifiedStageBridgeRequest('tournamentCreateQualifiedStage')
      Reflect.set(value.operation.template.sourceSlots[0]!, 'destination_seed', 7)
      return value
    })(),
    (() => {
      const value = qualifiedStageBridgeRequest('tournamentPreviewQualifiedStage')
      Reflect.set(value.operation.template.sourceSlots[0]!, 'unrecognized', true)
      return value
    })(),
    (() => {
      const value = qualifiedStageBridgeRequest('tournamentCreateQualifiedStage')
      Reflect.set(value.operation.template, 'randomSeed', Number.NaN)
      return value
    })(),
    (() => {
      const value = qualifiedStageBridgeRequest('tournamentPreviewQualifiedStage')
      Reflect.set(value.serverState, Symbol('hidden'), 'not JSON')
      return value
    })(),
  ]

  let dispatches = 0
  for (const invalid of invalidRequests) {
    await expect(callCompetitionEngine(invalid, async () => {
      dispatches += 1
      return null
    })).rejects.toThrow()
  }
  expect(dispatches).toBe(0)
})

test('qualified-stage TS bridge rejects request accessors without invoking them', async () => {
  const request = qualifiedStageBridgeRequest('tournamentPreviewQualifiedStage')
  let getterCalls = 0
  Object.defineProperty(request.operation.template, 'unexpected', {
    enumerable: true,
    get() {
      getterCalls += 1
      return true
    },
  })
  let dispatches = 0
  await expect(callCompetitionEngine(request, async () => {
    dispatches += 1
    return null
  })).rejects.toThrow()
  expect(getterCalls).toBe(0)
  expect(dispatches).toBe(0)
})

test('qualified-stage TS bridge rejects mismatched operation, private output and forged public projection', async () => {
  const previewRequest = qualifiedStageBridgeRequest('tournamentPreviewQualifiedStage')
  const validPreview = qualifiedStageBridgeRawResponse(previewRequest)
  const invalidResponses: unknown[] = []
  const wrongOperation = structuredClone(validPreview)
  wrongOperation.operation = 'tournamentCreateQualifiedStage'
  invalidResponses.push(wrongOperation)

  for (const field of ['private', 'plan', 'sourceProof']) {
    const value = structuredClone(validPreview)
    Reflect.set(value, field, { forged: true })
    invalidResponses.push(value)
  }
  for (const field of ['plan', 'rawPlan', 'sourceProof']) {
    const value = structuredClone(validPreview)
    Reflect.set(value.public, field, { forged: true })
    invalidResponses.push(value)
  }
  const publicCallerTeams = structuredClone(validPreview)
  Reflect.set(publicCallerTeams.public, 'teams', [{ teamId: 'forged', seed: 1 }])
  invalidResponses.push(publicCallerTeams)
  const topLevelExtra = structuredClone(validPreview)
  Reflect.set(topLevelExtra, 'private', { hidden: true })
  invalidResponses.push(topLevelExtra)
  const extraServerStateDomain = structuredClone(validPreview)
  Reflect.set(extraServerStateDomain.serverState, 'unrecognizedDomain', {})
  invalidResponses.push(extraServerStateDomain)
  const mismatchedMapping = structuredClone(validPreview)
  const qualification = mismatchedMapping.public.qualification
  if (isJsonObject(qualification) && Array.isArray(qualification.sourceMappings)) {
    Reflect.set(qualification.sourceMappings[0], 'sourceStageId', 'caller-changed-source')
  }
  invalidResponses.push(mismatchedMapping)
  const mismatchedDestinationSeed = structuredClone(validPreview)
  const seedQualification = mismatchedDestinationSeed.public.qualification
  if (isJsonObject(seedQualification) && Array.isArray(seedQualification.sourceMappings)) {
    Reflect.set(seedQualification.sourceMappings[0], 'destinationSeed', 99)
  }
  invalidResponses.push(mismatchedDestinationSeed)
  const mismatchedStage = structuredClone(validPreview)
  const plannedStage = mismatchedStage.public.plannedStage
  if (isJsonObject(plannedStage) && isJsonObject(plannedStage.config)) {
    Reflect.set(plannedStage.config, 'stageId', 'another-stage')
  }
  invalidResponses.push(mismatchedStage)

  for (const invalid of invalidResponses) {
    await expect(callCompetitionEngine(previewRequest, async () => invalid)).rejects.toThrow()
  }

  const createRequest = qualifiedStageBridgeRequest('tournamentCreateQualifiedStage')
  const validCreate = qualifiedStageBridgeRawResponse(createRequest)
  const mismatchedTournament = structuredClone(validCreate)
  if (isJsonObject(mismatchedTournament.public)) {
    Reflect.set(mismatchedTournament.public, 'currentTournament', { tournamentId: 'different', stages: [] })
  }
  invalidResponses.push(mismatchedTournament)
  const malformedTournamentRoot = structuredClone(validCreate)
  if (isJsonObject(malformedTournamentRoot.public)) {
    Reflect.set(malformedTournamentRoot.public, 'currentTournament', { tournamentId: 'event-1' })
  }
  Reflect.set(malformedTournamentRoot.serverState, 'tournament', { tournamentId: 'event-1' })
  invalidResponses.push(malformedTournamentRoot)
  const mutatedOtherDomain = structuredClone(validCreate)
  Reflect.set(mutatedOtherDomain.serverState, 'registration', { overwritten: true })
  invalidResponses.push(mutatedOtherDomain)
  const errorWithExtraField = {
    ok: false,
    error: { code: 'qualificationBlocked', layer: 'tournament', message: 'blocked', detail: 'private' },
  }
  const errorWithExtraTopLevelField = {
    ok: false,
    error: { code: 'qualificationBlocked', layer: 'tournament', message: 'blocked' },
    operation: 'tournamentCreateQualifiedStage',
  }
  for (const invalid of invalidResponses) {
    await expect(callCompetitionEngine(previewRequest, async () => invalid)).rejects.toThrow()
  }
  for (const invalid of [
    mismatchedTournament, malformedTournamentRoot, mutatedOtherDomain,
    errorWithExtraField, errorWithExtraTopLevelField,
  ]) {
    await expect(callCompetitionEngine(createRequest, async () => invalid)).rejects.toThrow()
  }
})

test('qualified-stage TS bridge preserves Rust semantic errors without deriving qualification', async () => {
  const template: QualifiedStageTemplate = {
    ...qualifiedStageBridgeTemplates[0]!,
    sourceSlots: [qualifiedStageBridgeTemplates[0]!.sourceSlots[0]!],
  }
  const request = qualifiedStageBridgeRequest('tournamentPreviewQualifiedStage', template)
  const rustError = {
    ok: false,
    error: { code: 'qualificationBlocked', layer: 'tournament', message: 'TooFewSlots: count=1' },
  }
  let dispatched = false
  const response = await callCompetitionEngine(request, async outbound => {
    dispatched = true
    expect(outbound).toStrictEqual(request)
    return rustError
  })
  expect(dispatched).toBe(true)
  expect(response).toStrictEqual(rustError)
})

test('qualified-stage TS bridge owns request and response snapshots across the async boundary', async () => {
  const request = qualifiedStageBridgeRequest('tournamentPreviewQualifiedStage')
  let outbound: CompetitionRequest | undefined
  let resolveEngine: ((value: unknown) => void) | undefined
  const pending = callCompetitionEngine(request, input => {
    outbound = input
    return new Promise(resolve => { resolveEngine = resolve })
  })
  if (!outbound || !resolveEngine) throw new Error('fake WASM call did not start')
  const outboundBefore = structuredClone(outbound)
  if (!request.serverState.registration) throw new Error('fixture registration is missing')
  request.serverState.registration.duringAwait = 'caller-mutated'
  request.operation.template.sourceSlots[0]!.sourceStageId = 'caller-mutated-source'
  expect(outbound).toStrictEqual(outboundBefore)

  const raw = qualifiedStageBridgeRawResponse(outbound)
  const responseSnapshot = structuredClone(raw)
  resolveEngine(raw)
  const response = await pending
  if (!response.ok) throw new Error('qualified-stage owned response fixture was rejected')
  const projected = projectCompetitionResponse(response)
  raw.public.qualification = { mutatedAfterAwait: true }
  if (!outbound.serverState.clock) throw new Error('outbound clock is missing')
  outbound.serverState.clock.afterReturn = 'engine-alias-mutated'
  expect(response).toStrictEqual(responseSnapshot)
  expect(projected).toStrictEqual({ ok: true, operation: response.operation, public: response.public })
  expect(response.serverState).toStrictEqual(responseSnapshot.serverState)
  expect(request.serverState.registration).not.toStrictEqual(response.serverState.registration)
})

test('qualified-stage TS bridge rejects create output that omits the requested stage', async () => {
  const template = qualifiedStageBridgeTemplates[0]!
  const request = qualifiedStageBridgeRequest('tournamentCreateQualifiedStage', template)
  const raw = qualifiedStageBridgeRawResponse(request)
  if (!isJsonObject(raw.public) || !isJsonObject(raw.public.currentTournament)
    || !Array.isArray(raw.public.currentTournament.stages)) {
    throw new Error('qualified-stage create fixture has no tournament stages')
  }

  const requestedStageId = template.stageId
  const stages = raw.public.currentTournament.stages
  expect(raw.public.currentTournament).toStrictEqual(raw.serverState.tournament)
  expect(stages).toContainEqual(
    expect.objectContaining({ config: expect.objectContaining({ stageId: requestedStageId }) }),
  )
  const withoutRequestedStage = stages.filter(stage => !isJsonObject(stage)
    || !isJsonObject(stage.config) || stage.config.stageId !== requestedStageId)
  expect(withoutRequestedStage).toHaveLength(stages.length - 1)
  const unchangedTournament = qualifiedStageBridgeJsonObject({
    ...raw.public.currentTournament,
    stages: withoutRequestedStage,
  })
  Reflect.set(raw.public, 'currentTournament', unchangedTournament)
  Reflect.set(raw.serverState, 'tournament', structuredClone(unchangedTournament))

  expect(Object.keys(unchangedTournament).sort()).toEqual(['stages', 'tournamentId'])
  expect(unchangedTournament.stages).toStrictEqual(raw.serverState.tournament?.stages)
  expect(unchangedTournament.stages).not.toContainEqual(
    expect.objectContaining({ config: expect.objectContaining({ stageId: requestedStageId }) }),
  )
  await expect(callCompetitionEngine(request, async () => raw)).rejects.toThrow()
})

test('qualified-stage TS bridge rejects preview teams unrelated to its qualification', async () => {
  const template = qualifiedStageBridgeTemplates[0]!
  const request = qualifiedStageBridgeRequest('tournamentPreviewQualifiedStage', template)
  const raw = qualifiedStageBridgeRawResponse(request)
  if (!isJsonObject(raw.public) || !isJsonObject(raw.public.plannedStage)
    || !isJsonObject(raw.public.plannedStage.config)
    || !Array.isArray(raw.public.plannedStage.config.teams)
    || !isJsonObject(raw.public.qualification)
    || !Array.isArray(raw.public.qualification.seededTeams)) {
    throw new Error('qualified-stage preview fixture has no configured teams')
  }

  const qualificationBefore = structuredClone(raw.public.qualification)
  const stateBefore = structuredClone(raw.serverState)
  const stageIdBefore = raw.public.plannedStage.config.stageId
  const teams = raw.public.plannedStage.config.teams
  expect(teams).toStrictEqual(raw.public.qualification.seededTeams)
  expect(stageIdBefore).toBe(template.stageId)
  if (!isJsonObject(teams[0])) throw new Error('qualified-stage preview fixture has no seeded team')
  Reflect.set(teams[0], 'teamId', 'unrelated-team')

  expect(teams).not.toStrictEqual(raw.public.qualification.seededTeams)
  expect(raw.public.qualification).toStrictEqual(qualificationBefore)
  expect(raw.serverState).toStrictEqual(stateBefore)
  expect(raw.public.plannedStage.config.stageId).toBe(stageIdBefore)
  await expect(callCompetitionEngine(request, async () => raw)).rejects.toThrow()
})
