import { createError, type H3Event } from 'h3'
import type {
  CompetitionAnnouncement,
  CompetitionOperation,
  JsonValue,
  RegistrationAction,
  StageConfig,
} from '../../shared/competition'
import type {
  CompetitionRoomAclValue,
  CompetitionRoomAuditValue,
  CompetitionRoomAuditQuery,
  CompetitionRoomExecuteValue,
  CompetitionRoomExecuteRequest,
  CompetitionRoomInitializeValue,
  CompetitionRoomInitializeRequest,
  CompetitionRoomMatchReadRequest,
  CompetitionRoomMatchReadValue,
  CompetitionRoomReadRequest,
  CompetitionRoomReadValue,
  CompetitionRoomRefereeRequest,
  CompetitionRoomResult,
  CompetitionRoomCreateTournamentRequest,
  CompetitionRoomCreateTournamentValue,
  CompetitionRoomStartScheduledMatchRequest,
  CompetitionRoomStartScheduledMatchValue,
  CompetitionRoomProvisionInitialScheduledGameRequest,
  CompetitionRoomProvisionInitialScheduledGameValue,
  CompetitionRoomAssignNextFirstPlayerByCaptainRequest,
  CompetitionRoomAssignNextFirstPlayerByCaptainValue,
  CompetitionRoomProvisionCaptainAssignedScheduledGameRequest,
  CompetitionRoomProvisionCaptainAssignedScheduledGameValue,
  CompetitionRoomMatchExecuteRequest,
  CompetitionRoomMatchExecuteValue,
} from '../../shared/competition-room'
import { validateCompetitionGameBinding } from '../../shared/competition-game-room'
import { parseCompetitionAnnouncementPlan } from '../../shared/competition-announcement-plan'
import { workerEnv, type CompetitionRoomNamespaceBinding } from './worker-env'

type JsonRecord = Record<string, unknown>
export type TrustedClock = () => number

const ANNOUNCEMENT_FIELDS = [
  'ruleVersion',
  'ruleset',
  'enabledRuleModules',
  'teamSize',
  'initialHp',
  'registrationMode',
  'personalDecks',
  'registrationDeadline',
  'checkInDeadline',
  'plan',
] as const

const IGNORED_CLIENT_IDENTITY_FIELDS = new Set([
  'organizer',
  'participantUserIds',
  'callerUserId',
  'serverActor',
  'trustedTimeMs',
  'serverState',
  'competitionId',
  'matchId',
])

const REGISTRATION_ACTION_TYPES = new Set([
  'open',
  'registerTeam',
  'replaceRoster',
  'confirmMembership',
  'submitDeck',
  'checkInTeam',
  'withdrawTeam',
  'lock',
])

const ROOM_ERROR_STATUS: Record<string, number> = {
  invalidRequest: 400,
  invalidOperation: 400,
  operationNotAllowed: 403,
  commandMismatch: 400,
  organizerMismatch: 403,
  notInitialized: 404,
  matchNotFound: 404,
  refereeNotFound: 404,
  competitionIdMismatch: 404,
  notParticipant: 403,
  forbidden: 403,
  organizerCannotBeReferee: 403,
  revisionConflict: 409,
  matchRevisionConflict: 409,
  trustedTimeRegression: 409,
  commandIdConflict: 409,
  alreadyInitialized: 409,
  alreadyExists: 409,
  alreadyReferee: 409,
  invalidStageConfig: 400,
  invalidStageTeams: 400,
  registrationNotLocked: 409,
  stageTeamMismatch: 409,
  tournamentAlreadyExists: 409,
  engineRejected: 422,
  engineProtocolError: 502,
  alarmSchedulingFailed: 503,
  invalidTournamentCheckpoint: 409,
  matchNotProvisionable: 409,
  firstPlayerPending: 409,
  gameAlreadyLinked: 409,
  gameRoomNotProvisioned: 409,
  matchIdMismatch: 400,
  randomSourceFailure: 502,
  invalidRefereeAcl: 403,
  invalidLockedRoster: 409,
  invalidLineup: 422,
  assignmentNotCurrent: 409,
  firstPlayerPolicyUnresolved: 409,
  eliminationDrawLossUnresolved: 409,
}

const ROOM_ERROR_MESSAGE: Record<string, string> = {
  invalidRequest: '賽事要求格式不正確。',
  invalidOperation: '此賽事操作不受支援。',
  operationNotAllowed: '此操作不可由 HTTP API 執行。',
  commandMismatch: '命令識別資料不一致。',
  organizerMismatch: '只有登入者可以代表自己建立賽事。',
  notInitialized: '找不到這個賽事。',
  matchNotFound: '找不到這場交手。',
  refereeNotFound: '找不到這位裁判。',
  competitionIdMismatch: '找不到這個賽事。',
  notParticipant: '你沒有這個賽事的存取權。',
  forbidden: '你沒有執行此操作的權限。',
  organizerCannotBeReferee: '此操作不被允許。',
  revisionConflict: '賽事狀態已更新，請重新讀取後再試。',
  matchRevisionConflict: '交手狀態已更新，請重新讀取後再試。',
  trustedTimeRegression: '賽事時間狀態無法倒退。',
  commandIdConflict: '命令識別碼已用於其他要求。',
  alreadyInitialized: '賽事已經建立。',
  alreadyExists: '目標已經存在。',
  alreadyReferee: '這位使用者已是裁判。',
  invalidStageConfig: '賽程階段設定格式不正確。',
  invalidStageTeams: '賽程隊伍設定格式不正確。',
  registrationNotLocked: '報名尚未鎖定，無法建立賽程。',
  stageTeamMismatch: '賽程隊伍與鎖定報名名單不一致。',
  tournamentAlreadyExists: '賽程已經建立。',
  engineRejected: '賽程要求無法完成。',
  engineProtocolError: '賽事服務暫時無法處理要求。',
  alarmSchedulingFailed: '報名截止提醒暫時無法排定，請使用相同命令重試。',
  invalidTournamentCheckpoint: '賽程狀態已變更，請重新讀取後再試。',
  matchNotProvisionable: '交手目前無法開始。',
  firstPlayerPending: '交手先手尚未完成指派。',
  gameAlreadyLinked: '此交手已建立對局。',
  gameRoomNotProvisioned: '對局尚未就緒，計時無法操作。',
  matchIdMismatch: '交手識別碼不一致。',
  randomSourceFailure: '賽事服務暫時無法處理要求。',
  invalidRefereeAcl: '裁判授權狀態已變更，請重新確認。',
  invalidLockedRoster: '鎖定名單狀態已變更，請重新讀取後再試。',
  invalidLineup: '交手名單無法安全驗證。',
  assignmentNotCurrent: '隊長先手指派已變更，請重新讀取後再試。',
  firstPlayerPolicyUnresolved: '此賽制的先手規則尚未確定，無法建立下一局。',
  eliminationDrawLossUnresolved: '淘汰賽和局的先手規則尚未確定，無法建立下一局。',
}

export interface CompetitionRoomStub {
  initialize(request: CompetitionRoomInitializeRequest): Promise<CompetitionRoomResult<CompetitionRoomInitializeValue>>
  execute(request: CompetitionRoomExecuteRequest): Promise<CompetitionRoomResult<CompetitionRoomExecuteValue>>
  createTournamentFromLockedRoster(
    request: CompetitionRoomCreateTournamentRequest,
  ): Promise<CompetitionRoomResult<CompetitionRoomCreateTournamentValue>>
  startScheduledMatch(
    request: CompetitionRoomStartScheduledMatchRequest,
  ): Promise<CompetitionRoomResult<CompetitionRoomStartScheduledMatchValue>>
  provisionInitialScheduledGame(
    request: CompetitionRoomProvisionInitialScheduledGameRequest,
  ): Promise<CompetitionRoomResult<CompetitionRoomProvisionInitialScheduledGameValue>>
  assignNextFirstPlayerByCaptain(
    request: CompetitionRoomAssignNextFirstPlayerByCaptainRequest,
  ): Promise<CompetitionRoomResult<CompetitionRoomAssignNextFirstPlayerByCaptainValue>>
  provisionCaptainAssignedScheduledGame(
    request: CompetitionRoomProvisionCaptainAssignedScheduledGameRequest,
  ): Promise<CompetitionRoomResult<CompetitionRoomProvisionCaptainAssignedScheduledGameValue>>
  executeMatch(
    request: CompetitionRoomMatchExecuteRequest,
  ): Promise<CompetitionRoomResult<CompetitionRoomMatchExecuteValue>>
  read(request: CompetitionRoomReadRequest): Promise<CompetitionRoomResult<CompetitionRoomReadValue>>
  readMatch(request: CompetitionRoomMatchReadRequest): Promise<CompetitionRoomResult<CompetitionRoomMatchReadValue>>
  grantReferee(request: CompetitionRoomRefereeRequest): Promise<CompetitionRoomResult<CompetitionRoomAclValue>>
  revokeReferee(request: CompetitionRoomRefereeRequest): Promise<CompetitionRoomResult<CompetitionRoomAclValue>>
  audit(request: CompetitionRoomAuditQuery): Promise<CompetitionRoomResult<CompetitionRoomAuditValue>>
}

export function serverTrustedTime(clock: TrustedClock = Date.now): number {
  const value = clock()
  if (!Number.isFinite(value)) {
    throw createError({ statusCode: 500, statusMessage: '伺服器時間暫時無法使用。' })
  }
  return value
}

export function competitionRoomNamespace(event: H3Event): CompetitionRoomNamespaceBinding {
  const namespace = workerEnv(event).COMPETITION_ROOM
  if (!namespace) {
    throw createError({
      statusCode: 501,
      statusMessage: '賽事服務目前不可用。',
      data: { code: 'competitionRoomUnavailable' },
    })
  }
  return namespace
}

export function competitionRoom(event: H3Event, competitionId: string): CompetitionRoomStub {
  const namespace = competitionRoomNamespace(event)
  return namespace.get(namespace.idFromName(competitionId)) as unknown as CompetitionRoomStub
}

export async function callCompetitionRoom<T>(
  event: H3Event,
  competitionId: string,
  operation: (room: CompetitionRoomStub) => Promise<CompetitionRoomResult<T>>,
): Promise<T> {
  const room = competitionRoom(event, competitionId)
  let result: CompetitionRoomResult<T>
  try {
    result = await operation(room)
  } catch {
    throw createError({
      statusCode: 502,
      statusMessage: '賽事服務暫時無法處理要求。',
      data: { code: 'competitionRoomFailure' },
    })
  }
  if (!result.ok) throw competitionRoomError(result.error)
  return result.value
}

/** E5c composite mutation 對 RPC envelope 也採 exact schema，避免忽略隱藏欄位。 */
export async function callCompetitionRoomExact<T>(
  event: H3Event,
  competitionId: string,
  operation: (room: CompetitionRoomStub) => Promise<unknown>,
): Promise<T> {
  const room = competitionRoom(event, competitionId)
  let result: unknown
  try {
    result = await operation(room)
  } catch {
    throw createError({
      statusCode: 502,
      statusMessage: '賽事服務暫時無法處理要求。',
      data: { code: 'competitionRoomFailure' },
    })
  }
  const root = captureCompetitionRootDisposer(result)
  try {
    if (!root.valid || !isPlainRecord(result)) throw protocolError()
    const ok = Object.getOwnPropertyDescriptor(result, 'ok')?.value
    if (ok === true) {
      if (!hasExactOwnDataKeys(result, ['ok', 'value'], true)) throw protocolError()
      return result.value as T
    }
    if (ok !== false || !hasExactOwnDataKeys(result, ['ok', 'error'], true)
      || !isExactClockRecord(result.error, ['code', 'message'])
      || typeof result.error.code !== 'string' || typeof result.error.message !== 'string') {
      throw protocolError()
    }
    throw competitionRoomError({ code: result.error.code })
  } finally {
    // 原生 RPC 只容許最外層的同步 disposer，必須以原 receiver 恰好釋放一次。
    if (root.dispose) {
      try {
        if (root.dispose.call(result) !== undefined) throw new Error('invalid synchronous disposer')
      } catch {
        throw competitionRoomError({ code: 'competitionRoomFailure' })
      }
    }
  }
}

export function competitionRoomError(error: { code: string }): ReturnType<typeof createError> {
  const receivedCode = typeof error.code === 'string' && error.code.trim()
    ? error.code.trim()
    : 'unknownFailure'
  const knownCode = receivedCode.startsWith('engine:')
    ? 'engineRejected'
    : Object.hasOwn(ROOM_ERROR_STATUS, receivedCode)
      ? receivedCode
      : 'competitionRoomFailure'
  const statusCode = ROOM_ERROR_STATUS[knownCode]
    ?? 502
  const statusMessage = ROOM_ERROR_MESSAGE[knownCode]
    ?? '賽事服務暫時無法處理要求。'
  return createError({ statusCode, statusMessage, data: { code: knownCode } })
}

export function projectCreateTournamentPublicValue(input: unknown): CompetitionRoomCreateTournamentValue {
  if (!isPlainRecord(input)
    || typeof input.competitionId !== 'string' || !input.competitionId.trim()
    || !Number.isSafeInteger(input.revision) || (input.revision as number) < 1
    || typeof input.commandId !== 'string' || !input.commandId.trim()
    || typeof input.idempotent !== 'boolean'
    || !isPlainRecord(input.projection)
    || input.projection.ok !== true
    || input.projection.operation !== 'tournamentCreate'
    || !isJsonValue(input.projection.public)) {
    throw createError({
      statusCode: 502,
      statusMessage: '賽事服務暫時無法處理要求。',
      data: { code: 'engineProtocolError' },
    })
  }
  return {
    competitionId: input.competitionId,
    revision: input.revision as number,
    commandId: input.commandId,
    idempotent: input.idempotent,
    projection: {
      ok: true,
      operation: 'tournamentCreate',
      public: input.projection.public,
    },
  }
}

export interface StartScheduledMatchCommandInput {
  stageId: string
  expectedRevision: number
  commandId: string
  referees: { refereeId: string; conflictedTeam: string | null }[]
}

export interface StartScheduledMatchCompositeRequests {
  rootCommandId: string
  startCommandId: string
  initialGameCommandId: string
  start: CompetitionRoomStartScheduledMatchRequest
  initialGame: CompetitionRoomProvisionInitialScheduledGameRequest
}

const STARTED_MATCH_PROJECTION_KEYS = ['ok', 'operation', 'public'] as const

export interface ClockControlCommandInput {
  action: 'pause' | 'resume'
  reason: string
  expectedMatchRevision: number
  commandId: string
}

export interface ClockControlPublicValue {
  competitionId: string
  matchId: string
  matchRevision: number
  commandId: string
  action: ClockControlCommandInput['action']
  idempotent: boolean
  nextClockDeadlineMs: number | null
}

/** 時鐘控制只接受四個公開欄位；caller、時間及交手識別皆由伺服器衍生。 */
export function buildClockControlCommand(input: unknown): ClockControlCommandInput {
  if (!isExactClockRecord(input, [
    'action', 'reason', 'expectedMatchRevision', 'commandId',
  ]) || !isClockControlJson(input)) {
    throw badRequest('計時控制要求欄位不正確。')
  }
  if (input.action !== 'pause' && input.action !== 'resume') {
    throw badRequest('action 必須是 pause 或 resume。')
  }
  const reason = nonEmptyString(input.reason)
  const commandId = nonEmptyString(input.commandId)
  if (!reason) throw badRequest('reason 必須是非空字串。')
  if (!commandId) throw badRequest('commandId 必須是非空字串。')
  if (!Number.isSafeInteger(input.expectedMatchRevision)
    || (input.expectedMatchRevision as number) < 1
    || (input.expectedMatchRevision as number) >= Number.MAX_SAFE_INTEGER) {
    throw badRequest('expectedMatchRevision 必須是有效的正整數。')
  }
  return {
    action: input.action,
    reason,
    expectedMatchRevision: input.expectedMatchRevision as number,
    commandId,
  }
}

export function buildClockControlRequest(
  competitionId: string,
  matchId: string,
  callerUserId: string,
  command: ClockControlCommandInput,
  trustedTimeMs: number,
): CompetitionRoomMatchExecuteRequest {
  const normalizedCompetitionId = requireNonEmpty(competitionId, '賽事識別碼無效。')
  const normalizedMatchId = requireNonEmpty(matchId, '交手識別碼無效。')
  const normalizedCallerId = requireNonEmpty(callerUserId, '登入者身分無效。')
  const time = serverTrustedTime(() => trustedTimeMs)
  if (!Number.isSafeInteger(time) || time < 0) throw badRequest('伺服器時間無效。')
  return {
    competitionId: normalizedCompetitionId,
    matchId: normalizedMatchId,
    callerUserId: normalizedCallerId,
    commandId: command.commandId,
    expectedMatchRevision: command.expectedMatchRevision,
    trustedTimeMs: time,
    operation: {
      type: command.action === 'pause' ? 'clockPause' : 'clockResume',
      matchId: normalizedMatchId,
      reason: command.reason,
    },
  }
}

export function projectClockControlPublicValue(
  input: unknown,
  request: CompetitionRoomMatchExecuteRequest,
  action: ClockControlCommandInput['action'],
): ClockControlPublicValue {
  const operation = action === 'pause' ? 'clockPause' : 'clockResume'
  if (!isExactClockRecord(input, [
      'competitionId', 'matchId', 'matchRevision', 'commandId', 'idempotent',
      'timeoutEvidence', 'nextClockDeadlineMs', 'matchProjection', 'clockProjection',
    ])
    || input.competitionId !== request.competitionId
    || input.matchId !== request.matchId
    || input.matchRevision !== request.expectedMatchRevision + 1
    || input.commandId !== request.commandId
    || typeof input.idempotent !== 'boolean'
    || input.timeoutEvidence !== null
    || !(input.matchProjection === null || isClockControlMatchProjection(input.matchProjection, request.matchId))
    || !(input.nextClockDeadlineMs === null
      || (Number.isSafeInteger(input.nextClockDeadlineMs) && (input.nextClockDeadlineMs as number) >= 0))
    || !isClockControlJson(input.clockProjection)
    || !isStrictPublicProjection(input.clockProjection, operation)) {
    throw protocolError()
  }
  return {
    competitionId: request.competitionId,
    matchId: request.matchId,
    matchRevision: input.matchRevision as number,
    commandId: request.commandId,
    action,
    idempotent: input.idempotent,
    nextClockDeadlineMs: input.nextClockDeadlineMs as number | null,
  }
}

/** 僅驗證已提交的公開 DTO 形狀與交手識別；比分、裁判語意及續局政策仍由 DO／Rust 決定。 */
function isClockControlMatchProjection(value: unknown, matchId: string): boolean {
  if (!isExactClockRecord(value, ['ok', 'operation', 'public']) || value.ok !== true
    || typeof value.operation !== 'string'
    || !['matchCreate', 'matchAssignFirstPlayer', 'matchSubmitGame', 'matchCorrectRuling'].includes(value.operation)
    || !isClockControlJson(value) || !isExactClockRecord(value.public, ['match'])
    || !isExactClockRecord(value.public.match, [
      'config', 'status', 'score', 'games', 'rulings', 'corrections', 'outcome', 'nextFirstPlayer', 'nextGame',
    ])) return false
  const match = value.public.match
  const config = match.config
  if (!isExactClockRecord(config, ['matchId', 'teams', 'format', 'firstGame', 'referees'])
    || config.matchId !== matchId || !Array.isArray(config.teams) || config.teams.length !== 2
    || !config.teams.every(isClockString) || config.teams[0] === config.teams[1]) return false
  const teams = config.teams
  const team = (input: unknown) => typeof input === 'string' && teams.includes(input)
  const pair = (input: unknown) => Array.isArray(input) && input.length === 2
    && input.every(team) && input[0] !== input[1]
  const firstPlayer = (input: unknown): boolean => (
    isExactClockRecord(input, ['type', 'team']) && input.type === 'assigned' && team(input.team)
  ) || (isExactClockRecord(input, ['type', 'reason']) && input.type === 'pending' && isClockString(input.reason))
  const continuation = (input: unknown): boolean => isExactClockRecord(input, ['teams'])
    && Array.isArray(input.teams) && input.teams.length === 2
    && input.teams.every(item => isExactClockRecord(item, ['team', 'mainTimeRemainingMs'])
      && team(item.team) && (item.mainTimeRemainingMs === null || isClockInteger(item.mainTimeRemainingMs)))
    && isPlainRecord(input.teams[0]) && isPlainRecord(input.teams[1])
    && input.teams[0].team !== input.teams[1].team
  const startKind = (input: unknown): boolean => (
    isExactClockRecord(input, ['type']) && input.type === 'ordinaryNewGame'
  ) || (isExactClockRecord(input, ['type', 'continuation']) && input.type === 'eliminationDrawContinuation'
    && continuation(input.continuation))
  const decisionReason = (input: unknown) => typeof input === 'string' && [
    'zeroTimeControl', 'simultaneousByoyomiEntry', 'unlimitedEliminationDraw', 'zeroTimeEliminationDraw',
  ].includes(input)
  const ruling = (input: unknown): boolean => {
    if (!isPlainRecord(input)) return false
    switch (input.type) {
      case 'timeoutLoss': return isExactClockRecord(input, ['type', 'loser']) && team(input.loser)
      case 'doubleTimeoutLoss': return isExactClockRecord(input, ['type', 'teams']) && pair(input.teams)
      case 'gameAccepted': return isExactClockRecord(input, ['type', 'outcome'])
        && (input.outcome === 'draw' || (isExactClockRecord(input.outcome, ['winner']) && team(input.outcome.winner)))
      case 'eliminationDrawLoss': return isExactClockRecord(input, ['type', 'loser', 'reason'])
        && team(input.loser) && input.reason === 'enteredByoyomiFirst'
      case 'needsTiebreak': return isExactClockRecord(input, ['type', 'continuation']) && continuation(input.continuation)
      case 'needsDecision': return isExactClockRecord(input, ['type', 'reason']) && decisionReason(input.reason)
      default: return false
    }
  }
  const terminalEvidence = (input: unknown): boolean => {
    if (isExactClockRecord(input, ['type', 'conclusion']) && input.type === 'canonicalGameEnded'
      && isExactClockRecord(input.conclusion, ['outcome', 'causes'])) {
      const conclusion = input.conclusion
      return ((isExactClockRecord(conclusion.outcome, ['type', 'team'])
        && conclusion.outcome.type === 'winner' && team(conclusion.outcome.team))
        || (isExactClockRecord(conclusion.outcome, ['type']) && conclusion.outcome.type === 'draw'))
        && Array.isArray(conclusion.causes) && conclusion.causes.every(cause => (
          isExactClockRecord(cause, ['type', 'teams']) && cause.type === 'teamHpDepleted'
            && Array.isArray(cause.teams) && cause.teams.every(team)
        ) || (isExactClockRecord(cause, ['type', 'rule', 'team']) && cause.type === 'directVictory'
          && isClockString(cause.rule) && team(cause.team)))
    }
    return isExactClockRecord(input, ['type', 'timeoutId', 'expiries', 'observedAtMonotonicMs', 'clockCheckpointDigest'])
      && input.type === 'timeout' && isClockString(input.timeoutId) && isClockString(input.clockCheckpointDigest)
      && isClockInteger(input.observedAtMonotonicMs) && Array.isArray(input.expiries)
      && input.expiries.every(expiry => isExactClockRecord(expiry, ['team', 'deadlineMonotonicMs'])
        && team(expiry.team) && isClockInteger(expiry.deadlineMonotonicMs))
  }
  const score = (input: unknown): boolean => Array.isArray(input) && input.length === 2
    && input.every(item => isClockInteger(item) && item <= 65535)
  const outcome = (input: unknown): boolean => {
    if (input === null) return true
    if (!isPlainRecord(input)) return false
    switch (input.type) {
      case 'winner': return isExactClockRecord(input, ['type', 'winnerId', 'loserId', 'score'])
        && team(input.winnerId) && team(input.loserId) && input.winnerId !== input.loserId && score(input.score)
      case 'draw': return isExactClockRecord(input, ['type', 'score']) && score(input.score)
      case 'doubleLoss': return isExactClockRecord(input, ['type', 'teams', 'progression']) && pair(input.teams)
        && (isExactClockRecord(input.progression, ['type']) && input.progression.type === 'stageExit'
          || (isExactClockRecord(input.progression, ['type', 'reason'])
            && input.progression.type === 'pendingReferee' && isClockString(input.progression.reason)))
      case 'pendingReferee': return isExactClockRecord(input, ['type', 'gameRecordId', 'reason'])
        && isClockString(input.gameRecordId) && decisionReason(input.reason)
      default: return false
    }
  }
  return isExactClockRecord(config.format, ['type', 'value'])
    && (config.format.type === 'fixedGames' || config.format.type === 'firstToWins')
    && isClockInteger(config.format.value, 1) && config.format.value <= 65535
    && firstPlayer(config.firstGame) && Array.isArray(config.referees)
    && config.referees.every(item => isExactClockRecord(item, ['refereeId', 'conflictedTeam'])
      && isClockString(item.refereeId) && (item.conflictedTeam === null || team(item.conflictedTeam)))
    && typeof match.status === 'string'
    && ['open', 'awaitingFirstPlayer', 'awaitingReferee', 'completed'].includes(match.status)
    && score(match.score) && Array.isArray(match.games)
    && match.games.every(game => isExactClockRecord(game, [
      'gameNumber', 'gameRecordId', 'startKind', 'firstPlayer', 'terminalEvidence', 'rulingRecordId',
    ]) && isClockInteger(game.gameNumber, 1) && game.gameNumber <= 65535
      && isClockString(game.gameRecordId) && isClockString(game.rulingRecordId)
      && startKind(game.startKind) && team(game.firstPlayer) && terminalEvidence(game.terminalEvidence))
    && Array.isArray(match.rulings) && match.rulings.every(item => (
      isExactClockRecord(item, ['rulingRecordId', 'gameRecordId', 'ruling'])
      && isClockString(item.rulingRecordId) && isClockString(item.gameRecordId) && ruling(item.ruling)
    ))
    && Array.isArray(match.corrections) && match.corrections.every(item => isExactClockRecord(item, [
      'correctionId', 'refereeId', 'gameRecordId', 'previousRulingRecordId',
      'replacementRulingRecordId', 'replacementRuling', 'reason',
    ]) && ['correctionId', 'refereeId', 'gameRecordId', 'previousRulingRecordId', 'replacementRulingRecordId', 'reason']
      .every(key => isClockString(item[key])) && ruling(item.replacementRuling))
    && outcome(match.outcome) && (match.nextFirstPlayer === null || team(match.nextFirstPlayer))
    && (match.nextGame === null || (isExactClockRecord(match.nextGame, ['gameNumber', 'kind', 'firstPlayer'])
      && isClockInteger(match.nextGame.gameNumber, 1) && match.nextGame.gameNumber <= 65535
      && startKind(match.nextGame.kind) && firstPlayer(match.nextGame.firstPlayer)))
}

function isClockString(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0
}

function isClockInteger(value: unknown, minimum = 0): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= minimum
}

function isExactClockRecord(value: unknown, keys: readonly string[]): value is JsonRecord {
  try {
    return isPlainRecord(value) && hasExactOwnDataKeys(value, keys)
  } catch {
    return false
  }
}

function hasExactOwnDataKeys(value: JsonRecord, expected: readonly string[], allowRootDisposer = false): boolean {
  const keys = Reflect.ownKeys(value).filter(key => !(allowRootDisposer && key === Symbol.dispose))
  return keys.length === expected.length && keys.every(key => typeof key === 'string' && expected.includes(key))
    && expected.every(key => {
      const descriptor = Object.getOwnPropertyDescriptor(value, key)
      return descriptor?.enumerable === true && Object.hasOwn(descriptor, 'value')
    })
}

function captureCompetitionRootDisposer(value: unknown): { valid: boolean; dispose?: (this: unknown) => unknown } {
  try {
    if (typeof value !== 'object' || value === null) return { valid: false }
    const symbols = Object.getOwnPropertySymbols(value)
    const descriptor = Object.getOwnPropertyDescriptor(value, Symbol.dispose)
    if (descriptor && (!Object.hasOwn(descriptor, 'value') || typeof descriptor.value !== 'function'
      || Object.getPrototypeOf(descriptor.value) !== Function.prototype)) return { valid: false }
    return { valid: symbols.every(symbol => symbol === Symbol.dispose),
      ...(descriptor ? { dispose: descriptor.value as (this: unknown) => unknown } : {}) }
  } catch {
    return { valid: false }
  }
}

// 只用於時計 HTTP 邊界；不變更其他既有 generic DTO 的解析／授權契約。
function isClockControlJson(value: unknown, seen = new Set<object>()): boolean {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return true
  if (typeof value === 'number') return Number.isFinite(value)
  if (typeof value !== 'object' || seen.has(value)) return false
  try {
    seen.add(value)
    const keys = Reflect.ownKeys(value)
    if (Array.isArray(value)) {
      return keys.length === value.length + 1 && keys.includes('length')
        && Array.from({ length: value.length }, (_, index) => {
          const descriptor = Object.getOwnPropertyDescriptor(value, String(index))
          return descriptor?.enumerable === true && Object.hasOwn(descriptor, 'value')
            && isClockControlJson(descriptor.value, seen)
        }).every(Boolean)
    }
    return isPlainRecord(value) && keys.every(key => {
      if (typeof key !== 'string' || [
        'private', 'serverState', 'authorizedPrivate', 'announcement', 'captain', 'members', 'decks',
        'rosterVersion', 'lockedRoster', 'roster', 'deck', 'deckSnapshot', 'deckSeed', 'binding', 'seed',
      ].includes(key)) return false
      const descriptor = Object.getOwnPropertyDescriptor(value, key)
      return descriptor?.enumerable === true && Object.hasOwn(descriptor, 'value')
        && isClockControlJson(descriptor.value, seen)
    })
  } catch {
    return false
  } finally {
    seen.delete(value)
  }
}

/** 組合啟動固定使用同一個 root identity 及伺服器時間，供失敗後安全重試。 */
export function buildStartScheduledMatchCommand(input: unknown): StartScheduledMatchCommandInput {
  if (!isJsonRecord(input)) throw badRequest('要求內容格式不正確。')
  const body = input
  if (Object.keys(body).some(key => !['stageId', 'expectedRevision', 'commandId', 'referees'].includes(key))) {
    throw badRequest('交手啟動欄位不受支援。')
  }
  const stageId = nonEmptyString(body.stageId)
  const commandId = nonEmptyString(body.commandId)
  if (!stageId) throw badRequest('stageId 必須是非空字串。')
  if (!commandId) throw badRequest('commandId 必須是非空字串。')
  if (!Number.isSafeInteger(body.expectedRevision) || (body.expectedRevision as number) < 1
    || (body.expectedRevision as number) >= Number.MAX_SAFE_INTEGER) {
    throw badRequest('expectedRevision 必須是有效的正整數。')
  }
  const referees = body.referees === undefined ? [] : body.referees
  if (!Array.isArray(referees) || !referees.every(referee => isPlainRecord(referee)
    && hasExactKeys(referee, ['refereeId', 'conflictedTeam'])
    && Boolean(nonEmptyString(referee.refereeId))
    && (referee.conflictedTeam === null || Boolean(nonEmptyString(referee.conflictedTeam))))) {
    throw badRequest('referees 格式不正確。')
  }
  const normalizedReferees = referees.map(referee => ({
    refereeId: (referee.refereeId as string).trim(),
    conflictedTeam: referee.conflictedTeam === null ? null : (referee.conflictedTeam as string).trim(),
  }))
  if (new Set(normalizedReferees.map(referee => referee.refereeId)).size !== normalizedReferees.length) {
    throw badRequest('referees 不可重複。')
  }
  return {
    stageId,
    expectedRevision: body.expectedRevision as number,
    commandId,
    referees: normalizedReferees,
  }
}

export function deriveStartScheduledMatchChildCommandIds(rootCommandId: string): {
  startCommandId: string
  initialGameCommandId: string
} {
  const root = requireNonEmpty(rootCommandId, 'commandId 必須是非空字串。')
  return {
    startCommandId: `${root}:start-schedule`,
    initialGameCommandId: `${root}:initial-game`,
  }
}

export function buildStartScheduledMatchCompositeRequests(
  competitionId: string,
  matchId: string,
  callerUserId: string,
  command: StartScheduledMatchCommandInput,
  trustedTimeMs: number,
): StartScheduledMatchCompositeRequests {
  const normalizedCompetitionId = requireNonEmpty(competitionId, '賽事識別碼無效。')
  const normalizedMatchId = requireNonEmpty(matchId, '交手識別碼無效。')
  const normalizedCallerId = requireNonEmpty(callerUserId, '登入者身分無效。')
  const time = serverTrustedTime(() => trustedTimeMs)
  if (!Number.isSafeInteger(time) || time < 0) throw badRequest('伺服器時間無效。')
  const { startCommandId, initialGameCommandId } = deriveStartScheduledMatchChildCommandIds(command.commandId)
  return {
    rootCommandId: command.commandId,
    startCommandId,
    initialGameCommandId,
    start: {
      competitionId: normalizedCompetitionId,
      stageId: command.stageId,
      matchId: normalizedMatchId,
      callerUserId: normalizedCallerId,
      commandId: startCommandId,
      expectedRevision: command.expectedRevision,
      trustedTimeMs: time,
      referees: command.referees,
    },
    initialGame: {
      competitionId: normalizedCompetitionId,
      matchId: normalizedMatchId,
      callerUserId: normalizedCallerId,
      commandId: initialGameCommandId,
      expectedMatchRevision: 1,
      trustedTimeMs: time,
    },
  }
}

export interface StartScheduledMatchPublicValue {
  competitionId: string
  matchId: string
  competitionRevision: number
  matchRevision: 2
  gameNumber: 1
  gameId: string
  commandId: string
  status: 'pending' | 'provisioned' | 'blocked'
}

export function validateStartedScheduledMatchValue(
  input: unknown,
  request: CompetitionRoomStartScheduledMatchRequest,
): void {
  if (!isPlainRecord(input)
    || !hasExactKeys(input, [
      'competitionId', 'stageId', 'matchId', 'revision', 'matchRevision', 'commandId', 'idempotent',
      'tournamentProjection', 'matchProjection', 'clockProjection',
    ])
    || input.competitionId !== request.competitionId
    || input.stageId !== request.stageId
    || input.matchId !== request.matchId
    || input.revision !== request.expectedRevision + 1
    || input.matchRevision !== 1
    || input.commandId !== request.commandId
    || typeof input.idempotent !== 'boolean'
    || !isStrictPublicProjection(input.tournamentProjection, 'tournamentStartMatch')
    || !isStrictPublicProjection(input.matchProjection, 'matchCreate')
    || !isStrictPublicProjection(input.clockProjection, 'clockCreate')) {
    throw protocolError()
  }
}

export function projectProvisionedInitialGameValue(
  input: unknown,
  request: CompetitionRoomProvisionInitialScheduledGameRequest,
  competitionRevision: number,
  rootCommandId: string,
): StartScheduledMatchPublicValue {
  if (!isPlainRecord(input)
    || !hasRequiredKeys(input, [
      'competitionId', 'matchId', 'matchRevision', 'gameNumber', 'gameId', 'commandId',
      'idempotent', 'status', 'binding',
    ])
    || input.competitionId !== request.competitionId
    || input.matchId !== request.matchId
    || input.matchRevision !== 2
    || input.gameNumber !== 1
    || input.commandId !== request.commandId
    || typeof input.idempotent !== 'boolean'
    || typeof input.gameId !== 'string' || !input.gameId.trim()
    || !['pending', 'provisioned', 'blocked'].includes(input.status as string)) {
    throw protocolError()
  }
  try {
    // 先驗完整原始 binding，再核身分及投影；只沿 shared required17 契約，不丟欄或重建模型。
    const descriptor = Object.getOwnPropertyDescriptor(input, 'binding')
    if (!descriptor || descriptor.enumerable !== true || !Object.hasOwn(descriptor, 'value')) throw protocolError()
    const binding: unknown = descriptor.value
    validateCompetitionGameBinding(binding)
    if (binding.competitionId !== request.competitionId
      || binding.matchId !== request.matchId
      || binding.gameNumber !== 1
      || binding.expectedMatchRevision !== 2) throw protocolError()
  } catch {
    throw protocolError()
  }
  return {
    competitionId: request.competitionId,
    matchId: request.matchId,
    competitionRevision,
    matchRevision: 2,
    gameNumber: 1,
    gameId: input.gameId,
    commandId: rootCommandId,
    status: input.status as StartScheduledMatchPublicValue['status'],
  }
}

export interface CaptainNextGameCommandInput {
  selectedFirstPlayerTeam: string
  expectedMatchRevision: number
  commandId: string
}

export interface CaptainNextGameCompositeRequests {
  rootCommandId: string
  assignmentCommandId: string
  provisionCommandId: string
  assignment: CompetitionRoomAssignNextFirstPlayerByCaptainRequest
}

export interface CaptainNextGamePublicValue {
  competitionId: string
  matchId: string
  matchRevision: number
  gameNumber: number
  gameId: string
  commandId: string
  status: 'pending' | 'provisioned' | 'blocked'
}

export function buildCaptainNextGameCommand(input: unknown): CaptainNextGameCommandInput {
  if (!isJsonRecord(input) || !hasExactKeys(input, [
    'selectedFirstPlayerTeam', 'expectedMatchRevision', 'commandId',
  ])) {
    throw badRequest('下一局要求欄位不正確。')
  }
  const selectedFirstPlayerTeam = nonEmptyString(input.selectedFirstPlayerTeam)
  const commandId = nonEmptyString(input.commandId)
  if (!selectedFirstPlayerTeam) throw badRequest('selectedFirstPlayerTeam 必須是非空字串。')
  if (!commandId) throw badRequest('commandId 必須是非空字串。')
  if (!Number.isSafeInteger(input.expectedMatchRevision)
    || (input.expectedMatchRevision as number) < 1
    || (input.expectedMatchRevision as number) >= Number.MAX_SAFE_INTEGER) {
    throw badRequest('expectedMatchRevision 必須是有效的正整數。')
  }
  return {
    selectedFirstPlayerTeam,
    expectedMatchRevision: input.expectedMatchRevision as number,
    commandId,
  }
}

export function deriveCaptainNextGameChildCommandIds(rootCommandId: string): {
  assignmentCommandId: string
  provisionCommandId: string
} {
  const root = requireNonEmpty(rootCommandId, 'commandId 必須是非空字串。')
  return {
    assignmentCommandId: `${root}:assign-first-player`,
    provisionCommandId: `${root}:captain-game`,
  }
}

export function buildCaptainNextGameCompositeRequests(
  competitionId: string,
  matchId: string,
  callerUserId: string,
  command: CaptainNextGameCommandInput,
  trustedTimeMs: number,
): CaptainNextGameCompositeRequests {
  const normalizedCompetitionId = requireNonEmpty(competitionId, '賽事識別碼無效。')
  const normalizedMatchId = requireNonEmpty(matchId, '交手識別碼無效。')
  const normalizedCallerId = requireNonEmpty(callerUserId, '登入者身分無效。')
  const time = serverTrustedTime(() => trustedTimeMs)
  if (!Number.isSafeInteger(time) || time < 0) throw badRequest('伺服器時間無效。')
  const { assignmentCommandId, provisionCommandId } = deriveCaptainNextGameChildCommandIds(command.commandId)
  return {
    rootCommandId: command.commandId,
    assignmentCommandId,
    provisionCommandId,
    assignment: {
      competitionId: normalizedCompetitionId,
      matchId: normalizedMatchId,
      callerUserId: normalizedCallerId,
      commandId: assignmentCommandId,
      expectedMatchRevision: command.expectedMatchRevision,
      trustedTimeMs: time,
      selectedFirstPlayerTeam: command.selectedFirstPlayerTeam,
    },
  }
}

export function buildCaptainNextGameProvisionRequest(
  composite: CaptainNextGameCompositeRequests,
  matchRevision: number,
): CompetitionRoomProvisionCaptainAssignedScheduledGameRequest {
  if (!Number.isSafeInteger(matchRevision) || matchRevision < 2) throw protocolError()
  return {
    competitionId: composite.assignment.competitionId,
    matchId: composite.assignment.matchId,
    callerUserId: composite.assignment.callerUserId,
    commandId: composite.provisionCommandId,
    assignmentCommandId: composite.assignmentCommandId,
    expectedMatchRevision: matchRevision,
    trustedTimeMs: composite.assignment.trustedTimeMs,
  }
}

export function validateCaptainFirstPlayerAssignmentValue(
  input: unknown,
  request: CompetitionRoomAssignNextFirstPlayerByCaptainRequest,
): CompetitionRoomAssignNextFirstPlayerByCaptainValue {
  if (!isPlainRecord(input)
    || !hasExactKeys(input, [
      'competitionId', 'matchId', 'matchRevision', 'commandId', 'idempotent', 'matchProjection',
    ])
    || input.competitionId !== request.competitionId
    || input.matchId !== request.matchId
    || input.matchRevision !== request.expectedMatchRevision + 1
    || input.commandId !== request.commandId
    || typeof input.idempotent !== 'boolean'
    || !isCaptainAssignmentProjection(input.matchProjection, request.selectedFirstPlayerTeam)) {
    throw protocolError()
  }
  return input as unknown as CompetitionRoomAssignNextFirstPlayerByCaptainValue
}

export function projectCaptainAssignedGameValue(
  input: unknown,
  request: CompetitionRoomProvisionCaptainAssignedScheduledGameRequest,
  assignment: CompetitionRoomAssignNextFirstPlayerByCaptainValue,
  rootCommandId: string,
): CaptainNextGamePublicValue {
  const assignedMatch = isPlainRecord(assignment.matchProjection.public)
    && isPlainRecord(assignment.matchProjection.public.match)
    ? assignment.matchProjection.public.match
    : null
  const assignedGameNumber = isPlainRecord(assignedMatch?.nextGame)
    ? assignedMatch.nextGame.gameNumber
    : null
  if (!isPlainRecord(input)
    || !hasExactKeys(input, [
      'competitionId', 'matchId', 'matchRevision', 'gameNumber', 'gameId', 'commandId',
      'idempotent', 'status',
    ])
    || input.competitionId !== request.competitionId
    || input.matchId !== request.matchId
    || input.matchRevision !== assignment.matchRevision
    || !Number.isSafeInteger(input.gameNumber) || (input.gameNumber as number) < 2
    || input.gameNumber !== assignedGameNumber
    || typeof input.gameId !== 'string' || !input.gameId.trim()
    || input.commandId !== request.commandId
    || typeof input.idempotent !== 'boolean'
    || !['pending', 'provisioned', 'blocked'].includes(input.status as string)
    || request.assignmentCommandId.length === 0
    || request.expectedMatchRevision !== assignment.matchRevision) {
    throw protocolError()
  }
  return {
    competitionId: request.competitionId,
    matchId: request.matchId,
    matchRevision: assignment.matchRevision,
    gameNumber: input.gameNumber as number,
    gameId: input.gameId,
    commandId: rootCommandId,
    status: input.status as CaptainNextGamePublicValue['status'],
  }
}

function isCaptainAssignmentProjection(value: unknown, selectedTeam: string): boolean {
  if (!isPlainRecord(value) || !hasExactKeys(value, ['ok', 'operation', 'public'])
    || value.ok !== true || value.operation !== 'matchAssignFirstPlayer'
    || !isJsonValue(value.public) || containsCaptainPrivateFields(value.public)
    || !isPlainRecord(value.public) || !hasExactKeys(value.public, ['match'])
    || !isPlainRecord(value.public.match)) return false
  const match = value.public.match
  const nextGame = match.nextGame
  return hasExactKeys(match, [
    'config', 'status', 'score', 'games', 'rulings', 'corrections', 'outcome',
    'nextFirstPlayer', 'nextGame',
  ])
    && isJsonValue(match.config)
    && isJsonValue(match.score)
    && isJsonValue(match.games)
    && isJsonValue(match.rulings)
    && isJsonValue(match.corrections)
    && match.status === 'open'
    && match.outcome === null
    && match.nextFirstPlayer === selectedTeam
    && isPlainRecord(nextGame)
    && hasExactKeys(nextGame, ['gameNumber', 'kind', 'firstPlayer'])
    && Number.isSafeInteger(nextGame.gameNumber)
    && (nextGame.gameNumber as number) >= 2
    && isPlainRecord(nextGame.kind)
    && hasExactKeys(nextGame.kind, ['type'])
    && nextGame.kind.type === 'ordinaryNewGame'
    && isPlainRecord(nextGame.firstPlayer)
    && hasExactKeys(nextGame.firstPlayer, ['type', 'team'])
    && nextGame.firstPlayer.type === 'assigned'
    && nextGame.firstPlayer.team === selectedTeam
}

function containsCaptainPrivateFields(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(containsCaptainPrivateFields)
  if (!isPlainRecord(value)) return false
  return Object.entries(value).some(([key, child]) => (
    ['private', 'serverState', 'authorizedPrivate', 'announcement', 'captain', 'members',
      'decks', 'rosterVersion', 'lockedRoster', 'roster', 'deck', 'deckSnapshot', 'deckSeed',
      'binding', 'seed'].includes(key)
    || containsCaptainPrivateFields(child)
  ))
}

function isStrictPublicProjection(value: unknown, operation: string): boolean {
  return isPlainRecord(value) && hasExactKeys(value, STARTED_MATCH_PROJECTION_KEYS)
    && value.ok === true && value.operation === operation && isJsonValue(value.public)
    && !containsPrivateRoomFields(value.public)
}

function containsPrivateRoomFields(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(containsPrivateRoomFields)
  if (!isPlainRecord(value)) return false
  return Object.entries(value).some(([key, child]) => (
    key === 'private' || key === 'serverState' || containsPrivateRoomFields(child)
  ))
}

function protocolError(): ReturnType<typeof createError> {
  return createError({
    statusCode: 502,
    statusMessage: ROOM_ERROR_MESSAGE.engineProtocolError,
    data: { code: 'engineProtocolError' },
  })
}

export function buildCompetitionAnnouncement(
  input: unknown,
  organizerUserId: string,
): CompetitionAnnouncement {
  const body = record(input)
  requireNonEmpty(organizerUserId, '登入者身分無效。')
  rejectUnknownAnnouncementFields(body)

  const ruleVersion = body.ruleVersion
  if (ruleVersion !== '5.16' && ruleVersion !== '5.17') {
    throw badRequest('ruleVersion 必須是有效的規則版本。')
  }
  const ruleset = nonEmptyString(body.ruleset)
  if (!ruleset) throw badRequest('ruleset 必須是非空字串。')

  const enabledRuleModules = stringArray(body.enabledRuleModules, 'enabledRuleModules')
  const teamSize = positiveInteger(body.teamSize, 'teamSize')
  const initialHp = positiveInteger(body.initialHp, 'initialHp')
  const registrationMode = registrationModeValue(body.registrationMode)
  const personalDecks = body.personalDecks
  if (personalDecks !== 'disabled' && personalDecks !== 'required') {
    throw badRequest('personalDecks 必須是有效的牌組政策。')
  }
  const registrationDeadline = nonNegativeInteger(body.registrationDeadline, 'registrationDeadline')
  const checkInDeadline = nonNegativeInteger(body.checkInDeadline, 'checkInDeadline')
  const plan = parseCompetitionAnnouncementPlan(body.plan)
  if (!plan || registrationDeadline % 60_000 !== 0 || checkInDeadline % 60_000 !== 0
    || registrationDeadline > checkInDeadline) {
    throw badRequest('公告必須包含完整的分鐘制賽事方案。')
  }

  return {
    organizer: organizerUserId,
    ruleVersion,
    ruleset,
    enabledRuleModules,
    teamSize,
    initialHp,
    registrationMode,
    personalDecks,
    registrationDeadline,
    checkInDeadline,
    plan,
  }
}

export function buildInitializeRequest(
  competitionId: string,
  callerUserId: string,
  announcement: CompetitionAnnouncement,
  clock: TrustedClock = Date.now,
): CompetitionRoomInitializeRequest {
  return {
    competitionId,
    callerUserId,
    announcement,
    participantUserIds: [],
    trustedTimeMs: serverTrustedTime(clock),
  }
}

export interface RegistrationCommandInput {
  commandId: string
  expectedRevision: number
  operation: Extract<CompetitionOperation, { type: 'registrationView' | 'registrationExecute' }>
}

export interface ExecuteCommandInput {
  commandId: string
  expectedRevision: number
  operation: CompetitionRoomExecuteRequest['operation']
}

/** 同分裁決 HTTP 入口只允許完整名單順序與公開理由，不接受一般 engine operation。 */
export function buildRankingTieDecisionCommand(input: unknown): ExecuteCommandInput {
  const body = record(input)
  if (Object.keys(body).some(key => ![
    'commandId', 'expectedRevision', 'stageId', 'orderedTeamIds', 'reason',
  ].includes(key))) throw badRequest('同分裁決欄位不受支援。')
  const commandId = commandIdValue(body.commandId)
  const expectedRevision = revisionValue(body.expectedRevision)
  const stageId = nonEmptyString(body.stageId)
  const reason = nonEmptyString(body.reason)
  if (!commandId || !stageId || !reason || reason.length > 2_000
    || !Array.isArray(body.orderedTeamIds)
    || body.orderedTeamIds.length < 2
    || !body.orderedTeamIds.every(nonEmptyString)
    || new Set(body.orderedTeamIds).size !== body.orderedTeamIds.length) {
    throw badRequest('同分裁決必須提供階段、不同隊伍的完整順序與公開理由。')
  }
  return {
    commandId,
    expectedRevision,
    operation: {
      type: 'tournamentAdjudicateRankingTie',
      stageId,
      orderedTeamIds: body.orderedTeamIds as string[],
      reason: reason.trim(),
    },
  }
}

export function buildRegistrationCommand(
  input: unknown,
  callerUserId: string,
): RegistrationCommandInput {
  const body = record(input)
  const operationValue = 'operation' in body ? body.operation : body
  const operation = record(operationValue)
  const type = operation.type
  if (type !== 'registrationView' && type !== 'registrationExecute') {
    throw createError({
      statusCode: 403,
      statusMessage: '此操作不可由 HTTP API 執行。',
      data: { code: 'operationNotAllowed' },
    })
  }

  const commandId = commandIdValue(body.commandId ?? operation.commandId)
    ?? crypto.randomUUID()
  const expectedRevision = revisionValue(
    body.expectedRevision ?? operation.expectedRevision,
  )
  if ('commandId' in body && 'commandId' in operation
    && body.commandId !== operation.commandId) {
    throw badRequest('命令識別資料不一致。')
  }
  if ('expectedRevision' in body && 'expectedRevision' in operation
    && body.expectedRevision !== operation.expectedRevision) {
    throw badRequest('命令識別資料不一致。')
  }

  if (type === 'registrationView') {
    return {
      commandId,
      expectedRevision,
      operation: { type: 'registrationView' },
    }
  }

  const action = registrationAction(operation.action ?? body.action, callerUserId)
  return {
    commandId,
    expectedRevision,
    operation: {
      type: 'registrationExecute',
      commandId,
      expectedRevision,
      action,
    },
  }
}

export function buildExecuteRequest(
  competitionId: string,
  callerUserId: string,
  command: ExecuteCommandInput,
  clock: TrustedClock = Date.now,
): CompetitionRoomExecuteRequest {
  return {
    competitionId,
    callerUserId,
    commandId: command.commandId,
    expectedRevision: command.expectedRevision,
    operation: command.operation,
    trustedTimeMs: serverTrustedTime(clock),
  }
}

export interface CreateTournamentCommandInput {
  commandId: string
  expectedRevision: number
  stage: StageConfig
}

/** 只解析公開欄位；StageConfig 內容由 CompetitionRoom／Rust 完整驗證。 */
export function buildCreateTournamentCommand(
  input: unknown,
  createCommandId: () => string = () => crypto.randomUUID(),
): CreateTournamentCommandInput {
  if (!isJsonRecord(input)) throw badRequest('要求內容格式不正確。')
  const body = input
  for (const key of Object.keys(body)) {
    if (key !== 'stage' && key !== 'expectedRevision' && key !== 'commandId') {
      throw badRequest('賽程建立欄位不受支援。')
    }
  }
  if (!('stage' in body)) throw badRequest('stage 必須是 JSON 物件。')
  const stage = validateStageConfigDto(body.stage)
  const commandId = 'commandId' in body
    ? commandIdValue(body.commandId)
    : createCommandId()
  if (!commandId) throw badRequest('commandId 必須是非空字串。')
  return {
    commandId,
    expectedRevision: revisionValue(body.expectedRevision),
    stage,
  }
}

export function buildCreateTournamentRequest(
  competitionId: string,
  callerUserId: string,
  command: CreateTournamentCommandInput,
  clock: TrustedClock = Date.now,
): CompetitionRoomCreateTournamentRequest {
  return {
    competitionId: requireNonEmpty(competitionId, '賽事識別碼無效。'),
    callerUserId: requireNonEmpty(callerUserId, '登入者身分無效。'),
    commandId: command.commandId,
    expectedRevision: command.expectedRevision,
    trustedTimeMs: serverTrustedTime(clock),
    stage: command.stage,
  }
}

export interface RefereeCommandInput {
  targetUserId: string
  commandId: string
  expectedRevision: number
}

export function buildRefereeCommand(input: unknown, pathTargetUserId?: string): RefereeCommandInput {
  const body = input === undefined || input === null ? {} : record(input)
  const targetUserId = pathTargetUserId ?? nonEmptyString(body.targetUserId)
  if (!targetUserId) throw badRequest('targetUserId 必須是非空字串。')
  if (pathTargetUserId && 'targetUserId' in body && body.targetUserId !== pathTargetUserId) {
    throw badRequest('路徑與要求中的使用者識別碼不一致。')
  }
  return {
    targetUserId,
    commandId: commandIdValue(body.commandId) ?? crypto.randomUUID(),
    expectedRevision: revisionValue(body.expectedRevision),
  }
}

export function buildRefereeRequest(
  competitionId: string,
  callerUserId: string,
  command: RefereeCommandInput,
  clock: TrustedClock = Date.now,
): CompetitionRoomRefereeRequest {
  return {
    competitionId,
    callerUserId,
    targetUserId: command.targetUserId,
    commandId: command.commandId,
    expectedRevision: command.expectedRevision,
    trustedTimeMs: serverTrustedTime(clock),
  }
}

export function buildReadRequest(
  competitionId: string,
  callerUserId: string,
): CompetitionRoomReadRequest {
  return { competitionId, callerUserId }
}

export function buildMatchReadRequest(
  competitionId: string,
  matchId: string,
  callerUserId: string,
): CompetitionRoomMatchReadRequest {
  return { competitionId, matchId, callerUserId }
}

export function buildAuditRequest(
  competitionId: string,
  callerUserId: string,
  limit?: number,
): CompetitionRoomAuditQuery {
  return {
    competitionId,
    callerUserId,
    ...(limit === undefined ? {} : { limit }),
  }
}

export function boundedAuditLimit(input: unknown): number | undefined {
  if (input === undefined || input === null || input === '') return undefined
  const value = typeof input === 'number'
    ? input
    : typeof input === 'string' && /^\d+$/.test(input.trim())
      ? Number(input)
      : Number.NaN
  if (!Number.isInteger(value) || value < 1 || value > 100) {
    throw badRequest('limit 必須是 1 到 100 的整數。')
  }
  return value
}

export type CompetitionRoomResponseValue =
  | CompetitionRoomInitializeValue
  | CompetitionRoomExecuteValue
  | CompetitionRoomReadValue
  | CompetitionRoomMatchReadValue
  | CompetitionRoomAclValue
  | CompetitionRoomAuditValue

function registrationAction(input: unknown, callerUserId: string): RegistrationAction {
  const action = record(input)
  if (typeof action.type !== 'string' || !REGISTRATION_ACTION_TYPES.has(action.type)) {
    throw badRequest('registration action 不受支援。')
  }
  switch (action.type) {
    case 'open':
      return { type: 'open' }
    case 'lock':
      return { type: 'lock' }
    case 'registerTeam':
      return {
        type: 'registerTeam',
        teamId: requiredString(action.teamId, 'teamId'),
        // captain 是可信 caller 的衍生欄位，不採用 client 值。
        captain: callerUserId,
        members: stringArray(action.members, 'members'),
      }
    case 'replaceRoster':
      return {
        type: 'replaceRoster',
        teamId: requiredString(action.teamId, 'teamId'),
        expectedRosterVersion: nonNegativeInteger(action.expectedRosterVersion, 'expectedRosterVersion'),
        members: stringArray(action.members, 'members'),
      }
    case 'confirmMembership':
      return {
        type: 'confirmMembership',
        teamId: requiredString(action.teamId, 'teamId'),
        rosterVersion: nonNegativeInteger(action.rosterVersion, 'rosterVersion'),
      }
    case 'submitDeck':
      return {
        type: 'submitDeck',
        teamId: requiredString(action.teamId, 'teamId'),
        rosterVersion: nonNegativeInteger(action.rosterVersion, 'rosterVersion'),
        name: requiredString(action.name, 'name'),
        cards: stringArray(action.cards, 'cards'),
      }
    case 'checkInTeam':
      return {
        type: 'checkInTeam',
        teamId: requiredString(action.teamId, 'teamId'),
        rosterVersion: nonNegativeInteger(action.rosterVersion, 'rosterVersion'),
      }
    case 'withdrawTeam':
      return {
        type: 'withdrawTeam',
        teamId: requiredString(action.teamId, 'teamId'),
      }
    default:
      throw badRequest('registration action 不受支援。')
  }
}

function rejectUnknownAnnouncementFields(body: JsonRecord): void {
  for (const key of Object.keys(body)) {
    if ((ANNOUNCEMENT_FIELDS as readonly string[]).includes(key)) continue
    if (IGNORED_CLIENT_IDENTITY_FIELDS.has(key)) continue
    throw badRequest('賽事建立欄位不受支援。')
  }
}

function registrationModeValue(value: unknown): CompetitionAnnouncement['registrationMode'] {
  if (value === 'public') return 'public'
  const mode = record(value)
  if (!('inviteOnly' in mode)) {
    throw badRequest('registrationMode 格式不正確。')
  }
  const inviteOnly = record(mode.inviteOnly)
  return { inviteOnly: { invitedCaptains: stringArray(inviteOnly.invitedCaptains, 'invitedCaptains') } }
}

function record(value: unknown): JsonRecord {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw badRequest('要求內容格式不正確。')
  }
  return value as JsonRecord
}

function validateStageConfigDto(input: unknown): StageConfig {
  const stage = record(input)
  if (!hasExactKeys(stage, [
    'stageId', 'format', 'teams', 'randomSeed', 'matchFormat', 'clockConfig', 'swissRounds', 'swissPolicy',
  ]) || !nonEmptyString(stage.stageId)
    || !['singleElimination', 'roundRobin', 'doubleElimination', 'swiss'].includes(stage.format as string)
    || !Number.isSafeInteger(stage.randomSeed) || (stage.randomSeed as number) < 0
    || !Array.isArray(stage.teams)
    || !stage.teams.every(team => isPlainRecord(team)
      && hasExactKeys(team, ['teamId', 'seed'])
      && typeof team.teamId === 'string' && Boolean(team.teamId.trim())
      && Number.isSafeInteger(team.seed) && (team.seed as number) > 0)
    || !isMatchFormatDto(stage.matchFormat)
    || !isClockConfigDto(stage.clockConfig)
    || !(stage.swissRounds === null
      || (Number.isSafeInteger(stage.swissRounds) && (stage.swissRounds as number) >= 0))
    || !(stage.swissPolicy === null || isJsonRecord(stage.swissPolicy))) {
    throw badRequest('StageConfig 格式不正確。')
  }
  return stage as unknown as StageConfig
}

function isMatchFormatDto(value: unknown): boolean {
  return isPlainRecord(value)
    && hasExactKeys(value, ['type', 'value'])
    && (value.type === 'fixedGames' || value.type === 'firstToWins')
    && Number.isSafeInteger(value.value)
    && (value.value as number) > 0
}

function isClockConfigDto(value: unknown): boolean {
  return isPlainRecord(value)
    && hasExactKeys(value, ['mainTimeMs', 'actionTimeMs', 'outOfTurnChoiceTimeMs'])
    && ['mainTimeMs', 'actionTimeMs', 'outOfTurnChoiceTimeMs'].every((field) => {
      const fieldValue = value[field]
      return fieldValue === null
        || (Number.isSafeInteger(fieldValue) && (fieldValue as number) >= 0)
    })
}

function isJsonRecord(value: unknown): value is JsonRecord {
  return isPlainRecord(value) && isJsonValue(value)
}

function isPlainRecord(value: unknown): value is JsonRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    && (Object.getPrototypeOf(value) === Object.prototype || Object.getPrototypeOf(value) === null)
}

function hasExactKeys(value: JsonRecord, expected: readonly string[]): boolean {
  const keys = Object.keys(value)
  return keys.length === expected.length && expected.every(key => Object.hasOwn(value, key))
}

function hasRequiredKeys(value: JsonRecord, required: readonly string[]): boolean {
  return required.every(key => Object.hasOwn(value, key))
}

function isJsonValue(value: unknown, seen = new Set<object>()): value is JsonValue {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return true
  if (typeof value === 'number') return Number.isFinite(value)
  if (typeof value !== 'object' || seen.has(value)) return false
  seen.add(value)
  const valid = Array.isArray(value)
    ? value.every(item => isJsonValue(item, seen))
    : isPlainRecord(value) && Object.values(value).every(item => isJsonValue(item, seen))
  seen.delete(value)
  return valid
}

function nonEmptyString(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim() ? value.trim() : undefined
}

function requireNonEmpty(value: unknown, message: string): string {
  const result = nonEmptyString(value)
  if (!result) throw badRequest(message)
  return result
}

function requiredString(value: unknown, field: string): string {
  const result = nonEmptyString(value)
  if (!result) throw badRequest(`${field} 必須是非空字串。`)
  return result
}

function stringArray(value: unknown, field: string): string[] {
  if (!Array.isArray(value) || value.some(item => !nonEmptyString(item))) {
    throw badRequest(`${field} 必須是非空字串陣列。`)
  }
  return value.map(item => (item as string).trim())
}

function positiveInteger(value: unknown, field: string): number {
  if (!Number.isInteger(value) || (value as number) <= 0) {
    throw badRequest(`${field} 必須是正整數。`)
  }
  return value as number
}

function nonNegativeInteger(value: unknown, field: string): number {
  if (!Number.isInteger(value) || (value as number) < 0) {
    throw badRequest(`${field} 必須是非負整數。`)
  }
  return value as number
}

function revisionValue(value: unknown): number {
  if (!Number.isSafeInteger(value) || (value as number) < 1) {
    throw badRequest('expectedRevision 必須是正整數。')
  }
  return value as number
}

function commandIdValue(value: unknown): string | undefined {
  return nonEmptyString(value)
}

function badRequest(message: string): ReturnType<typeof createError> {
  return createError({ statusCode: 400, statusMessage: message, data: { code: 'invalidRequest' } })
}
