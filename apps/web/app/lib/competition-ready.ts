import type { GameRoomCompetitionContext } from '../../shared/game-room'

export type CompetitionRoomMode =
  | { kind: 'ordinary' }
  | { kind: 'competition'; context: GameRoomCompetitionContext }
  | { kind: 'invalid' }

export interface CompetitionReadyEligibilityInput {
  mode: CompetitionRoomMode
  roomStatus: string | null
  sessionUserId: string | null
  memberUserId: string | null
  memberConnected: boolean
  socketConnected: boolean
  loading: boolean
}

export interface CompetitionWaitingMemberStatusInput {
  ready: boolean
  connected: boolean
  owner: boolean
}

function isRecord(value: unknown): value is Record<PropertyKey, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function isCanonicalIdentifier(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0 && value.trim() === value
}

/** 競賽模式只接受 server response 的精確 context；present malformed 不降級成普通房。 */
export function parseGameRoomCompetitionMode(value: unknown): CompetitionRoomMode {
  try {
    if (!isRecord(value)) return { kind: 'invalid' }
    if (!Reflect.has(value, 'competition')) return { kind: 'ordinary' }

    const context = value.competition
    if (!isRecord(context)) return { kind: 'invalid' }
    const keys = Reflect.ownKeys(context)
    if (keys.length !== 3
      || !keys.includes('competitionId')
      || !keys.includes('matchId')
      || !keys.includes('gameNumber')) {
      return { kind: 'invalid' }
    }

    const competitionId = context.competitionId
    const matchId = context.matchId
    const gameNumber = context.gameNumber
    if (!isCanonicalIdentifier(competitionId)
      || !isCanonicalIdentifier(matchId)
      || typeof gameNumber !== 'number'
      || !Number.isSafeInteger(gameNumber)
      || gameNumber < 1) {
      return { kind: 'invalid' }
    }

    return {
      kind: 'competition',
      context: { competitionId, matchId, gameNumber },
    }
  } catch {
    return { kind: 'invalid' }
  }
}

/** 固定席位本人只能在競賽等待中、房間與本人連線且沒有其他房間工作時操作。 */
export function canSetCompetitionReady(input: CompetitionReadyEligibilityInput): boolean {
  if (input.mode.kind !== 'competition'
    || input.roomStatus !== 'Waiting'
    || !input.sessionUserId
    || !input.memberUserId
    || input.sessionUserId.trim() !== input.memberUserId.trim()
    || !input.memberConnected
    || !input.socketConnected
    || input.loading) {
    return false
  }

  return true
}

/** 競賽名單要顯示包含房主在內的每席 ready；普通房維持既有房主標示。 */
export function competitionWaitingMemberStatus(
  member: CompetitionWaitingMemberStatusInput | undefined,
  mode: CompetitionRoomMode,
): string {
  if (!member) return '等待中'
  if (mode.kind === 'invalid') return '狀態無法確認'
  if (!member.connected) return '未連線'
  if (mode.kind === 'competition') return member.ready ? '已準備' : '未準備'
  if (member.owner) return '房主'
  return member.ready ? '已準備' : '未準備'
}

/** HTTP 成功只承認 exact { ready }，而且值必須等於本次要求的狀態。 */
export function isExactCompetitionReadyAck(value: unknown, desired: boolean): boolean {
  try {
    if (!isRecord(value)) return false
    const keys = Reflect.ownKeys(value)
    return keys.length === 1
      && keys[0] === 'ready'
      && typeof value.ready === 'boolean'
      && value.ready === desired
  } catch {
    return false
  }
}

function statusCodeFrom(error: unknown): number | null {
  if (!isRecord(error)) return null
  const candidate = error.statusCode ?? error.status
  return typeof candidate === 'number' && Number.isSafeInteger(candidate) ? candidate : null
}

/** 對使用者只呈現固定文案，不回顯 API／RPC 錯誤內部資料。 */
export function competitionReadyErrorMessage(error: unknown): string {
  if (isRecord(error) && error.name === 'CompetitionReadyProtocolError') {
    return '準備回應格式不正確，狀態尚未確認；可使用相同狀態重試。'
  }

  switch (statusCodeFrom(error)) {
    case 400:
      return '準備狀態無法送出，請重新查詢房間。'
    case 403:
      return '你目前無法變更這場競賽的準備狀態。'
    case 409:
      return '房間狀態已改變，請重新查詢目前狀態。'
    case 502:
    case 503:
      return '競賽服務暫時無法確認準備狀態，可使用相同狀態重試。'
    default:
      return '準備狀態尚未確認，可使用相同狀態重試。'
  }
}
