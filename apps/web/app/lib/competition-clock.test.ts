import { expect, test } from 'bun:test'
import { CompetitionClockProtocolError, competitionClockErrorMessage, competitionClockPath, createCompetitionClockScope, formatCompetitionClockTime, parseCompetitionClockResponse } from './competition-clock'
import type { ClockCheckpointWindow, ClockDisplay } from '../../shared/competition'
import type { CompetitionRoomMatchClockReadValue } from '../../shared/competition-room'

const scopeInput = () => ({
  mode: { kind: 'competition', context: { competitionId: 'competition a', matchId: 'match/one', gameNumber: 2 } },
  gameId: 'room/one', roomGameId: 'room/one', sessionUserId: 'member-a', memberUserId: 'member-a',
})

function scope() {
  const result = createCompetitionClockScope(scopeInput())
  if (!result) throw new Error('合法固定成員 fixture 應取得 scope')
  return result
}

function response(): CompetitionRoomMatchClockReadValue {
  return {
    competitionId: 'competition a', matchId: 'match/one', matchRevision: 4, matchStatus: 'open',
    game: { gameId: 'room/one', gameNumber: 2 },
    clock: {
      sampledAtMs: 4000,
      teams: [
        { team: 'team-a', mainTimeRemainingMs: 299001, byoyomiTimeRemainingMs: 30000, byoyomiActive: false },
        { team: 'team-b', mainTimeRemainingMs: 300000, byoyomiTimeRemainingMs: null, byoyomiActive: false },
      ],
      running: { type: 'action', team: 'team-a', turn: 0 }, paused: null, awaitingAuthority: false,
    },
  }
}

test('時計僅接受伺服器競賽 context、同一房間及本人固定成員身分', () => {
  expect(createCompetitionClockScope(scopeInput())).toEqual({
    competitionId: 'competition a', matchId: 'match/one', gameNumber: 2,
    gameId: 'room/one', sessionUserId: 'member-a',
  })
})

test('只建立專用 GET 路徑並複製相同局的 exact 公開權威 DTO，不重新扣時', () => {
  expect(competitionClockPath(scope())).toBe('/api/competitions/competition%20a/matches/match%2Fone/clock')
  const source = response()
  const projected = parseCompetitionClockResponse(source, scope())
  expect(projected).toEqual(response())
  expect(projected).not.toBe(source)
  expect(projected.clock).not.toBe(source.clock)
  expect(projected.clock.teams[0]).not.toBe(source.clock.teams[0])
  expect(projected.clock.running).not.toBe(source.clock.running)
  source.clock.teams[0]!.mainTimeRemainingMs = 1
  expect(projected.clock.teams[0]?.mainTimeRemainingMs).toBe(299001)
})

test('顯示只轉換權威毫秒數的單位，零與不限時不推導判負；錯誤不反射內部資料', () => {
  expect(formatCompetitionClockTime(299001)).toBe('299.001 秒')
  expect(formatCompetitionClockTime(30000)).toBe('30 秒')
  expect(formatCompetitionClockTime(0)).toBe('0 秒')
  expect(formatCompetitionClockTime(null)).toBe('不限時')
  expect(competitionClockErrorMessage({ statusCode: 403, data: { message: 'PRIVATE_TEAM_DECK' } }))
    .toBe('你沒有讀取這場交手時計的權限，已停止自動更新。')
  expect(competitionClockErrorMessage(new Error('PRIVATE_TEAM_DECK')))
    .toBe('時計服務暫時無法使用，已停止自動更新。請手動重試。')
})

test('即使錯誤型別正確但 message 被改寫，仍只顯示固定安全文案', () => {
  const error = new CompetitionClockProtocolError()
  error.message = 'PRIVATE_INTERNAL_PAYLOAD'
  expect(competitionClockErrorMessage(error)).toBe('時計回應無法安全驗證，已停止自動更新。')
})

test('普通／無效 context、非本人固定成員、未登入與 route/metadata 不同房均不可查', () => {
  const blocked = [
    { mode: { kind: 'ordinary' } }, { mode: { kind: 'invalid' } }, { mode: null },
    { mode: { kind: 'competition', context: { ...scopeInput().mode.context, organizer: 'attacker' } } },
    { mode: { kind: 'competition', context: { ...scopeInput().mode.context, gameNumber: 0 } } },
    { mode: { kind: 'competition', context: { ...scopeInput().mode.context, gameNumber: '2' } } },
    { mode: { kind: 'competition', context: { ...scopeInput().mode.context, matchId: ' match/one ' } } },
    { sessionUserId: null }, { sessionUserId: ' ' }, { memberUserId: null },
    { memberUserId: 'member-b' }, { sessionUserId: ' member-a ' },
    { gameId: 'other-room' }, { roomGameId: 'previous-room' }, { gameId: ['room/one'] },
  ]
  for (const input of blocked) expect(createCompetitionClockScope({ ...scopeInput(), ...input })).toBeNull()
  expect(createCompetitionClockScope({ ...scopeInput(), gameId: ' room/one ' })?.gameId).toBe('room/one')
  let reads = 0
  const context = scopeInput().mode.context
  Object.defineProperty(context, 'matchId', { enumerable: true, get: () => { reads += 1; return 'match/one' } })
  expect(createCompetitionClockScope({ ...scopeInput(), mode: { kind: 'competition', context } })).toBeNull()
  expect(reads).toBe(0)
})

test('四種 matchStatus 的 null 狀態完整保留，不把 null 推導為 completed', () => {
  for (const matchStatus of ['open', 'awaitingFirstPlayer', 'awaitingReferee', 'completed'] as const) {
    const source = { ...response(), matchStatus, game: null }
    source.clock.running = null
    expect(parseCompetitionClockResponse(source, scope())).toEqual(source)
  }
})

test('準備／行動／回合外選擇／paused／awaitingAuthority／不限時原樣保留', () => {
  const windows: (ClockCheckpointWindow | null)[] = [
    { type: 'preparation', teams: ['team-a', 'team-b'] },
    { type: 'preparation', teams: ['team-b'] },
    { type: 'action', team: 'team-a', turn: 0 },
    { type: 'outOfTurnChoice', team: 'team-b', choiceId: 'choice-a', remainingMs: 0 },
    { type: 'outOfTurnChoice', team: 'team-b', choiceId: 'choice-a', remainingMs: null },
    null,
  ]
  for (const window of windows) for (const paused of [false, true]) {
    const source = response()
    const clock: ClockDisplay = {
      ...source.clock, running: paused ? null : window, paused: paused ? window : null, awaitingAuthority: true,
      teams: [
        { team: 'team-a', mainTimeRemainingMs: null, byoyomiTimeRemainingMs: null, byoyomiActive: false },
        { team: 'team-b', mainTimeRemainingMs: 0, byoyomiTimeRemainingMs: 0, byoyomiActive: true },
      ],
    }
    expect(parseCompetitionClockResponse({ ...source, clock }, scope()).clock).toEqual(clock)
  }
})

test('身份／閉合 status／gameNumber／revision／不安全時計及 team-window 關聯矛盾均拒絕', () => {
  const mutations: ((current: CompetitionRoomMatchClockReadValue) => void)[] = [
    current => { current.competitionId = 'other' }, current => { current.matchId = 'other' },
    current => { current.matchRevision = 0 }, current => { current.matchRevision = 1.5 },
    current => { current.matchRevision = Number.MAX_SAFE_INTEGER + 1 },
    current => { Object.assign(current, { matchStatus: 'unknown' }) },
    current => { current.matchStatus = 'completed' },
    current => { current.game = { gameId: 'other-room', gameNumber: 2 } },
    current => { current.game = { gameId: 'room/one', gameNumber: 1 } },
    current => { Object.assign(current.game!, { gameNumber: '2' }) },
    current => { current.clock.sampledAtMs = -1 }, current => { current.clock.sampledAtMs = NaN },
    current => { Object.assign(current.clock, { awaitingAuthority: 'false' }) },
    current => { current.clock.teams = [] },
    current => { current.clock.teams[1]!.team = 'team-a' },
    current => { current.clock.teams[0]!.mainTimeRemainingMs = Infinity },
    current => { current.clock.teams[0]!.mainTimeRemainingMs = -1 },
    current => { current.clock.teams[0]!.byoyomiTimeRemainingMs = Number.MAX_SAFE_INTEGER + 1 },
    current => { Object.assign(current.clock.teams[0]!, { byoyomiActive: 'false' }) },
    current => { current.clock.teams[0]!.byoyomiActive = true },
    current => { current.clock.running = { type: 'action', team: 'other-team', turn: 1 } },
    current => { current.clock.running = { type: 'action', team: 'team-a', turn: -1 } },
    current => { current.clock.running = { type: 'preparation', teams: ['team-a', 'team-a'] } },
    current => { current.clock.running = { type: 'preparation', teams: [] } },
    current => { current.clock.running = { type: 'outOfTurnChoice', team: 'team-a', choiceId: '', remainingMs: 0 } },
    current => { current.clock.running = { type: 'outOfTurnChoice', team: 'team-a', choiceId: 'choice', remainingMs: -1 } },
    current => { current.clock.paused = { type: 'action', team: 'team-b', turn: 1 } },
  ]
  expect(parseCompetitionClockResponse(response(), scope())).toEqual(response())
  for (const mutate of mutations) {
    const current = response()
    mutate(current)
    expect(() => parseCompetitionClockResponse(current, scope())).toThrow(CompetitionClockProtocolError)
  }
})

test('unknown extra／symbols／hidden／accessor／非 JSON array 拒絕且 getter 不執行', () => {
  const mutations: ((current: CompetitionRoomMatchClockReadValue) => void)[] = [
    current => { Object.assign(current, { callerUserId: 'PRIVATE_CALLER' }) },
    current => { Object.assign(current.clock, { checkpoint: 'PRIVATE_CHECKPOINT' }) },
    current => { Object.assign(current.game!, { privateDeck: ['SECRET'] }) },
    current => { Object.assign(current.clock.teams[0]!, { cards: ['SECRET'] }) },
    current => { Object.assign(current.clock.running!, { deadlineMs: 10 }) },
    current => { Object.defineProperty(current, 'hidden', { value: true }) },
    current => { Object.defineProperty(current, Symbol.dispose, { value: () => {} }) },
    current => { Object.setPrototypeOf(current.clock, { private: 'SECRET' }) },
    current => { delete current.clock.teams[0] },
    current => { Object.assign(current.clock.teams, { private: 'SECRET' }) },
    current => { Object.setPrototypeOf(current.clock.teams, null) },
  ]
  for (const mutate of mutations) {
    const current = response()
    mutate(current)
    expect(() => parseCompetitionClockResponse(current, scope())).toThrow(CompetitionClockProtocolError)
  }
  for (const target of ['root', 'clock', 'team', 'window'] as const) {
    let reads = 0
    const current = response()
    const [object, key] = target === 'root' ? [current, 'clock']
      : target === 'clock' ? [current.clock, 'teams']
      : target === 'team' ? [current.clock.teams[0]!, 'team'] : [current.clock.running!, 'type']
    Object.defineProperty(object, key, { enumerable: true, get: () => { reads += 1; throw new Error('PRIVATE_GETTER') } })
    expect(() => parseCompetitionClockResponse(current, scope())).toThrow('時計回應無法安全驗證，已停止自動更新。')
    expect(reads).toBe(0)
  }
  expect(() => parseCompetitionClockResponse(new Proxy({}, { ownKeys() { throw new Error('PRIVATE_PROXY') } }), scope()))
    .toThrow('時計回應無法安全驗證，已停止自動更新。')
})

test('所有 error status 都固定呈現且 unsafe 數字不格式化、不 coercion', () => {
  for (const status of [400, 401, 403, 404, 409, 502, 503, '403', null, Infinity, NaN]) {
    expect(competitionClockErrorMessage({ status, message: 'PRIVATE', data: { message: 'PRIVATE' } })).not.toContain('PRIVATE')
  }
  for (const value of [-1, Infinity, NaN, 0.5, Number.MAX_SAFE_INTEGER + 1]) {
    expect(formatCompetitionClockTime(value)).toBe('時間無法顯示')
  }
  expect(formatCompetitionClockTime(1001)).toBe('1.001 秒')
  expect(formatCompetitionClockTime(1010)).toBe('1.01 秒')
  expect(competitionClockErrorMessage({ get statusCode() { throw new Error('PRIVATE') } })).not.toContain('PRIVATE')
})

test('錯誤 Proxy 的 prototype trap 也不能逃出安全錯誤呈現介面', () => {
  const error = new Proxy({}, { getPrototypeOf() { throw new Error('PRIVATE_PROXY_ERROR') } })
  expect(competitionClockErrorMessage(error)).toBe('時計服務暫時無法使用，已停止自動更新。請手動重試。')
})
