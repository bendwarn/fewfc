import { createError, type H3Event } from 'h3'
import type { CompetitionRuling } from '../../shared/competition'
import type { CompetitionRoomMatchExecuteRequest } from '../../shared/competition-room'
import { workerEnv } from './worker-env'

type JsonRecord = Record<string, unknown>
type ExecuteMatchStub = {
  executeMatch(request: CompetitionRoomMatchExecuteRequest): Promise<unknown>
}
type ExecuteMatchNamespace = {
  idFromName(name: string): unknown
  get(id: unknown): unknown
}

export interface RulingCorrectionCommand {
  commandId: string
  expectedMatchRevision: number
  correctionId: string
  gameRecordId: string
  replacementRulingRecordId: string
  replacementRuling: CompetitionRuling
  reason: string
}

export interface RulingCorrectionAcknowledgement {
  competitionId: string
  matchId: string
  matchRevision: number
  commandId: string
  idempotent: boolean
}

const ACK_ERROR = {
  statusCode: 502,
  statusMessage: 'CompetitionRulingCorrectionProtocolError',
  data: { code: 'competitionRulingCorrectionProtocolError', message: '賽事服務回應格式不正確。' },
} as const

const TRANSPORT_ERROR = {
  statusCode: 503,
  statusMessage: 'CompetitionRulingCorrectionUnavailable',
  data: { code: 'competitionRulingCorrectionUnavailable', message: '賽事服務暫時無法使用。' },
} as const

const ERROR_RESPONSES: Record<string, { statusCode: number; statusMessage: string; code: string; message: string }> = {
  invalidRequest: { statusCode: 400, statusMessage: 'InvalidCompetitionRulingCorrectionRequest', code: 'invalidRequest', message: '更正要求格式不正確。' },
  invalidOperation: { statusCode: 400, statusMessage: 'InvalidCompetitionRulingCorrectionOperation', code: 'invalidOperation', message: '此更正操作不受支援。' },
  invalidReason: { statusCode: 400, statusMessage: 'InvalidCompetitionRulingCorrectionReason', code: 'invalidReason', message: '更正理由不可空白。' },
  forbidden: { statusCode: 403, statusMessage: 'CompetitionRulingCorrectionForbidden', code: 'forbidden', message: '你沒有更正這場交手判定的權限。' },
  invalidRefereeAcl: { statusCode: 403, statusMessage: 'CompetitionRefereeAuthorizationChanged', code: 'invalidRefereeAcl', message: '裁判授權狀態已變更。' },
  notInitialized: { statusCode: 404, statusMessage: 'CompetitionNotFound', code: 'notInitialized', message: '找不到這個賽事。' },
  competitionIdMismatch: { statusCode: 404, statusMessage: 'CompetitionNotFound', code: 'competitionNotFound', message: '找不到這個賽事。' },
  matchNotFound: { statusCode: 404, statusMessage: 'CompetitionMatchNotFound', code: 'matchNotFound', message: '找不到這場交手。' },
  gameNotFound: { statusCode: 404, statusMessage: 'CompetitionGameNotFound', code: 'gameNotFound', message: '找不到這場對局。' },
  refereeNotFound: { statusCode: 403, statusMessage: 'CompetitionRefereeNotAssigned', code: 'refereeNotFound', message: '登入者不是這場交手的有效裁判。' },
  unknownReferee: { statusCode: 403, statusMessage: 'CompetitionRefereeNotAssigned', code: 'refereeNotFound', message: '登入者不是這場交手的有效裁判。' },
  matchRevisionConflict: { statusCode: 409, statusMessage: 'CompetitionMatchRevisionConflict', code: 'matchRevisionConflict', message: '交手狀態已更新，請重新讀取後再試。' },
  revisionConflict: { statusCode: 409, statusMessage: 'CompetitionRevisionConflict', code: 'revisionConflict', message: '賽事狀態已更新，請重新讀取後再試。' },
  trustedTimeRegression: { statusCode: 409, statusMessage: 'CompetitionTimeConflict', code: 'trustedTimeRegression', message: '賽事時間狀態無法倒退。' },
  commandIdConflict: { statusCode: 409, statusMessage: 'CompetitionCommandConflict', code: 'commandIdConflict', message: '命令識別碼已用於其他要求。' },
  scheduleMismatch: { statusCode: 409, statusMessage: 'CompetitionScheduleConflict', code: 'scheduleMismatch', message: '交手與賽程狀態不一致。' },
  invalidMatchCheckpoint: { statusCode: 409, statusMessage: 'CompetitionMatchCheckpointConflict', code: 'matchCheckpointConflict', message: '交手狀態無法安全驗證。' },
  invalidTournamentCheckpoint: { statusCode: 409, statusMessage: 'CompetitionScheduleCheckpointConflict', code: 'scheduleCheckpointConflict', message: '賽程狀態無法安全驗證。' },
  correctionWouldReopenScheduledResult: { statusCode: 409, statusMessage: 'CompetitionCorrectionWouldReopenSchedule', code: 'correctionWouldReopenSchedule', message: '此更正會使已完成賽程重新開啟。' },
  'engine:downstreamStarted': { statusCode: 409, statusMessage: 'CompetitionDownstreamConflict', code: 'downstreamConflict', message: '更正會影響已開始的後續賽程。' },
  'engine:scheduleError': { statusCode: 409, statusMessage: 'CompetitionScheduleCorrectionRejected', code: 'scheduleConflict', message: '賽程狀態不允許此判定更正。' },
  'engine:matchError': { statusCode: 422, statusMessage: 'CompetitionRulingRejected', code: 'rulingRejected', message: '賽事核心拒絕此判定更正。' },
  internalFailure: { statusCode: 503, statusMessage: 'CompetitionRulingCorrectionUnavailable', code: 'competitionRoomUnavailable', message: '賽事服務暫時無法使用。' },
}

const CLOCK_PROJECTION_OPERATIONS = new Set([
  'clockCreate', 'clockBeginPreparation', 'clockFinishPreparation', 'clockBeginAction',
  'clockBeginOutOfTurnChoice', 'clockObserve', 'clockEndWindow', 'clockPause',
  'clockResume', 'clockNewGame', 'clockRuleGame',
])

const PRIVATE_KEYS = new Set([
  'private', 'authorizedPrivate', 'serverState', 'serverActor', 'callerUserId', 'actor',
  'binding', 'deck', 'decks', 'deckSnapshot', 'deckSeed', 'seed', 'announcement',
  'captain', 'members', 'roster', 'lockedRoster', 'rosterVersion',
])

/** 更正 API 只接受明列的命令欄位；身份、裁判與可信時間都由伺服器補入。 */
export function buildRulingCorrectionCommand(input: unknown): RulingCorrectionCommand {
  if (!hasExactDataKeys(input, [
    'commandId', 'expectedMatchRevision', 'correctionId', 'gameRecordId',
    'replacementRulingRecordId', 'replacementRuling', 'reason',
  ])) {
    throw badRequest('invalidRequest', '更正要求欄位不正確。')
  }
  const body = input
  const commandId = nonEmpty(body.commandId)
  const correctionId = nonEmpty(body.correctionId)
  const gameRecordId = nonEmpty(body.gameRecordId)
  const replacementRulingRecordId = nonEmpty(body.replacementRulingRecordId)
  const reason = nonEmpty(body.reason)
  const expectedMatchRevision = body.expectedMatchRevision
  const replacementRuling = parseShallowRuling(body.replacementRuling)
  if (!commandId || !correctionId || !gameRecordId || !replacementRulingRecordId) {
    throw badRequest('invalidRequest', '更正識別欄位不可空白。')
  }
  if (!reason) throw badRequest('invalidReason', '更正理由不可空白。')
  if (!Number.isSafeInteger(expectedMatchRevision) || (expectedMatchRevision as number) < 0
    || (expectedMatchRevision as number) >= Number.MAX_SAFE_INTEGER) {
    throw badRequest('invalidRequest', '交手版本必須是有效的非負整數。')
  }
  if (!replacementRuling) throw badRequest('invalidRequest', '替代判定格式不正確。')
  return {
    commandId,
    expectedMatchRevision: expectedMatchRevision as number,
    correctionId,
    gameRecordId,
    replacementRulingRecordId,
    replacementRuling,
    reason,
  }
}

export function buildRulingCorrectionRequest(
  competitionId: string,
  matchId: string,
  callerUserId: string,
  command: RulingCorrectionCommand,
  trustedTimeMs: number,
): CompetitionRoomMatchExecuteRequest {
  const normalizedCompetitionId = nonEmpty(competitionId)
  const normalizedMatchId = nonEmpty(matchId)
  const normalizedCallerUserId = nonEmpty(callerUserId)
  if (!normalizedCompetitionId || !normalizedMatchId || !normalizedCallerUserId) {
    throw badRequest('invalidRequest', '賽事、交手或登入者識別碼不正確。')
  }
  if (!Number.isSafeInteger(trustedTimeMs) || trustedTimeMs < 0) {
    throw createError({ statusCode: 503, statusMessage: 'CompetitionServerTimeUnavailable', data: { code: 'competitionServerTimeUnavailable', message: '伺服器時間暫時無法使用。' } })
  }
  return {
    competitionId: normalizedCompetitionId,
    matchId: normalizedMatchId,
    callerUserId: normalizedCallerUserId,
    commandId: command.commandId,
    expectedMatchRevision: command.expectedMatchRevision,
    trustedTimeMs,
    operation: {
      type: 'matchCorrectRuling',
      matchId: normalizedMatchId,
      correctionId: command.correctionId,
      refereeId: normalizedCallerUserId,
      gameRecordId: command.gameRecordId,
      replacementRulingRecordId: command.replacementRulingRecordId,
      replacementRuling: command.replacementRuling,
      reason: command.reason,
    },
  }
}

export function serverRulingCorrectionTime(clock: () => number = Date.now): number {
  const value = clock()
  if (!Number.isSafeInteger(value) || value < 0) {
    throw createError({ statusCode: 503, statusMessage: 'CompetitionServerTimeUnavailable', data: { code: 'competitionServerTimeUnavailable', message: '伺服器時間暫時無法使用。' } })
  }
  return value
}

/** 專用 HTTP capability：只呼叫 executeMatch 一次，不經通用 execute/read fallback。 */
export async function executeRulingCorrection(
  event: H3Event,
  request: CompetitionRoomMatchExecuteRequest,
): Promise<RulingCorrectionAcknowledgement> {
  let invokeExecuteMatch: () => Promise<unknown>
  try {
    const candidateNamespace = workerEnv(event).COMPETITION_ROOM as unknown
    if (!isCapabilityObject(candidateNamespace)) {
      throw new Error('missing binding')
    }
    const namespace = candidateNamespace as unknown as ExecuteMatchNamespace
    const idFromName = Reflect.get(namespace, 'idFromName')
    const get = Reflect.get(namespace, 'get')
    if (typeof idFromName !== 'function' || typeof get !== 'function') {
      throw new Error('missing binding capability')
    }
    const id = Reflect.apply(idFromName, namespace, [request.competitionId])
    const candidate = Reflect.apply(get, namespace, [id])
    if (!isCapabilityObject(candidate)) throw new Error('missing stub')
    const executeMatch = Reflect.get(candidate, 'executeMatch')
    if (typeof executeMatch !== 'function') throw new Error('missing capability')
    invokeExecuteMatch = async () => Reflect.apply(executeMatch, candidate, [request])
  } catch {
    throw createError(TRANSPORT_ERROR)
  }

  let raw: unknown
  try {
    raw = await invokeExecuteMatch()
  } catch {
    throw createError(TRANSPORT_ERROR)
  }

  const root = captureRootDisposer(raw)
  let outcome: { value: RulingCorrectionAcknowledgement } | { error: ReturnType<typeof createError> }
  try {
    outcome = inspectExecuteMatchResponse(raw, request, root.valid)
  } catch {
    outcome = { error: createError(ACK_ERROR) }
  } finally {
    if (root.dispose) {
      try {
        if (root.dispose.call(raw) !== undefined) throw new Error('disposer must be synchronous and void')
      } catch {
        throw createError(TRANSPORT_ERROR)
      }
    }
  }
  if ('error' in outcome) throw outcome.error
  return outcome.value
}

export function projectRulingCorrectionAcknowledgement(
  input: unknown,
  request: CompetitionRoomMatchExecuteRequest,
): RulingCorrectionAcknowledgement {
  if (!hasExactDataKeys(input, [
    'competitionId', 'matchId', 'matchRevision', 'commandId', 'idempotent',
    'timeoutEvidence', 'nextClockDeadlineMs', 'matchProjection', 'clockProjection',
  ])
    || input.competitionId !== request.competitionId
    || input.matchId !== request.matchId
    || input.matchRevision !== request.expectedMatchRevision + 1
    || input.commandId !== request.commandId
    || typeof input.idempotent !== 'boolean'
    || input.timeoutEvidence !== null
    || !(input.nextClockDeadlineMs === null
      || (Number.isSafeInteger(input.nextClockDeadlineMs) && (input.nextClockDeadlineMs as number) >= 0))
    || !isProjection(input.matchProjection, 'matchCorrectRuling')
    || !(input.clockProjection === null || isClockProjection(input.clockProjection))) {
    throw createError(ACK_ERROR)
  }
  return {
    competitionId: request.competitionId,
    matchId: request.matchId,
    matchRevision: input.matchRevision as number,
    commandId: request.commandId,
    idempotent: input.idempotent,
  }
}

function parseShallowRuling(input: unknown): CompetitionRuling | undefined {
  if (!isRecord(input) || typeof input.type !== 'string') return undefined
  switch (input.type) {
    case 'timeoutLoss':
      return hasExactDataKeys(input, ['type', 'loser']) && nonEmpty(input.loser)
        ? { type: 'timeoutLoss', loser: nonEmpty(input.loser)! }
        : undefined
    case 'doubleTimeoutLoss':
      return hasExactDataKeys(input, ['type', 'teams']) && isPairOfStrings(input.teams)
        ? { type: 'doubleTimeoutLoss', teams: [input.teams[0].trim(), input.teams[1].trim()] }
        : undefined
    case 'gameAccepted':
      if (!hasExactDataKeys(input, ['type', 'outcome'])) return undefined
      if (input.outcome === 'draw') return { type: 'gameAccepted', outcome: 'draw' }
      if (hasExactDataKeys(input.outcome, ['winner']) && nonEmpty(input.outcome.winner)) {
        return { type: 'gameAccepted', outcome: { winner: nonEmpty(input.outcome.winner)! } }
      }
      return undefined
    case 'eliminationDrawLoss':
      return hasExactDataKeys(input, ['type', 'loser', 'reason'])
        && nonEmpty(input.loser) && input.reason === 'enteredByoyomiFirst'
        ? { type: 'eliminationDrawLoss', loser: nonEmpty(input.loser)!, reason: 'enteredByoyomiFirst' }
        : undefined
    case 'needsTiebreak':
      if (!hasExactDataKeys(input, ['type', 'continuation'])
        || !hasExactDataKeys(input.continuation, ['teams'])
        || !isArrayPair(input.continuation.teams)) return undefined
      {
        const [first, second] = input.continuation.teams
        if (!validContinuationTeam(first) || !validContinuationTeam(second)) return undefined
        return {
          type: 'needsTiebreak',
          continuation: {
            teams: [
              { team: first.team.trim(), mainTimeRemainingMs: first.mainTimeRemainingMs as number | null },
              { team: second.team.trim(), mainTimeRemainingMs: second.mainTimeRemainingMs as number | null },
            ],
          },
        }
      }
    case 'needsDecision':
      if (!hasExactDataKeys(input, ['type', 'reason']) || typeof input.reason !== 'string') return undefined
      if (input.reason === 'zeroTimeControl' || input.reason === 'simultaneousByoyomiEntry'
        || input.reason === 'unlimitedEliminationDraw' || input.reason === 'zeroTimeEliminationDraw') {
        return { type: 'needsDecision', reason: input.reason }
      }
      return undefined
    default:
      return undefined
  }
}

function validContinuationTeam(value: unknown): value is JsonRecord & { team: string; mainTimeRemainingMs: number | null } {
  return hasExactDataKeys(value, ['team', 'mainTimeRemainingMs'])
    && Boolean(nonEmpty(value.team))
    && (value.mainTimeRemainingMs === null
      || (Number.isSafeInteger(value.mainTimeRemainingMs) && (value.mainTimeRemainingMs as number) >= 0))
}

function isPairOfStrings(value: unknown): value is [string, string] {
  return isArrayPair(value) && nonEmpty(value[0]) !== undefined && nonEmpty(value[1]) !== undefined
}

function isArrayPair(value: unknown): value is [unknown, unknown] {
  if (!Array.isArray(value) || value.length !== 2) return false
  const keys = Reflect.ownKeys(value)
  return keys.length === 3 && keys.includes('0') && keys.includes('1') && keys.includes('length')
    && Object.getOwnPropertyDescriptor(value, '0')?.value !== undefined
    && Object.getOwnPropertyDescriptor(value, '1')?.value !== undefined
}

function isProjection(value: unknown, operation: string): boolean {
  return hasExactDataKeys(value, ['ok', 'operation', 'public'])
    && value.ok === true && value.operation === operation
    && isJsonValue(value.public) && !containsPrivateKeys(value.public)
}

function isClockProjection(value: unknown): boolean {
  return isRecord(value) && typeof value.operation === 'string'
    && CLOCK_PROJECTION_OPERATIONS.has(value.operation)
    && isProjection(value, value.operation)
}

function containsPrivateKeys(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(containsPrivateKeys)
  if (!isRecord(value)) return false
  return Object.entries(value).some(([key, child]) => PRIVATE_KEYS.has(key) || containsPrivateKeys(child))
}

function isJsonValue(value: unknown, seen = new Set<object>()): boolean {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return true
  if (typeof value === 'number') return Number.isFinite(value)
  if (typeof value !== 'object' || seen.has(value)) return false
  seen.add(value)
  let valid = false
  if (Array.isArray(value)) {
    valid = isArrayShape(value) && value.every(item => isJsonValue(item, seen))
  } else if (isRecord(value)) {
    valid = Reflect.ownKeys(value).every(key => typeof key === 'string'
      && Object.getOwnPropertyDescriptor(value, key)?.enumerable === true
      && Object.hasOwn(Object.getOwnPropertyDescriptor(value, key) ?? {}, 'value'))
      && Object.values(value).every(item => isJsonValue(item, seen))
  }
  seen.delete(value)
  return valid
}

function isArrayShape(value: unknown[]): boolean {
  const keys = Reflect.ownKeys(value)
  return keys.length === value.length + 1 && keys.includes('length')
    && Array.from({ length: value.length }, (_, index) => keys.includes(String(index))).every(Boolean)
}

function captureRootDisposer(value: unknown): { valid: boolean; dispose?: (this: unknown) => unknown } {
  if ((typeof value !== 'object' || value === null) && typeof value !== 'function') return { valid: false }
  try {
    const symbols = Object.getOwnPropertySymbols(value)
    const unexpectedSymbols = symbols.filter(symbol => symbol !== Symbol.dispose)
    const descriptor = symbols.includes(Symbol.dispose)
      ? Object.getOwnPropertyDescriptor(value, Symbol.dispose)
      : undefined
    if (symbols.includes(Symbol.dispose)
      && (!descriptor || !Object.hasOwn(descriptor, 'value') || typeof descriptor.value !== 'function'
        || Object.prototype.toString.call(descriptor.value) !== '[object Function]')) {
      return { valid: false }
    }
    return {
      valid: unexpectedSymbols.length === 0,
      ...(descriptor ? { dispose: descriptor.value as (this: unknown) => unknown } : {}),
    }
  } catch {
    return { valid: false }
  }
}

function inspectExecuteMatchResponse(
  raw: unknown,
  request: CompetitionRoomMatchExecuteRequest,
  rootSymbolsValid: boolean,
): { value: RulingCorrectionAcknowledgement } | { error: ReturnType<typeof createError> } {
  try {
    if (!rootSymbolsValid || !isRecord(raw)) return { error: createError(ACK_ERROR) }
    const ok = ownDataValue(raw, 'ok')
    if (typeof ok !== 'boolean') return { error: createError(ACK_ERROR) }
    if (ok === false) {
      if (!hasExactRootDataKeys(raw, ['ok', 'error'])) return { error: createError(ACK_ERROR) }
      const error = ownDataValue(raw, 'error')
      if (!hasExactDataKeys(error, ['code', 'message'])) return { error: createError(ACK_ERROR) }
      const code = ownDataValue(error, 'code')
      const message = ownDataValue(error, 'message')
      if (typeof code !== 'string' || !code || typeof message !== 'string') {
        return { error: createError(ACK_ERROR) }
      }
      const safe = ERROR_RESPONSES[code]
      if (!safe) return { error: createError(ACK_ERROR) }
      return {
        error: createError({
          statusCode: safe.statusCode,
          statusMessage: safe.statusMessage,
          data: { code: safe.code, message: safe.message },
        }),
      }
    }
    if (!hasExactRootDataKeys(raw, ['ok', 'value'])) return { error: createError(ACK_ERROR) }
    return { value: projectRulingCorrectionAcknowledgement(ownDataValue(raw, 'value'), request) }
  } catch {
    return { error: createError(ACK_ERROR) }
  }
}

function ownDataValue(value: unknown, key: string): unknown {
  if (!isRecord(value)) return undefined
  const descriptor = Object.getOwnPropertyDescriptor(value, key)
  return descriptor && Object.hasOwn(descriptor, 'value') ? descriptor.value : undefined
}

function hasExactDataKeys(value: unknown, expected: readonly string[]): value is JsonRecord {
  if (!isRecord(value)) return false
  let keys: (string | symbol)[]
  try {
    keys = Reflect.ownKeys(value)
  } catch {
    return false
  }
  if (keys.length !== expected.length || keys.some(key => typeof key !== 'string')
    || expected.some(key => !keys.includes(key))) return false
  return expected.every((key) => {
    const descriptor = Object.getOwnPropertyDescriptor(value, key)
    return Boolean(descriptor && descriptor.enumerable && Object.hasOwn(descriptor, 'value'))
  })
}

function hasExactRootDataKeys(value: unknown, expected: readonly string[]): value is JsonRecord {
  if (!isRecord(value)) return false
  let keys: (string | symbol)[]
  try {
    keys = Reflect.ownKeys(value)
  } catch {
    return false
  }
  const stringKeys = keys.filter((key): key is string => typeof key === 'string')
  const symbols = keys.filter((key): key is symbol => typeof key === 'symbol')
  if (symbols.some(symbol => symbol !== Symbol.dispose)
    || stringKeys.length !== expected.length
    || expected.some(key => !stringKeys.includes(key))) return false
  return expected.every((key) => {
    const descriptor = Object.getOwnPropertyDescriptor(value, key)
    return Boolean(descriptor && descriptor.enumerable && Object.hasOwn(descriptor, 'value'))
  })
}

function isRecord(value: unknown): value is JsonRecord {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false
  try {
    const prototype = Object.getPrototypeOf(value)
    return prototype === Object.prototype || prototype === null
  } catch {
    return false
  }
}

/** RPC namespace/stub 是平台能力，不是 JSON DTO；只檢查可呼叫物件形狀。 */
function isCapabilityObject(value: unknown): value is object {
  return (typeof value === 'object' && value !== null) || typeof value === 'function'
}

function nonEmpty(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim() ? value.trim() : undefined
}

function badRequest(code: string, message: string) {
  return createError({ statusCode: 400, statusMessage: 'InvalidCompetitionRulingCorrectionRequest', data: { code, message } })
}
