import { createError, type H3Event } from 'h3'
import type { ClockCheckpointWindow, ClockDisplay, ClockDisplayTeam } from '../../shared/competition'
import type { CompetitionRoomMatchClockReadRequest, CompetitionRoomMatchClockReadValue } from '../../shared/competition-room'
import { workerEnv } from './worker-env'

type MatchClockReadRpc = {
  readMatchClock(request: CompetitionRoomMatchClockReadRequest): Promise<unknown>
}

const HTTP_ERRORS = {
  invalidRequest: { statusCode: 400, statusMessage: 'Bad Request', message: '賽事、交手識別碼或查詢格式不正確。' },
  serverTimeUnavailable: { statusCode: 503, statusMessage: 'Service Unavailable', message: '伺服器時間暫時無法使用。' },
  notInitialized: { statusCode: 404, statusMessage: 'Not Found', message: '找不到這場賽事。' },
  competitionIdMismatch: { statusCode: 404, statusMessage: 'Not Found', message: '找不到這場賽事。' },
  matchNotFound: { statusCode: 404, statusMessage: 'Not Found', message: '找不到這場交手。' },
  forbidden: { statusCode: 403, statusMessage: 'Forbidden', message: '你沒有讀取這場交手時計的權限。' },
  notParticipant: { statusCode: 403, statusMessage: 'Forbidden', message: '你沒有讀取這場交手時計的權限。' },
  notReady: { statusCode: 409, statusMessage: 'Conflict', message: '交手目前尚未準備就緒。' },
  trustedTimeRegression: { statusCode: 409, statusMessage: 'Conflict', message: '賽事時間狀態無法倒退。' },
  invalidMatchCheckpoint: { statusCode: 409, statusMessage: 'Conflict', message: '交手狀態無法安全驗證。' },
  invalidClockCheckpoint: { statusCode: 409, statusMessage: 'Conflict', message: '時計狀態無法安全驗證。' },
  invalidLockedRoster: { statusCode: 409, statusMessage: 'Conflict', message: '參賽名單狀態無法安全驗證。' },
  invalidGameProvision: { statusCode: 409, statusMessage: 'Conflict', message: '交手房間狀態無法安全驗證。' },
  engineProtocolError: { statusCode: 502, statusMessage: 'Bad Gateway', message: '賽事服務回應格式不正確。' },
  competitionMatchClockProtocolError: { statusCode: 502, statusMessage: 'Bad Gateway', message: '賽事服務回應格式不正確。' },
  internalFailure: { statusCode: 503, statusMessage: 'Service Unavailable', message: '賽事服務暫時無法使用。' },
  competitionMatchClockUnavailable: { statusCode: 503, statusMessage: 'Service Unavailable', message: '賽事服務暫時無法使用。' },
} as const

function httpError(code: keyof typeof HTTP_ERRORS) {
  const error = HTTP_ERRORS[code]
  return createError({ ...error, data: { code, message: error.message } })
}

/** 路徑識別碼只在 HTTP server trim；session 身分保留 canonical 原值。 */
export function normalizeCompetitionMatchClockPathId(value: unknown): string {
  if (typeof value !== 'string' || !value.trim()) throw httpError('invalidRequest')
  return value.trim()
}

export function buildCompetitionMatchClockReadRequest(
  competitionId: unknown,
  matchId: unknown,
  callerUserId: unknown,
  trustedTimeMs: number,
): CompetitionRoomMatchClockReadRequest {
  if (typeof callerUserId !== 'string' || !callerUserId.trim()
    || !isNonNegativeInteger(trustedTimeMs)) throw httpError('invalidRequest')
  return {
    competitionId: normalizeCompetitionMatchClockPathId(competitionId),
    matchId: normalizeCompetitionMatchClockPathId(matchId),
    callerUserId,
    trustedTimeMs,
  }
}

/** 只取樣一次；不能以 caller 時間或重試取樣替代不合法伺服器時間。 */
export function serverCompetitionMatchClockTime(clock: () => number = Date.now): number {
  try {
    const sampledAtMs = clock()
    if (!isNonNegativeInteger(sampledAtMs)) throw httpError('serverTimeUnavailable')
    return sampledAtMs
  } catch {
    throw httpError('serverTimeUnavailable')
  }
}

function isNonNegativeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
}

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false
  const prototype = Object.getPrototypeOf(value)
  return prototype === Object.prototype || prototype === null
}

function hasExactDataKeys(value: unknown, keys: readonly string[], allowRootDisposer = false): value is Record<string, unknown> {
  if (!isPlainRecord(value)) return false
  const actual = Reflect.ownKeys(value)
  const strings = actual.filter(key => typeof key === 'string')
  const symbols = actual.filter(key => typeof key === 'symbol')
  return strings.length === keys.length
    && (symbols.length === 0 || (allowRootDisposer && symbols.length === 1 && symbols[0] === Symbol.dispose))
    && keys.every(key => {
      const descriptor = Object.getOwnPropertyDescriptor(value, key)
      return Boolean(descriptor?.enumerable && Object.hasOwn(descriptor, 'value'))
    })
}

function isDataArray(value: unknown): value is unknown[] {
  if (!Array.isArray(value) || value.length > 2 || Object.getPrototypeOf(value) !== Array.prototype) return false
  const keys = Reflect.ownKeys(value)
  return keys.length === value.length + 1 && keys.every(key => typeof key === 'string')
    && Array.from({ length: value.length }, (_, index) => {
      const descriptor = Object.getOwnPropertyDescriptor(value, String(index))
      return Boolean(descriptor?.enumerable && Object.hasOwn(descriptor, 'value'))
    }).every(Boolean)
}

function isId(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0
}

function isLimit(value: unknown): value is number | null {
  return value === null || isNonNegativeInteger(value)
}

class ClockReadFailure extends Error {
  constructor(readonly code: keyof typeof HTTP_ERRORS) { super(code) }
}

function protocolError() {
  return new ClockReadFailure('competitionMatchClockProtocolError')
}

function parseTeam(value: unknown): ClockDisplayTeam {
  if (!hasExactDataKeys(value, ['team', 'mainTimeRemainingMs', 'byoyomiTimeRemainingMs', 'byoyomiActive'])
    || !isId(value.team) || !isLimit(value.mainTimeRemainingMs) || !isLimit(value.byoyomiTimeRemainingMs)
    || typeof value.byoyomiActive !== 'boolean'
    || (value.byoyomiActive && value.mainTimeRemainingMs !== 0)) throw protocolError()
  return {
    team: value.team, mainTimeRemainingMs: value.mainTimeRemainingMs,
    byoyomiTimeRemainingMs: value.byoyomiTimeRemainingMs, byoyomiActive: value.byoyomiActive,
  }
}

function parseWindow(value: unknown, teams: readonly string[]): ClockCheckpointWindow | null {
  if (value === null) return null
  if (!isPlainRecord(value)) throw protocolError()
  const descriptor = Object.getOwnPropertyDescriptor(value, 'type')
  if (!descriptor?.enumerable || !Object.hasOwn(descriptor, 'value')) throw protocolError()
  const type: unknown = descriptor.value
  // 僅驗顯示視窗的 shape／兩隊關聯；不自行計算扣時、deadline 或裁判結果。
  if (type === 'preparation' && hasExactDataKeys(value, ['type', 'teams'])
    && isDataArray(value.teams) && value.teams.length >= 1 && value.teams.length <= 2) {
    const selected: string[] = []
    for (const team of value.teams) {
      if (!isId(team) || !teams.includes(team) || selected.includes(team)) throw protocolError()
      selected.push(team)
    }
    return { type: 'preparation', teams: selected }
  }
  if (type === 'action' && hasExactDataKeys(value, ['type', 'team', 'turn'])
    && isId(value.team) && teams.includes(value.team) && isNonNegativeInteger(value.turn)) {
    return { type: 'action', team: value.team, turn: value.turn }
  }
  if (type === 'outOfTurnChoice' && hasExactDataKeys(value, ['type', 'team', 'choiceId', 'remainingMs'])
    && isId(value.team) && teams.includes(value.team) && isId(value.choiceId) && isLimit(value.remainingMs)) {
    return { type: 'outOfTurnChoice', team: value.team, choiceId: value.choiceId, remainingMs: value.remainingMs }
  }
  throw protocolError()
}

function parseClock(value: unknown, sampledAtMs: number): ClockDisplay {
  if (!hasExactDataKeys(value, ['sampledAtMs', 'teams', 'running', 'paused', 'awaitingAuthority'])
    || !isNonNegativeInteger(value.sampledAtMs) || value.sampledAtMs !== sampledAtMs
    || !isDataArray(value.teams) || value.teams.length !== 2
    || typeof value.awaitingAuthority !== 'boolean' || (value.running !== null && value.paused !== null)) throw protocolError()
  const teams = value.teams.map(parseTeam)
  const [first, second] = teams
  if (!first || !second || first.team === second.team) throw protocolError()
  const teamIds = teams.map(team => team.team)
  return {
    sampledAtMs: value.sampledAtMs, teams,
    running: parseWindow(value.running, teamIds), paused: parseWindow(value.paused, teamIds),
    awaitingAuthority: value.awaitingAuthority,
  }
}

function parseGame(value: unknown): CompetitionRoomMatchClockReadValue['game'] {
  if (value === null) return null
  if (!hasExactDataKeys(value, ['gameId', 'gameNumber']) || !isId(value.gameId)
    || !isNonNegativeInteger(value.gameNumber) || value.gameNumber < 1) throw protocolError()
  return { gameId: value.gameId, gameNumber: value.gameNumber }
}

function isMatchStatus(value: unknown): value is CompetitionRoomMatchClockReadValue['matchStatus'] {
  return value === 'open' || value === 'awaitingFirstPlayer' || value === 'awaitingReferee' || value === 'completed'
}

function isRoomFailureCode(value: unknown): value is keyof typeof HTTP_ERRORS {
  return typeof value === 'string' && Object.hasOwn(HTTP_ERRORS, value)
    && value !== 'serverTimeUnavailable' && value !== 'competitionMatchClockUnavailable'
    && value !== 'competitionMatchClockProtocolError'
}

function projectReadResult(
  input: unknown,
  request: CompetitionRoomMatchClockReadRequest,
  allowRootDisposer: boolean,
): CompetitionRoomMatchClockReadValue {
  try {
    if (hasExactDataKeys(input, ['ok', 'error'], allowRootDisposer) && input.ok === false) {
      if (!hasExactDataKeys(input.error, ['code', 'message']) || !isRoomFailureCode(input.error.code)
        || typeof input.error.message !== 'string') throw protocolError()
      throw new ClockReadFailure(input.error.code)
    }
    if (!hasExactDataKeys(input, ['ok', 'value'], allowRootDisposer) || input.ok !== true) throw protocolError()
    const value = input.value
    if (!hasExactDataKeys(value, ['competitionId', 'matchId', 'matchRevision', 'matchStatus', 'game', 'clock'])
      || value.competitionId !== request.competitionId || value.matchId !== request.matchId
      || !isNonNegativeInteger(value.matchRevision) || value.matchRevision < 1 || !isMatchStatus(value.matchStatus)) throw protocolError()
    const game = parseGame(value.game)
    if (value.matchStatus !== 'open' && game !== null) throw protocolError()
    return {
      competitionId: request.competitionId, matchId: request.matchId,
      matchRevision: value.matchRevision, matchStatus: value.matchStatus, game,
      clock: parseClock(value.clock, request.trustedTimeMs),
    }
  } catch (error) {
    // 只轉換本 helper 的固定拒絕；不回傳不可信 getter／Proxy 丟出的 H3 error。
    throw httpError(error instanceof ClockReadFailure ? error.code : 'competitionMatchClockProtocolError')
  }
}

/** 純 semantic DTO 檢核；native root resource 只由 transport scope 接受與清理。 */
export function projectCompetitionMatchClockReadResult(
  input: unknown,
  request: CompetitionRoomMatchClockReadRequest,
): CompetitionRoomMatchClockReadValue {
  return projectReadResult(input, request, false)
}

/** GET 不接收任何 query/body；無效 caller 欄位必須在 RPC 前拒絕。 */
export function assertCompetitionMatchClockQuery(query: unknown, bodyPresent = false): void {
  try {
    if (!bodyPresent && hasExactDataKeys(query, [])) return
  } catch { /* 不反射 Proxy／getter 錯誤。 */ }
  throw httpError('invalidRequest')
}

function isCapability(value: unknown): value is object {
  return (typeof value === 'object' && value !== null) || typeof value === 'function'
}

function isSynchronousFunction(value: unknown): value is (this: unknown) => unknown {
  return typeof value === 'function' && Object.getPrototypeOf(value) === Function.prototype
}

function captureRootResource(value: unknown): { valid: boolean; dispose?: (this: unknown) => unknown } {
  if (!isCapability(value)) return { valid: false }
  try {
    const symbols = Object.getOwnPropertySymbols(value)
    const descriptor = Object.getOwnPropertyDescriptor(value, Symbol.dispose)
    let dispose: ((this: unknown) => unknown) | undefined
    if (descriptor) {
      const candidate: unknown = descriptor.value
      if (!Object.hasOwn(descriptor, 'value') || !isSynchronousFunction(candidate)) return { valid: false }
      dispose = candidate
    }
    return {
      valid: symbols.every(symbol => symbol === Symbol.dispose),
      ...(dispose ? { dispose } : {}),
    }
  } catch {
    return { valid: false }
  }
}

/** 只派 readMatchClock 一次；平台 capability 不套 JSON plain-object guard。 */
export async function readCompetitionMatchClock(
  event: H3Event,
  request: CompetitionRoomMatchClockReadRequest,
): Promise<CompetitionRoomMatchClockReadValue> {
  if (!hasExactDataKeys(request, ['competitionId', 'matchId', 'callerUserId', 'trustedTimeMs'])) throw httpError('invalidRequest')
  const safeRequest = buildCompetitionMatchClockReadRequest(request.competitionId, request.matchId, request.callerUserId, request.trustedTimeMs)
  let invoke: MatchClockReadRpc['readMatchClock']
  let raw: unknown
  try {
    const namespace: unknown = workerEnv(event).COMPETITION_ROOM
    if (!isCapability(namespace)) throw new Error('missing namespace')
    const idFromName: unknown = Reflect.get(namespace, 'idFromName')
    const get: unknown = Reflect.get(namespace, 'get')
    if (typeof idFromName !== 'function' || typeof get !== 'function') throw new Error('missing namespace capability')
    const stub: unknown = Reflect.apply(get, namespace, [Reflect.apply(idFromName, namespace, [safeRequest.competitionId])])
    if (!isCapability(stub)) throw new Error('missing stub')
    const method: unknown = Reflect.get(stub, 'readMatchClock')
    if (typeof method !== 'function') throw new Error('missing read capability')
    invoke = async input => Reflect.apply(method, stub, [input])
    raw = await invoke(safeRequest)
  } catch {
    throw httpError('competitionMatchClockUnavailable')
  }
  const root = captureRootResource(raw)
  try {
    if (!root.valid) throw httpError('competitionMatchClockProtocolError')
    return projectReadResult(raw, safeRequest, true)
  } finally {
    if (root.dispose) {
      try {
        if (root.dispose.call(raw) !== undefined) throw new Error('disposer must be synchronous and void')
      } catch {
        throw httpError('competitionMatchClockUnavailable')
      }
    }
  }
}
