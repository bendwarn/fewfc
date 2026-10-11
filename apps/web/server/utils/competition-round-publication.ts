import { createError, getRequestHeader, getRequestURL, type H3Event } from 'h3'
import {
  isCompetitionRoomAnnounceRoundRequest,
  isCompetitionRoomAnnounceRoundValue,
  isCompetitionRoomRoundPublicationReadRequest,
  isCompetitionRoomRoundPublicationReadValue,
  type CompetitionRoomAnnounceRoundRequest,
  type CompetitionRoomAnnounceRoundValue,
  type CompetitionRoomRoundPublicationReadRequest,
  type CompetitionRoomRoundPublicationReadValue,
} from '../../shared/competition-room'
import { competitionRoomError } from './competition-room'
import { workerEnv, type CompetitionRoomNamespaceBinding } from './worker-env'

type RoundPublicationRoomStub = ReturnType<CompetitionRoomNamespaceBinding['get']>
type RoundPublicationValueGuard<Request, Value> = (
  value: unknown,
  request: Request,
) => value is Value

type RootDisposer = (this: unknown) => unknown

interface CapturedRootDisposer {
  valid: boolean
  dispose?: RootDisposer
}

type DecodedRoomResult<Value> =
  | { type: 'success'; value: Value }
  | { type: 'failure'; code: string }
  | { type: 'invalid' }

const MAX_ROUND_NUMBER = 0xffff_ffff
const MAX_EPOCH_MS = 8_640_000_000_000_000

const BOUNDARY_ERRORS = {
  invalidRequest: { statusCode: 400, message: '輪次公告要求格式不正確。' },
  malformedJSON: { statusCode: 400, message: 'JSON 要求格式不正確。' },
  serverTimeUnavailable: { statusCode: 503, message: '伺服器時間暫時無法使用。' },
  competitionRoomUnavailable: { statusCode: 501, message: '賽事服務目前不可用。' },
} as const

const ROUND_ROOM_ERRORS: Record<string, { statusCode: number; message: string }> = {
  invalidRoundPublication: { statusCode: 409, message: '輪次公告狀態無法安全驗證。' },
  roundNotPublishable: { statusCode: 409, message: '目前尚不能公告此輪。' },
  roundAlreadyPublished: { statusCode: 409, message: '此輪已有公告。' },
  invalidRefereeAssignment: { statusCode: 400, message: '裁判指派格式不正確。' },
}

function boundaryError(code: keyof typeof BOUNDARY_ERRORS) {
  const error = BOUNDARY_ERRORS[code]
  return createError({
    statusCode: error.statusCode,
    statusMessage: error.message,
    message: error.message,
    data: { code },
  })
}

function roundRoomError(code: string) {
  const local = Object.hasOwn(ROUND_ROOM_ERRORS, code) ? ROUND_ROOM_ERRORS[code] : undefined
  if (!local) return competitionRoomError({ code })
  return createError({
    statusCode: local.statusCode,
    statusMessage: local.message,
    message: local.message,
    data: { code },
  })
}

/** 路徑身分只在 HTTP 邊界 trim；session 身分與命令識別碼不改寫。 */
export function normalizeCompetitionRoundPublicationPathId(value: unknown): string {
  if (typeof value !== 'string' || value.trim().length === 0) throw boundaryError('invalidRequest')
  return value.trim()
}

/** roundNumber 僅收 canonical 正十進位，避免 URL 數字多種拼法指向同一輪。 */
export function parseCompetitionRoundPublicationRoundNumber(value: unknown): number {
  if (typeof value !== 'string' || !/^[1-9][0-9]*$/.test(value)) {
    throw boundaryError('invalidRequest')
  }
  const roundNumber = Number(value)
  if (!Number.isSafeInteger(roundNumber) || roundNumber > MAX_ROUND_NUMBER) {
    throw boundaryError('invalidRequest')
  }
  return roundNumber
}

export function buildCompetitionRoundPublicationReadRequest(
  competitionId: unknown,
  stageId: unknown,
  roundNumber: unknown,
  callerUserId: unknown,
): CompetitionRoomRoundPublicationReadRequest {
  const request = {
    competitionId: normalizeCompetitionRoundPublicationPathId(competitionId),
    stageId: normalizeCompetitionRoundPublicationPathId(stageId),
    roundNumber: parseCompetitionRoundPublicationRoundNumber(roundNumber),
    callerUserId,
  }
  if (!isCompetitionRoomRoundPublicationReadRequest(request)) throw boundaryError('invalidRequest')
  return request
}

/** 命令本體先做 descriptor exact 檢查，再補入 session actor 與一次可信伺服器時間。 */
export function buildCompetitionRoundAnnouncementRequest(
  competitionId: unknown,
  stageId: unknown,
  roundNumber: unknown,
  callerUserId: unknown,
  body: unknown,
  clock: () => number = Date.now,
): CompetitionRoomAnnounceRoundRequest {
  const pathCompetitionId = normalizeCompetitionRoundPublicationPathId(competitionId)
  const pathStageId = normalizeCompetitionRoundPublicationPathId(stageId)
  const pathRoundNumber = parseCompetitionRoundPublicationRoundNumber(roundNumber)
  const command = extractAnnouncementCommand(body)
  const trustedTimeMs = serverCompetitionRoundPublicationTime(clock)
  const request = {
    competitionId: pathCompetitionId,
    stageId: pathStageId,
    roundNumber: pathRoundNumber,
    callerUserId,
    commandId: command.commandId,
    expectedRevision: command.expectedRevision,
    trustedTimeMs,
    roundStartMs: command.roundStartMs,
    refereeAssignments: command.refereeAssignments,
  }
  if (!isCompetitionRoomAnnounceRoundRequest(request)) throw boundaryError('invalidRequest')
  return request
}

function extractAnnouncementCommand(value: unknown): {
  commandId: unknown
  expectedRevision: unknown
  roundStartMs: unknown
  refereeAssignments: unknown
} {
  if (!hasExactOwnDataKeys(value, [
    'commandId', 'expectedRevision', 'roundStartMs', 'refereeAssignments',
  ])) throw boundaryError('invalidRequest')
  return {
    commandId: value.commandId,
    expectedRevision: value.expectedRevision,
    roundStartMs: value.roundStartMs,
    refereeAssignments: value.refereeAssignments,
  }
}

/** POST 時間必須是 shared round-release epoch 可表示的安全整數。 */
export function serverCompetitionRoundPublicationTime(clock: () => number = Date.now): number {
  try {
    const trustedTimeMs = clock()
    if (!Number.isSafeInteger(trustedTimeMs) || trustedTimeMs < 0 || trustedTimeMs > MAX_EPOCH_MS) {
      throw new Error('invalid server time')
    }
    return trustedTimeMs
  } catch {
    throw boundaryError('serverTimeUnavailable')
  }
}

/** 空白 query 物件、裸問號或任何 GET body 都拒絕；反射異常一律 fail closed。 */
export function assertCompetitionRoundPublicationNoQuery(
  query: unknown,
  queryMarkerPresent: boolean,
  bodyPresent = false,
): void {
  try {
    if (!queryMarkerPresent && !bodyPresent && hasExactOwnDataKeys(query, [])) return
  } catch {
    // 不反射不可信 query 的 getter 或 Proxy 錯誤。
  }
  throw boundaryError('invalidRequest')
}

export function competitionRoundPublicationHasQueryMarker(event: H3Event): boolean {
  try {
    const webRequest = event.web?.request
    if (webRequest) {
      const requestUrl = webRequest.url
      if (typeof requestUrl !== 'string') return true
      return requestUrl.includes('?')
    }

    const nodeRequestUrl = event.node?.req?.url
    if (typeof nodeRequestUrl === 'string' && nodeRequestUrl.length > 0) {
      return nodeRequestUrl.includes('?')
    }
    if (nodeRequestUrl !== undefined && nodeRequestUrl !== null && nodeRequestUrl !== '') return true

    return getRequestURL(event).href.includes('?')
  } catch {
    return true
  }
}

export function competitionRoundPublicationHasRequestBody(event: H3Event): boolean {
  try {
    const requestBody = event._requestBody
    if (requestBody !== undefined && requestBody !== null && requestBody !== '') return true
    if (event.web?.request?.body != null) return true

    const contentLength = getRequestHeader(event, 'content-length')
    if (contentLength !== undefined && contentLength.trim() !== '' && Number(contentLength) !== 0) {
      return true
    }
    return Boolean(getRequestHeader(event, 'transfer-encoding')?.trim())
  } catch {
    return true
  }
}

export function malformedCompetitionRoundPublicationJsonError() {
  return boundaryError('malformedJSON')
}

export async function readCompetitionRoundPublication(
  event: H3Event,
  request: CompetitionRoomRoundPublicationReadRequest,
): Promise<CompetitionRoomRoundPublicationReadValue> {
  if (!isCompetitionRoomRoundPublicationReadRequest(request)) throw boundaryError('invalidRequest')
  return await callRoundPublicationRpc(
    event,
    request.competitionId,
    request,
    room => room.readRoundPublication(request),
    isCompetitionRoomRoundPublicationReadValue,
  )
}

export async function announceCompetitionRound(
  event: H3Event,
  request: CompetitionRoomAnnounceRoundRequest,
): Promise<CompetitionRoomAnnounceRoundValue> {
  if (!isCompetitionRoomAnnounceRoundRequest(request)) throw boundaryError('invalidRequest')
  return await callRoundPublicationRpc(
    event,
    request.competitionId,
    request,
    room => room.announceRound(request),
    isCompetitionRoomAnnounceRoundValue,
  )
}

async function callRoundPublicationRpc<Request, Value>(
  event: H3Event,
  competitionId: string,
  request: Request,
  operation: (room: RoundPublicationRoomStub) => Promise<unknown>,
  valueGuard: RoundPublicationValueGuard<Request, Value>,
): Promise<Value> {
  let namespace: CompetitionRoomNamespaceBinding | undefined
  try {
    namespace = workerEnv(event).COMPETITION_ROOM
  } catch {
    throw boundaryError('competitionRoomUnavailable')
  }
  if (!namespace) throw boundaryError('competitionRoomUnavailable')

  let room: RoundPublicationRoomStub
  try {
    room = namespace.get(namespace.idFromName(competitionId))
  } catch {
    throw boundaryError('competitionRoomUnavailable')
  }

  let raw: unknown
  try {
    raw = await operation(room)
  } catch {
    throw competitionRoomError({ code: 'competitionRoomFailure' })
  }

  const root = captureRootDisposer(raw)
  let decoded: DecodedRoomResult<Value>
  try {
    decoded = decodeRoomResult(raw, request, valueGuard, root.valid)
  } finally {
    if (root.dispose) {
      try {
        if (Reflect.apply(root.dispose, raw, []) !== undefined) {
          throw new Error('invalid synchronous disposer result')
        }
      } catch {
        throw competitionRoomError({ code: 'competitionRoomFailure' })
      }
    }
  }

  if (decoded.type === 'success') return decoded.value
  if (decoded.type === 'failure') throw roundRoomError(decoded.code)
  throw competitionRoomError({ code: 'engineProtocolError' })
}

function decodeRoomResult<Request, Value>(
  raw: unknown,
  request: Request,
  valueGuard: RoundPublicationValueGuard<Request, Value>,
  rootValid: boolean,
): DecodedRoomResult<Value> {
  try {
    if (!rootValid || !isPlainRecord(raw)) return { type: 'invalid' }
    const okDescriptor = Object.getOwnPropertyDescriptor(raw, 'ok')
    if (!okDescriptor || !Object.hasOwn(okDescriptor, 'value')) return { type: 'invalid' }

    if (okDescriptor.value === true) {
      if (!hasExactOwnDataKeys(raw, ['ok', 'value'], true)) return { type: 'invalid' }
      const valueDescriptor = Object.getOwnPropertyDescriptor(raw, 'value')
      if (!valueDescriptor || !Object.hasOwn(valueDescriptor, 'value')
        || !valueGuard(valueDescriptor.value, request)) return { type: 'invalid' }
      const snapshot = structuredClone(valueDescriptor.value)
      if (!valueGuard(snapshot, request)) return { type: 'invalid' }
      return { type: 'success', value: snapshot }
    }

    if (okDescriptor.value !== false || !hasExactOwnDataKeys(raw, ['ok', 'error'], true)) {
      return { type: 'invalid' }
    }
    const errorDescriptor = Object.getOwnPropertyDescriptor(raw, 'error')
    const errorValue = errorDescriptor?.value
    if (!errorDescriptor || !Object.hasOwn(errorDescriptor, 'value')
      || !hasExactOwnDataKeys(errorValue, ['code', 'message'])) return { type: 'invalid' }
    const codeDescriptor = Object.getOwnPropertyDescriptor(errorValue, 'code')
    const messageDescriptor = Object.getOwnPropertyDescriptor(errorValue, 'message')
    if (!codeDescriptor || !Object.hasOwn(codeDescriptor, 'value') || typeof codeDescriptor.value !== 'string'
      || !messageDescriptor || !Object.hasOwn(messageDescriptor, 'value') || typeof messageDescriptor.value !== 'string') {
      return { type: 'invalid' }
    }
    return { type: 'failure', code: codeDescriptor.value }
  } catch {
    return { type: 'invalid' }
  }
}

function captureRootDisposer(value: unknown): CapturedRootDisposer {
  if ((typeof value !== 'object' || value === null) && typeof value !== 'function') {
    return { valid: false }
  }
  try {
    const symbols = Object.getOwnPropertySymbols(value)
    const descriptor = Object.getOwnPropertyDescriptor(value, Symbol.dispose)
    let dispose: RootDisposer | undefined
    if (descriptor) {
      if (!Object.hasOwn(descriptor, 'value') || !isSynchronousFunction(descriptor.value)) {
        return { valid: false }
      }
      dispose = descriptor.value
    }
    return {
      valid: symbols.every(symbol => symbol === Symbol.dispose)
        && symbols.includes(Symbol.dispose) === Boolean(descriptor),
      ...(dispose ? { dispose } : {}),
    }
  } catch {
    return { valid: false }
  }
}

function isSynchronousFunction(value: unknown): value is RootDisposer {
  try {
    return typeof value === 'function' && Object.getPrototypeOf(value) === Function.prototype
  } catch {
    return false
  }
}

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false
  try {
    const prototype = Object.getPrototypeOf(value)
    return prototype === Object.prototype || prototype === null
  } catch {
    return false
  }
}

function hasExactOwnDataKeys(
  value: unknown,
  expected: readonly string[],
  allowRootDisposer = false,
): value is Record<string, unknown> {
  try {
    if (!isPlainRecord(value)) return false
    const actual = Reflect.ownKeys(value)
    const dataKeys = actual.filter(key => !(allowRootDisposer && key === Symbol.dispose))
    return dataKeys.length === expected.length
      && dataKeys.every(key => typeof key === 'string' && expected.includes(key))
      && expected.every(key => {
        const descriptor = Object.getOwnPropertyDescriptor(value, key)
        return descriptor?.enumerable === true && Object.hasOwn(descriptor, 'value')
      })
  } catch {
    return false
  }
}
