import { createError, type H3Event } from 'h3'
import {
  isCompetitionGameReadyRequest,
  type CompetitionGameReadyRequest,
  type CompetitionGameReadyResult,
} from '../../shared/competition-game-room'
import { workerEnv } from './worker-env'

interface ReadyRpc {
  setCompetitionReady(request: CompetitionGameReadyRequest): Promise<CompetitionGameReadyResult>
}
type ReadyFailureCode = Extract<CompetitionGameReadyResult, { ok: false }>['code']
type ReadyErrorCode = ReadyFailureCode
  | 'competitionGameProtocolError'
  | 'competitionGameRoomUnavailable'

const READY_ERRORS = {
  invalidRequest: { statusCode: 400, statusMessage: 'Bad Request', message: '競賽準備要求格式不正確。' },
  notCompetitionRoom: { statusCode: 404, statusMessage: 'Not Found', message: '找不到競賽對局。' },
  invalidCompetitionRoom: { statusCode: 409, statusMessage: 'Conflict', message: '競賽房間狀態無法驗證。' },
  fixedMemberRequired: { statusCode: 403, statusMessage: 'Forbidden', message: '僅固定參賽者可設定準備狀態。' },
  playerNotConnected: { statusCode: 409, statusMessage: 'Conflict', message: '請先連線至競賽房間。' },
  lockedDeckMissing: { statusCode: 409, statusMessage: 'Conflict', message: '競賽鎖定牌組尚未就緒。' },
  gameAlreadyStarted: { statusCode: 409, statusMessage: 'Conflict', message: '對局已開始，無法變更準備狀態。' },
  gameFinished: { statusCode: 409, statusMessage: 'Conflict', message: '對局已結束，無法變更準備狀態。' },
  roomNotWaiting: { statusCode: 409, statusMessage: 'Conflict', message: '房間目前不在準備階段。' },
  competitionGameProtocolError: { statusCode: 502, statusMessage: 'Bad Gateway', message: '競賽房間回應格式不正確。' },
  competitionGameRoomUnavailable: { statusCode: 503, statusMessage: 'Service Unavailable', message: '競賽房間暫時無法使用，請稍後重試。' },
} satisfies Record<ReadyErrorCode, { statusCode: number; statusMessage: string; message: string }>

export function competitionGameReadyError(code: ReadyErrorCode) {
  const error = READY_ERRORS[code]
  // HTTP reason 使用固定 ASCII，中文僅放安全 message／data，絕不採用 RPC 訊息。
  return createError({ ...error, data: { code, message: error.message } })
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  const actualKeys = Reflect.ownKeys(value)
  return actualKeys.length === keys.length && keys.every(key => Object.hasOwn(value, key))
}

export function normalizeCompetitionGameId(value: unknown): string {
  if (typeof value !== 'string' || !value.trim()) {
    throw competitionGameReadyError('invalidRequest')
  }
  return value.trim()
}

export function buildCompetitionGameReadyRequest(
  body: unknown,
  actorUserId: string,
): CompetitionGameReadyRequest {
  if (!isRecord(body) || !hasExactKeys(body, ['ready']) || typeof body.ready !== 'boolean') {
    throw competitionGameReadyError('invalidRequest')
  }
  const request = { actorUserId, ready: body.ready }
  if (!isCompetitionGameReadyRequest(request)) {
    throw competitionGameReadyError('invalidRequest')
  }
  return request
}

function hasReadyRpc(value: unknown): value is ReadyRpc {
  return isRecord(value) && typeof value.setCompetitionReady === 'function'
}

function isReadyFailureCode(value: unknown): value is ReadyFailureCode {
  return typeof value === 'string'
    && Object.hasOwn(READY_ERRORS, value)
    && value !== 'competitionGameProtocolError'
    && value !== 'competitionGameRoomUnavailable'
}

function projectReadyResult(value: unknown, desiredReady: boolean): { ready: boolean } {
  if (!isRecord(value)) throw competitionGameReadyError('competitionGameProtocolError')
  if (value.ok === true && hasExactKeys(value, ['ok', 'ready'])
    && typeof value.ready === 'boolean' && value.ready === desiredReady) {
    return { ready: value.ready }
  }
  if (value.ok === false && hasExactKeys(value, ['ok', 'code']) && isReadyFailureCode(value.code)) {
    throw competitionGameReadyError(value.code)
  }
  throw competitionGameReadyError('competitionGameProtocolError')
}

/** RPC disposer 屬傳輸生命週期，僅剝離已知同步資源鍵；JSON 語意仍維持 exact。 */
function projectReadyRpcResult(value: unknown, desiredReady: boolean): { ready: boolean } {
  let disposer: unknown
  let semanticValue: unknown
  try {
    const record = isRecord(value) ? value : undefined
    const hasDisposer = record !== undefined && Object.hasOwn(record, Symbol.dispose)
    if (hasDisposer) disposer = Reflect.get(record, Symbol.dispose)

    if (!record || (hasDisposer && typeof disposer !== 'function')) {
      throw competitionGameReadyError('competitionGameProtocolError')
    }
    const keys = record.ok === true ? ['ok', 'ready'] : ['ok', 'code']
    const semanticKeys = Reflect.ownKeys(record).filter(key => key !== Symbol.dispose)
    if (semanticKeys.length !== keys.length || !keys.every(key => Object.hasOwn(record, key))) {
      throw competitionGameReadyError('competitionGameProtocolError')
    }
    // 只複製白名單欄位；原 RPC 物件與資源不得逃逸到 HTTP 回應。
    semanticValue = Object.fromEntries(keys.map(key => [key, record[key]]))
  } catch {
    throw competitionGameReadyError('competitionGameProtocolError')
  } finally {
    if (typeof disposer === 'function') {
      try {
        Reflect.apply(disposer, value, [])
      } catch {
        // 釋放失敗亦固定拒絕，不反射傳輸或遠端內部例外。
        throw competitionGameReadyError('competitionGameProtocolError')
      }
    }
  }
  return projectReadyResult(semanticValue, desiredReady)
}

/** 局部驗證專用 RPC 能力；不得降級為 fetch、toggle 或帳號牌組查詢。 */
export async function callCompetitionGameReady(
  event: H3Event,
  gameId: string,
  request: CompetitionGameReadyRequest,
): Promise<{ ready: boolean }> {
  const normalizedGameId = normalizeCompetitionGameId(gameId)
  if (!isCompetitionGameReadyRequest(request)) throw competitionGameReadyError('invalidRequest')

  let result: unknown
  try {
    const namespace = workerEnv(event).GAME_ROOM
    if (!namespace || typeof namespace.idFromName !== 'function' || typeof namespace.get !== 'function') {
      throw competitionGameReadyError('competitionGameRoomUnavailable')
    }
    const room: unknown = namespace.get(namespace.idFromName(normalizedGameId))
    if (!hasReadyRpc(room)) throw competitionGameReadyError('competitionGameRoomUnavailable')
    result = await room.setCompetitionReady(request)
  } catch {
    // 不反射 binding、傳輸例外或 DO 內部訊息，且不在此自動翻轉／重試。
    throw competitionGameReadyError('competitionGameRoomUnavailable')
  }
  return projectReadyRpcResult(result, request.ready)
}
