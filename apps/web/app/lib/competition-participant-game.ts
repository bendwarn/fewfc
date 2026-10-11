export interface CompetitionParticipantGame {
  gameId: string
  gameNumber: number
}

export interface CompetitionParticipantGameProjection {
  competitionId: string
  matchId: string
  matchRevision: number
  game: CompetitionParticipantGame | null
}

export type ParticipantGameEntryLifecycle = 'draft' | 'open' | 'locked'
export type ParticipantGameEntryTeamStatus = 'registered' | 'checkedIn' | 'withdrawn' | 'notCheckedIn'

export interface ParticipantGameEntryTeam {
  status: ParticipantGameEntryTeamStatus
  members: readonly string[]
}

export class CompetitionParticipantGameProtocolError extends Error {
  constructor() {
    super('競賽對局回應格式無法辨識。')
    this.name = 'CompetitionParticipantGameProtocolError'
  }
}

/** 路徑 ID 只做 trim 與空值檢查；不推導或改寫識別碼。 */
export function normalizeCompetitionParticipantGameId(value: unknown): string | null {
  if (typeof value !== 'string') return null
  const normalized = value.trim()
  return normalized ? normalized : null
}

/** 鎖定後只允許已報到且名單包含登入者的隊員查詢目前對局。 */
export function canShowCompetitionParticipantGameEntry(
  lifecycle: ParticipantGameEntryLifecycle | null | undefined,
  currentUserId: string | null | undefined,
  team: ParticipantGameEntryTeam | null | undefined,
): boolean {
  if (lifecycle !== 'locked' || !currentUserId?.trim() || !team || team.status !== 'checkedIn') return false
  return team.members.includes(currentUserId)
}

/** 只接受 participant-game GET 的最小 camelCase 投影，不保留未知欄位。 */
export function parseCompetitionParticipantGameResponse(
  input: unknown,
  expectedCompetitionId: string,
  expectedMatchId: string,
): CompetitionParticipantGameProjection {
  const competitionId = normalizeCompetitionParticipantGameId(expectedCompetitionId)
  const matchId = normalizeCompetitionParticipantGameId(expectedMatchId)
  const envelope = asRecord(input)
  if (!competitionId || !matchId || !envelope
    || !hasExactKeys(envelope, ['competitionId', 'matchId', 'matchRevision', 'game'])
    || envelope.competitionId !== competitionId
    || envelope.matchId !== matchId
    || !isSafePositiveInteger(envelope.matchRevision)) {
    throw new CompetitionParticipantGameProtocolError()
  }

  let game: CompetitionParticipantGame | null = null
  if (envelope.game !== null) {
    const value = asRecord(envelope.game)
    if (!value || !hasExactKeys(value, ['gameId', 'gameNumber'])
      || typeof value.gameId !== 'string'
      || !value.gameId
      || value.gameId !== value.gameId.trim()
      || !isSafePositiveInteger(value.gameNumber)) {
      throw new CompetitionParticipantGameProtocolError()
    }
    game = { gameId: value.gameId, gameNumber: value.gameNumber }
  }

  return { competitionId, matchId, matchRevision: envelope.matchRevision, game }
}

/** 只回傳固定安全文案，不展示 HTTP、RPC 或 runtime 的原始錯誤內容。 */
export function competitionParticipantGameErrorMessage(error: unknown): string {
  const record = asRecord(error)
  const status = record?.statusCode ?? record?.status
  if (typeof status !== 'number' || !Number.isSafeInteger(status)) {
    return '暫時無法查詢競賽對局，請檢查連線後重試。'
  }

  switch (status) {
    case 400:
      return '賽事或交手識別碼格式不正確，請重新確認。'
    case 401:
      return '登入狀態已失效，請重新登入後再查詢。'
    case 403:
      return '你沒有這場競賽對局的存取權。'
    case 404:
      return '找不到這場競賽對局，請確認主辦方提供的交手 ID。'
    case 409:
      return '對局狀態目前無法安全確認，請稍後重試。'
    case 502:
      return '競賽房間回應格式不正確，請稍後重試。'
    case 503:
      return '競賽服務暫時無法使用，請稍後重試。'
    default:
      return status >= 500
        ? '競賽服務暫時無法使用，請稍後重試。'
        : '無法查詢這場競賽對局，請重新確認交手 ID。'
  }
}

/** gameId 只有在通過白名單 parser 後才會用於一般房間頁連結。 */
export function competitionParticipantRoomPath(game: CompetitionParticipantGame | null): string | null {
  if (!game || !game.gameId || game.gameId !== game.gameId.trim()
    || !isSafePositiveInteger(game.gameNumber)) return null
  return `/rooms/${encodeURIComponent(game.gameId)}`
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null
}

function hasExactKeys(value: Record<string, unknown>, expected: readonly string[]): boolean {
  const actual = Reflect.ownKeys(value)
  return actual.length === expected.length
    && actual.every((key): key is string => typeof key === 'string')
    && expected.every(key => Object.hasOwn(value, key))
}

function isSafePositiveInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value > 0
}
