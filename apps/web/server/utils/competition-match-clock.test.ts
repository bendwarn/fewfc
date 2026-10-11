import { expect, test } from 'bun:test'
import {
  assertCompetitionMatchClockQuery,
  buildCompetitionMatchClockReadRequest,
  projectCompetitionMatchClockReadResult,
  serverCompetitionMatchClockTime,
} from './competition-match-clock'
import type { ClockCheckpointWindow, CompetitionOperation } from '../../shared/competition'
import type { CompetitionRoomMatchClockReadValue } from '../../shared/competition-room'

function value(): CompetitionRoomMatchClockReadValue {
  return {
    competitionId: 'competition-1', matchId: 'match-1', matchRevision: 4, matchStatus: 'open',
    game: { gameId: 'game-1', gameNumber: 1 },
    clock: {
      sampledAtMs: 4000,
      teams: [
        { team: 'team-a', mainTimeRemainingMs: 299000, byoyomiTimeRemainingMs: 30000, byoyomiActive: false },
        { team: 'team-b', mainTimeRemainingMs: 300000, byoyomiTimeRemainingMs: 30000, byoyomiActive: false },
      ],
      running: { type: 'action', team: 'team-a', turn: 1 }, paused: null, awaitingAuthority: false,
    },
  }
}

const request = () => buildCompetitionMatchClockReadRequest('competition-1', 'match-1', 'session-user', 4000)

test('時計 request 僅由路徑、canonical session 與單次伺服器時間建立', () => {
  let samples = 0
  const time = serverCompetitionMatchClockTime(() => { samples += 1; return 4000 })
  expect(buildCompetitionMatchClockReadRequest(' competition-1 ', ' match-1 ', ' canonical-user ', time))
    .toEqual({ competitionId: 'competition-1', matchId: 'match-1', callerUserId: ' canonical-user ', trustedTimeMs: 4000 })
  expect(samples).toBe(1)
})

test('時計成功保留 exact 六欄 DTO 與權威取樣，不重新扣時或推導超時', () => {
  const current = value()
  expect(projectCompetitionMatchClockReadResult({ ok: true, value: current }, request())).toEqual(current)
  for (const matchStatus of ['open', 'awaitingFirstPlayer', 'awaitingReferee', 'completed'] as const) {
    const idle: CompetitionRoomMatchClockReadValue = {
      ...value(), matchStatus, game: null,
      clock: { ...value().clock, running: null, awaitingAuthority: matchStatus === 'awaitingReferee' },
    }
    expect(projectCompetitionMatchClockReadResult({ ok: true, value: idle }, request())).toEqual(idle)
  }
})

test('巢狀視窗 accessor 直接拒絕且不得執行不可信 getter', () => {
  const current = value()
  let reads = 0
  Object.defineProperty(current.clock.running, 'type', { enumerable: true, get: () => { reads += 1; return 'action' } })
  expect(() => projectCompetitionMatchClockReadResult({ ok: true, value: current }, request())).toThrow()
  expect(reads).toBe(0)
})

test('所有 malformed request／非安全時間／query／body 皆固定拒絕，不改為 caller 時間', () => {
  for (const bad of [undefined, null, 3, '', '  ']) {
    expect(() => buildCompetitionMatchClockReadRequest(bad, 'match-1', 'user', 0)).toThrow()
    expect(() => buildCompetitionMatchClockReadRequest('competition-1', bad, 'user', 0)).toThrow()
    expect(() => buildCompetitionMatchClockReadRequest('competition-1', 'match-1', bad, 0)).toThrow()
  }
  for (const bad of [-1, 0.5, Infinity, NaN, Number.MAX_SAFE_INTEGER + 1]) {
    expect(() => serverCompetitionMatchClockTime(() => bad)).toThrow()
    expect(() => buildCompetitionMatchClockReadRequest('competition-1', 'match-1', 'user', bad)).toThrow()
  }
  expect(() => serverCompetitionMatchClockTime(() => { throw new Error('private clock exception') })).toThrow('伺服器時間暫時無法使用。')
  expect(serverCompetitionMatchClockTime(() => 0)).toBe(0)
  expect(serverCompetitionMatchClockTime(() => Number.MAX_SAFE_INTEGER)).toBe(Number.MAX_SAFE_INTEGER)
  assertCompetitionMatchClockQuery({})
  for (const query of [undefined, null, [], { callerUserId: 'attacker' }, { [Symbol('unknown')]: 1 }]) {
    expect(() => assertCompetitionMatchClockQuery(query)).toThrow()
  }
  expect(() => assertCompetitionMatchClockQuery({}, true)).toThrow()
})

test('準備／行動／回合外選擇／paused／不限時／awaitingAuthority 完整保留，沒有 HTTP 時計演算', () => {
  const windows: (ClockCheckpointWindow | null)[] = [
    null, { type: 'preparation', teams: ['team-a', 'team-b'] }, { type: 'preparation', teams: ['team-b'] },
    { type: 'action', team: 'team-a', turn: 0 },
    { type: 'outOfTurnChoice', team: 'team-b', choiceId: 'choice-1', remainingMs: 0 },
    { type: 'outOfTurnChoice', team: 'team-a', choiceId: 'choice-2', remainingMs: null },
  ]
  for (const window of windows) {
    for (const paused of [false, true]) {
      const current = value()
      current.clock.running = paused ? null : window
      current.clock.paused = paused ? window : null
      current.clock.teams = [
        { team: 'team-a', mainTimeRemainingMs: null, byoyomiTimeRemainingMs: null, byoyomiActive: false },
        { team: 'team-b', mainTimeRemainingMs: 0, byoyomiTimeRemainingMs: 0, byoyomiActive: true },
      ]
      current.clock.awaitingAuthority = !paused
      expect(projectCompetitionMatchClockReadResult({ ok: true, value: current }, request())).toEqual(current)
    }
  }
})

test('wrong identity/time/status/game 與 clock-team-window 關聯矛盾全部 fail closed', () => {
  const mutations: ((current: CompetitionRoomMatchClockReadValue) => void)[] = [
    current => { current.competitionId = 'other' }, current => { current.matchId = 'other' },
    current => { current.matchRevision = 0 }, current => { current.matchRevision = Number.MAX_SAFE_INTEGER + 1 },
    current => { Object.assign(current, { matchStatus: 'unknown' }) },
    current => { current.matchStatus = 'completed' }, current => { current.matchStatus = 'awaitingFirstPlayer' },
    current => { current.matchStatus = 'awaitingReferee' },
    current => { current.game = { gameId: '', gameNumber: 1 } }, current => { current.game = { gameId: 'old', gameNumber: 0 } },
    current => { current.clock.sampledAtMs = 3999 }, current => { current.clock.sampledAtMs = NaN },
    current => { current.clock.teams = [] },
    current => { const first = current.clock.teams[0]; if (!first) throw new Error('fixture'); current.clock.teams = [first, { ...first }] },
    current => { current.clock.running = { type: 'action', team: 'team-other', turn: 1 } },
    current => { current.clock.running = { type: 'preparation', teams: ['team-a', 'team-a'] } },
    current => { current.clock.running = { type: 'preparation', teams: [] } },
    current => { current.clock.running = { type: 'action', team: 'team-a', turn: -1 } },
    current => { current.clock.running = { type: 'outOfTurnChoice', team: 'team-a', choiceId: '', remainingMs: 5 } },
    current => { current.clock.paused = { type: 'action', team: 'team-a', turn: 1 } },
    current => { Object.assign(current.clock, { awaitingAuthority: 'true' }) },
    current => { const first = current.clock.teams[0]; if (!first) throw new Error('fixture'); first.byoyomiActive = true },
    current => { const first = current.clock.teams[0]; if (!first) throw new Error('fixture'); first.mainTimeRemainingMs = -1 },
    current => { const first = current.clock.teams[0]; if (!first) throw new Error('fixture'); first.byoyomiTimeRemainingMs = Infinity },
  ]
  for (const mutate of mutations) {
    const current = value()
    mutate(current)
    expect(() => projectCompetitionMatchClockReadResult({ ok: true, value: current }, request())).toThrow('賽事服務回應格式不正確。')
  }
})

test('semantic 各層未知/private/symbol/nonenumerable/accessor/nonJSON 不得剝除後通過', () => {
  function parts(current = value()) {
    const root = { ok: true, value: current }
    const team = current.clock.teams[0]
    if (!team || !current.game || !current.clock.running) throw new Error('缺少 fixture 巢狀資料。')
    return { root, nodes: [root, current, current.game, current.clock, team, current.clock.running] }
  }
  for (const index of [0, 1, 2, 3, 4, 5]) {
    for (const key of ['private', 'serverState', 'binding', 'deckSeed', 'deckSnapshot', 'roster', 'config', 'unknown']) {
      const { root, nodes } = parts()
      const node = nodes[index]
      if (!node) throw new Error('fixture node')
      Object.defineProperty(node, key, { enumerable: true, value: 'private sentinel' })
      expect(() => projectCompetitionMatchClockReadResult(root, request())).toThrow()
    }
    for (const key of [Symbol('unknown'), Symbol.dispose]) {
      const { root, nodes } = parts()
      const node = nodes[index]
      if (!node) throw new Error('fixture node')
      Object.defineProperty(node, key, { value: () => undefined })
      expect(() => projectCompetitionMatchClockReadResult(root, request())).toThrow()
    }
  }
  for (const replacement of [new Date(), new Map(), 1n, () => 1, undefined, NaN, Infinity]) {
    const current = value()
    Object.assign(current.clock, { sampledAtMs: replacement })
    expect(() => projectCompetitionMatchClockReadResult({ ok: true, value: current }, request())).toThrow()
  }
  for (const mode of ['sparse', 'symbol', 'extra', 'accessor']) {
    const current = value()
    if (mode === 'sparse') delete current.clock.teams[0]
    if (mode === 'symbol') Object.defineProperty(current.clock.teams, Symbol('extra'), { value: 1 })
    if (mode === 'extra') Object.defineProperty(current.clock.teams, 'extra', { value: 1 })
    if (mode === 'accessor') Object.defineProperty(current.clock.teams, '0', { enumerable: true, get: () => { throw new Error('private getter') } })
    expect(() => projectCompetitionMatchClockReadResult({ ok: true, value: current }, request())).toThrow('賽事服務回應格式不正確。')
  }
  const nonenumerable = { ok: true, value: value() }
  Object.defineProperty(nonenumerable, 'ok', { value: true, enumerable: false })
  expect(() => projectCompetitionMatchClockReadResult(nonenumerable, request())).toThrow()
  const accessor = { ok: true, value: value() }
  let reads = 0
  Object.defineProperty(accessor, 'value', { enumerable: true, get: () => { reads += 1; return value() } })
  expect(() => projectCompetitionMatchClockReadResult(accessor, request())).toThrow()
  expect(reads).toBe(0)
})

test('typed refusal 保留固定安全status，unknown/errorextra一律502不反射message', () => {
  for (const [code, status] of [
    ['invalidRequest', 400], ['notInitialized', 404], ['competitionIdMismatch', 404], ['matchNotFound', 404],
    ['forbidden', 403], ['notParticipant', 403], ['notReady', 409], ['trustedTimeRegression', 409],
    ['invalidMatchCheckpoint', 409], ['invalidClockCheckpoint', 409], ['invalidLockedRoster', 409],
    ['invalidGameProvision', 409], ['engineProtocolError', 502], ['internalFailure', 503],
    ['unknown private code', 502],
  ] as const) {
    let captured: unknown
    try { projectCompetitionMatchClockReadResult({ ok: false, error: { code, message: 'private message sentinel' } }, request()) } catch (error) { captured = error }
    if (!(captured instanceof Error) || !('statusCode' in captured)) throw new Error('缺少安全 H3 error。')
    expect(captured.statusCode).toBe(status)
    expect(captured.message).not.toContain('private message sentinel')
    expect(captured.message).not.toContain('unknown private code')
  }
  for (const malformed of [null, true, {}, { ok: false }, { ok: 1, value: value() },
    { ok: false, error: { code: 'forbidden', message: 'private', binding: {} } }]) {
    expect(() => projectCompetitionMatchClockReadResult(malformed, request())).toThrow('賽事服務回應格式不正確。')
  }
})

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

async function formalClockWasm() {
  // 直接隔離實例化既有正式 artifact；不 import 會受其他 test mock.module 影響的 engine。
  const path = Bun.env.FEWC_COMPETITION_TEST_WASM ?? new URL('../../worker/wasm/fewfc.wasm', import.meta.url)
  const instance = new WebAssembly.Instance(new WebAssembly.Module(await Bun.file(path).arrayBuffer()), {})
  const { memory, fewfc_alloc: alloc, fewfc_dealloc: dealloc, fewfc_handle_competition_request: invoke } = instance.exports
  if (!(memory instanceof WebAssembly.Memory) || typeof alloc !== 'function' || typeof dealloc !== 'function' || typeof invoke !== 'function') {
    throw new Error('正式 competition WASM ABI export 缺失。')
  }
  return async (input: unknown): Promise<Record<string, unknown>> => {
    const bytes = new TextEncoder().encode(JSON.stringify(input))
    const pointer: unknown = alloc(bytes.length)
    if (typeof pointer !== 'number' || !Number.isSafeInteger(pointer) || pointer < 0) throw new Error('WASM allocation 失敗。')
    let packed: unknown
    try {
      new Uint8Array(memory.buffer, pointer, bytes.length).set(bytes)
      packed = invoke(pointer, bytes.length)
    } finally { dealloc(pointer, bytes.length) }
    if (typeof packed !== 'bigint') throw new Error('WASM JSON result 非合法 pointer。')
    const outputPointer = Number(packed >> 32n)
    const length = Number(packed & 0xffffffffn)
    let result: unknown
    try {
      result = JSON.parse(new TextDecoder().decode(new Uint8Array(memory.buffer, outputPointer, length)))
    } finally { dealloc(outputPointer, length) }
    if (!record(result) || result.ok !== true || !record(result.serverState)) throw new Error('正式時計 ABI 拒絕此正常前綴。')
    return result
  }
}

test('真 Rust WASM ClockDisplay 的準備/action/暫停/恢復/到期限/restart 原值可安全投影，不改checkpoint', async () => {
  const wasm = await formalClockWasm()
  let state: unknown = { registration: null, clock: null, tournament: null, matches: {} }
  const execute = async (operation: CompetitionOperation, time: number, actor: 'organizer' | 'referee' = 'organizer') => {
    const result = await wasm({ serverActor: { type: actor, playerId: actor }, trustedTimeMs: time, operation, serverState: state })
    expect(result.operation).toBe(operation.type)
    state = result.serverState
  }
  const query = async (time: number, engine = wasm) => {
    const before = structuredClone(state)
    const result = await engine({ serverActor: { type: 'player', playerId: 'fixed-player' }, trustedTimeMs: time,
      operation: { type: 'clockView' }, serverState: state })
    expect(result.operation).toBe('clockView')
    expect(result.serverState).toEqual(before)
    expect(state).toEqual(before)
    const current = value()
    Object.assign(current, { clock: result.public })
    return projectCompetitionMatchClockReadResult({ ok: true, value: current }, { ...request(), trustedTimeMs: time })
  }
  await execute({ type: 'clockCreate', teams: ['team-a', 'team-b'], config: {
    mainTimeMs: 1000, actionTimeMs: 100, outOfTurnChoiceTimeMs: 20,
  } }, 3000)
  await execute({ type: 'clockBeginAction', team: 'team-a', turn: 1 }, 3000)
  expect((await query(3400)).clock.teams).toEqual([
    { team: 'team-a', mainTimeRemainingMs: 600, byoyomiTimeRemainingMs: 100, byoyomiActive: false },
    { team: 'team-b', mainTimeRemainingMs: 1000, byoyomiTimeRemainingMs: 100, byoyomiActive: false },
  ])
  expect((await query(4100)).clock.awaitingAuthority).toBe(true)
  await execute({ type: 'clockPause', reason: '正式顯示暫停回歸' }, 3300, 'referee')
  const paused = await query(8000)
  expect(paused.clock.running).toBeNull()
  expect(paused.clock.paused).toEqual({ type: 'action', team: 'team-a', turn: 1 })
  expect(paused.clock.teams[0]?.mainTimeRemainingMs).toBe(700)
  await execute({ type: 'clockResume', reason: '正式顯示恢復回歸' }, 9000, 'referee')
  const resumed = await query(9100)
  expect(resumed.clock.teams[0]?.mainTimeRemainingMs).toBe(600)
  expect(resumed.clock.teams[1]?.mainTimeRemainingMs).toBe(1000)
  expect(await query(9100, await formalClockWasm())).toEqual(resumed)
  await execute({ type: 'clockNewGame', time: 'reset' }, 10000)
  await execute({ type: 'clockBeginPreparation' }, 10000)
  const preparing = await query(10100)
  expect(preparing.clock.running).toEqual({ type: 'preparation', teams: ['team-a', 'team-b'] })
  expect(preparing.clock.teams.map(team => team.mainTimeRemainingMs)).toEqual([900, 900])
  await execute({ type: 'clockFinishPreparation', team: 'team-a' }, 10100)
  const oneWaiting = await query(10200)
  expect(oneWaiting.clock.running).toEqual({ type: 'preparation', teams: ['team-b'] })
  expect(oneWaiting.clock.teams.map(team => team.mainTimeRemainingMs)).toEqual([900, 800])
})
