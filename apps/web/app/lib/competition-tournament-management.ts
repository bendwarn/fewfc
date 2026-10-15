import type { StageConfig, SwissStandingPolicy, SwissTieBreaker, TournamentFormat } from '../../shared/competition'
import { parseCompetitionTournamentViewResponse } from './competition-tournament-view'

export interface TournamentManagementContext {
  competitionId: string
  sessionUserId: string
  revision: number
  teamIds: string[]
}
interface ClockDraft { mainMinutes: string, byoyomiSeconds: string }
interface PercentDraft { mode: '' | 'none' | 'value', value: string }
export interface TournamentManagementDraft {
  format: '' | TournamentFormat
  teams: { teamId: string, seed: string }[]
  randomSeed: string
  matchGames: string
  clock: ClockDraft
  swiss: {
    rounds: string
    points: { win: string, draw: string, loss: string, bye: string }
    credit: { numerator: string, denominator: string }
    floor: PercentDraft
    cap: PercentDraft
    excludeByes: '' | 'yes' | 'no'
    tieMode: '' | 'none' | 'configured'
    tieBreakers: ('' | SwissTieBreaker)[]
  }
}
export type TournamentStageSettings = Omit<StageConfig, 'stageId'>
export interface TournamentCreateIntent {
  scope: { competitionId: string, sessionUserId: string }
  path: string
  method: 'POST'
  body: { commandId: string, expectedRevision: number, stage: StageConfig }
}

export class TournamentManagementValidationError extends Error {
  constructor(message = '賽程設定不完整或格式不正確，請檢查各欄位。') {
    super(message)
    this.name = 'TournamentManagementValidationError'
  }
}
export class TournamentCreateProtocolError extends Error {
  constructor() {
    super('建立賽程的收據無法確認。')
    this.name = 'TournamentCreateProtocolError'
  }
}

/** 只取 parent 已驗報名 context 的必要欄位；主辦提示不是 server ACL。 */
export function createTournamentManagementContext(
  competitionId: unknown, sessionUserId: unknown, registration: unknown,
): TournamentManagementContext | null {
  try {
    const id = normalizedId(competitionId)
    const user = normalizedId(sessionUserId)
    const value = record(registration)
    if (!id || !user || !value || value.competitionId !== id || value.organizer !== user
      || value.lifecycle !== 'locked' || !positiveInteger(value.revision, Number.MAX_SAFE_INTEGER - 1)
      || !Array.isArray(value.teams)) return null
    const seen = new Set<string>()
    const teamIds: string[] = []
    for (const input of value.teams) {
      const team = record(input)
      if (!team || typeof team.teamId !== 'string' || normalizedId(team.teamId) !== team.teamId
        || !team.teamId || seen.has(team.teamId)
        || !['registered', 'checkedIn', 'withdrawn', 'notCheckedIn'].some(status => status === team.status)) return null
      seen.add(team.teamId)
      if (team.status === 'checkedIn') teamIds.push(team.teamId)
    }
    if (teamIds.length < 2) return null
    return freeze({ competitionId: id, sessionUserId: user, revision: value.revision, teamIds })
  } catch { return null }
}

export function createTournamentManagementDraft(context: TournamentManagementContext): TournamentManagementDraft {
  return {
    format: '', teams: context.teamIds.map(teamId => ({ teamId, seed: '' })), randomSeed: '', matchGames: '',
    clock: { mainMinutes: '5', byoyomiSeconds: '30' },
    swiss: { rounds: '', points: { win: '', draw: '', loss: '', bye: '' }, credit: { numerator: '', denominator: '' },
      floor: { mode: '', value: '' }, cap: { mode: '', value: '' }, excludeByes: '', tieMode: '', tieBreakers: [] },
  }
}

/** 收集設定而非排程／排名演算法；尚未選定的政策不能從 fixture 補值。 */
export function parseTournamentStageDraft(context: TournamentManagementContext, input: unknown): TournamentStageSettings {
  try {
    const value = exact(input, ['format', 'teams', 'randomSeed', 'matchGames', 'clock', 'swiss'])
    if (!value || !isFormat(value.format) || !Array.isArray(value.teams)
      || value.teams.length !== context.teamIds.length || context.teamIds.length < 2
      || new Set(context.teamIds).size !== context.teamIds.length) invalid()
    const seenIds = new Set<string>()
    const seenSeeds = new Set<number>()
    const teams: StageConfig['teams'] = []
    for (const inputTeam of value.teams) {
      const team = exact(inputTeam, ['teamId', 'seed'])
      if (!team || typeof team.teamId !== 'string' || !context.teamIds.includes(team.teamId) || seenIds.has(team.teamId)) {
        invalid('參賽隊伍必須與已鎖定名單完全一致。')
      }
      const seed = integerText(team.seed, 1, 0xffff_ffff)
      if (seenSeeds.has(seed)) invalid('每隊種子必須是不同的正整數。')
      seenIds.add(team.teamId)
      seenSeeds.add(seed)
      teams.push({ teamId: team.teamId, seed })
    }
    const clock = exact(value.clock, ['mainMinutes', 'byoyomiSeconds'])
    if (!clock) invalid()
    const mainMinutes = integerText(clock.mainMinutes, 0, Math.floor(Number.MAX_SAFE_INTEGER / 60_000))
    const byoyomiMs = byoyomiTime(clock.byoyomiSeconds)
    const format = value.format
    return {
      format, teams, randomSeed: integerText(value.randomSeed, 0, Number.MAX_SAFE_INTEGER),
      matchFormat: { type: format === 'singleElimination' || format === 'doubleElimination' ? 'firstToWins' : 'fixedGames',
        value: integerText(value.matchGames, 1, 0xffff) },
      clockConfig: { mainTimeMs: mainMinutes * 60_000, actionTimeMs: byoyomiMs, outOfTurnChoiceTimeMs: byoyomiMs },
      swissRounds: format === 'swiss' ? swissRounds(value.swiss) : null,
      swissPolicy: format === 'swiss' ? swissPolicy(value.swiss) : null,
    }
  } catch (error) {
    if (error instanceof TournamentManagementValidationError) throw error
    throw new TournamentManagementValidationError()
  }
}

export function buildTournamentCreateIntent(
  context: TournamentManagementContext, input: unknown, uuid: () => string = () => crypto.randomUUID(),
): TournamentCreateIntent {
  const settings = parseTournamentStageDraft(context, input)
  if (!normalizedId(context.competitionId) || normalizedId(context.competitionId) !== context.competitionId
    || !normalizedId(context.sessionUserId) || normalizedId(context.sessionUserId) !== context.sessionUserId
    || !positiveInteger(context.revision, Number.MAX_SAFE_INTEGER - 1)) invalid()
  const stageId = uuid()
  const commandId = uuid()
  if (!isUuid(stageId) || !isUuid(commandId) || stageId === commandId) invalid('無法建立命令識別，請稍後重試。')
  const intent: TournamentCreateIntent = {
    scope: { competitionId: context.competitionId, sessionUserId: context.sessionUserId },
    path: `/api/competitions/${encodeURIComponent(context.competitionId)}/tournament`, method: 'POST',
    body: { commandId, expectedRevision: context.revision, stage: { stageId, ...settings } },
  }
  return freeze(intent)
}

/** 先驗原五欄收據，再明列 adapter 驗同一 Rust public snapshot；GET 永遠不替代收據。 */
export function parseTournamentCreateAck(input: unknown, intent: TournamentCreateIntent): { revision: number, idempotent: boolean } {
  try {
    const value = exact(input, ['competitionId', 'revision', 'commandId', 'idempotent', 'projection'])
    if (!value || !positiveInteger(value.revision, Number.MAX_SAFE_INTEGER)
      || value.competitionId !== intent.scope.competitionId || value.commandId !== intent.body.commandId
      || value.revision !== intent.body.expectedRevision + 1 || typeof value.idempotent !== 'boolean') throw new TournamentCreateProtocolError()
    const projection = exact(value.projection, ['ok', 'operation', 'public'])
    if (!projection || projection.ok !== true || projection.operation !== 'tournamentCreate') throw new TournamentCreateProtocolError()
    const parsed = parseCompetitionTournamentViewResponse({ competitionId: value.competitionId, revision: value.revision,
      projection: { ok: true, operation: 'tournamentView', public: projection.public } }, intent.scope.competitionId)
    const stage = parsed.stages[0]
    if (parsed.stages.length !== 1 || !stage || !sameSettings(stage.config, intent.body.stage)) throw new TournamentCreateProtocolError()
    return { revision: value.revision, idempotent: value.idempotent }
  } catch { throw new TournamentCreateProtocolError() }
}

/** 按鈕每次只呼叫一次；公開排程 seed 與牌組／房間的秘密 seed 無關。 */
export function generateTournamentScheduleRandomSeed(
  fill: (target: Uint32Array) => void = target => { crypto.getRandomValues(target) },
): number {
  const target = new Uint32Array(1)
  fill(target)
  const seed = target[0]
  if (seed === undefined) invalid('無法產生排程亂數種子，請自行輸入。')
  return seed
}

/** 首次確定未提交的拒絕才可放下 intent；曾未知後任何拒絕均不能解收據。 */
export function keepTournamentCreateIntent(wasUnknown: boolean, error: unknown): boolean {
  if (wasUnknown) return true
  try {
    if (!isRecord(error)) return true
    const value = error
    const status = value.statusCode ?? value.status
    const data = record(value.data)
    const inner = data && record(data.data)
    const code = inner?.code ?? data?.code
    const rejected: Record<string, number> = {
      invalidRequest: 400, invalidStageConfig: 400, invalidStageTeams: 400, forbidden: 403,
      notInitialized: 404, competitionIdMismatch: 404, revisionConflict: 409, registrationNotLocked: 409,
      stageTeamMismatch: 409, tournamentAlreadyExists: 409, commandIdConflict: 409, engineRejected: 422,
    }
    return !(typeof code === 'string' && Object.hasOwn(rejected, code) && rejected[code] === status)
  } catch { return true }
}

export function tournamentManagementValidationMessage(error: unknown): string {
  return error instanceof TournamentManagementValidationError ? error.message : '賽程設定不完整或格式不正確，請檢查各欄位。'
}

function swissRounds(input: unknown): number {
  const value = record(input)
  if (!value) invalid()
  return integerText(value.rounds, 1, 0xffff_ffff)
}
function swissPolicy(input: unknown): SwissStandingPolicy {
  const value = exact(input, ['rounds', 'points', 'credit', 'floor', 'cap', 'excludeByes', 'tieMode', 'tieBreakers'])
  if (!value) invalid()
  const points = exact(value.points, ['win', 'draw', 'loss', 'bye'])
  const credit = exact(value.credit, ['numerator', 'denominator'])
  if (!points || !credit || (value.excludeByes !== 'yes' && value.excludeByes !== 'no')
    || (value.tieMode !== 'none' && value.tieMode !== 'configured') || !Array.isArray(value.tieBreakers)) invalid()
  const numerator = integerText(credit.numerator, 0, 0xffff_ffff)
  const denominator = integerText(credit.denominator, 1, 0xffff_ffff)
  if (numerator > denominator) invalid('對手勝率的和局折算必須介於零與一之間。')
  const floor = percent(value.floor)
  const cap = percent(value.cap)
  if (floor !== null && cap !== null && floor > cap) invalid('對手勝率下限不可高於上限。')
  const tieBreakers: SwissTieBreaker[] = []
  if (value.tieMode === 'configured') {
    for (const tie of value.tieBreakers) {
      if (!isTieBreaker(tie) || tieBreakers.includes(tie)) invalid('同分比較順序不可重複。')
      tieBreakers.push(tie)
    }
    if (tieBreakers.length === 0) invalid('請設定同分比較順序，或選擇不設定。')
  } else if (value.tieBreakers.length !== 0) invalid()
  return { matchPoints: { win: integerText(points.win, 0, 0xffff_ffff), draw: integerText(points.draw, 0, 0xffff_ffff),
    loss: integerText(points.loss, 0, 0xffff_ffff), bye: integerText(points.bye, 0, 0xffff_ffff) },
    drawCredit: { numerator, denominator }, opponentWinRateFloorPercent: floor, opponentWinRateCapPercent: cap,
    excludeByesFromOpponentWinRate: value.excludeByes === 'yes', tieBreakers }
}
function percent(input: unknown): number | null {
  const value = exact(input, ['mode', 'value'])
  if (!value) invalid()
  if (value.mode === 'none') return null
  if (value.mode !== 'value') invalid('請選擇是否設定對手勝率上下限。')
  return integerText(value.value, 0, 100)
}
function byoyomiTime(input: unknown): number {
  if (input !== '30' && input !== '60' && input !== '180') invalid('讀秒僅可選擇 30 秒、1 分鐘或 3 分鐘。')
  return Number(input) * 1000
}
function integerText(input: unknown, min: number, max: number): number {
  if (typeof input !== 'string' || !/^\d+$/.test(input.trim())) invalid('各項設定都必須是整數，不能留空或填小數。')
  const number = Number(input.trim())
  if (!Number.isSafeInteger(number) || number < min || number > max) invalid('整數設定超出允許範圍。')
  return number
}
function isFormat(input: unknown): input is TournamentFormat {
  return input === 'singleElimination' || input === 'roundRobin' || input === 'doubleElimination' || input === 'swiss'
}
function isTieBreaker(input: unknown): input is SwissTieBreaker {
  return input === 'matchPoints' || input === 'headToHeadMatchPoints' || input === 'opponentWinRate'
    || input === 'netGameWins' || input === 'seedOrder'
}
function positiveInteger(input: unknown, max: number): input is number {
  return typeof input === 'number' && Number.isSafeInteger(input) && input > 0 && input <= max
}
function normalizedId(input: unknown): string | null { return typeof input === 'string' && input.trim() ? input.trim() : null }
function isUuid(input: unknown): input is string {
  return typeof input === 'string' && /^[\da-f]{8}-(?:[\da-f]{4}-){3}[\da-f]{12}$/i.test(input)
}
function invalid(message?: string): never { throw new TournamentManagementValidationError(message) }
function record(input: unknown): Record<string, unknown> | null {
  if (!isRecord(input)) return null
  const prototype = Object.getPrototypeOf(input)
  if (prototype !== Object.prototype && prototype !== null) return null
  for (const key of Reflect.ownKeys(input)) {
    if (typeof key !== 'string') return null
    const descriptor = Object.getOwnPropertyDescriptor(input, key)
    if (!descriptor || !descriptor.enumerable || !('value' in descriptor)) return null
  }
  return input
}
function isRecord(input: unknown): input is Record<string, unknown> {
  return typeof input === 'object' && input !== null && !Array.isArray(input)
}
function exact(input: unknown, keys: string[]): Record<string, unknown> | null {
  const value = record(input)
  return value && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key)) ? value : null
}
function freeze<T>(value: T): T {
  if (typeof value === 'object' && value !== null) {
    for (const child of Object.values(value)) freeze(child)
    Object.freeze(value)
  }
  return value
}
function sameSettings(left: unknown, right: unknown): boolean {
  if (left === right) return true
  if (Array.isArray(left) && Array.isArray(right)) return left.length === right.length && left.every((item, i) => sameSettings(item, right[i]))
  const a = record(left)
  const b = record(right)
  return !!a && !!b && Object.keys(a).length === Object.keys(b).length
    && Object.keys(a).every(key => Object.hasOwn(b, key) && sameSettings(a[key], b[key]))
}
