import {
  isExactCompetitionJson,
  isJsonObject,
} from '../../shared/competition'
import {
  COMPETITION_ROUND_ROOM_LEAD_TIME_MS,
  competitionRoundReadyCutoffMs,
} from '../../shared/competition-game-room'
import {
  isCompetitionRoomRoundPublication,
  isCompetitionRoomRoundPublicationReadValue,
  type CompetitionRoomRoundPublication,
  type CompetitionRoomRoundPublicationReadRequest,
} from '../../shared/competition-room'
import {
  formatCompetitionDeadline,
  localMinuteDateTimeToEpochMs,
  type CompetitionRegistrationState,
} from './competition-registration'
import {
  type CompetitionTournamentViewData,
  type TournamentViewMatch,
} from './competition-tournament-view'

export interface CompetitionRoundPublicationContext {
  competitionId: string
  sessionUserId: string
}

export interface CompetitionRoundPublicationScope extends CompetitionRoundPublicationContext {
  stageId: string
  roundNumber: number
}

export interface CompetitionRoundPublicationTarget {
  stageId: string
  roundNumber: number
  matchIds: string[]
}

export type CompetitionRoundPublicationTargetResult =
  | { kind: 'ready', target: CompetitionRoundPublicationTarget }
  | { kind: 'blocked', reason: CompetitionRoundPublicationBlockReason }

export type CompetitionRoundPublicationBlockReason =
  | 'singleActiveStageRequired'
  | 'scheduleNeedsReview'
  | 'noCurrentRound'
  | 'previousRoundIncomplete'
  | 'currentRoundStarted'
  | 'currentRoundUnresolved'
  | 'noPendingRooms'

export interface CompetitionRoundPublicationCommandBody {
  commandId: string
  expectedRevision: number
  roundStartMs: number
  refereeAssignments: { matchId: string, referees: { refereeId: string, conflictedTeam: string | null }[] }[]
}

export interface CompetitionRoundPublicationIntent {
  scope: CompetitionRoundPublicationScope
  path: string
  method: 'POST'
  body: CompetitionRoundPublicationCommandBody
  expectedMatchIds: string[]
}

export interface CompetitionRoundPublicationReadSnapshot {
  competitionId: string
  revision: number
  publication: CompetitionRoomRoundPublication | null
}

export interface CompetitionRoundPublicationReceipt {
  competitionId: string
  stageId: string
  roundNumber: number
  commandId: string
  revision: number
  idempotent: boolean
  publication: CompetitionRoomRoundPublication
}

export class CompetitionRoundPublicationProtocolError extends Error {
  constructor() {
    super('輪次公告回應無法安全驗證。')
    this.name = 'CompetitionRoundPublicationProtocolError'
  }
}

export class CompetitionRoundPublicationValidationError extends Error {
  constructor(readonly code: 'minutePrecision' | 'invalidTime' | 'tooSoon' | 'invalidCommand' | 'invalidCurrentRound') {
    const messages = {
      minutePrecision: '比賽開始時間請以分鐘為單位輸入，不含秒或毫秒。',
      invalidTime: '請選擇有效的比賽開始時間；本地時區不存在的時間不能公告。',
      tooSoon: '比賽開始時間 T 至少須比目前時間晚五分鐘；此提示不取代伺服器判定。',
      invalidCommand: '無法安全建立公告命令，請重新查詢後再試。',
      invalidCurrentRound: '目前輪次狀態無法安全確認，請重新查詢賽程。',
    } as const
    super(messages[code])
    this.name = 'CompetitionRoundPublicationValidationError'
  }
}

const MINUTE_INPUT = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/u
const MAX_EPOCH_MS = 8_640_000_000_000_000

/** 只有同賽事、已鎖定的原公告主辦者才可使用目前僅限主辦者的 HTTP 邊界。 */
export function createCompetitionRoundPublicationContext(
  competitionId: unknown,
  sessionUserId: unknown,
  registration: CompetitionRegistrationState | null,
): CompetitionRoundPublicationContext | null {
  if (typeof competitionId !== 'string' || !competitionId || competitionId.trim() !== competitionId
    || typeof sessionUserId !== 'string' || !sessionUserId || sessionUserId.trim() !== sessionUserId
    || !registration || registration.competitionId !== competitionId || registration.organizer !== sessionUserId
    || registration.lifecycle !== 'locked') return null
  return Object.freeze({ competitionId, sessionUserId })
}

/** 只從 parser 已驗證的進行中階段判定最早未完成輪次；不推導或重算配對。 */
export function findCurrentCompetitionRoundPublicationTarget(
  view: CompetitionTournamentViewData,
): CompetitionRoundPublicationTargetResult {
  if (view.stages.length !== 1) return { kind: 'blocked', reason: 'singleActiveStageRequired' }
  const activeStages = view.stages.filter(stage => stage.status === 'active')
  if (activeStages.length !== 1) return { kind: 'blocked', reason: 'singleActiveStageRequired' }
  const stage = activeStages[0]
  if (!stage) return { kind: 'blocked', reason: 'singleActiveStageRequired' }
  if (stage.hasScheduleWarning) return { kind: 'blocked', reason: 'scheduleNeedsReview' }
  if (!stage.rounds.length) return { kind: 'blocked', reason: 'noCurrentRound' }

  const rounds = [...stage.rounds].sort((left, right) => left.roundNumber - right.roundNumber)
  // roundNumber 是此公告 scope 的身分；同號不同 kind 不可攤平合併。
  if (rounds.some((round, index) => index > 0 && rounds[index - 1]?.roundNumber === round.roundNumber)) {
    return { kind: 'blocked', reason: 'noCurrentRound' }
  }
  const currentRound = rounds.find(round => !isCompletedRound(round.matches))
  if (!currentRound || !Number.isSafeInteger(currentRound.roundNumber) || currentRound.roundNumber < 1) {
    return { kind: 'blocked', reason: 'noCurrentRound' }
  }
  const matches = [...currentRound.matches].sort((left, right) => left.sequence - right.sequence)
  if (!matches.length) return { kind: 'blocked', reason: 'noCurrentRound' }
  const matchIds: string[] = []
  for (const match of matches) {
    if (isCanonicalCompletedBye(match)) continue
    if (match.started || match.status === 'inProgress') {
      return { kind: 'blocked', reason: 'currentRoundStarted' }
    }
    if (match.status !== 'pending' || match.result !== null || match.cancellationReason !== null
      || match.participants.length !== 2) {
      return { kind: 'blocked', reason: 'currentRoundUnresolved' }
    }
    matchIds.push(match.matchId)
  }
  if (!matchIds.length) return { kind: 'blocked', reason: 'noPendingRooms' }
  return {
    kind: 'ready',
    target: { stageId: stage.config.stageId, roundNumber: currentRound.roundNumber, matchIds },
  }
}

function isCompletedRound(matches: readonly TournamentViewMatch[]): boolean {
  return matches.length > 0 && matches.every(match => match.status === 'completed' && match.result !== null)
}

function isCanonicalCompletedBye(match: TournamentViewMatch): boolean {
  return match.status === 'completed' && !match.started && match.cancellationReason === null
    && match.participants.length === 1 && match.result?.type === 'bye'
    && match.result.teamId === match.participants[0]
}

/** 命令 revision 取 tournament GET 的 global revision；registration revision 不參與。 */
export function buildCompetitionRoundPublicationIntent(
  context: CompetitionRoundPublicationContext,
  view: CompetitionTournamentViewData,
  roundStartLocal: string,
  nowMs: number,
  uuid: () => string = () => crypto.randomUUID(),
): CompetitionRoundPublicationIntent {
  if (!isValidContext(context) || view.competitionId !== context.competitionId
    || !Number.isSafeInteger(view.revision) || view.revision < 1 || view.revision >= Number.MAX_SAFE_INTEGER) {
    throw new CompetitionRoundPublicationValidationError('invalidCurrentRound')
  }
  const selected = findCurrentCompetitionRoundPublicationTarget(view)
  if (selected.kind !== 'ready') throw new CompetitionRoundPublicationValidationError('invalidCurrentRound')
  if (typeof roundStartLocal !== 'string' || !MINUTE_INPUT.test(roundStartLocal)) {
    throw new CompetitionRoundPublicationValidationError('minutePrecision')
  }
  let roundStartMs: number
  try {
    roundStartMs = localMinuteDateTimeToEpochMs(roundStartLocal, '比賽開始時間')
  } catch {
    throw new CompetitionRoundPublicationValidationError('invalidTime')
  }
  const cutoff = competitionRoundReadyCutoffMs(roundStartMs)
  if (cutoff === null || roundStartMs > MAX_EPOCH_MS
    || !Number.isSafeInteger(nowMs) || nowMs < 0 || nowMs > Number.MAX_SAFE_INTEGER - COMPETITION_ROUND_ROOM_LEAD_TIME_MS) {
    throw new CompetitionRoundPublicationValidationError('invalidTime')
  }
  if (roundStartMs < nowMs + COMPETITION_ROUND_ROOM_LEAD_TIME_MS) {
    throw new CompetitionRoundPublicationValidationError('tooSoon')
  }
  let commandId: string
  try { commandId = uuid() } catch { throw new CompetitionRoundPublicationValidationError('invalidCommand') }
  if (typeof commandId !== 'string' || !commandId || commandId.trim() !== commandId) {
    throw new CompetitionRoundPublicationValidationError('invalidCommand')
  }

  const scope: CompetitionRoundPublicationScope = {
    ...context, stageId: selected.target.stageId, roundNumber: selected.target.roundNumber,
  }
  const intent: CompetitionRoundPublicationIntent = {
    scope,
    path: competitionRoundPublicationPath(scope),
    method: 'POST',
    body: {
      commandId,
      expectedRevision: view.revision,
      roundStartMs,
      refereeAssignments: [],
    },
    expectedMatchIds: [...selected.target.matchIds],
  }
  freezeTree(intent)
  return intent
}

export function competitionTournamentPath(context: CompetitionRoundPublicationContext): string {
  return `/api/competitions/${encodeURIComponent(context.competitionId)}/tournament`
}

export function competitionRoundPublicationPath(scope: CompetitionRoundPublicationScope): string {
  return `/api/competitions/${encodeURIComponent(scope.competitionId)}/stages/${encodeURIComponent(scope.stageId)}`
    + `/rounds/${scope.roundNumber}/publication`
}

export function isSameCompetitionRoundPublicationScope(
  left: CompetitionRoundPublicationScope,
  right: CompetitionRoundPublicationScope | null,
): boolean {
  return right !== null && left.competitionId === right.competitionId && left.sessionUserId === right.sessionUserId
    && left.stageId === right.stageId && left.roundNumber === right.roundNumber
}

/** 共享 HTTP guard 後複製為自己的 immutable public snapshot，絕不回傳原始 server object。 */
export function parseCompetitionRoundPublicationReadSnapshot(
  input: unknown,
  scope: CompetitionRoundPublicationScope,
): CompetitionRoundPublicationReadSnapshot {
  const request: CompetitionRoomRoundPublicationReadRequest = {
    competitionId: scope.competitionId, stageId: scope.stageId,
    roundNumber: scope.roundNumber, callerUserId: scope.sessionUserId,
  }
  try {
    if (!isValidScope(scope) || !isCompetitionRoomRoundPublicationReadValue(input, request)) throw new Error()
    const clone: unknown = structuredClone(input)
    if (!isCompetitionRoomRoundPublicationReadValue(clone, request) || !isJsonObject(clone)) throw new Error()
    const snapshot: CompetitionRoundPublicationReadSnapshot = {
      competitionId: clone.competitionId as string,
      revision: clone.revision as number,
      publication: clone.publication as CompetitionRoomRoundPublication | null,
    }
    freezeTree(snapshot)
    return snapshot
  } catch {
    throw new CompetitionRoundPublicationProtocolError()
  }
}

/** 不以 GET 推定 mutation 成功；只接受 exact announcement receipt 與完整預期房間集合。 */
export function parseCompetitionRoundAnnouncementAck(
  input: unknown,
  intent: CompetitionRoundPublicationIntent,
): CompetitionRoundPublicationReceipt {
  try {
    if (!isValidIntent(intent) || !exactRecord(input, ['competitionId', 'revision', 'commandId', 'idempotent', 'publication'])
      || input.competitionId !== intent.scope.competitionId
      || input.commandId !== intent.body.commandId
      || !Number.isSafeInteger(input.revision) || input.revision !== intent.body.expectedRevision + 1
      || typeof input.idempotent !== 'boolean' || !isCompetitionRoomRoundPublication(input.publication)) throw new Error()
    const clone: unknown = structuredClone(input)
    if (!exactRecord(clone, ['competitionId', 'revision', 'commandId', 'idempotent', 'publication'])
      || !isCompetitionRoomRoundPublication(clone.publication)) throw new Error()
    const publication = clone.publication
    if (publication.roundRef.competitionId !== intent.scope.competitionId
      || publication.roundRef.stageId !== intent.scope.stageId
      || publication.roundRef.roundNumber !== intent.scope.roundNumber
      || publication.roundStartMs !== intent.body.roundStartMs
      || publication.publicationVersion !== 1 || publication.status !== 'materializing'
      || publication.releaseDecisionId !== null || publication.requiresReferee
      || publication.rooms.length !== intent.expectedMatchIds.length
      || publication.rooms.some(room => room.ackConfirmedAtMs !== null)
      || !sameStringSet(publication.rooms.map(room => room.matchId), intent.expectedMatchIds)) throw new Error()
    const receipt: CompetitionRoundPublicationReceipt = {
      competitionId: clone.competitionId as string,
      stageId: intent.scope.stageId,
      roundNumber: intent.scope.roundNumber,
      commandId: clone.commandId as string,
      revision: clone.revision as number,
      idempotent: clone.idempotent as boolean,
      publication,
    }
    freezeTree(receipt)
    return receipt
  } catch {
    throw new CompetitionRoundPublicationProtocolError()
  }
}

function sameStringSet(left: readonly string[], right: readonly string[]): boolean {
  if (left.length !== right.length || new Set(left).size !== left.length || new Set(right).size !== right.length) return false
  const expected = new Set(right)
  return left.every(value => expected.has(value))
}

/** 一旦曾未知，任何後續拒絕都不能解除原 command；GET 不是收據。 */
export function keepCompetitionRoundPublicationIntent(wasUnknown: boolean, error: unknown): boolean {
  if (wasUnknown || isProtocolError(error)) return true
  const status = safeStatus(error)
  return ![400, 401, 403, 404, 409, 422].includes(status ?? -1)
}

export function competitionRoundPublicationErrorMessage(error: unknown, phase: 'read' | 'announce'): string {
  if (isProtocolError(error)) {
    return phase === 'read' ? '輪次公告回應無法安全驗證；未顯示不完整資料，請重新查詢。'
      : '公告收據無法安全驗證；原命令仍保留，請以相同命令手動重試。'
  }
  const status = safeStatus(error)
  if (phase === 'read') {
    if (status === 401) return '登入狀態已失效，請重新登入後查詢輪次公告。'
    if (status === 403) return '目前帳號沒有讀取輪次公告的權限；僅原公告主辦者可查詢。'
    if (status === 404) return '找不到賽事或輪次公告。'
    if (status === 409) return '賽事狀態暫時無法確認，請手動重新查詢。'
    return '目前無法查詢輪次公告，請稍後手動重試。'
  }
  if (status !== undefined && [400, 401, 403, 404, 409, 422].includes(status)) {
    return '伺服器未接受這次公告命令；請重新查詢狀態後由主辦重新確認。'
  }
  return '公告結果尚未確認，可能已送達伺服器；請只用原命令手動重試。'
}

function safeStatus(error: unknown): number | undefined {
  try {
    if (typeof error !== 'object' || error === null) return undefined
    const statusCode = Reflect.get(error, 'statusCode')
    const status = statusCode ?? Reflect.get(error, 'status')
    return typeof status === 'number' && Number.isSafeInteger(status) ? status : undefined
  } catch { return undefined }
}

function isProtocolError(error: unknown): boolean {
  try { return error instanceof CompetitionRoundPublicationProtocolError } catch { return false }
}

export function competitionRoundPublicationStatusLabel(status: CompetitionRoomRoundPublication['status']): string {
  return {
    materializing: '整輪房間正在準備中',
    released: '後端已釋出本輪公告',
    quiescing: '本輪公告正在處置中',
    retired: '本輪公告已退役',
  }[status]
}

export function competitionRoundPublicationDeadlineLabel(timestamp: number): string {
  return `比賽開始時間 T：${formatCompetitionDeadline(timestamp)}`
}

function isValidContext(value: CompetitionRoundPublicationContext): boolean {
  return typeof value.competitionId === 'string' && value.competitionId.length > 0
    && value.competitionId.trim() === value.competitionId
    && typeof value.sessionUserId === 'string' && value.sessionUserId.length > 0
    && value.sessionUserId.trim() === value.sessionUserId
}

function isValidScope(value: CompetitionRoundPublicationScope): boolean {
  return isValidContext(value) && typeof value.stageId === 'string' && value.stageId.length > 0
    && value.stageId.trim() === value.stageId && Number.isSafeInteger(value.roundNumber) && value.roundNumber > 0
}

function isValidIntent(value: CompetitionRoundPublicationIntent): boolean {
  return isValidScope(value.scope) && value.method === 'POST'
    && value.path === competitionRoundPublicationPath(value.scope)
    && exactRecord(value.body, ['commandId', 'expectedRevision', 'roundStartMs', 'refereeAssignments'])
    && typeof value.body.commandId === 'string' && value.body.commandId.length > 0
    && value.body.commandId.trim() === value.body.commandId
    && Number.isSafeInteger(value.body.expectedRevision) && value.body.expectedRevision > 0
    && Number.isSafeInteger(value.body.roundStartMs) && competitionRoundReadyCutoffMs(value.body.roundStartMs) !== null
    && Array.isArray(value.body.refereeAssignments) && value.body.refereeAssignments.length === 0
    && Array.isArray(value.expectedMatchIds) && value.expectedMatchIds.length > 0
    && value.expectedMatchIds.every(id => typeof id === 'string' && id.length > 0)
}

function exactRecord(input: unknown, keys: readonly string[]): input is Record<string, unknown> {
  if (!isExactCompetitionJson(input) || !isJsonObject(input)) return false
  const ownKeys = Reflect.ownKeys(input)
  if (ownKeys.length !== keys.length || ownKeys.some(key => typeof key !== 'string')
    || !keys.every(key => Object.hasOwn(input, key))) return false
  return keys.every(key => {
    const descriptor = Object.getOwnPropertyDescriptor(input, key)
    return descriptor !== undefined && descriptor.enumerable && Object.hasOwn(descriptor, 'value')
  })
}

function freezeTree(value: unknown): void {
  if (typeof value !== 'object' || value === null || Object.isFrozen(value)) return
  for (const child of Object.values(value)) freezeTree(child)
  Object.freeze(value)
}
