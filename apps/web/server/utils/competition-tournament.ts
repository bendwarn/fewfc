import { createError, type H3Event } from 'h3'
import type { CompetitionRoomTournamentReadRequest, CompetitionRoomTournamentReadValue } from '../../shared/competition-room'
import type { JsonValue } from '../../shared/competition'
import { workerEnv } from './worker-env'

type TournamentReadRpc = {
  readTournament(request: CompetitionRoomTournamentReadRequest): Promise<unknown>
}

type TournamentNamespace = {
  idFromName(name: string): unknown
  get(id: unknown): unknown
}

const HTTP_ERRORS = {
  invalidRequest: { statusCode: 400, statusMessage: 'Bad Request', message: '賽事識別碼或查詢格式不正確。' },
  notInitialized: { statusCode: 404, statusMessage: 'Not Found', message: '找不到這場賽事。' },
  competitionIdMismatch: { statusCode: 404, statusMessage: 'Not Found', message: '找不到這場賽事。' },
  notParticipant: { statusCode: 403, statusMessage: 'Forbidden', message: '你沒有這場賽事的存取權。' },
  forbidden: { statusCode: 403, statusMessage: 'Forbidden', message: '你沒有這場賽事的存取權。' },
  notReady: { statusCode: 409, statusMessage: 'Conflict', message: '賽事目前尚未準備就緒。' },
  invalidTournamentCheckpoint: { statusCode: 502, statusMessage: 'Bad Gateway', message: '賽事房間回應格式不正確。' },
  engineProtocolError: { statusCode: 502, statusMessage: 'Bad Gateway', message: '賽事房間回應格式不正確。' },
  competitionRoomProtocolError: { statusCode: 502, statusMessage: 'Bad Gateway', message: '賽事房間回應格式不正確。' },
  competitionRoomUnavailable: { statusCode: 503, statusMessage: 'Service Unavailable', message: '賽事服務暫時無法使用，請稍後重試。' },
} as const

type HttpErrorCode = keyof typeof HTTP_ERRORS
type RoomFailureCode = Extract<HttpErrorCode,
  | 'invalidRequest'
  | 'notInitialized'
  | 'competitionIdMismatch'
  | 'notParticipant'
  | 'forbidden'
  | 'notReady'
  | 'invalidTournamentCheckpoint'
  | 'engineProtocolError'
>

const PRIVATE_PUBLIC_KEYS = new Set([
  'announcement', 'authorizedprivate', 'binding', 'calleruserid', 'captain', 'cards', 'commandid',
  'deck', 'decks', 'decklist', 'decklists', 'deckseed', 'decksnapshot', 'gameseed', 'history',
  'lockeddeck', 'lockeddecks', 'lockedroster', 'memberids', 'members',
  'player', 'playerids', 'players', 'private', 'privateprojection', 'privateroster', 'roster',
  'rosterversion', 'serverstate', 'seedmaterial', 'trustedtimems',
])

function httpError(code: HttpErrorCode) {
  const error = HTTP_ERRORS[code]
  return createError({
    statusCode: error.statusCode,
    statusMessage: error.statusMessage,
    message: error.message,
    data: { code, message: error.message },
  })
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  if (!isRecord(value)) return false
  const prototype = Object.getPrototypeOf(value)
  return prototype === Object.prototype || prototype === null
}

function isSynchronousFunction(value: unknown): value is (...args: never[]) => unknown {
  return typeof value === 'function' && Object.getPrototypeOf(value) === Function.prototype
}

function hasExactKeys(
  value: unknown,
  expected: readonly string[],
  allowRootDisposer = false,
): value is Record<string, unknown> {
  if (!isPlainRecord(value)) return false
  const keys = Reflect.ownKeys(value)
  const stringKeys = keys.filter((key): key is string => typeof key === 'string')
  const symbolKeys = keys.filter((key): key is symbol => typeof key === 'symbol')
  if (stringKeys.length !== expected.length || !expected.every(key => Object.hasOwn(value, key))) return false
  if (symbolKeys.length > 0 && !(allowRootDisposer
    && symbolKeys.length === 1 && symbolKeys[0] === Symbol.dispose)) return false

  for (const key of expected) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key)
    if (!descriptor || !descriptor.enumerable || !('value' in descriptor)) return false
  }
  if (symbolKeys.length === 1) {
    const disposer = Object.getOwnPropertyDescriptor(value, Symbol.dispose)
    if (!disposer || !('value' in disposer) || !isSynchronousFunction(disposer.value)) return false
  }
  return true
}

function privateKeyName(key: string): string {
  return key.replace(/[_-]/g, '').toLowerCase()
}

function isJsonPublicValue(
  value: unknown,
  ancestors = new Set<object>(),
  depth = 0,
): value is JsonValue {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return true
  if (typeof value === 'number') return Number.isFinite(value)
  if (typeof value !== 'object' || depth > 128 || ancestors.has(value)) return false

  ancestors.add(value)
  try {
    if (Array.isArray(value)) {
      if (Object.getPrototypeOf(value) !== Array.prototype) return false
      const keys = Reflect.ownKeys(value)
      if (keys.length !== value.length + 1 || keys.some(key => typeof key !== 'string')) return false
      const lengthDescriptor = Object.getOwnPropertyDescriptor(value, 'length')
      if (!lengthDescriptor || lengthDescriptor.enumerable) return false
      for (let index = 0; index < value.length; index += 1) {
        const descriptor = Object.getOwnPropertyDescriptor(value, String(index))
        if (!descriptor || !descriptor.enumerable || !('value' in descriptor)
          || !isJsonPublicValue(descriptor.value, ancestors, depth + 1)) return false
      }
      return true
    }

    if (!isPlainRecord(value)) return false
    for (const key of Reflect.ownKeys(value)) {
      if (typeof key !== 'string' || PRIVATE_PUBLIC_KEYS.has(privateKeyName(key))) return false
      const descriptor = Object.getOwnPropertyDescriptor(value, key)
      if (!descriptor || !descriptor.enumerable || !('value' in descriptor)
        || !isJsonPublicValue(descriptor.value, ancestors, depth + 1)) return false
    }
    return true
  } finally {
    ancestors.delete(value)
  }
}

/** 路徑賽事識別碼只在 HTTP server 邊界 trim；空值不會 dispatch。 */
export function normalizeCompetitionTournamentPathId(value: unknown): string {
  if (typeof value !== 'string' || !value.trim()) throw httpError('invalidRequest')
  return value.trim()
}

/** 唯讀 request 的 actor 只接受 requireSession 的 canonical user id。 */
export function buildCompetitionTournamentReadRequest(
  competitionId: unknown,
  callerUserId: unknown,
): CompetitionRoomTournamentReadRequest {
  const normalizedCompetitionId = normalizeCompetitionTournamentPathId(competitionId)
  if (typeof callerUserId !== 'string' || !callerUserId.trim()) throw httpError('invalidRequest')
  return { competitionId: normalizedCompetitionId, callerUserId }
}

/** GET 不接受任何 caller、時間、版本或 projection query；也拒絕 request body。 */
export function assertCompetitionTournamentQuery(query: unknown, bodyPresent = false): void {
  if (bodyPresent || !isRecord(query) || Reflect.ownKeys(query).length !== 0) {
    throw httpError('invalidRequest')
  }
}

function isRoomFailureCode(value: unknown): value is RoomFailureCode {
  return typeof value === 'string'
    && Object.hasOwn(HTTP_ERRORS, value)
    && value !== 'competitionRoomProtocolError'
    && value !== 'competitionRoomUnavailable'
}

function projectReadResult(
  input: unknown,
  expectedCompetitionId: string,
  allowRootDisposer: boolean,
): CompetitionRoomTournamentReadValue {
  try {
    if (!hasExactKeys(input, ['ok', 'value'], allowRootDisposer) && !hasExactKeys(input, ['ok', 'error'], allowRootDisposer)) {
      throw httpError('competitionRoomProtocolError')
    }
    if (input.ok === false) {
      if (!hasExactKeys(input, ['ok', 'error'], allowRootDisposer)
        || !hasExactKeys(input.error, ['code', 'message'])
        || typeof input.error.code !== 'string'
        || typeof input.error.message !== 'string'
        || !isRoomFailureCode(input.error.code)) {
        throw httpError('competitionRoomProtocolError')
      }
      throw httpError(input.error.code)
    }

    if (input.ok !== true || !hasExactKeys(input, ['ok', 'value'], allowRootDisposer)) {
      throw httpError('competitionRoomProtocolError')
    }
    const value = input.value
    if (!hasExactKeys(value, ['competitionId', 'revision', 'projection'])
      || value.competitionId !== expectedCompetitionId
      || typeof value.revision !== 'number'
      || !Number.isSafeInteger(value.revision)
      || value.revision < 1
      || !hasExactKeys(value.projection, ['ok', 'operation', 'public'])
      || value.projection.ok !== true
      || value.projection.operation !== 'tournamentView') {
      throw httpError('competitionRoomProtocolError')
    }

    const publicValue = value.projection.public
    if (!hasExactKeys(publicValue, ['tournamentId', 'stages'])
      || publicValue.tournamentId !== expectedCompetitionId
      || !Array.isArray(publicValue.stages)
      || publicValue.stages.length === 0
      || !publicValue.stages.every(stage => isPlainRecord(stage))
      || !isJsonPublicValue(publicValue)) {
      throw httpError('competitionRoomProtocolError')
    }

    return {
      competitionId: value.competitionId,
      revision: value.revision,
      projection: { ok: true, operation: 'tournamentView', public: publicValue as JsonValue },
    }
  } catch (error) {
    if (isRecord(error) && typeof error.statusCode === 'number'
      && typeof error.statusMessage === 'string'
      && isRecord(error.data) && typeof error.data.code === 'string'
      && Object.hasOwn(HTTP_ERRORS, error.data.code)) {
      throw error
    }
    throw httpError('competitionRoomProtocolError')
  }
}

/** 測試可直接驗證 semantic DTO；native RPC disposer 僅由 read helper 的 transport scope 處理。 */
export function projectCompetitionTournamentReadResult(
  input: unknown,
  expectedCompetitionId: string,
): CompetitionRoomTournamentReadValue {
  return projectReadResult(input, expectedCompetitionId, false)
}

function isTournamentNamespace(value: unknown): value is TournamentNamespace {
  return isRecord(value)
    && typeof value.idFromName === 'function'
    && typeof value.get === 'function'
}

function isTournamentReadRpc(value: unknown): value is TournamentReadRpc {
  return isRecord(value) && typeof value.readTournament === 'function'
}

/** 只呼叫 dedicated readTournament 一次；不 fallback、不重試，並清理最外層 RPC resource。 */
export async function readCompetitionTournament(
  event: H3Event,
  competitionId: unknown,
  callerUserId: unknown,
): Promise<CompetitionRoomTournamentReadValue> {
  const request = buildCompetitionTournamentReadRequest(competitionId, callerUserId)
  let room: unknown
  let rawResult: unknown
  try {
    const namespace: unknown = workerEnv(event).COMPETITION_ROOM
    if (!isTournamentNamespace(namespace)) throw new Error('tournament query unavailable')
    room = namespace.get(namespace.idFromName(request.competitionId))
    if (!isTournamentReadRpc(room)) throw new Error('tournament query unavailable')
    rawResult = await room.readTournament(request)
  } catch {
    throw httpError('competitionRoomUnavailable')
  }

  let disposer: unknown
  try {
    if (isRecord(rawResult)) {
      const descriptor = Object.getOwnPropertyDescriptor(rawResult, Symbol.dispose)
      if (descriptor && 'value' in descriptor && isSynchronousFunction(descriptor.value)) {
        disposer = descriptor.value
      }
    }
    return projectReadResult(rawResult, request.competitionId, typeof disposer === 'function')
  } catch (error) {
    if (isRecord(error) && typeof error.statusCode === 'number'
      && typeof error.statusMessage === 'string'
      && isRecord(error.data) && typeof error.data.code === 'string'
      && Object.hasOwn(HTTP_ERRORS, error.data.code)) {
      throw error
    }
    throw httpError('competitionRoomProtocolError')
  } finally {
    if (typeof disposer === 'function') {
      try {
        disposer.call(rawResult)
      } catch {
        throw httpError('competitionRoomUnavailable')
      }
    }
  }
}
