import { expect, test } from 'bun:test'
import type { StageConfig, TournamentFormat } from '../../shared/competition'
import {
  buildTournamentCreateIntent, createTournamentManagementContext,
  createTournamentManagementDraft, generateTournamentScheduleRandomSeed, keepTournamentCreateIntent,
  parseTournamentCreateAck, parseTournamentStageDraft,
} from './competition-tournament-management'

const teamIds: [string, string] = ['b45abe94-a252-4e3d-bff2-7d26c6e2fabc', '535ec115-7bf4-4bbf-b0d3-c6e0ee76c8b7']
const stageId = 'cd0d3e8b-7c87-46c1-b50d-deb6fb2ad5fa'
const commandId = 'b9ae10af-54da-429e-a10c-5e8de84021a2'
function registration() {
  return { competitionId: 'competition a', organizer: 'organizer-a', revision: 4, lifecycle: 'locked',
    teams: teamIds.map(teamId => ({ teamId, status: 'checkedIn' })) }
}
function context() {
  const value = createTournamentManagementContext(' competition a ', 'organizer-a', registration())
  if (!value) throw new Error('正常鎖定名單應可設定')
  return value
}
function draft() {
  const value = createTournamentManagementDraft(context())
  value.format = 'roundRobin'
  value.teams[0]!.seed = '2'
  value.teams[1]!.seed = '1'
  value.randomSeed = '77'
  value.matchGames = '3'
  return value
}
function stage(): StageConfig {
  return { stageId, format: 'roundRobin', teams: [{ teamId: teamIds[0], seed: 2 }, { teamId: teamIds[1], seed: 1 }],
    randomSeed: 77, matchFormat: { type: 'fixedGames', value: 3 },
    clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
    swissRounds: null, swissPolicy: null }
}
// 依 Rust Stage Serialize 的真 camelCase 欄位建立，不是 native／actual export 證明。
function publicSnapshot(config: StageConfig = stage()) {
  return { tournamentId: 'competition a', stages: [{ config, status: 'active', rounds: [], standings: [], corrections: [],
    lastScheduleError: null, scheduleGeneration: 0, supersededRounds: [],
    rankingTieDecisions: [], resolution: null,
    teams: Object.fromEntries(config.teams.map(team => [team.teamId, { seed: team.seed, dropped: false, hadBye: false }])),
    doubleState: config.format === 'doubleElimination' ? { phase: 'winners', winnersPool: [...teamIds], losersWaiting: [],
      lowerPool: [], winnersChampion: null, losersChampion: null, grandFinalWinner: null } : null }] }
}
function ack() {
  return { competitionId: 'competition a', revision: 5, commandId, idempotent: false,
    projection: { ok: true, operation: 'tournamentCreate', public: publicSnapshot() } }
}

test('種子／設定建立 exact 八欄 stage、三欄 POST，原五欄 ACK 確認後才可完成', () => {
  const ids = [stageId, commandId]
  const intent = buildTournamentCreateIntent(context(), draft(), () => {
    const value = ids.shift()
    if (!value) throw new Error('UUID 不應重生')
    return value
  })
  expect(intent.path).toBe('/api/competitions/competition%20a/tournament')
  expect(intent.body).toEqual({ commandId, expectedRevision: 4, stage: stage() })
  expect(parseTournamentCreateAck(ack(), intent)).toEqual({ revision: 5, idempotent: false })
  expect(ids).toHaveLength(0)
})

function intent(input = draft()) { return buildTournamentCreateIntent(context(), input, uuidFactory()) }
function uuidFactory() {
  const ids = [stageId, commandId]
  return () => { const id = ids.shift(); if (!id) throw new Error('不能重生 UUID'); return id }
}
function swissDraft() {
  const value = draft()
  value.format = 'swiss'
  value.swiss = { rounds: '3', points: { win: '3', draw: '1', loss: '0', bye: '3' },
    credit: { numerator: '1', denominator: '2' }, floor: { mode: 'value', value: '33' },
    cap: { mode: 'none', value: '' }, excludeByes: 'yes', tieMode: 'configured', tieBreakers: ['matchPoints', 'opponentWinRate'] }
  return value
}

test('四賽制採既有 closed tag、淘汰勝局門檻與非淘汰固定局數；ACK 同一 config 可正常通過', () => {
  const formats: TournamentFormat[] = ['singleElimination', 'roundRobin', 'doubleElimination', 'swiss']
  for (const format of formats) {
    const input = format === 'swiss' ? swissDraft() : draft()
    input.format = format
    const command = intent(input)
    expect(command.body.stage.matchFormat).toEqual({ type: format.includes('Elimination') ? 'firstToWins' : 'fixedGames', value: 3 })
    expect(Object.keys(command.body.stage)).toEqual(['stageId', 'format', 'teams', 'randomSeed', 'matchFormat', 'clockConfig', 'swissRounds', 'swissPolicy'])
    expect(parseTournamentCreateAck({ ...ack(), projection: { ok: true, operation: 'tournamentCreate', public: publicSnapshot(command.body.stage) } }, command))
      .toEqual({ revision: 5, idempotent: false })
  }
})

test('只以已驗主辦／locked checkedIn 同集合建立 context，白名單不保留私人資料', () => {
  expect(context()).toEqual({ competitionId: 'competition a', sessionUserId: 'organizer-a', revision: 4, teamIds })
  expect(createTournamentManagementContext('competition a', 'organizer-a', { ...registration(), cards: ['private'],
    teams: [...registration().teams, { teamId: 'excluded', status: 'notCheckedIn', members: ['private'] }] })).toEqual(context())
  for (const value of [
    { ...registration(), competitionId: 'other' }, { ...registration(), organizer: 'other' }, { ...registration(), lifecycle: 'open' },
    { ...registration(), revision: 0 }, { ...registration(), revision: NaN }, { ...registration(), revision: '4' },
    { ...registration(), teams: registration().teams.slice(0, 1) }, { ...registration(), teams: [...registration().teams, registration().teams[0]] },
    { ...registration(), teams: [{ teamId: teamIds[0], status: 'unknown' }, registration().teams[1]] },
  ]) expect(createTournamentManagementContext('competition a', 'organizer-a', value)).toBeNull()
  expect(createTournamentManagementContext('competition a', null, registration())).toBeNull()
  const getter = Object.defineProperty(registration(), 'revision', { get() { throw new Error('secret') } })
  expect(createTournamentManagementContext('competition a', 'organizer-a', getter)).toBeNull()
})

test('只預填五分鐘主時間與三十秒讀秒，其餘欄位必填；驗證失敗不消耗 UUID', () => {
  const empty = createTournamentManagementDraft(context())
  expect(empty.clock).toEqual({ mainMinutes: '5', byoyomiSeconds: '30' })
  expect(empty.randomSeed).toBe('')
  expect(empty.matchGames).toBe('')
  expect(empty.swiss.excludeByes).toBe('')
  let calls = 0
  expect(() => buildTournamentCreateIntent(context(), empty, () => { calls += 1; return stageId })).toThrow()
  expect(calls).toBe(0)
  const required = draft()
  required.clock.byoyomiSeconds = ''
  expect(() => intent(required)).toThrow()
})

test('整數分鐘轉毫秒、零表示不限時且讀秒共用；拒絕非整數分鐘與未支援讀秒', () => {
  const input = draft()
  input.clock.mainMinutes = '1'
  input.clock.byoyomiSeconds = '60'
  expect(intent(input).body.stage.clockConfig).toEqual({ mainTimeMs: 60_000, actionTimeMs: 60_000, outOfTurnChoiceTimeMs: 60_000 })
  const unlimited = draft()
  unlimited.clock.mainMinutes = '0'
  expect(intent(unlimited).body.stage.clockConfig.mainTimeMs).toBe(0)
  for (const minutes of ['', 'oops', '-1', '1.5', '9007199254741']) {
    const bad = draft(); bad.clock.mainMinutes = minutes
    expect(() => intent(bad)).toThrow()
  }
  for (const seconds of ['', '15', '60.0', '30000']) {
    const bad = draft(); bad.clock.byoyomiSeconds = seconds
    expect(() => intent(bad)).toThrow()
  }
})

test('完整鎖定隊伍同集合、unique u32 正種子，允許非連續種子與重排', () => {
  const input = draft()
  input.teams.reverse()
  const first = input.teams[0]
  if (!first) throw new Error('正常 fixture 缺隊伍')
  first.seed = '100'
  expect(intent(input).body.stage.teams).toEqual([{ teamId: teamIds[1], seed: 100 }, { teamId: teamIds[0], seed: 2 }])
  for (const teams of [[], input.teams.slice(0, 1), [...input.teams, { teamId: 'extra', seed: '5' }],
    [{ teamId: teamIds[0], seed: '1' }, { teamId: teamIds[0], seed: '2' }],
    [{ teamId: teamIds[0], seed: '1' }, { teamId: teamIds[1], seed: '1' }]]) {
    expect(() => intent({ ...draft(), teams })).toThrow()
  }
  for (const seed of ['0', '-1', '1.5', '4294967296', 'NaN']) {
    expect(() => intent({ ...draft(), teams: [{ teamId: teamIds[0], seed }, { teamId: teamIds[1], seed: '1' }] })).toThrow()
  }
})

test('Swiss 完整政策原樣序列化，無同分比較可為空；未決政策不默補', () => {
  const input = swissDraft()
  expect(intent(input).body.stage.swissPolicy).toEqual({ matchPoints: { win: 3, draw: 1, loss: 0, bye: 3 }, drawCredit: { numerator: 1, denominator: 2 },
    opponentWinRateFloorPercent: 33, opponentWinRateCapPercent: null, excludeByesFromOpponentWinRate: true, tieBreakers: ['matchPoints', 'opponentWinRate'] })
  input.swiss.tieMode = 'none'; input.swiss.tieBreakers = []
  expect(intent(input).body.stage.swissPolicy?.tieBreakers).toEqual([])
  for (const patch of [
    { excludeByes: '' }, { tieMode: '' }, { tieMode: 'configured', tieBreakers: [] },
    { tieBreakers: ['matchPoints', 'matchPoints'] }, { credit: { numerator: '3', denominator: '2' } },
    { credit: { numerator: '1', denominator: '0' } }, { rounds: '0' },
    { floor: { mode: '', value: '' } }, { floor: { mode: 'value', value: '101' } },
    { floor: { mode: 'value', value: '50' }, cap: { mode: 'value', value: '49' } },
    { points: { win: '3', draw: 'oops', loss: '0', bye: '3' } },
  ]) expect(() => parseTournamentStageDraft(context(), { ...swissDraft(), swiss: { ...swissDraft().swiss, ...patch } })).toThrow()
})

test('runtime 表單不以 cast／coercion 接受損壞值；非 Swiss 明列 null 不帶未使用政策', () => {
  for (const patch of [{ randomSeed: NaN }, { randomSeed: null }, { randomSeed: 77 }, { randomSeed: '-1' },
    { randomSeed: '9007199254740992' }, { format: 'unknown' }, { matchGames: '65536' }, { matchGames: '0' }, { callerUserId: 'forged' }]) {
    expect(() => parseTournamentStageDraft(context(), { ...draft(), ...patch })).toThrow()
  }
  const value = { ...draft(), swiss: { secret: 'unused' } }
  expect(parseTournamentStageDraft(context(), value).swissPolicy).toBeNull()
  const accessor = Object.defineProperty(draft(), 'randomSeed', { get() { throw new Error('secret') } })
  expect(() => parseTournamentStageDraft(context(), accessor)).toThrow('賽程設定')
})

test('每次產生公開排程亂數只呼叫一次 Uint32，不從時間／fixture 產生', () => {
  let calls = 0
  expect(generateTournamentScheduleRandomSeed(target => { calls += 1; expect(target).toHaveLength(1); target[0] = 0xffff_ffff })).toBe(4294967295)
  expect(calls).toBe(1)
})

test('immutable copied snapshot 不隨原草稿、context 或 retry 改 seed／params／UUID', () => {
  const input = swissDraft()
  const command = intent(input)
  const serialized = JSON.stringify(command.body)
  input.randomSeed = '999'
  input.swiss.points.win = '123'
  input.swiss.tieBreakers.reverse()
  input.teams.reverse()
  expect(JSON.stringify(command.body)).toBe(serialized)
  expect(Object.isFrozen(command.body)).toBe(true)
  const firstTeam = command.body.stage.teams[0]
  const policy = command.body.stage.swissPolicy
  if (!firstTeam || !policy) throw new Error('正常 Swiss snapshot 必須有隊伍與政策')
  expect(Object.isFrozen(command.body.stage.teams)).toBe(true)
  expect(Object.isFrozen(firstTeam)).toBe(true)
  expect(Object.isFrozen(policy.matchPoints)).toBe(true)
  expect(command.scope).toEqual({ competitionId: 'competition a', sessionUserId: 'organizer-a' })
})

test('原五欄 ACK identity／operation／revision／public 關係逐欄拒絕，不以合法 GET 反推成功', () => {
  const command = intent()
  const valid = ack()
  expect(parseTournamentCreateAck({ ...valid, idempotent: true }, command)).toEqual({ revision: 5, idempotent: true })
  for (const patch of [{ competitionId: 'other' }, { commandId: stageId }, { revision: 6 }, { revision: '5' }, { idempotent: 1 },
    { private: 'secret' }, { projection: { ...valid.projection, operation: 'tournamentView' } },
    { projection: { ...valid.projection, public: { ...publicSnapshot(), tournamentId: 'other' } } },
    { projection: { ...valid.projection, public: publicSnapshot({ ...stage(), randomSeed: 78 }) } },
    { projection: { ...valid.projection, public: { ...publicSnapshot(), cards: ['secret'] } } }]) {
    expect(() => parseTournamentCreateAck({ ...valid, ...patch }, command)).toThrow('收據無法確認')
  }
  expect(() => parseTournamentCreateAck(Object.defineProperty(valid, 'commandId', { get() { throw new Error('secret') } }), command)).toThrow('收據無法確認')
})

test('首次已辨識 pre-commit 拒絕可分類，曾未知之後 403／409／404 都不能解原命令', () => {
  const refusals = [{ statusCode: 403, data: { code: 'forbidden' } }, { statusCode: 409, data: { data: { code: 'revisionConflict' } } },
    { statusCode: 404, data: { code: 'notInitialized' } }]
  for (const error of refusals) {
    expect(keepTournamentCreateIntent(false, error)).toBe(false)
    expect(keepTournamentCreateIntent(true, error)).toBe(true)
  }
  for (const error of [{ statusCode: 503 }, { statusCode: 409 }, { statusCode: 422, data: { code: 'unknown' } }, new Error('secret')]) {
    expect(keepTournamentCreateIntent(false, error)).toBe(true)
  }
})
