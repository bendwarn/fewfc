import type { ClockCheckpointWindow, ClockDisplay, ClockDisplayTeam } from '../../shared/competition'
import type { CompetitionRoomMatchClockReadValue } from '../../shared/competition-room'

export interface CompetitionClockScope {
  competitionId: string
  matchId: string
  gameNumber: number
  gameId: string
  sessionUserId: string
}

export interface CompetitionClockScopeInput {
  mode: unknown
  gameId: unknown
  roomGameId: unknown
  sessionUserId: unknown
  memberUserId: unknown
}

function hasExactDataKeys(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false
  const prototype = Object.getPrototypeOf(value)
  return (prototype === Object.prototype || prototype === null)
    && Reflect.ownKeys(value).length === keys.length
    && keys.every(key => {
      const descriptor = Object.getOwnPropertyDescriptor(value, key)
      return Boolean(descriptor?.enumerable && Object.hasOwn(descriptor, 'value'))
    })
}

function isId(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0
}

function isNonNegativeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
}

/** 身分只比對 session 與已接受的固定成員；路徑不能替舊房間 metadata 提供權威。 */
export function createCompetitionClockScope(input: CompetitionClockScopeInput): CompetitionClockScope | null {
  try {
    const { mode, gameId, roomGameId, sessionUserId, memberUserId } = input
    if (!hasExactDataKeys(mode, ['kind', 'context']) || mode.kind !== 'competition'
      || !hasExactDataKeys(mode.context, ['competitionId', 'matchId', 'gameNumber'])
      || !isId(mode.context.competitionId) || mode.context.competitionId !== mode.context.competitionId.trim()
      || !isId(mode.context.matchId) || mode.context.matchId !== mode.context.matchId.trim()
      || !isNonNegativeInteger(mode.context.gameNumber) || mode.context.gameNumber < 1
      || !isId(gameId) || !isId(roomGameId) || gameId.trim() !== roomGameId
      || !isId(sessionUserId) || sessionUserId !== memberUserId) return null
    return {
      competitionId: mode.context.competitionId, matchId: mode.context.matchId,
      gameNumber: mode.context.gameNumber, gameId: roomGameId, sessionUserId,
    }
  } catch {
    return null
  }
}

/** URL 只有路徑識別碼；session、時間、gameId 與 revision 不進入 HTTP 輸入。 */
export function competitionClockPath(scope: CompetitionClockScope): string {
  return `/api/competitions/${encodeURIComponent(scope.competitionId)}/matches/${encodeURIComponent(scope.matchId)}/clock`
}

export class CompetitionClockProtocolError extends Error {
  constructor() { super('時計回應無法安全驗證，已停止自動更新。') }
}

function rejectClock(): never {
  throw new CompetitionClockProtocolError()
}

function isLimit(value: unknown): value is number | null {
  return value === null || isNonNegativeInteger(value)
}

function isDataArray(value: unknown): value is unknown[] {
  if (!Array.isArray(value) || value.length > 2 || Object.getPrototypeOf(value) !== Array.prototype) return false
  return Reflect.ownKeys(value).length === value.length + 1
    && Array.from({ length: value.length }, (_, index) => {
      const descriptor = Object.getOwnPropertyDescriptor(value, String(index))
      return Boolean(descriptor?.enumerable && Object.hasOwn(descriptor, 'value'))
    }).every(Boolean)
}

function parseTeam(value: unknown): ClockDisplayTeam {
  if (!hasExactDataKeys(value, ['team', 'mainTimeRemainingMs', 'byoyomiTimeRemainingMs', 'byoyomiActive'])
    || !isId(value.team) || !isLimit(value.mainTimeRemainingMs) || !isLimit(value.byoyomiTimeRemainingMs)
    || typeof value.byoyomiActive !== 'boolean' || (value.byoyomiActive && value.mainTimeRemainingMs !== 0)) rejectClock()
  return {
    team: value.team, mainTimeRemainingMs: value.mainTimeRemainingMs,
    byoyomiTimeRemainingMs: value.byoyomiTimeRemainingMs, byoyomiActive: value.byoyomiActive,
  }
}

function parseWindow(value: unknown, teams: readonly string[]): ClockCheckpointWindow | null {
  if (value === null) return null
  if (hasExactDataKeys(value, ['type', 'teams']) && value.type === 'preparation') {
    const selected = value.teams
    if (!isDataArray(selected) || selected.length === 0) rejectClock()
    const ids: string[] = []
    for (const team of selected) {
      if (!isId(team) || !teams.includes(team) || ids.includes(team)) rejectClock()
      ids.push(team)
    }
    return { type: 'preparation', teams: ids }
  }
  if (hasExactDataKeys(value, ['type', 'team', 'turn']) && value.type === 'action'
    && isId(value.team) && teams.includes(value.team) && isNonNegativeInteger(value.turn)) {
    return { type: 'action', team: value.team, turn: value.turn }
  }
  if (hasExactDataKeys(value, ['type', 'team', 'choiceId', 'remainingMs']) && value.type === 'outOfTurnChoice'
    && isId(value.team) && teams.includes(value.team) && isId(value.choiceId) && isLimit(value.remainingMs)) {
    return { type: 'outOfTurnChoice', team: value.team, choiceId: value.choiceId, remainingMs: value.remainingMs }
  }
  return rejectClock()
}

function parseClock(value: unknown): ClockDisplay {
  if (!hasExactDataKeys(value, ['sampledAtMs', 'teams', 'running', 'paused', 'awaitingAuthority'])
    || !isNonNegativeInteger(value.sampledAtMs) || typeof value.awaitingAuthority !== 'boolean'
    || !isDataArray(value.teams) || value.teams.length !== 2
    || (value.running !== null && value.paused !== null)) rejectClock()
  const teams = value.teams.map(parseTeam)
  if (teams[0]?.team === teams[1]?.team) rejectClock()
  const ids = teams.map(team => team.team)
  return {
    sampledAtMs: value.sampledAtMs, teams, running: parseWindow(value.running, ids),
    paused: parseWindow(value.paused, ids), awaitingAuthority: value.awaitingAuthority,
  }
}

function parseGame(value: unknown, scope: CompetitionClockScope): CompetitionRoomMatchClockReadValue['game'] {
  if (value === null) return null
  if (!hasExactDataKeys(value, ['gameId', 'gameNumber']) || value.gameId !== scope.gameId
    || value.gameNumber !== scope.gameNumber) rejectClock()
  return { gameId: scope.gameId, gameNumber: scope.gameNumber }
}

function isMatchStatus(value: unknown): value is CompetitionRoomMatchClockReadValue['matchStatus'] {
  return value === 'open' || value === 'awaitingFirstPlayer' || value === 'awaitingReferee' || value === 'completed'
}

/** 只檢核顯示 DTO，不重寫核心計時、裁判或終局演算法。全部資料另建白名單副本。 */
export function parseCompetitionClockResponse(input: unknown, scope: CompetitionClockScope): CompetitionRoomMatchClockReadValue {
  try {
    if (!hasExactDataKeys(input, ['competitionId', 'matchId', 'matchRevision', 'matchStatus', 'game', 'clock'])
      || input.competitionId !== scope.competitionId || input.matchId !== scope.matchId
      || !isNonNegativeInteger(input.matchRevision) || input.matchRevision < 1 || !isMatchStatus(input.matchStatus)) rejectClock()
    const game = parseGame(input.game, scope)
    if (input.matchStatus !== 'open' && game !== null) rejectClock()
    return {
      competitionId: scope.competitionId, matchId: scope.matchId, matchRevision: input.matchRevision,
      matchStatus: input.matchStatus, game, clock: parseClock(input.clock),
    }
  } catch {
    // 不反射不可信 getter／Proxy 錯誤，也不接受 RPC resource symbol 當 HTTP JSON 欄位。
    return rejectClock()
  }
}

/** 僅單位呈現，保留毫秒精度；不使用瀏覽器時間調整伺服器取樣。 */
export function formatCompetitionClockTime(milliseconds: number | null): string {
  if (milliseconds === null) return '不限時'
  if (!isNonNegativeInteger(milliseconds)) return '時間無法顯示'
  const seconds = Math.floor(milliseconds / 1000).toLocaleString('zh-TW')
  const fraction = String(milliseconds % 1000).padStart(3, '0').replace(/0+$/, '')
  return `${seconds}${fraction ? `.${fraction}` : ''} 秒`
}

/** 只讀 status 作固定對照；不顯示 server message、錯誤內容或私有資料。 */
export function competitionClockErrorMessage(error: unknown): string {
  let status: unknown
  try {
    if (error instanceof CompetitionClockProtocolError) return '時計回應無法安全驗證，已停止自動更新。'
    if (typeof error === 'object' && error !== null) {
      if ('statusCode' in error) status = error.statusCode
      if (status === undefined && 'status' in error) status = error.status
    }
  } catch { /* 不反射不可信錯誤物件的 getter／Proxy。 */ }
  switch (status) {
    case 400: return '時計查詢資料無法確認，已停止自動更新。'
    case 401: return '請重新登入後查詢時計，已停止自動更新。'
    case 403: return '你沒有讀取這場交手時計的權限，已停止自動更新。'
    case 404: return '找不到這場賽事或交手，已停止自動更新。'
    case 409: return '目前無法取得可安全顯示的時計，已停止自動更新。請稍後重試。'
    case 502: return '時計回應無法安全驗證，已停止自動更新。'
    default: return '時計服務暫時無法使用，已停止自動更新。請手動重試。'
  }
}
