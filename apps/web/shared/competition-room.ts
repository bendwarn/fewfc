import type {
  CompetitionAnnouncement,
  ClockConfig,
  ClockDisplay,
  StageConfig,
  CompetitionMatchConfig,
  CompetitionOperation,
  CompetitionRuling,
  GameConclusion,
  GameStartKind,
  GameTerminalEvidence,
  ByoyomiEntryOrder,
  GameResultMode,
  TrustedGameResult,
  CompetitionOperationName,
  JsonValue,
  PublicCompetitionSuccess,
} from './competition'
import { isExactCompetitionJson, isJsonObject } from './competition'
import {
  competitionRoundReadyCutoffMs,
  isCompetitionGameBindingRef,
  isCompetitionGameInitialStartWindow,
  isCompetitionRoundRef,
  type CompetitionGameBinding,
  type CompetitionGameInitialStartEvidence,
  type CompetitionRoundRef,
} from './competition-game-room'

export type CompetitionRoomRole = 'organizer' | 'referee' | 'player'

export interface CompetitionRoomError {
  code: string
  message: string
}

export type CompetitionRoomResult<T> =
  | { ok: true; value: T }
  | { ok: false; error: CompetitionRoomError }

export interface CompetitionRoomInitializeRequest {
  competitionId: string
  callerUserId: string
  announcement: CompetitionAnnouncement
  participantUserIds: string[]
  trustedTimeMs: number
}

/** 正常建賽只提供規則與報名設定；生命與主辦者由可信入口注入。 */
export type CompetitionAnnouncementDraft = Omit<CompetitionAnnouncement, 'organizer' | 'initialHp'>

export interface CompetitionRoomInitializeFromRulesRequest {
  competitionId: string
  callerUserId: string
  announcement: CompetitionAnnouncementDraft
  trustedTimeMs: number
}

export interface CompetitionRoomInitializeValue {
  competitionId: string
  organizerUserId: string
  revision: number
  projection: PublicCompetitionSuccess
}

export interface CompetitionRoomStartScheduledMatchRequest {
  competitionId: string
  stageId: string
  matchId: string
  callerUserId: string
  commandId: string
  expectedRevision: number
  trustedTimeMs: number
  referees: { refereeId: string; conflictedTeam: string | null }[]
}

/** 舊呼叫名稱保留為型別相容；其 RPC 必須執行完整原子啟動流程。 */
export type CompetitionRoomMaterializeScheduledMatchRequest = CompetitionRoomStartScheduledMatchRequest

export interface CompetitionRoomCreateTournamentRequest {
  competitionId: string
  callerUserId: string
  commandId: string
  expectedRevision: number
  trustedTimeMs: number
  stage: StageConfig
}

export interface CompetitionRoomCreateTournamentValue {
  competitionId: string
  revision: number
  commandId: string
  idempotent: boolean
  projection: CompetitionRoomPublicProjection
}

export interface CompetitionRoomMaterializeScheduledMatchValue {
  competitionId: string
  stageId: string
  matchId: string
  revision: number
  matchRevision: number
  commandId: string
  idempotent: boolean
  tournamentProjection: CompetitionRoomPublicProjection
  matchProjection: CompetitionRoomMatchProjection | null
  clockProjection: CompetitionRoomMatchProjection | null
}

export type CompetitionRoomStartScheduledMatchValue = CompetitionRoomMaterializeScheduledMatchValue

export interface CompetitionRoomExecuteRequest {
  competitionId: string
  callerUserId: string
  commandId: string
  expectedRevision: number
  operation: Exclude<CompetitionOperation, {
    type: 'registrationCreate' | 'registrationResolveSettings' | 'clockCheckpointMetadata'
  }>
  trustedTimeMs: number
}

export interface CompetitionRoomExecuteValue {
  competitionId: string
  revision: number
  commandId: string
  idempotent: boolean
  projection: PublicCompetitionSuccess
}

export interface CompetitionRoomReadRequest {
  competitionId: string
  callerUserId: string
}

export interface CompetitionRoomReadValue {
  competitionId: string
  revision: number
  projection: PublicCompetitionSuccess
}

/** 供目前登入者唯讀取得已驗證的完整公開賽程；不接受時間或投影選擇器。 */
export interface CompetitionRoomTournamentReadRequest {
  competitionId: string
  callerUserId: string
}

export interface CompetitionRoomTournamentReadValue {
  competitionId: string
  revision: number
  projection: {
    ok: true
    operation: 'tournamentView'
    public: JsonValue
  }
}

export type CompetitionRoomTournamentReadResult =
  CompetitionRoomResult<CompetitionRoomTournamentReadValue>

export type CompetitionRoomMatchOperation =
  | { type: 'matchCreate'; config: CompetitionMatchConfig }
  | { type: 'matchAssignFirstPlayer'; matchId: string; team: string; reason: string }
  | { type: 'matchSubmitGame'; matchId: string; submission: TrustedGameResult }
  | {
      type: 'matchCorrectRuling'
      matchId: string
      correctionId: string
      refereeId: string
      gameRecordId: string
      replacementRulingRecordId: string
      replacementRuling: CompetitionRuling
      reason: string
    }
  | { type: 'clockCreate'; matchId: string; teams: [string, string]; config: ClockConfig }
  | { type: 'clockBeginPreparation'; matchId: string }
  | { type: 'clockFinishPreparation'; matchId: string; team: string }
  | { type: 'clockBeginAction'; matchId: string; team: string; turn: number }
  | { type: 'clockBeginOutOfTurnChoice'; matchId: string; team: string; choiceId: string }
  | { type: 'clockObserve'; matchId: string }
  | { type: 'clockEndWindow'; matchId: string }
  | { type: 'clockPause'; matchId: string; reason: string }
  | { type: 'clockResume'; matchId: string; reason: string }
  | { type: 'clockNewGame'; matchId: string; time: 'reset' | 'carryForward' }
  | {
      type: 'clockRuleGame'
      matchId: string
      conclusion: GameConclusion
      mode: GameResultMode
      byoyomiEntryOrder?: ByoyomiEntryOrder
    }

export interface CompetitionRoomMatchExecuteRequest {
  competitionId: string
  matchId: string
  callerUserId: string
  commandId: string
  expectedMatchRevision: number
  trustedTimeMs: number
  operation: CompetitionRoomMatchOperation
}

/**
 * GameRoom 只能把已終止的遊戲證據交給 CompetitionRoom；判定不得由呼叫端指定。
 */
export interface CompetitionRoomTerminalIngestRequest {
  competitionId: string
  matchId: string
  callerUserId: string
  commandId: string
  expectedMatchRevision: number
  trustedTimeMs: number
  submissionId: string
  gameRecordId: string
  rulingRecordId: string
  startKind: GameStartKind
  firstPlayer: string
  terminalEvidence: GameTerminalEvidence
}

export interface CompetitionRoomMatchProjection {
  ok: true
  operation: CompetitionOperation['type']
  public: JsonValue
}

export interface CompetitionRoomMatchExecuteValue {
  competitionId: string
  matchId: string
  matchRevision: number
  commandId: string
  idempotent: boolean
  timeoutEvidence: Extract<GameTerminalEvidence, { type: 'timeout' }> | null
  nextClockDeadlineMs: number | null
  matchProjection: CompetitionRoomMatchProjection | null
  clockProjection: CompetitionRoomMatchProjection | null
}

/** 新公告只選 current 輪次與時間；完整房間、名單與先手仍由父房派生。 */
export interface CompetitionRoomAnnounceRoundRequest {
  competitionId: string
  stageId: string
  roundNumber: number
  callerUserId: string
  commandId: string
  expectedRevision: number
  trustedTimeMs: number
  roundStartMs: number
  refereeAssignments: {
    matchId: string
    referees: { refereeId: string; conflictedTeam: string | null }[]
  }[]
}

/** 公開公告只描述整輪房間可用性，不附私有 manifest/binding 或玩家 ready。 */
export interface CompetitionRoomRoundPublication {
  roundRef: CompetitionRoundRef
  roundStartMs: number
  publicationVersion: number
  status: 'materializing' | 'released' | 'quiescing' | 'retired'
  releaseDecisionId: string | null
  requiresReferee: boolean
  rooms: { matchId: string; gameId: string; ackConfirmedAtMs: number | null }[]
}

export interface CompetitionRoomRoundPublicationReadRequest {
  competitionId: string
  stageId: string
  roundNumber: number
  callerUserId: string
}

export interface CompetitionRoomRoundPublicationReadValue {
  competitionId: string
  revision: number
  publication: CompetitionRoomRoundPublication | null
}

export interface CompetitionRoomAnnounceRoundValue {
  competitionId: string
  revision: number
  commandId: string
  idempotent: boolean
  publication: CompetitionRoomRoundPublication
}

/** S 只取 evidence.startedAtMs；父端自行取提交 A，不接受呼叫端覆寫收件時間。 */
export interface CompetitionRoomInitialGameStartedRequest {
  competitionId: string
  matchId: string
  callerUserId: string
  commandId: string
  expectedMatchRevision: number
  evidence: CompetitionGameInitialStartEvidence
}

/** 首局與後續時計共用同一九欄 cursor/deadline ACK，不另造 projection 模型。 */
export type CompetitionRoomInitialGameStartedResult =
  CompetitionRoomResult<CompetitionRoomMatchExecuteValue>

export type CompetitionRoomAnnounceRoundResult = CompetitionRoomResult<CompetitionRoomAnnounceRoundValue>
export type CompetitionRoomRoundPublicationReadResult =
  CompetitionRoomResult<CompetitionRoomRoundPublicationReadValue>

/** 可信 server-only capability；不加入 generic execute 或公開 raw HTTP allowlist。 */
export interface CompetitionRoomRoundReleaseRpc {
  announceRound(request: CompetitionRoomAnnounceRoundRequest): Promise<CompetitionRoomAnnounceRoundResult>
  readRoundPublication(request: CompetitionRoomRoundPublicationReadRequest): Promise<CompetitionRoomRoundPublicationReadResult>
  ingestInitialGameStarted(request: CompetitionRoomInitialGameStartedRequest): Promise<CompetitionRoomInitialGameStartedResult>
}

export function isCompetitionRoomAnnounceRoundRequest(value: unknown): value is CompetitionRoomAnnounceRoundRequest {
  try {
    if (!roundReleaseRecord(value, [
      'competitionId', 'stageId', 'roundNumber', 'callerUserId', 'commandId',
      'expectedRevision', 'trustedTimeMs', 'roundStartMs', 'refereeAssignments',
    ]) || !roundReleaseReadIdentity(value)
      || !roundReleaseId(value.commandId) || !roundReleaseRevision(value.expectedRevision)
      || !roundReleaseEpoch(value.trustedTimeMs)
      || competitionRoundReadyCutoffMs(value.roundStartMs) === null
      || !roundReleaseArray(value.refereeAssignments)) return false
    const matchIds = new Set<string>()
    return value.refereeAssignments.every(assignment => {
      if (!roundReleaseRecord(assignment, ['matchId', 'referees'])
        || !roundReleaseId(assignment.matchId) || matchIds.has(assignment.matchId)
        || !roundReleaseArray(assignment.referees)) return false
      matchIds.add(assignment.matchId)
      const refereeIds = new Set<string>()
      return assignment.referees.every(referee => {
        if (!roundReleaseRecord(referee, ['refereeId', 'conflictedTeam'])
          || !roundReleaseId(referee.refereeId) || refereeIds.has(referee.refereeId)
          || (referee.conflictedTeam !== null && !roundReleaseId(referee.conflictedTeam))) return false
        refereeIds.add(referee.refereeId)
        return true
      })
    })
  } catch {
    return false
  }
}

export function isCompetitionRoomRoundPublicationReadRequest(
  value: unknown,
): value is CompetitionRoomRoundPublicationReadRequest {
  try {
    return roundReleaseRecord(value, ['competitionId', 'stageId', 'roundNumber', 'callerUserId'])
      && roundReleaseReadIdentity(value)
  } catch {
    return false
  }
}

/** 只驗 wire，供 receipt lookup 之前使用；全輪證據／release 相等性仍須取父持久資料驗證。 */
export function isCompetitionRoomInitialGameStartedRequest(
  value: unknown,
): value is CompetitionRoomInitialGameStartedRequest {
  try {
    return roundReleaseRecord(value, [
      'competitionId', 'matchId', 'callerUserId', 'commandId', 'expectedMatchRevision', 'evidence',
    ]) && roundReleaseId(value.competitionId) && roundReleaseId(value.matchId)
      && roundReleaseId(value.callerUserId) && roundReleaseId(value.commandId)
      && roundReleaseRevision(value.expectedMatchRevision)
      && roundReleaseInitialEvidence(value.evidence)
      && value.competitionId === value.evidence.bindingRef.roundRef.competitionId
      && value.matchId === value.evidence.bindingRef.matchId
  } catch {
    return false
  }
}

/** 沿共同 evidence DTO 驗結構；不拿 received bindingRef 自己當權威 expectedRelease。 */
function roundReleaseInitialEvidence(value: unknown): value is CompetitionGameInitialStartEvidence {
  return roundReleaseRecord(value, [
    'bindingRef', 'releaseDecisionId', 'gameInstanceId', 'startSequence', 'startedAtMs', 'initialWindow',
  ]) && isCompetitionGameBindingRef(value.bindingRef) && value.bindingRef.gameNumber === 1
    && roundReleaseId(value.releaseDecisionId) && roundReleaseId(value.gameInstanceId)
    && typeof value.startSequence === 'number' && Number.isSafeInteger(value.startSequence) && value.startSequence >= 0
    && roundReleaseEpoch(value.startedAtMs)
    && isCompetitionGameInitialStartWindow(value.initialWindow)
}

export function isCompetitionRoomRoundPublication(value: unknown): value is CompetitionRoomRoundPublication {
  try {
    if (!roundReleaseRecord(value, [
      'roundRef', 'roundStartMs', 'publicationVersion', 'status', 'releaseDecisionId', 'requiresReferee', 'rooms',
    ]) || !isCompetitionRoundRef(value.roundRef)
      || competitionRoundReadyCutoffMs(value.roundStartMs) === null
      || !roundReleaseRevision(value.publicationVersion)
      || typeof value.status !== 'string'
      || !['materializing', 'released', 'quiescing', 'retired'].includes(value.status)
      || typeof value.requiresReferee !== 'boolean'
      || (value.releaseDecisionId !== null && !roundReleaseId(value.releaseDecisionId))
      || (value.status === 'released' && value.releaseDecisionId === null)
      || (value.status === 'materializing' && value.releaseDecisionId !== null)
      || !roundReleaseArray(value.rooms) || value.rooms.length === 0) return false
    const matchIds = new Set<string>()
    const gameIds = new Set<string>()
    const cutoff = competitionRoundReadyCutoffMs(value.roundStartMs)
    if (cutoff === null) return false
    return value.rooms.every(room => {
      if (!roundReleaseRecord(room, ['matchId', 'gameId', 'ackConfirmedAtMs'])
        || !roundReleaseId(room.matchId) || !roundReleaseId(room.gameId)
        || matchIds.has(room.matchId) || gameIds.has(room.gameId)
        || (room.ackConfirmedAtMs !== null && !roundReleaseEpoch(room.ackConfirmedAtMs))) return false
      if (value.status === 'released' && (!roundReleaseEpoch(room.ackConfirmedAtMs)
        || room.ackConfirmedAtMs > cutoff)) return false
      matchIds.add(room.matchId)
      gameIds.add(room.gameId)
      return true
    })
  } catch {
    return false
  }
}

export function isCompetitionRoomRoundPublicationReadValue(
  value: unknown,
  expected: CompetitionRoomRoundPublicationReadRequest,
): value is CompetitionRoomRoundPublicationReadValue {
  try {
    return isCompetitionRoomRoundPublicationReadRequest(expected)
      && roundReleaseRecord(value, ['competitionId', 'revision', 'publication'])
      && value.competitionId === expected.competitionId && roundReleaseRevision(value.revision)
      && (value.publication === null || (isCompetitionRoomRoundPublication(value.publication)
        && roundReleasePublicationIdentity(value.publication, expected)))
  } catch {
    return false
  }
}

export function isCompetitionRoomAnnounceRoundValue(
  value: unknown,
  expected: CompetitionRoomAnnounceRoundRequest,
): value is CompetitionRoomAnnounceRoundValue {
  try {
    return isCompetitionRoomAnnounceRoundRequest(expected)
      && roundReleaseRecord(value, ['competitionId', 'revision', 'commandId', 'idempotent', 'publication'])
      && value.competitionId === expected.competitionId && value.commandId === expected.commandId
      && roundReleaseRevision(value.revision) && value.revision === expected.expectedRevision + 1
      && typeof value.idempotent === 'boolean' && isCompetitionRoomRoundPublication(value.publication)
      && roundReleasePublicationIdentity(value.publication, expected)
      && value.publication.roundStartMs === expected.roundStartMs
      && value.publication.publicationVersion === 1 && value.publication.status === 'materializing'
      && value.publication.requiresReferee === false
      && value.publication.rooms.every(room => room.ackConfirmedAtMs === null)
  } catch {
    return false
  }
}

function roundReleasePublicationIdentity(
  publication: CompetitionRoomRoundPublication,
  expected: CompetitionRoomRoundPublicationReadRequest,
): boolean {
  return publication.roundRef.competitionId === expected.competitionId
    && publication.roundRef.stageId === expected.stageId
    && publication.roundRef.roundNumber === expected.roundNumber
}

/** 每個邊界先驗 descriptors，再讀欄位；拒絕 Symbol、getter、hidden/private extra。 */
function roundReleaseRecord(value: unknown, keys: readonly string[]): value is Record<string, JsonValue> {
  return isExactCompetitionJson(value) && isJsonObject(value)
    && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key))
}

function roundReleaseReadIdentity(value: Record<string, JsonValue>): boolean {
  return roundReleaseId(value.competitionId) && roundReleaseId(value.stageId)
    && roundReleaseId(value.callerUserId) && typeof value.roundNumber === 'number'
    && Number.isSafeInteger(value.roundNumber) && value.roundNumber > 0 && value.roundNumber <= 0xffff_ffff
}

function roundReleaseArray(value: unknown): value is JsonValue[] {
  return Array.isArray(value) && Object.getPrototypeOf(value) === Array.prototype
}

function roundReleaseId(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0 && value === value.trim()
}

function roundReleaseRevision(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value > 0
}

function roundReleaseEpoch(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 && value <= 8_640_000_000_000_000
}

export interface CompetitionRoomMatchReadRequest {
  competitionId: string
  matchId: string
  callerUserId: string
}

export interface CompetitionRoomMatchReadValue {
  competitionId: string
  matchId: string
  matchRevision: number
  matchProjection: CompetitionRoomMatchProjection | null
  clockProjection: CompetitionRoomMatchProjection | null
}

/** 可信伺服器取樣目前時計；不得讓 HTTP 呼叫端指定時間或核心 operation。 */
export interface CompetitionRoomMatchClockReadRequest {
  competitionId: string
  matchId: string
  callerUserId: string
  trustedTimeMs: number
}

export interface CompetitionRoomMatchClockReadValue {
  competitionId: string
  matchId: string
  matchRevision: number
  matchStatus: 'open' | 'awaitingFirstPlayer' | 'awaitingReferee' | 'completed'
  game: { gameId: string; gameNumber: number } | null
  clock: ClockDisplay
}

export type CompetitionRoomMatchClockReadResult =
  CompetitionRoomResult<CompetitionRoomMatchClockReadValue>

/** 只供可信賽事編排層呼叫；規則、隊伍與牌組一律由 CompetitionRoom 派生。 */
export interface CompetitionRoomProvisionScheduledGameRequest {
  competitionId: string
  matchId: string
  callerUserId: string
  commandId: string
  expectedMatchRevision: number
  trustedTimeMs: number
  firstPlayer: string
  turnOrder: string[]
}

export type CompetitionRoomGameProvisionStatus = 'pending' | 'provisioned' | 'blocked'

export interface CompetitionRoomProvisionScheduledGameValue {
  competitionId: string
  matchId: string
  gameNumber: number
  gameId: string
  commandId: string
  idempotent: boolean
  status: CompetitionRoomGameProvisionStatus
  binding: CompetitionGameBinding
}

/** 後續局唯一物化入口；先手與座次必須由鎖定名單及 match checkpoint 推導。 */
export interface CompetitionRoomProvisionAssignedScheduledGameRequest {
  competitionId: string
  matchId: string
  callerUserId: string
  commandId: string
  expectedMatchRevision: number
  trustedTimeMs: number
}

export interface CompetitionRoomProvisionAssignedScheduledGameValue {
  competitionId: string
  matchId: string
  matchRevision: number
  gameNumber: number
  gameId: string
  commandId: string
  idempotent: boolean
  status: CompetitionRoomGameProvisionStatus
  binding: CompetitionGameBinding
}

/** 後續局的敗方隊長選先手；核心敗方及隊長身分均由伺服器重播與名冊推導。 */
export interface CompetitionRoomAssignNextFirstPlayerByCaptainRequest {
  competitionId: string
  matchId: string
  callerUserId: string
  commandId: string
  expectedMatchRevision: number
  trustedTimeMs: number
  selectedFirstPlayerTeam: string
}

export interface CompetitionRoomAssignNextFirstPlayerByCaptainValue {
  competitionId: string
  matchId: string
  matchRevision: number
  commandId: string
  idempotent: boolean
  matchProjection: CompetitionRoomPublicProjection
}

/** 敗方隊長已完成選先手後，才能由同一隊長建立該局的 GameRoom。 */
export interface CompetitionRoomProvisionCaptainAssignedScheduledGameRequest {
  competitionId: string
  matchId: string
  callerUserId: string
  commandId: string
  assignmentCommandId: string
  expectedMatchRevision: number
  trustedTimeMs: number
}

/** 不在回應或收據重複輸出鎖定名單；私有 binding 僅留在 provision linkage。 */
export interface CompetitionRoomProvisionCaptainAssignedScheduledGameValue {
  competitionId: string
  matchId: string
  matchRevision: number
  gameNumber: number
  gameId: string
  commandId: string
  idempotent: boolean
  status: CompetitionRoomGameProvisionStatus
}

/** 首局先手與固定座次只能由 CompetitionRoom 從隨機來源及鎖定名單推導。 */
export interface CompetitionRoomProvisionInitialScheduledGameRequest {
  competitionId: string
  matchId: string
  callerUserId: string
  commandId: string
  expectedMatchRevision: number
  trustedTimeMs: number
}

export interface CompetitionRoomProvisionInitialScheduledGameValue {
  competitionId: string
  matchId: string
  matchRevision: number
  gameNumber: 1
  gameId: string
  commandId: string
  idempotent: boolean
  status: CompetitionRoomGameProvisionStatus
  binding: CompetitionGameBinding
}

/** 後續 server-only start／pause 編排取得已持久化 gameId 與 binding。 */
export interface CompetitionRoomGameProvisionReadRequest {
  competitionId: string
  matchId: string
  callerUserId: string
}

export type CompetitionRoomGameProvisionReadValue = Omit<
  CompetitionRoomProvisionScheduledGameValue,
  'commandId' | 'idempotent'
>

/** 固定參賽者只查目前已就緒對局；名冊與房間 binding 不離開 CompetitionRoom。 */
export interface CompetitionRoomParticipantGameReadRequest {
  competitionId: string
  matchId: string
  callerUserId: string
}

export interface CompetitionRoomParticipantGameReadValue {
  competitionId: string
  matchId: string
  matchRevision: number
  game: { gameId: string; gameNumber: number } | null
}

export type CompetitionRoomParticipantGameReadResult =
  CompetitionRoomResult<CompetitionRoomParticipantGameReadValue>

export interface CompetitionRoomRefereeRequest {
  competitionId: string
  callerUserId: string
  targetUserId: string
  commandId: string
  expectedRevision: number
  trustedTimeMs: number
}

export interface CompetitionRoomAclValue {
  competitionId: string
  revision: number
  targetUserId: string
  role: CompetitionRoomRole
  active: boolean
  idempotent: boolean
}

export interface CompetitionRoomAuditQuery {
  competitionId: string
  callerUserId: string
  limit?: number
}

export interface CompetitionRoomAuditRecord {
  sequence: number
  matchId: string | null
  actorUserId: string
  role: CompetitionRoomRole
  operationType: CompetitionOperationName | 'grantReferee' | 'revokeReferee'
    | 'materializeScheduledMatch' | 'startScheduledMatch' | 'ingestGameTerminal'
    | 'provisionScheduledGame' | 'provisionInitialScheduledGame'
    | 'provisionAssignedScheduledGame'
    | 'assignNextFirstPlayerByCaptain'
    | 'provisionCaptainAssignedScheduledGame'
    | 'announceRound'
  commandId: string | null
  revision: number
  outcomeCode: string
  trustedTimeMs: number
  reason: string | null
}

export interface CompetitionRoomAuditValue {
  competitionId: string
  records: CompetitionRoomAuditRecord[]
}

export type CompetitionRoomExecuteResult = CompetitionRoomResult<CompetitionRoomExecuteValue>

export type CompetitionRoomMaterializeScheduledMatchResult =
  CompetitionRoomResult<CompetitionRoomMaterializeScheduledMatchValue>

export type CompetitionRoomStartScheduledMatchResult =
  CompetitionRoomResult<CompetitionRoomStartScheduledMatchValue>

export type CompetitionRoomCreateTournamentResult =
  CompetitionRoomResult<CompetitionRoomCreateTournamentValue>

export type CompetitionRoomTerminalIngestResult =
  CompetitionRoomResult<CompetitionRoomMatchExecuteValue>

export type CompetitionRoomProvisionScheduledGameResult =
  CompetitionRoomResult<CompetitionRoomProvisionScheduledGameValue>

export type CompetitionRoomProvisionInitialScheduledGameResult =
  CompetitionRoomResult<CompetitionRoomProvisionInitialScheduledGameValue>

export type CompetitionRoomProvisionAssignedScheduledGameResult =
  CompetitionRoomResult<CompetitionRoomProvisionAssignedScheduledGameValue>

export type CompetitionRoomAssignNextFirstPlayerByCaptainResult =
  CompetitionRoomResult<CompetitionRoomAssignNextFirstPlayerByCaptainValue>

export type CompetitionRoomProvisionCaptainAssignedScheduledGameResult =
  CompetitionRoomResult<CompetitionRoomProvisionCaptainAssignedScheduledGameValue>

export type CompetitionRoomPublicProjection = Pick<PublicCompetitionSuccess, 'ok' | 'operation' | 'public'>

export type CompetitionRoomPrivateProjection = JsonValue

/**
 * Clock checkpoint 的跨執行環境穩定 JSON；測試與 production 必須共用同一算法。
 */
export function stableJsonStringify(value: JsonValue): string {
  if (value === null || typeof value !== 'object') return JSON.stringify(value)
  if (Array.isArray(value)) return `[${value.map(stableJsonStringify).join(',')}]`
  const object = value as Record<string, JsonValue>
  return `{${Object.keys(object).sort().map(key => `${JSON.stringify(key)}:${stableJsonStringify(object[key]!)}`).join(',')}}`
}

export async function clockCheckpointDigest(checkpoint: JsonValue): Promise<string> {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(stableJsonStringify(checkpoint)),
  )
  return [...new Uint8Array(digest)].map(byte => byte.toString(16).padStart(2, '0')).join('')
}

/**
 * 以結構化 tuple 產生可重試且不會因分隔符碰撞的 timeout 識別碼。
 */
export async function clockTimeoutId(tuple: {
  competitionId: string
  matchId: string
  commandId: string
  revision: number
}): Promise<string> {
  return clockCheckpointDigest({
    kind: 'competition-clock-timeout',
    competitionId: tuple.competitionId,
    matchId: tuple.matchId,
    commandId: tuple.commandId,
    revision: tuple.revision,
  })
}
