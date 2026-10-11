import { createError, type H3Event } from 'h3'
import type {
  CompetitionRoomParticipantGameReadRequest,
  CompetitionRoomParticipantGameReadValue,
} from '../../shared/competition-room'
import { workerEnv } from './worker-env'

type ParticipantGameRpc = {
  readParticipantGame(
    request: CompetitionRoomParticipantGameReadRequest,
  ): Promise<unknown>
}

type ParticipantGameNamespace = {
  idFromName(name: string): unknown
  get(id: unknown): unknown
}

const HTTP_ERRORS = {
  invalidRequest: { statusCode: 400, statusMessage: 'Bad Request', safeMessage: '賽事或交手識別碼格式不正確。' },
  notInitialized: { statusCode: 404, statusMessage: 'Not Found', safeMessage: '找不到這場競賽對局。' },
  competitionIdMismatch: { statusCode: 404, statusMessage: 'Not Found', safeMessage: '找不到這場競賽對局。' },
  matchNotFound: { statusCode: 404, statusMessage: 'Not Found', safeMessage: '找不到這場競賽對局。' },
  forbidden: { statusCode: 403, statusMessage: 'Forbidden', safeMessage: '你沒有這場競賽對局的存取權。' },
  notParticipant: { statusCode: 403, statusMessage: 'Forbidden', safeMessage: '你沒有這場競賽對局的存取權。' },
  invalidMatchCheckpoint: { statusCode: 409, statusMessage: 'Conflict', safeMessage: '對局狀態目前無法安全確認，請稍後重試。' },
  invalidLockedRoster: { statusCode: 409, statusMessage: 'Conflict', safeMessage: '參賽名單目前無法安全確認，請稍後重試。' },
  invalidGameProvision: { statusCode: 409, statusMessage: 'Conflict', safeMessage: '目前對局尚未就緒，請稍後重新讀取。' },
  competitionRoomProtocolError: { statusCode: 502, statusMessage: 'Bad Gateway', safeMessage: '競賽房間回應格式不正確。' },
  competitionRoomUnavailable: { statusCode: 503, statusMessage: 'Service Unavailable', safeMessage: '競賽服務暫時無法使用，請稍後重試。' },
} as const

type HttpErrorCode = keyof typeof HTTP_ERRORS
type RoomFailureCode = Extract<HttpErrorCode,
  | 'invalidRequest'
  | 'notInitialized'
  | 'competitionIdMismatch'
  | 'matchNotFound'
  | 'forbidden'
  | 'notParticipant'
  | 'invalidMatchCheckpoint'
  | 'invalidLockedRoster'
  | 'invalidGameProvision'
>

function participantGameError(code: HttpErrorCode) {
  const error = HTTP_ERRORS[code]
  return createError({
    statusCode: error.statusCode,
    statusMessage: error.statusMessage,
    message: error.safeMessage,
    data: { code, statusMessage: error.safeMessage },
  })
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function hasExactKeys(
  value: Record<string, unknown>,
  expected: readonly string[],
  allowRpcDisposer = false,
): boolean {
  const actual = Reflect.ownKeys(value)
  const stringKeys = actual.filter((key): key is string => typeof key === 'string')
  const symbolKeys = actual.filter((key): key is symbol => typeof key === 'symbol')
  const onlyPlatformDisposer = allowRpcDisposer
    && symbolKeys.length === 1
    && symbolKeys[0] === Symbol.dispose
    && typeof Reflect.get(value, Symbol.dispose) === 'function'
  return stringKeys.length === expected.length
    && expected.every(key => Object.hasOwn(value, key))
    && (symbolKeys.length === 0 || onlyPlatformDisposer)
}

function disposeRpcResponse(value: unknown): void {
  if (!isRecord(value)) return
  const disposer: unknown = Reflect.get(value, Symbol.dispose)
  if (typeof disposer === 'function') disposer.call(value)
}

function isRoomFailureCode(value: unknown): value is RoomFailureCode {
  return typeof value === 'string'
    && Object.hasOwn(HTTP_ERRORS, value)
    && value !== 'competitionRoomProtocolError'
    && value !== 'competitionRoomUnavailable'
}

function isParticipantGameNamespace(value: unknown): value is ParticipantGameNamespace {
  return isRecord(value)
    && typeof value.idFromName === 'function'
    && typeof value.get === 'function'
}

function isParticipantGameRpc(value: unknown): value is ParticipantGameRpc {
  return isRecord(value) && typeof value.readParticipantGame === 'function'
}

/** 路徑識別碼只在 server 邊界 trim；空值不會觸發 DO dispatch。 */
export function normalizeCompetitionParticipantGamePathId(value: unknown): string {
  if (typeof value !== 'string' || !value.trim()) {
    throw participantGameError('invalidRequest')
  }
  return value.trim()
}

/** caller 只由可信 session 提供，保留其 canonical id，不從客戶端欄位讀取。 */
export function buildCompetitionParticipantGameReadRequest(
  competitionId: unknown,
  matchId: unknown,
  callerUserId: unknown,
): CompetitionRoomParticipantGameReadRequest {
  const normalizedCompetitionId = normalizeCompetitionParticipantGamePathId(competitionId)
  const normalizedMatchId = normalizeCompetitionParticipantGamePathId(matchId)
  if (typeof callerUserId !== 'string' || !callerUserId.trim()) {
    throw participantGameError('invalidRequest')
  }
  return {
    competitionId: normalizedCompetitionId,
    matchId: normalizedMatchId,
    callerUserId,
  }
}

/** GET 專用查詢不接受任何客戶端參數，避免 caller、game 或版本被偽造。 */
export function assertCompetitionParticipantGameQuery(query: unknown, bodyPresent = false): void {
  if (bodyPresent || !isRecord(query) || Reflect.ownKeys(query).length !== 0) {
    throw participantGameError('invalidRequest')
  }
}

/** 只投影共享契約白名單；錯誤訊息、私有資料及未知欄位永不回傳。 */
export function projectCompetitionParticipantGameResult(
  input: unknown,
  expectedCompetitionId: string,
  expectedMatchId: string,
): CompetitionRoomParticipantGameReadValue {
  if (!isRecord(input) || typeof input.ok !== 'boolean') {
    throw participantGameError('competitionRoomProtocolError')
  }

  if (input.ok === false) {
    if (!hasExactKeys(input, ['ok', 'error'], true) || !isRecord(input.error)
      || !hasExactKeys(input.error, ['code', 'message'])
      || typeof input.error.message !== 'string'
      || !isRoomFailureCode(input.error.code)) {
      throw participantGameError('competitionRoomProtocolError')
    }
    throw participantGameError(input.error.code)
  }

  if (!hasExactKeys(input, ['ok', 'value'], true) || !isRecord(input.value)) {
    throw participantGameError('competitionRoomProtocolError')
  }
  const value = input.value
  const resultCompetitionId = value.competitionId
  const resultMatchId = value.matchId
  const matchRevision = value.matchRevision
  if (!hasExactKeys(value, ['competitionId', 'matchId', 'matchRevision', 'game'])
    || typeof resultCompetitionId !== 'string'
    || resultCompetitionId !== expectedCompetitionId
    || typeof resultMatchId !== 'string'
    || resultMatchId !== expectedMatchId
    || typeof matchRevision !== 'number'
    || !Number.isSafeInteger(matchRevision)
    || matchRevision < 1) {
    throw participantGameError('competitionRoomProtocolError')
  }

  let game: CompetitionRoomParticipantGameReadValue['game'] = null
  if (value.game !== null) {
    const gameValue = value.game
    if (!isRecord(gameValue) || !hasExactKeys(gameValue, ['gameId', 'gameNumber'])) {
      throw participantGameError('competitionRoomProtocolError')
    }
    const gameId = gameValue.gameId
    const gameNumber = gameValue.gameNumber
    if (typeof gameId !== 'string' || !gameId.trim() || gameId !== gameId.trim()
      || typeof gameNumber !== 'number'
      || !Number.isSafeInteger(gameNumber)
      || gameNumber < 1) throw participantGameError('competitionRoomProtocolError')
    game = {
      gameId,
      gameNumber,
    }
  }

  return {
    competitionId: resultCompetitionId,
    matchId: resultMatchId,
    matchRevision,
    game,
  }
}

/** 固定參賽者唯讀 RPC；無 generic read fallback、重試或時間／版本輸入。 */
export async function readCompetitionParticipantGame(
  event: H3Event,
  competitionId: unknown,
  matchId: unknown,
  callerUserId: unknown,
): Promise<CompetitionRoomParticipantGameReadValue> {
  const request = buildCompetitionParticipantGameReadRequest(competitionId, matchId, callerUserId)
  let result: unknown
  try {
    const namespace: unknown = workerEnv(event).COMPETITION_ROOM
    if (!isParticipantGameNamespace(namespace)) throw new Error('participant query unavailable')
    const room: unknown = namespace.get(namespace.idFromName(request.competitionId))
    if (!isParticipantGameRpc(room)) throw new Error('participant query unavailable')
    result = await room.readParticipantGame(request)
  } catch {
    throw participantGameError('competitionRoomUnavailable')
  }

  try {
    return projectCompetitionParticipantGameResult(
      result,
      request.competitionId,
      request.matchId,
    )
  } finally {
    try {
      disposeRpcResponse(result)
    } catch {
      throw participantGameError('competitionRoomUnavailable')
    }
  }
}
