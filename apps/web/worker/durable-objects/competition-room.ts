/// <reference types="@cloudflare/workers-types" />

import { DurableObject } from 'cloudflare:workers'
import {
  isJsonObject,
  isJsonValue,
  isExactCompetitionJson,
  type ClockConfig,
  type ClockDisplay,
  type CompetitionOperation,
  type CompetitionEngineResponse,
  type CompetitionOperationName,
  type CompetitionRequest,
  type CompetitionServerActor,
  type CompetitionServerState,
  type JsonObject,
  type JsonValue,
  type MatchFormat,
  type StageConfig,
  type RoundKind,
  type PublicCompetitionSuccess,
  type CompetitionRuling,
  type GameResultMode,
  type GameStartKind,
  type GameTerminalEvidence,
} from '../../shared/competition'
import { parseCompetitionAnnouncementPlan } from '../../shared/competition-announcement-plan'
import {
  clockCheckpointDigest, clockTimeoutId,
  isCompetitionRoomAnnounceRoundRequest,
  isCompetitionRoomAnnounceRoundValue,
  isCompetitionRoomInitialGameStartedRequest,
  isCompetitionRoomRoundPublication,
  isCompetitionRoomRoundPublicationReadRequest,
} from '../../shared/competition-room'
import {
  validateCompetitionGameBinding,
  type CompetitionGameBinding,
  type CompetitionGameCreateRequest,
  type CompetitionClockControlRequest,
  type CompetitionGameClockControlEligibilityRequest,
  type CompetitionGameNoShowDecision,
  type CompetitionGameNoShowRequest,
  isCompetitionClockControlAck,
  competitionGameBindingRefForRequest,
  competitionRoundReadyCutoffMs,
  competitionGameNoShowCommandId,
  isCompetitionGameNoShowDecision,
  isCompetitionGameBindingRef,
  isCompetitionRoundRef,
  isCompetitionGameInitialStartEvidence,
  isCompetitionRoomReadyAck,
  isCompetitionGameReleaseResult,
  type CompetitionGameAdmissionRequest,
  type CompetitionRoomReadyAck,
  type CompetitionGameBindingRef,
  type CompetitionRoundRef,
} from '../../shared/competition-game-room'
import {
  type CompetitionRoomAclValue,
  type CompetitionRoomAuditQuery,
  type CompetitionRoomAuditValue,
  type CompetitionRoomExecuteRequest,
  type CompetitionRoomExecuteResult,
  type CompetitionRoomExecuteValue,
  type CompetitionRoomInitializeRequest,
  type CompetitionRoomInitializeFromRulesRequest,
  type CompetitionRoomInitializeValue,
  type CompetitionRoomMatchExecuteRequest,
  type CompetitionRoomMatchExecuteValue,
  type CompetitionRoomMatchOperation,
  type CompetitionRoomMatchReadRequest,
  type CompetitionRoomMatchReadValue,
  type CompetitionRoomMatchClockReadRequest,
  type CompetitionRoomMatchClockReadResult,
  type CompetitionRoomMatchClockReadValue,
  type CompetitionRoomMatchProjection,
  type CompetitionRoomMaterializeScheduledMatchRequest,
  type CompetitionRoomMaterializeScheduledMatchResult,
  type CompetitionRoomStartScheduledMatchRequest,
  type CompetitionRoomStartScheduledMatchResult,
  type CompetitionRoomMaterializeScheduledMatchValue,
  type CompetitionRoomCreateTournamentRequest,
  type CompetitionRoomCreateTournamentResult,
  type CompetitionRoomCreateTournamentValue,
  type CompetitionRoomTerminalIngestRequest,
  type CompetitionRoomTerminalIngestResult,
  type CompetitionRoomPrivateProjection,
  type CompetitionRoomPublicProjection,
  type CompetitionRoomReadRequest,
  type CompetitionRoomReadValue,
  type CompetitionRoomRefereeRequest,
  type CompetitionRoomResult,
  type CompetitionRoomRole,
  type CompetitionRoomGameProvisionReadRequest,
  type CompetitionRoomGameProvisionReadValue,
  type CompetitionRoomParticipantGameReadRequest,
  type CompetitionRoomParticipantGameReadResult,
  type CompetitionRoomParticipantGameReadValue,
  type CompetitionRoomTournamentReadRequest,
  type CompetitionRoomTournamentReadResult,
  type CompetitionRoomTournamentReadValue,
  type CompetitionRoomProvisionScheduledGameRequest,
  type CompetitionRoomProvisionScheduledGameResult,
  type CompetitionRoomProvisionScheduledGameValue,
  type CompetitionRoomProvisionInitialScheduledGameRequest,
  type CompetitionRoomProvisionInitialScheduledGameResult,
  type CompetitionRoomProvisionInitialScheduledGameValue,
  type CompetitionRoomProvisionAssignedScheduledGameRequest,
  type CompetitionRoomProvisionAssignedScheduledGameResult,
  type CompetitionRoomProvisionAssignedScheduledGameValue,
  type CompetitionRoomAssignNextFirstPlayerByCaptainRequest,
  type CompetitionRoomAssignNextFirstPlayerByCaptainResult,
  type CompetitionRoomAssignNextFirstPlayerByCaptainValue,
  type CompetitionRoomProvisionCaptainAssignedScheduledGameRequest,
  type CompetitionRoomProvisionCaptainAssignedScheduledGameResult,
  type CompetitionRoomProvisionCaptainAssignedScheduledGameValue,
  type CompetitionRoomAnnounceRoundRequest,
  type CompetitionRoomAnnounceRoundResult,
  type CompetitionRoomAnnounceRoundValue,
  type CompetitionRoomInitialGameStartedRequest,
  type CompetitionRoomInitialGameStartedResult,
  type CompetitionRoomRoundPublication,
  type CompetitionRoomRoundPublicationReadRequest,
  type CompetitionRoomRoundPublicationReadResult,
} from '../../shared/competition-room'
import {
  callCompetitionEngine,
  isClockCheckpointMetadata,
  isResolvedCompetitionSettings,
  parseCompetitionEngineResponse,
  projectCompetitionResponse,
} from '../competition-bridge'

interface CompetitionRoomEnv {
  GAME_ROOM?: GameRoomNamespace
}

interface GameRoomNamespace {
  idFromName(name: string): DurableObjectId
  get(id: DurableObjectId): {
    createCompetitionGame(request: CompetitionGameCreateRequest): Promise<unknown>
    releaseCompetitionGame?(request: CompetitionGameAdmissionRequest): Promise<unknown>
    synchronizeCompetitionClock(request: CompetitionClockControlRequest): Promise<unknown>
    getCompetitionClockControlEligibility?(request: CompetitionGameClockControlEligibilityRequest): Promise<unknown>
    adjudicateCompetitionNoShow?(request: CompetitionGameNoShowRequest): Promise<unknown>
  }
}

interface CompetitionMetaRow {
  [key: string]: ArrayBuffer | string | number | null
  competition_id: string
  organizer_user_id: string
  revision: number
  server_state_json: string
  public_projection_json: string
  updated_trusted_time_ms: number
}

interface CompetitionTournamentReadMetaRow {
  [key: string]: ArrayBuffer | string | number | null
  competition_id: string
  organizer_user_id: string
  revision: number
  server_state_json: string
  updated_trusted_time_ms: number
}

interface AclRow {
  [key: string]: ArrayBuffer | string | number | null
  user_id: string
  role: CompetitionRoomRole
  active: number
  participant: number
}

interface PrivateProjectionRow {
  [key: string]: ArrayBuffer | string | number | null
  payload_json: string
  revision: number
  operation_type: CompetitionOperationName
}

interface AuditRow {
  [key: string]: ArrayBuffer | string | number | null
  id: number
  match_id: string | null
  actor_user_id: string
  role: CompetitionRoomRole
  operation_type: CompetitionOperationName | 'grantReferee' | 'revokeReferee'
    | 'materializeScheduledMatch' | 'startScheduledMatch' | 'ingestGameTerminal' | 'provisionScheduledGame'
    | 'provisionInitialScheduledGame' | 'provisionAssignedScheduledGame'
    | 'assignNextFirstPlayerByCaptain'
    | 'provisionCaptainAssignedScheduledGame'
    | 'announceRound'
  command_id: string | null
  revision: number
  outcome_code: string
  trusted_time_ms: number
  reason: string | null
}

const REDACTED_TERMINAL_ENGINE_FAILURE_MESSAGE = 'competition engine rejected terminal ingest'
const REDACTED_MATCH_CORRECTION_FAILURE_MESSAGE = 'competition engine rejected match correction'
const INITIAL_FIRST_PLAYER_REASON = '賽事伺服器可信隨機先手'
const CAPTAIN_FIRST_PLAYER_REASON = '敗方隊長選擇本局先手隊伍'

interface CommandReceiptRow {
  [key: string]: ArrayBuffer | string | number | null
  receipt_kind: ReceiptKind
  match_id: string | null
  fingerprint: string
  response_json: string
}

interface MatchRow {
  [key: string]: ArrayBuffer | string | number | null
  match_id: string
  stage_id: string | null
  match_checkpoint_json: string
  clock_checkpoint_json: string | null
  match_projection_json: string | null
  clock_projection_json: string | null
  revision: number
  last_trusted_time_ms: number
  tournament_result_confirmed_at_ms: number | null
}

interface GameProvisionRow {
  [key: string]: ArrayBuffer | string | number | null
  match_id: string
  game_number: number
  game_id: string
  binding_json: string
  match_revision: number
  next_game_json: string
  command_id: string
  fingerprint: string
  status: 'pending' | 'provisioned' | 'blocked'
  attempts: number
  next_attempt_at_ms: number
  error_code: string | null
  created_trusted_time_ms: number
  claim_id: string | null
  claim_version: number
}

interface RoundPublicationRow {
  [key: string]: ArrayBuffer | string | number | null
  round_generation_id: string
  stage_id: string
  round_number: number
  round_start_ms: number
  release_not_before_ms: number
  publication_version: number
  status: CompetitionRoomRoundPublication['status']
  release_decision_id: string | null
  requires_referee: number
  manifest_json: string
  announced_trusted_time_ms: number
}

interface RoundRoomLedgerRow {
  [key: string]: ArrayBuffer | string | number | null
  round_generation_id: string
  match_id: string
  game_number: number
  game_id: string
  binding_json: string
  binding_ref_json: string
  next_game_json: string
  match_revision: number
  status: 'pending' | 'confirmed' | 'blocked' | 'revoked' | 'started'
  ack_confirmed_at_ms: number | null
  claim_id: string | null
  claim_version: number
  attempts: number
  next_attempt_at_ms: number
}

interface RoundManifest {
  announcementCommandId: string
  roundRef: CompetitionRoundRef
  roundStartMs: number
  tournamentRound: JsonObject
  rooms: { matchId: string; gameId: string; bindingRef: CompetitionGameBindingRef }[]
}

interface RoundReleaseRow {
  [key: string]: ArrayBuffer | string | number | null
  round_generation_id: string
  game_id: string
  release_decision_id: string
  status: 'pending' | 'completed'
  claim_id: string | null
  claim_version: number
  attempts: number
  next_attempt_at_ms: number
  completed_at_ms: number | null
}

interface NoShowOutboxRow {
  [key: string]: ArrayBuffer | string | number | null
  round_generation_id: string
  match_id: string
  game_id: string
  deadline_ms: number
  command_id: string
  status: 'pending' | 'completed'
  decision_json: string | null
  claim_id: string | null
  claim_version: number
  attempts: number
  next_attempt_at_ms: number
  result_confirmed_at_ms: number | null
}

interface RoundDeliveryAuthority {
  meta: CompetitionMetaRow
  header: RoundPublicationRow
  manifest: RoundManifest
  rows: RoundRoomLedgerRow[]
  matches: MatchRow[]
}

type RoundDeliveryClaim = {
  kind: 'create'
  authority: RoundDeliveryAuthority
  row: RoundRoomLedgerRow
  request: CompetitionGameCreateRequest
  bindingRef: CompetitionGameBindingRef
} | {
  kind: 'release'
  authority: RoundDeliveryAuthority
  row: RoundReleaseRow
  request: CompetitionGameAdmissionRequest
}

interface NoShowDeliveryClaim {
  row: NoShowOutboxRow
  request: CompetitionGameNoShowRequest
  decision: CompetitionGameNoShowDecision | null
}

interface PreparedRoundRoom {
  matchId: string
  checkpoint: JsonObject
  clock: JsonObject
  matchProjection: CompetitionRoomMatchProjection
  clockProjection: CompetitionRoomMatchProjection
  nextGame: JsonObject
  binding: CompetitionGameBinding
  bindingRef: CompetitionGameBindingRef
}

interface ClockControlRow {
  [key: string]: ArrayBuffer | string | number | null
  control_id: string
  competition_id: string
  match_id: string
  game_id: string
  game_number: number
  source_command_id: string
  match_revision: number
  operation: 'clockPause' | 'clockResume'
  trusted_time_ms: number
  next_clock_deadline_ms: number | null
  attempts: number
  next_attempt_at_ms: number
  status: 'pending' | 'completed'
  claim_id: string | null
  claim_version: number
}

interface InitialSeedingIntentRow {
  [key: string]: ArrayBuffer | string | number | null
  competition_id: string
  command_id: string
  stage_id: string
  stage_json: string
  status: 'pending' | 'completed'
  attempts: number
  next_attempt_at_ms: number
  last_error: string | null
  created_trusted_time_ms: number
}

interface GameProvisionDeliveryClaim {
  row: GameProvisionRow
  binding: CompetitionGameBinding
}

interface ClockControlDeliveryClaim {
  row: ClockControlRow
  request: CompetitionClockControlRequest
}

interface ClockEligibilityProbePlan {
  kind: 'clockEligibilityProbe'
  snapshot: string
  request: CompetitionGameClockControlEligibilityRequest
}

interface ClockEligibilityProbeResult {
  snapshot: string
  eligible: boolean
}

type MatchExecuteInternalResult =
  | CompetitionRoomResult<CompetitionRoomMatchExecuteValue>
  | ClockEligibilityProbePlan

interface LegacyAlarmBatch {
  remaining: number
}

export type CompetitionEngineAdapter = (
  request: CompetitionRequest,
) => Promise<CompetitionEngineResponse>

type ExecuteOperation = CompetitionRoomExecuteRequest['operation']
type MatchExecuteOperation = CompetitionRoomMatchOperation
type RoomFailure = { ok: false; error: { code: string; message: string } }
type RoomSuccess<T> = { ok: true; value: T }
type ReceiptKind =
  | 'announceRound'
  | 'execute'
  | 'createTournamentFromLockedRoster'
  | 'startScheduledMatch'
  | 'matchExecute'
  | 'materializeScheduledMatch'
  | 'ingestGameTerminal'
  | 'ingestInitialGameStarted'
  | 'provisionScheduledGame'
  | 'provisionInitialScheduledGame'
  | 'provisionAssignedScheduledGame'
  | 'assignNextFirstPlayerByCaptain'
  | 'provisionCaptainAssignedScheduledGame'
  | 'grantReferee'
  | 'revokeReferee'

interface ScheduledMatchMaterialization {
  matchId: string
  stageId: string
  teams: [string, string]
  format: MatchFormat
  clockConfig: ClockConfig
  status: 'pending' | 'inProgress' | 'completed' | 'cancelled'
  started: boolean
  result: JsonValue
}

const SCHEMA_VERSION = 15

const OPERATION_NAMES: CompetitionOperationName[] = [
  'registrationCreate',
  'registrationExecute',
  'registrationView',
  'registrationExportLockedRoster',
  'clockCreate',
  'clockBeginPreparation',
  'clockFinishPreparation',
  'clockBeginAction',
  'clockBeginOutOfTurnChoice',
  'clockObserve',
  'clockEndWindow',
  'clockPause',
  'clockResume',
  'clockNewGame',
  'clockRuleGame',
  'tournamentCreate',
  'tournamentStartMatch',
  'tournamentSubmitResult',
  'tournamentCorrectResult',
  'tournamentDropTeam',
  'matchCreate',
  'matchView',
  'matchAssignFirstPlayer',
  'matchSubmitGame',
  'matchCorrectRuling',
]

const MATCH_MUTATION_NAMES = new Set<CompetitionOperationName>([
  'matchAssignFirstPlayer',
  'matchSubmitGame',
  'matchCorrectRuling',
])

export class CompetitionRoom extends DurableObject<CompetitionRoomEnv> {
  private operationTail: Promise<void> = Promise.resolve()
  private readonly engine: CompetitionEngineAdapter
  private readonly storage: DurableObjectState['storage']
  private readonly roomEnv: CompetitionRoomEnv
  private readonly randomBit: () => number
  private readonly randomUint32: () => number

  constructor(
    ctx: DurableObjectState,
    env: CompetitionRoomEnv,
    engine: CompetitionEngineAdapter = callCompetitionEngine,
    randomBit: () => number = secureRandomBit,
    randomUint32: () => number = secureRandomUint32,
  ) {
    super(ctx, env)
    this.storage = ctx.storage
    this.roomEnv = env
    this.engine = engine
    this.randomBit = randomBit
    this.randomUint32 = randomUint32
    void ctx.blockConcurrencyWhile(async () => {
      this.migrate()
    })
  }

  async initialize(
    request: CompetitionRoomInitializeRequest,
  ): Promise<CompetitionRoomResult<CompetitionRoomInitializeValue>> {
    return this.serialized(() => this.initializeInternal(request))
  }

  async initializeFromRules(
    request: CompetitionRoomInitializeFromRulesRequest,
  ): Promise<CompetitionRoomResult<CompetitionRoomInitializeValue>> {
    return this.serialized(() => this.initializeFromRulesInternal(request))
  }

  async execute(request: CompetitionRoomExecuteRequest): Promise<CompetitionRoomExecuteResult> {
    return this.serialized(async () => {
      // 明示 Lock 本身就是截止凍結，避免先自動鎖定再讓原命令遇到過期版號。
      const rawOperation: unknown = request?.operation
      const isExplicitLock = isJsonObject(rawOperation)
        && rawOperation.type === 'registrationExecute'
        && isJsonObject(rawOperation.action)
        && rawOperation.action.type === 'lock'
      const freezeWasDue = !validateExecuteRequest(request) && !isExplicitLock
        ? await this.freezeRegistrationIfDue(request.trustedTimeMs)
        : false
      const result = await this.executeInternal(request)
      if (request?.operation?.type === 'registrationExecute' && result.ok) {
        await this.pumpInitialSeedingInternal()
      }
      if ((request?.operation?.type === 'registrationExecute' || freezeWasDue)
        && !(result.ok === false && result.error.code === 'alarmSchedulingFailed')) {
        try { await this.scheduleSharedAlarm() } catch {
          console.warn('CompetitionRoom retained registration freeze after alarm scheduling failure')
        }
      }
      return result
    })
  }

  /** 先原子保存完整整輪 manifest/房間 ledger；foreign 建房另由持久 claim 交付。 */
  async announceRound(request: CompetitionRoomAnnounceRoundRequest): Promise<CompetitionRoomAnnounceRoundResult> {
    const cutoff = isCompetitionRoomAnnounceRoundRequest(request)
      ? competitionRoundReadyCutoffMs(request.roundStartMs)
      : null
    const releaseNotBeforeMs = cutoff ?? 0
    const result = await this.serialized(() => this.announceRoundInternal(request, releaseNotBeforeMs))
    if (result.ok && !result.value.idempotent) {
      // 原成功收據不回填 ACK；先持久成功再排工作，排程失敗亦獨立嘗試交付。
      const scheduled = await this.serialized(async () => {
        try { await this.scheduleSharedAlarm(); return true } catch { return false }
      })
      if (!scheduled) await this.pumpRoundDeliveries()
    }
    return result
  }

  /** 只讀主辦者公告；不存在公告時回 null，絕不補造 T 或觸發交付。 */
  async readRoundPublication(
    request: CompetitionRoomRoundPublicationReadRequest,
  ): Promise<CompetitionRoomRoundPublicationReadResult> {
    return this.serialized(() => this.readRoundPublicationInternal(request))
  }

  /** 首局 FIFO 專用；只以父房本地 proof 接受既成 start，不反呼子房。 */
  async ingestInitialGameStarted(
    request: CompetitionRoomInitialGameStartedRequest,
  ): Promise<CompetitionRoomInitialGameStartedResult> {
    return this.serialized(() => this.ingestInitialGameStartedInternal(request))
  }

  async executeMatch(
    request: CompetitionRoomMatchExecuteRequest,
  ): Promise<CompetitionRoomResult<CompetitionRoomMatchExecuteValue>> {
    const pauseResume = request?.operation?.type === 'clockPause'
      || request?.operation?.type === 'clockResume'
    const command = pauseResume ? structuredClone(request) : request
    const first = await this.serialized(() => this.executeMatchInternal(command))
    let result: CompetitionRoomResult<CompetitionRoomMatchExecuteValue>
    if (isClockEligibilityProbePlan(first)) {
      // 此 await 必須在 serialized 呼叫之外，讓其他 parent 工作可進 tail。
      const probeResult = await this.probeClockControlEligibility(first)
      const second = await this.serialized(() => this.executeMatchInternal(command, probeResult))
      result = isClockEligibilityProbePlan(second)
        ? roomFailure(
            'gameRoomNotProvisioned',
            'the current match game must be active before its clock can be controlled',
          )
        : second
    } else {
      result = first
    }
    if (result.ok && pauseResume) {
      await this.dispatchClockControlForCommand(result.value.commandId)
    }
    if (result.ok) await this.kickAutomaticRoundAdvance()
    return result
  }

  async materializeScheduledMatch(
    request: CompetitionRoomMaterializeScheduledMatchRequest,
  ): Promise<CompetitionRoomMaterializeScheduledMatchResult> {
    return this.startScheduledMatch(request)
  }

  /** 唯一原子入口：啟動 Rust 賽程交手後，建立其 match 與 clock checkpoint。 */
  async startScheduledMatch(
    request: CompetitionRoomStartScheduledMatchRequest,
  ): Promise<CompetitionRoomStartScheduledMatchResult> {
    return this.serialized(() => this.startScheduledMatchInternal(request))
  }

  /** 唯一可建立賽程的入口；先核對報到鎖定名單，再原子保存 tournament。 */
  async createTournamentFromLockedRoster(
    request: CompetitionRoomCreateTournamentRequest,
  ): Promise<CompetitionRoomCreateTournamentResult> {
    return this.serialized(() => this.createTournamentFromLockedRosterInternal(request))
  }

  /** 可信賽事編排層專用；先持久化，再以 outbox 至少一次建立 Waiting GameRoom。 */
  async provisionScheduledGame(
    request: CompetitionRoomProvisionScheduledGameRequest,
  ): Promise<CompetitionRoomProvisionScheduledGameResult> {
    return roomFailure(
      'unsupportedOperation',
      'caller-provided first player and turn order are no longer accepted',
    )
  }

  /** 後續局唯一入口：由目前 assigned first player 與鎖定名單推導座次。 */
  async provisionAssignedScheduledGame(
    request: CompetitionRoomProvisionAssignedScheduledGameRequest,
  ): Promise<CompetitionRoomProvisionAssignedScheduledGameResult> {
    const result = await this.serialized(() => this.provisionAssignedScheduledGameInternal(request))
    if (!result.ok) return result
    const delivered = await this.dispatchGameProvision(result.value.matchId, result.value.gameNumber)
    if (!delivered) return roomFailure('internalFailure', 'game provision linkage disappeared after commit')
    return roomSuccess({ ...result.value, status: delivered.status })
  }

  /** 後續普通局僅由前局有效敗方隊長指定先手隊；身分以核心重播及鎖定名冊核實。 */
  async assignNextFirstPlayerByCaptain(
    request: CompetitionRoomAssignNextFirstPlayerByCaptainRequest,
  ): Promise<CompetitionRoomAssignNextFirstPlayerByCaptainResult> {
    return this.serialized(() => this.assignNextFirstPlayerByCaptainInternal(request))
  }

  /** 僅憑前一個有效隊長指派收據，為已指派的後續局建立 GameRoom。 */
  async provisionCaptainAssignedScheduledGame(
    request: CompetitionRoomProvisionCaptainAssignedScheduledGameRequest,
  ): Promise<CompetitionRoomProvisionCaptainAssignedScheduledGameResult> {
    const result = await this.serialized(() => this.provisionCaptainAssignedScheduledGameInternal(request))
    if (!result.ok) return result
    const delivered = await this.dispatchGameProvision(result.value.matchId, result.value.gameNumber)
    if (!delivered) return roomFailure('internalFailure', 'game provision linkage disappeared after commit')
    return roomSuccess({ ...result.value, status: delivered.status })
  }

  /** 首局唯一入口：伺服器隨機先手、鎖定名單座次與房間連結原子提交。 */
  async provisionInitialScheduledGame(
    request: CompetitionRoomProvisionInitialScheduledGameRequest,
  ): Promise<CompetitionRoomProvisionInitialScheduledGameResult> {
    const result = await this.serialized(() => this.provisionInitialScheduledGameInternal(request))
    if (!result.ok) return result
    const delivered = await this.dispatchGameProvision(result.value.matchId, result.value.gameNumber)
    if (!delivered) return roomFailure('internalFailure', 'game provision linkage disappeared after commit')
    return roomSuccess({ ...result.value, status: delivered.status })
  }

  /** 可信伺服器取得目前賽程對局房連結；不提供公開 HTTP。 */
  async readProvisionedGame(
    request: CompetitionRoomGameProvisionReadRequest,
  ): Promise<CompetitionRoomResult<CompetitionRoomGameProvisionReadValue>> {
    return this.serialized(() => this.readProvisionedGameInternal(request))
  }

  /** 固定參賽者唯讀取得目前已就緒對局；不輸出或改寫私有房間 binding。 */
  async readParticipantGame(
    request: CompetitionRoomParticipantGameReadRequest,
  ): Promise<CompetitionRoomParticipantGameReadResult> {
    return this.serialized(() => this.readParticipantGameInternal(request))
  }

  /** 授權成員只讀目前局時計；取樣不提交裁判結果或觸發任何 outbox。 */
  async readMatchClock(
    request: CompetitionRoomMatchClockReadRequest,
  ): Promise<CompetitionRoomMatchClockReadResult> {
    return this.serialized(() => this.readMatchClockInternal(request))
  }

  /** 活躍賽事成員唯讀取得 Rust tournamentView；不讀取最後一次 global projection。 */
  async readTournament(
    request: CompetitionRoomTournamentReadRequest,
  ): Promise<CompetitionRoomTournamentReadResult> {
    return this.serialized(() => this.readTournamentInternal(request))
  }

  /**
   * GameRoom 的唯一 terminal 接收入口；判定與提交在同一個 serial turn 內完成。
   */
  async ingestGameTerminal(
    request: CompetitionRoomTerminalIngestRequest,
  ): Promise<CompetitionRoomTerminalIngestResult> {
    const result = await this.serialized(() => this.ingestGameTerminalInternal(request))
    if (result.ok) {
      await this.dispatchAutomaticDrawProvision(result.value.matchId, result.value)
      await this.kickAutomaticRoundAdvance()
    }
    return result
  }

  async read(
    request: CompetitionRoomReadRequest,
  ): Promise<CompetitionRoomResult<CompetitionRoomReadValue>> {
    return this.serialized(() => this.readInternal(request))
  }

  async readMatch(
    request: CompetitionRoomMatchReadRequest,
  ): Promise<CompetitionRoomResult<CompetitionRoomMatchReadValue>> {
    return this.serialized(() => this.readMatchInternal(request))
  }

  async grantReferee(
    request: CompetitionRoomRefereeRequest,
  ): Promise<CompetitionRoomResult<CompetitionRoomAclValue>> {
    return this.serialized(() => this.changeRefereeInternal(request, 'grantReferee'))
  }

  async revokeReferee(
    request: CompetitionRoomRefereeRequest,
  ): Promise<CompetitionRoomResult<CompetitionRoomAclValue>> {
    return this.serialized(() => this.changeRefereeInternal(request, 'revokeReferee'))
  }

  async audit(
    request: CompetitionRoomAuditQuery,
  ): Promise<CompetitionRoomResult<CompetitionRoomAuditValue>> {
    return this.serialized(() => this.auditInternal(request))
  }

  override async alarm(): Promise<void> {
    await this.serialized(async () => {
      try {
        const current = this.meta()
        if (current) await this.freezeRegistrationIfDue(Math.max(Date.now(), current.updated_trusted_time_ms))
      } catch {
        console.warn('CompetitionRoom retained registration freeze after alarm execution failure')
      }
      this.expireMissedRoundReadinessWindows(Date.now())
    })
    // 每種工作各自有界循環；claim 與 ACK 短暫進 tail，foreign RPC 全在 tail 外。
    const legacyBatch: LegacyAlarmBatch = { remaining: 16 }
    await Promise.all([
      this.serialized(() => this.pumpInitialSeedingInternal()),
      this.pumpRoundDeliveries(),
      this.pumpNoShowRulings(),
      this.pumpGameProvisionDeliveries(legacyBatch),
      this.pumpClockControlDeliveries(legacyBatch),
    ])
    await this.serialized(async () => {
      let advancement: 'announced' | 'retry' | 'none' = 'none'
      try { advancement = await this.publishNextAutomaticRoundIfReady() } catch {
        console.warn('CompetitionRoom retained automatic round advancement after an attempt failure')
        advancement = 'retry'
      }
      this.expireMissedRoundReadinessWindows(Date.now())
      const retry = advancement === 'retry' ? [checkedRoundRetryTime(Date.now(), 5_000)] : []
      try { await this.scheduleSharedAlarm(undefined, retry) } catch {
        console.warn('CompetitionRoom retained due work after alarm scheduling failure')
      }
    })
  }

  private async serialized<T>(operation: () => Promise<T>): Promise<T> {
    const previous = this.operationTail
    let release = () => {}
    this.operationTail = new Promise<void>((resolve) => {
      release = resolve
    })

    await previous
    try {
      return await operation()
    } catch (error) {
      console.error('CompetitionRoom operation failed', error)
      return roomFailure('internalFailure', 'competition room operation failed') as T
    } finally {
      release()
    }
  }

  private async kickAutomaticRoundAdvance(): Promise<void> {
    const announced = await this.serialized(async () => {
      try { return await this.publishNextAutomaticRoundIfReady() } catch {
        console.warn('CompetitionRoom retained automatic round advancement after an attempt failure')
        return 'retry' as const
      }
    })
    if (announced === 'none') return
    const retry = announced === 'retry' ? [checkedRoundRetryTime(Date.now(), 5_000)] : []
    const scheduled = await this.serialized(async () => {
      try { await this.scheduleSharedAlarm(undefined, retry); return true } catch { return false }
    })
    if (!scheduled) await this.pumpRoundDeliveries()
  }

  /** 先在短 serial turn 取得目前狀態，再由既有 durable queue 在 tail 外派送。 */
  private async dispatchGameProvision(matchId: string, gameNumber: number): Promise<GameProvisionRow | undefined> {
    const before = await this.serialized(async () => {
      const row = this.gameProvision(matchId, gameNumber)
      if (row?.status === 'pending') {
        try { await this.scheduleSharedAlarm() } catch {
          console.warn('CompetitionRoom retained game provision after alarm scheduling failure')
        }
      }
      return row
    })
    if (before?.status === 'pending') {
      await this.pumpGameProvisionDeliveries({ remaining: 1 })
    }
    return this.serialized(async () => this.gameProvision(matchId, gameNumber))
  }

  /** 命令收據已提交後才啟動 clock queue；foreign await 不佔 parent operationTail。 */
  private async dispatchClockControlForCommand(commandId: string): Promise<void> {
    const pending = await this.serialized(async () => {
      const row = this.storage.sql.exec<{ control_id: string }>(
        `SELECT control_id FROM competition_clock_control_outbox
         WHERE source_command_id = ? AND status = 'pending' LIMIT 1`,
        commandId,
      ).toArray()[0]
      if (row) {
        try { await this.scheduleSharedAlarm() } catch {
          console.warn('CompetitionRoom retained clock control after alarm scheduling failure')
        }
      }
      return row
    })
    if (pending) await this.pumpClockControlDeliveries({ remaining: 1 })
  }

  /** foreign eligibility RPC 在 tail 外；結果只交給同一筆命令的晚重驗。 */
  private async probeClockControlEligibility(
    plan: ClockEligibilityProbePlan,
  ): Promise<ClockEligibilityProbeResult> {
    let eligible = false
    try {
      const namespace = this.env.GAME_ROOM
      if (namespace) {
        const stub = namespace.get(namespace.idFromName(plan.request.gameId))
        const probe = stub.getCompetitionClockControlEligibility
        // RPC callable 的 call 屬性會查找遠端方法；直接 apply 並保留原 receiver。
        if (typeof probe === 'function') {
          eligible = await Reflect.apply(probe, stub, [plan.request]) === true
        }
      }
    } catch {
      eligible = false
    }
    return { snapshot: plan.snapshot, eligible }
  }

  /** terminal 收據只在確有自動續局 linkage 時喚醒同一個 durable game-provision queue。 */
  private async dispatchAutomaticDrawProvision(
    matchId: string,
    result: CompetitionRoomMatchExecuteValue,
  ): Promise<void> {
    const projection = result.matchProjection?.public
    const match = isJsonObject(projection) && isJsonObject(projection.match) ? projection.match : null
    const nextGame = match?.nextGame
    if (!isJsonObject(nextGame)) return
    const gameNumber = nextGame.gameNumber
    if (!safeNonNegativeInteger(gameNumber) || gameNumber < 1) return
    const row = await this.serialized(async () => this.gameProvision(matchId, gameNumber))
    if (row?.status === 'pending' && row.command_id.startsWith('auto-draw-provision:')) {
      await this.dispatchGameProvision(matchId, row.game_number)
    }
  }

  private migrate(): void {
    this.storage.transactionSync(() => {
      this.storage.sql.exec(`
        CREATE TABLE IF NOT EXISTS competition_schema_migrations (
          version INTEGER PRIMARY KEY,
          applied_at_trusted_time_ms INTEGER NOT NULL
        )
      `)
      const currentVersion = this.storage.sql.exec<{
        [key: string]: ArrayBuffer | string | number | null
        version: number
      }>(
        'SELECT COALESCE(MAX(version), 0) AS version FROM competition_schema_migrations',
      ).one().version

      if (currentVersion < 1) {
        this.storage.sql.exec(`
          CREATE TABLE IF NOT EXISTS competition_meta (
            singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
            competition_id TEXT NOT NULL,
            organizer_user_id TEXT NOT NULL,
            revision INTEGER NOT NULL,
            server_state_json TEXT NOT NULL,
            public_projection_json TEXT NOT NULL,
            updated_trusted_time_ms INTEGER NOT NULL
          )
        `)
        this.storage.sql.exec(`
          CREATE TABLE IF NOT EXISTS competition_acl (
            user_id TEXT PRIMARY KEY,
            role TEXT NOT NULL CHECK (role IN ('organizer', 'referee', 'player')),
            active INTEGER NOT NULL,
            participant INTEGER NOT NULL
          )
        `)
        this.storage.sql.exec(`
          CREATE TABLE IF NOT EXISTS competition_private_projection (
            user_id TEXT PRIMARY KEY,
            payload_json TEXT NOT NULL
          )
        `)
        this.storage.sql.exec(`
          CREATE TABLE IF NOT EXISTS competition_audit (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            actor_user_id TEXT NOT NULL,
            role TEXT NOT NULL,
            operation_type TEXT NOT NULL,
            command_id TEXT,
            revision INTEGER NOT NULL,
            outcome_code TEXT NOT NULL,
            trusted_time_ms INTEGER NOT NULL
          )
        `)
        this.storage.sql.exec(`
          CREATE TABLE IF NOT EXISTS competition_command_receipt (
            command_id TEXT PRIMARY KEY,
            fingerprint TEXT NOT NULL,
            response_json TEXT NOT NULL
          )
        `)
        this.storage.sql.exec(
          'INSERT INTO competition_schema_migrations (version, applied_at_trusted_time_ms) VALUES (1, 0)',
        )
      }

      if (currentVersion < 2) {
        this.storage.sql.exec(
          `ALTER TABLE competition_private_projection
           ADD COLUMN revision INTEGER NOT NULL DEFAULT 0`,
        )
        this.storage.sql.exec(
          `ALTER TABLE competition_private_projection
           ADD COLUMN operation_type TEXT NOT NULL DEFAULT 'unknown'`,
        )
        this.storage.sql.exec(
          'INSERT INTO competition_schema_migrations (version, applied_at_trusted_time_ms) VALUES (2, 0)',
        )
      }

      if (currentVersion < 3) {
        this.storage.sql.exec(
          `ALTER TABLE competition_command_receipt
           ADD COLUMN receipt_kind TEXT NOT NULL DEFAULT 'execute'`,
        )
        this.storage.sql.exec(
          'INSERT INTO competition_schema_migrations (version, applied_at_trusted_time_ms) VALUES (3, 0)',
        )
      }

      if (currentVersion < 4) {
        this.storage.sql.exec(`
          CREATE TABLE IF NOT EXISTS competition_matches (
            match_id TEXT PRIMARY KEY,
            match_checkpoint_json TEXT NOT NULL,
            clock_checkpoint_json TEXT,
            match_projection_json TEXT,
            clock_projection_json TEXT,
            revision INTEGER NOT NULL,
            last_trusted_time_ms INTEGER NOT NULL
          )
        `)
        this.storage.sql.exec(
          `ALTER TABLE competition_audit ADD COLUMN match_id TEXT`,
        )
        this.storage.sql.exec(
          `ALTER TABLE competition_command_receipt ADD COLUMN match_id TEXT`,
        )

        const legacyMeta = this.storage.sql.exec<{
          [key: string]: ArrayBuffer | string | number | null
          server_state_json: string
        }>(
          'SELECT server_state_json FROM competition_meta WHERE singleton = 1',
        ).toArray()[0]
        if (legacyMeta) {
          const parsed = JSON.parse(legacyMeta.server_state_json) as unknown
          if (isJsonObject(parsed) && isJsonObject(parsed.matches)) {
            for (const [matchId, checkpoint] of Object.entries(parsed.matches)) {
              if (!isJsonObject(checkpoint)) continue
              this.storage.sql.exec(
                `INSERT OR IGNORE INTO competition_matches
                   (match_id, match_checkpoint_json, clock_checkpoint_json,
                    match_projection_json, clock_projection_json, revision,
                    last_trusted_time_ms)
                 VALUES (?, ?, NULL, NULL, NULL, 0, 0)`,
                matchId,
                JSON.stringify(checkpoint),
              )
            }
          }
        }
        this.storage.sql.exec(
          'INSERT INTO competition_schema_migrations (version, applied_at_trusted_time_ms) VALUES (4, 0)',
        )
      }

      if (currentVersion < 5) {
        this.storage.sql.exec(
          `ALTER TABLE competition_audit ADD COLUMN reason TEXT`,
        )
        this.storage.sql.exec(
          'INSERT INTO competition_schema_migrations (version, applied_at_trusted_time_ms) VALUES (5, 0)',
        )
      }

      if (currentVersion < 6) {
        this.storage.sql.exec(
          'ALTER TABLE competition_matches ADD COLUMN stage_id TEXT',
        )
        this.storage.sql.exec(
          'INSERT INTO competition_schema_migrations (version, applied_at_trusted_time_ms) VALUES (6, 0)',
        )
      }

      if (currentVersion < 7) {
        this.storage.sql.exec(`
          CREATE TABLE IF NOT EXISTS competition_game_provision (
            match_id TEXT NOT NULL,
            game_number INTEGER NOT NULL,
            game_id TEXT NOT NULL UNIQUE,
            binding_json TEXT NOT NULL,
            match_revision INTEGER NOT NULL,
            next_game_json TEXT NOT NULL,
            command_id TEXT NOT NULL UNIQUE,
            fingerprint TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('pending', 'provisioned', 'blocked')),
            attempts INTEGER NOT NULL,
            next_attempt_at_ms INTEGER NOT NULL,
            error_code TEXT,
            created_trusted_time_ms INTEGER NOT NULL,
            PRIMARY KEY (match_id, game_number)
          )
        `)
        this.storage.sql.exec(
          'INSERT INTO competition_schema_migrations (version, applied_at_trusted_time_ms) VALUES (7, 0)',
        )
      }

      if (currentVersion < 8) {
        this.storage.sql.exec(`
          CREATE TABLE IF NOT EXISTS competition_clock_control_outbox (
            control_id TEXT PRIMARY KEY,
            competition_id TEXT NOT NULL,
            match_id TEXT NOT NULL,
            game_id TEXT NOT NULL,
            game_number INTEGER NOT NULL,
            source_command_id TEXT NOT NULL,
            match_revision INTEGER NOT NULL,
            operation TEXT NOT NULL CHECK (operation IN ('clockPause', 'clockResume')),
            trusted_time_ms INTEGER NOT NULL,
            next_clock_deadline_ms INTEGER,
            attempts INTEGER NOT NULL,
            next_attempt_at_ms INTEGER NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('pending', 'completed'))
          )
        `)
        this.storage.sql.exec(
          'INSERT INTO competition_schema_migrations (version, applied_at_trusted_time_ms) VALUES (8, 0)',
        )
      }

      if (currentVersion < 9) {
        this.storage.sql.exec(`
          CREATE TABLE competition_round_publication (
            round_generation_id TEXT PRIMARY KEY,
            stage_id TEXT NOT NULL,
            round_number INTEGER NOT NULL CHECK (round_number > 0),
            round_start_ms INTEGER NOT NULL CHECK (round_start_ms >= 300000),
            publication_version INTEGER NOT NULL CHECK (publication_version > 0),
            status TEXT NOT NULL CHECK (status IN ('materializing', 'released', 'quiescing', 'retired')),
            release_decision_id TEXT,
            requires_referee INTEGER NOT NULL CHECK (requires_referee IN (0, 1)),
            manifest_json TEXT NOT NULL,
            announced_trusted_time_ms INTEGER NOT NULL
          )
        `)
        this.storage.sql.exec(`
          CREATE UNIQUE INDEX competition_round_current_publication
          ON competition_round_publication (stage_id, round_number) WHERE status != 'retired'
        `)
        this.storage.sql.exec(`
          CREATE TABLE competition_round_room_ledger (
            game_id TEXT PRIMARY KEY,
            round_generation_id TEXT NOT NULL,
            match_id TEXT NOT NULL,
            game_number INTEGER NOT NULL CHECK (game_number > 0),
            binding_json TEXT NOT NULL,
            binding_ref_json TEXT NOT NULL,
            next_game_json TEXT NOT NULL,
            match_revision INTEGER NOT NULL CHECK (match_revision > 0),
            status TEXT NOT NULL CHECK (status IN ('pending', 'confirmed', 'blocked', 'revoked', 'started')),
            ack_confirmed_at_ms INTEGER,
            claim_id TEXT,
            claim_version INTEGER NOT NULL CHECK (claim_version >= 0),
            attempts INTEGER NOT NULL CHECK (attempts >= 0),
            next_attempt_at_ms INTEGER NOT NULL,
            UNIQUE (round_generation_id, match_id, game_number)
          )
        `)
        this.storage.sql.exec(
          'INSERT INTO competition_schema_migrations (version, applied_at_trusted_time_ms) VALUES (9, 0)',
        )
      }

      if (currentVersion < 10) {
        this.storage.sql.exec(`
          CREATE TABLE competition_round_release_outbox (
            round_generation_id TEXT NOT NULL,
            game_id TEXT NOT NULL,
            release_decision_id TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('pending', 'completed')),
            claim_id TEXT,
            claim_version INTEGER NOT NULL CHECK (claim_version >= 0),
            attempts INTEGER NOT NULL CHECK (attempts >= 0),
            next_attempt_at_ms INTEGER NOT NULL,
            PRIMARY KEY (round_generation_id, game_id)
          )
        `)
        this.storage.sql.exec(
          'INSERT INTO competition_schema_migrations (version, applied_at_trusted_time_ms) VALUES (10, 0)',
        )
      }

      if (currentVersion < 11) {
        this.storage.sql.exec('ALTER TABLE competition_game_provision ADD COLUMN claim_id TEXT')
        this.storage.sql.exec(`ALTER TABLE competition_game_provision
          ADD COLUMN claim_version INTEGER NOT NULL DEFAULT 0 CHECK (claim_version >= 0)`)
        this.storage.sql.exec('ALTER TABLE competition_clock_control_outbox ADD COLUMN claim_id TEXT')
        this.storage.sql.exec(`ALTER TABLE competition_clock_control_outbox
          ADD COLUMN claim_version INTEGER NOT NULL DEFAULT 0 CHECK (claim_version >= 0)`)
        this.storage.sql.exec(
          'INSERT INTO competition_schema_migrations (version, applied_at_trusted_time_ms) VALUES (11, 0)',
        )
      }

      if (currentVersion < 12) {
        this.storage.sql.exec(`
          CREATE TABLE competition_initial_seeding (
            competition_id TEXT PRIMARY KEY,
            command_id TEXT NOT NULL UNIQUE,
            stage_id TEXT NOT NULL,
            stage_json TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('pending', 'completed')),
            attempts INTEGER NOT NULL CHECK (attempts >= 0),
            next_attempt_at_ms INTEGER NOT NULL,
            last_error TEXT,
            created_trusted_time_ms INTEGER NOT NULL
          )
        `)
        this.storage.sql.exec(
          'INSERT INTO competition_schema_migrations (version, applied_at_trusted_time_ms) VALUES (12, 0)',
        )
      }

      if (currentVersion < 13) {
        this.storage.sql.exec(
          'ALTER TABLE competition_round_release_outbox ADD COLUMN completed_at_ms INTEGER',
        )
        this.storage.sql.exec(`
          CREATE TABLE competition_no_show_outbox (
            round_generation_id TEXT NOT NULL,
            match_id TEXT NOT NULL,
            game_id TEXT NOT NULL,
            deadline_ms INTEGER NOT NULL,
            command_id TEXT NOT NULL UNIQUE,
            status TEXT NOT NULL CHECK (status IN ('pending', 'completed')),
            decision_json TEXT,
            claim_id TEXT,
            claim_version INTEGER NOT NULL CHECK (claim_version >= 0),
            attempts INTEGER NOT NULL CHECK (attempts >= 0),
            next_attempt_at_ms INTEGER NOT NULL,
            PRIMARY KEY (round_generation_id, match_id)
          )
        `)
        this.storage.sql.exec(
          'INSERT INTO competition_schema_migrations (version, applied_at_trusted_time_ms) VALUES (13, 0)',
        )
      }

      if (currentVersion < 14) {
        this.storage.sql.exec(`ALTER TABLE competition_round_publication
          ADD COLUMN release_not_before_ms INTEGER NOT NULL DEFAULT 0 CHECK (release_not_before_ms >= 0)`)
        this.storage.sql.exec(
          'INSERT INTO competition_schema_migrations (version, applied_at_trusted_time_ms) VALUES (14, 0)',
        )
      }

      if (currentVersion < 15) {
        this.storage.sql.exec(`ALTER TABLE competition_matches
          ADD COLUMN tournament_result_confirmed_at_ms INTEGER
          CHECK (tournament_result_confirmed_at_ms IS NULL OR tournament_result_confirmed_at_ms >= 0)`)
        this.storage.sql.exec(`ALTER TABLE competition_no_show_outbox
          ADD COLUMN result_confirmed_at_ms INTEGER
          CHECK (result_confirmed_at_ms IS NULL OR result_confirmed_at_ms >= 0)`)
        this.storage.sql.exec(`UPDATE competition_no_show_outbox
          SET result_confirmed_at_ms = (
            SELECT MAX(audit.trusted_time_ms) FROM competition_audit audit
            WHERE audit.command_id = competition_no_show_outbox.command_id
              AND audit.operation_type = 'tournamentSubmitResult'
              AND audit.outcome_code = 'success'
          )
          WHERE result_confirmed_at_ms IS NULL`)
        this.storage.sql.exec(
          'INSERT INTO competition_schema_migrations (version, applied_at_trusted_time_ms) VALUES (15, 0)',
        )
      }

      const finalVersion = this.storage.sql.exec<{
        [key: string]: ArrayBuffer | string | number | null
        version: number
      }>(
        'SELECT COALESCE(MAX(version), 0) AS version FROM competition_schema_migrations',
      ).one().version
      if (finalVersion !== SCHEMA_VERSION) {
        throw new Error('competition room schema migration did not reach the expected version')
      }
    })
  }

  private async announceRoundInternal(
    input: CompetitionRoomAnnounceRoundRequest,
    releaseNotBeforeMs = 0,
    requiresReferee = false,
  ): Promise<CompetitionRoomAnnounceRoundResult> {
    if (!isCompetitionRoomAnnounceRoundRequest(input)) {
      return roomFailure('invalidRequest', 'round announcement request is invalid')
    }
    // 在任何 await 前保存 exact wire；後續不再讀取呼叫端的可變物件。
    const request = structuredClone(input)
    const current = this.meta()
    if (!current) return roomFailure('notInitialized', 'competition room is not initialized')
    if (current.competition_id !== request.competitionId) {
      return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
    }
    const { trustedTimeMs: _sample, ...semanticRequest } = request
    const fingerprint = await semanticFingerprint({ kind: 'announceRound', ...semanticRequest })
    const receipt = this.commandReceipt(request.commandId)
    if (receipt) {
      if (receipt.receipt_kind !== 'announceRound' || receipt.match_id !== null
        || receipt.fingerprint !== fingerprint) {
        return roomFailure('commandIdConflict', 'command id was used with another payload')
      }
      let stored: unknown
      try {
        stored = JSON.parse(receipt.response_json)
      } catch {
        return roomFailure('invalidRoundPublication', 'stored round announcement receipt is invalid')
      }
      if (!isExactCompetitionJson(stored) || !isJsonObject(stored)
        || !hasExactKeys(stored, ['ok', 'value']) || stored.ok !== true
        || !isCompetitionRoomAnnounceRoundValue(stored.value, request)) {
        return roomFailure('invalidRoundPublication', 'stored round announcement receipt is invalid')
      }
      try {
        // 由原 command 定位歷史 manifest，不以收據內的 generation 或可變 current 狀態當權威。
        const original = this.originalRoundPublication(request.commandId)
        const manifest = parseRoundManifest(original, request.competitionId, 'announcementReceipt')
        const publication = stored.value.publication
        if (manifest.announcementCommandId !== request.commandId
          || !sameJson(publication.roundRef, manifest.roundRef)
          || publication.roundStartMs !== manifest.roundStartMs
          || publication.rooms.length !== manifest.rooms.length
          || !publication.rooms.every((room, index) => {
            const expected = manifest.rooms[index]
            return expected !== undefined && room.matchId === expected.matchId && room.gameId === expected.gameId
          })) {
          throw new Error('announcement receipt differs from its immutable origin')
        }
      } catch {
        return roomFailure('invalidRoundPublication', 'stored round announcement receipt is invalid')
      }
      return roomSuccess<CompetitionRoomAnnounceRoundValue>({
        ...(stored.value as unknown as CompetitionRoomAnnounceRoundValue), idempotent: true,
      })
    }
    if (!Number.isSafeInteger(current.revision) || current.revision < 1
      || !safeNonNegativeInteger(current.updated_trusted_time_ms) || !nonEmpty(current.organizer_user_id)) {
      return roomFailure('invalidTournamentCheckpoint', 'round authority metadata is malformed')
    }
    const caller = this.acl(request.callerUserId)
    if (!caller || caller.active !== 1 || caller.role !== 'organizer'
      || request.callerUserId !== current.organizer_user_id) {
      return roomFailure('forbidden', 'only the active persisted organizer may announce a round')
    }
    if (request.expectedRevision !== current.revision) {
      return roomFailure('revisionConflict', 'expected revision does not match room revision')
    }
    if (request.trustedTimeMs < current.updated_trusted_time_ms) {
      return roomFailure('trustedTimeRegression', 'trusted server time cannot move backwards')
    }
    const cutoff = competitionRoundReadyCutoffMs(request.roundStartMs)
    if (!safeNonNegativeInteger(releaseNotBeforeMs)
      || (releaseNotBeforeMs !== 0 && releaseNotBeforeMs !== cutoff)
      || (requiresReferee && releaseNotBeforeMs === 0)
      || cutoff === null || (!requiresReferee && request.trustedTimeMs > cutoff)) {
      return roomFailure('roundNotPublishable', 'round start must allow the complete room readiness window')
    }
    if (this.currentRoundPublication(request.stageId, request.roundNumber)) {
      return roomFailure('roundAlreadyPublished', 'the current round already has an immutable publication')
    }
    const state = parseParticipantServerState(current.server_state_json)
    if (!state || registrationCheckpointRevision(state.registration, current.organizer_user_id) === null) {
      return roomFailure('invalidTournamentCheckpoint', 'round authority checkpoint is malformed')
    }
    const tournamentResponse = await this.computeRoundEngine({
      serverActor: actorFor('organizer', request.callerUserId), trustedTimeMs: request.trustedTimeMs,
      operation: { type: 'tournamentView' }, serverState: state,
    })
    if (!tournamentResponse.ok) {
      return isRetryableAutomaticRoundAdvancementError(tournamentResponse.error.code)
        ? roomFailure(tournamentResponse.error.code, 'current tournament query failed')
        : roomFailure('invalidTournamentCheckpoint', 'current tournament query is invalid')
    }
    if (!isValidRoundTournamentView(tournamentResponse, state, request.competitionId)) {
      return roomFailure('invalidTournamentCheckpoint', 'current tournament query is invalid')
    }
    let round: JsonObject
    let schedules: ScheduledMatchMaterialization[]
    try {
      const selected = selectPublishableRound(
        state.tournament, request.stageId, request.roundNumber,
        match => this.isAuthoritativeNoShowResult(match),
      )
      round = selected.round
      schedules = selected.schedules
      // 整個前輪的實際結果與 parent mirror 都必須接受，不只看任一晉級分支。
      for (const previous of selected.previousMatches) {
        if (isCoreBye(previous)) continue
        if (this.isAuthoritativeNoShowResult(previous)) continue
        const row = typeof previous.matchId === 'string' ? this.match(previous.matchId) : undefined
        if (!row || row.stage_id !== request.stageId) throw new Error('previous round match is missing')
        const checkpoint = parseJsonObject(row.match_checkpoint_json, 'previous match checkpoint')
        const submission = latestSubmissionFromCheckpoint(checkpoint)
        const scopedState: CompetitionServerState = {
          registration: null, clock: null, tournament: null, matches: { [row.match_id]: checkpoint },
        }
        const view = await this.computeRoundEngine({
          serverActor: actorFor('organizer', request.callerUserId), trustedTimeMs: request.trustedTimeMs,
          operation: { type: 'matchView', matchId: row.match_id }, serverState: scopedState,
        })
        if (!view.ok) {
          if (isRetryableAutomaticRoundAdvancementError(view.error.code)) {
            return roomFailure(view.error.code, 'previous-round match query failed')
          }
          throw new Error('previous-round match query was rejected')
        }
        if (!isValidStoredMatchView(view, scopedState, row) || !submission
          || !isJsonObject(view.public) || !isJsonObject(view.public.match)
          || !isJsonObject(checkpoint.config) || !isValidMatchFormat(checkpoint.config.format)
          || !Array.isArray(checkpoint.history) || row.revision < 1 + checkpoint.history.length
          || !Number.isSafeInteger(row.revision) || !safeNonNegativeInteger(row.last_trusted_time_ms)
          || row.last_trusted_time_ms > request.trustedTimeMs || checkpoint.config.matchId !== row.match_id
          || !sameJson(checkpoint.config.format, schedules[0]?.format)
          || !sameJson(state.matches?.[row.match_id], checkpoint)
          || !sameJson(checkpoint.config.teams, previous.participants)) {
          throw new Error('previous round match authority is invalid')
        }
        const mapped = mapCompletedMatchResult(view.public.match, checkpoint.config,
          checkpoint.config.format, checkpoint, submission)
        if (!mapped.ok || mapped.value.status !== 'completed'
          || !sameJson(mapped.value.result, previous.result)) {
          throw new Error('previous round result is not accepted')
        }
      }
    } catch {
      return roomFailure('roundNotPublishable', 'the complete previous round and current pairings must be authoritative')
    }
    const matchIds = schedules.map(schedule => schedule.matchId)
    if (request.refereeAssignments.some(assignment => !matchIds.includes(assignment.matchId))) {
      return roomFailure('invalidRefereeAssignment', 'referee assignments must belong to the complete current round')
    }
    const configuredReferees = request.refereeAssignments.flatMap(assignment => assignment.referees)
    if (configuredReferees.some(referee => {
      const entry = this.acl(referee.refereeId)
      return !entry || entry.active !== 1 || entry.role !== 'referee'
    })) return roomFailure('invalidRefereeAcl', 'every configured referee must be an active referee')
    if (schedules.some(schedule => this.match(schedule.matchId)
      || this.latestGameProvision(schedule.matchId) || state.matches?.[schedule.matchId] !== undefined)) {
      return roomFailure('roundNotPublishable', 'a scheduled match already has materialized state')
    }
    const exportResponse = await this.computeRoundEngine({
      serverActor: actorFor('organizer', request.callerUserId), trustedTimeMs: request.trustedTimeMs,
      operation: { type: 'registrationExportLockedRoster' }, serverState: state,
    })
    if (!exportResponse.ok) {
      return isRetryableAutomaticRoundAdvancementError(exportResponse.error.code)
        ? roomFailure(exportResponse.error.code, 'locked roster export query failed')
        : roomFailure('invalidLockedRoster', 'locked roster export is invalid')
    }
    if (!isExactCompetitionJson(exportResponse)
      || !isJsonObject(exportResponse) || !hasExactKeys(exportResponse, ['ok', 'operation', 'public', 'private', 'serverState'])
      || exportResponse.operation !== 'registrationExportLockedRoster'
      || !sameJson(exportResponse.serverState, state) || !isJsonObject(exportResponse.public)
      || !hasExactKeys(exportResponse.public, ['exported']) || exportResponse.public.exported !== true
      || !parseLockedRoster(exportResponse.private, current.organizer_user_id)) {
      return roomFailure('invalidLockedRoster', 'locked roster export is invalid')
    }
    const roundRef: CompetitionRoundRef = {
      competitionId: request.competitionId, stageId: request.stageId,
      roundNumber: request.roundNumber, roundGenerationId: crypto.randomUUID(),
    }
    const prepared: PreparedRoundRoom[] = []
    for (const schedule of schedules) {
      const referees = request.refereeAssignments.find(entry => entry.matchId === schedule.matchId)?.referees ?? []
      if (referees.some(referee => referee.conflictedTeam !== null
        && !schedule.teams.includes(referee.conflictedTeam))) {
        return roomFailure('invalidRefereeAssignment', 'configured conflict must belong to its scheduled match')
      }
      const result = await this.prepareRoundRoom(current, request, roundRef, schedule, referees, exportResponse.private)
      if (!result.ok) return result
      prepared.push(result.value)
    }
    const manifest: RoundManifest = {
      announcementCommandId: request.commandId,
      roundRef, roundStartMs: request.roundStartMs, tournamentRound: round,
      rooms: prepared.map(room => ({ matchId: room.matchId, gameId: room.bindingRef.gameId, bindingRef: room.bindingRef })),
    }
    const publication: CompetitionRoomRoundPublication = {
      roundRef, roundStartMs: request.roundStartMs, publicationVersion: 1, status: 'materializing',
      releaseDecisionId: null, requiresReferee,
      rooms: manifest.rooms.map(room => ({ matchId: room.matchId, gameId: room.gameId, ackConfirmedAtMs: null })),
    }
    const result = roomSuccess<CompetitionRoomAnnounceRoundValue>({
      competitionId: request.competitionId, revision: current.revision + 1,
      commandId: request.commandId, idempotent: false, publication,
    })
    if (!isCompetitionRoomAnnounceRoundValue(result.value, request)) {
      return roomFailure('invalidRoundPublication', 'round publication is invalid')
    }
    // 全部 engine 計算、digest、隨機值成功後才存完整集合；交易內禁止 foreign I/O。
    this.storage.transactionSync(() => {
      const latest = this.meta()
      const active = this.acl(request.callerUserId)
      if (!latest || latest.competition_id !== current.competition_id
        || latest.organizer_user_id !== current.organizer_user_id || latest.revision !== current.revision
        || latest.server_state_json !== current.server_state_json
        || latest.public_projection_json !== current.public_projection_json
        || latest.updated_trusted_time_ms !== current.updated_trusted_time_ms
        || !active || active.active !== 1 || active.role !== 'organizer'
        || this.currentRoundPublication(request.stageId, request.roundNumber)
        || configuredReferees.some(referee => {
          const entry = this.acl(referee.refereeId)
          return !entry || entry.active !== 1 || entry.role !== 'referee'
        }) || prepared.some(room => this.match(room.matchId) || this.latestGameProvision(room.matchId))) {
        throw new Error('round publication authority changed before commit')
      }
      const updated = parseServerState(latest.server_state_json)
      updated.matches = { ...(updated.matches ?? {}) }
      this.storage.sql.exec(
        `INSERT INTO competition_round_publication
          (round_generation_id, stage_id, round_number, round_start_ms, release_not_before_ms, publication_version,
           status, release_decision_id, requires_referee, manifest_json, announced_trusted_time_ms)
         VALUES (?, ?, ?, ?, ?, 1, 'materializing', NULL, ?, ?, ?)`,
        roundRef.roundGenerationId, request.stageId, request.roundNumber, request.roundStartMs, releaseNotBeforeMs,
        requiresReferee ? 1 : 0, JSON.stringify(manifest), request.trustedTimeMs,
      )
      for (const room of prepared) {
        updated.matches[room.matchId] = room.checkpoint
        this.storage.sql.exec(
          `INSERT INTO competition_matches
            (match_id, stage_id, match_checkpoint_json, clock_checkpoint_json, match_projection_json,
             clock_projection_json, revision, last_trusted_time_ms)
           VALUES (?, ?, ?, ?, ?, ?, 2, ?)`,
          room.matchId, request.stageId, JSON.stringify(room.checkpoint), JSON.stringify(room.clock),
          JSON.stringify(room.matchProjection), JSON.stringify(room.clockProjection), request.trustedTimeMs,
        )
        this.storage.sql.exec(
          `INSERT INTO competition_round_room_ledger
            (round_generation_id, match_id, game_number, game_id, binding_json, binding_ref_json,
             next_game_json, match_revision, status, ack_confirmed_at_ms, claim_id, claim_version,
             attempts, next_attempt_at_ms)
           VALUES (?, ?, 1, ?, ?, ?, ?, 2, 'pending', NULL, NULL, 0, 0, ?)`,
          roundRef.roundGenerationId, room.matchId, room.bindingRef.gameId, JSON.stringify(room.binding),
          JSON.stringify(room.bindingRef), JSON.stringify(room.nextGame), request.trustedTimeMs,
        )
        this.storage.sql.exec(
          `INSERT INTO competition_no_show_outbox
            (round_generation_id, match_id, game_id, deadline_ms, command_id, status, decision_json,
             claim_id, claim_version, attempts, next_attempt_at_ms)
           VALUES (?, ?, ?, ?, ?, 'pending', NULL, NULL, 0, 0, ?)`,
          roundRef.roundGenerationId, room.matchId, room.bindingRef.gameId, request.roundStartMs,
          competitionGameNoShowCommandId(room.bindingRef), request.roundStartMs,
        )
      }
      this.storage.sql.exec(
        `UPDATE competition_meta SET revision = ?, server_state_json = ?, updated_trusted_time_ms = ?
         WHERE singleton = 1`, result.value.revision, JSON.stringify(updated), request.trustedTimeMs,
      )
      this.insertAudit({ actorUserId: request.callerUserId, role: 'organizer', operationType: 'announceRound',
        commandId: request.commandId, revision: result.value.revision, outcomeCode: 'success',
        trustedTimeMs: request.trustedTimeMs })
      this.insertCommandReceipt(request.commandId, 'announceRound', fingerprint, result)
    })
    // 派送在公開 wrapper／alarm 的本地 critical lane 外，不借舊 InProgress outbox。
    return result
  }

  private async computeRoundEngine(request: CompetitionRequest): Promise<CompetitionEngineResponse> {
    try {
      // 純計算的輸入／輸出都取本地快照，engine 不得改動持久 CAS 前提或後續 digest 來源。
      const response = await this.engine(structuredClone(request))
      if (!isExactCompetitionJson(response) || !isJsonObject(response) || typeof response.ok !== 'boolean') {
        throw new Error('round engine returned invalid JSON envelope')
      }
      return structuredClone(response)
    } catch {
      return { ok: false, error: { code: 'engineProtocolError', layer: 'room', message: 'round engine computation failed' } }
    }
  }

  private async prepareRoundRoom(
    current: CompetitionMetaRow,
    request: CompetitionRoomAnnounceRoundRequest,
    roundRef: CompetitionRoundRef,
    schedule: ScheduledMatchMaterialization,
    referees: { refereeId: string; conflictedTeam: string | null }[],
    lockedRoster: JsonValue | undefined,
  ): Promise<RoomSuccess<PreparedRoundRoom> | RoomFailure> {
    const matchOperation: CompetitionRoomMatchOperation = {
      type: 'matchCreate', config: { matchId: schedule.matchId, teams: schedule.teams, format: schedule.format,
        firstGame: { type: 'pending', reason: '等待可信隨機先手' }, referees },
    }
    const created = await this.computeRoundEngine({
      serverActor: actorFor('organizer', request.callerUserId), trustedTimeMs: request.trustedTimeMs,
      operation: matchOperation, serverState: emptyServerState(),
    })
    if (!created.ok) {
      return isRetryableAutomaticRoundAdvancementError(created.error.code)
        ? roomFailure(created.error.code, 'round match creation failed temporarily')
        : roomFailure('engineInvalidMatchCheckpoint', 'round match creation returned an invalid checkpoint')
    }
    if (!isExactCompetitionJson(created) || !isJsonObject(created)
      || !hasExactKeys(created, ['ok', 'operation', 'public', 'serverState']) || created.operation !== 'matchCreate'
      || validateScopedEngineState(created.serverState, schedule.matchId, null, null, matchOperation)) {
      return roomFailure('engineInvalidMatchCheckpoint', 'round match creation returned an invalid checkpoint')
    }
    const checkpoint = created.serverState.matches?.[schedule.matchId]
    if (!isJsonObject(checkpoint)) return roomFailure('engineInvalidMatchCheckpoint', 'round match checkpoint is missing')
    const clockOperation: CompetitionRoomMatchOperation = {
      type: 'clockCreate', matchId: schedule.matchId, teams: schedule.teams, config: schedule.clockConfig,
    }
    const clockCreated = await this.computeRoundEngine({
      serverActor: actorFor('organizer', request.callerUserId), trustedTimeMs: request.trustedTimeMs,
      operation: engineOperation(clockOperation),
      serverState: { registration: null, clock: null, tournament: null, matches: { [schedule.matchId]: checkpoint } },
    })
    if (!clockCreated.ok) {
      return isRetryableAutomaticRoundAdvancementError(clockCreated.error.code)
        ? roomFailure(clockCreated.error.code, 'round clock creation failed temporarily')
        : roomFailure('engineInvalidClockCheckpoint', 'round clock creation returned an invalid checkpoint')
    }
    if (!isExactCompetitionJson(clockCreated) || !isJsonObject(clockCreated)
      || !hasExactKeys(clockCreated, ['ok', 'operation', 'public', 'serverState']) || clockCreated.operation !== 'clockCreate'
      || validateScopedEngineState(clockCreated.serverState, schedule.matchId, checkpoint, null, clockOperation)) {
      return roomFailure('engineInvalidClockCheckpoint', 'round clock creation returned an invalid checkpoint')
    }
    const clock = clockCreated.serverState.clock
    if (!isJsonObject(clock)) return roomFailure('engineInvalidClockCheckpoint', 'round clock checkpoint is missing')
    const createdProjection = stripPrivate(publicProjection(created, 'organizer'))
    const clockProjection = stripPrivate(publicProjection(clockCreated, 'organizer'))
    const state = parseServerState(current.server_state_json)
    const pending = deriveInitialScheduledGameState({ ...current,
      server_state_json: JSON.stringify({ ...state, matches: { ...(state.matches ?? {}), [schedule.matchId]: checkpoint } }),
    }, {
      match_id: schedule.matchId, stage_id: request.stageId, match_checkpoint_json: JSON.stringify(checkpoint),
      clock_checkpoint_json: JSON.stringify(clock), match_projection_json: JSON.stringify(createdProjection),
      clock_projection_json: JSON.stringify(clockProjection), revision: 1, last_trusted_time_ms: request.trustedTimeMs,
      tournament_result_confirmed_at_ms: null,
    }, {
      competitionId: request.competitionId, matchId: schedule.matchId, callerUserId: request.callerUserId,
      commandId: request.commandId, expectedMatchRevision: 1, trustedTimeMs: request.trustedTimeMs,
    }, 'pending')
    let randomIndex: number
    try {
      randomIndex = this.randomBit()
    } catch {
      return roomFailure('randomSourceFailure', 'trusted random source failed')
    }
    if (randomIndex !== 0 && randomIndex !== 1) {
      return roomFailure('randomSourceFailure', 'trusted random source returned an invalid bit')
    }
    const firstTeam = pending.teams[randomIndex]
    const assigned = await this.computeRoundEngine({
      serverActor: actorFor('organizer', request.callerUserId), trustedTimeMs: request.trustedTimeMs,
      operation: { type: 'matchAssignFirstPlayer', matchId: schedule.matchId, team: firstTeam, reason: INITIAL_FIRST_PLAYER_REASON },
      serverState: { registration: null, clock, tournament: null, matches: { [schedule.matchId]: checkpoint } },
    })
    if (!assigned.ok && isRetryableAutomaticRoundAdvancementError(assigned.error.code)) {
      return roomFailure(assigned.error.code, 'round first-player assignment failed temporarily')
    }
    const verified = isExactCompetitionJson(assigned)
      ? validateInitialFirstPlayerResponse(assigned, schedule.matchId, pending, firstTeam, INITIAL_FIRST_PLAYER_REASON)
      : null
    if (!verified || !isJsonObject(verified.projection.public)
      || !isJsonObject(verified.projection.public.match) || !isJsonObject(verified.projection.public.match.nextGame)) {
      return roomFailure('engineInvalidMatchCheckpoint', 'round first-player assignment returned an invalid checkpoint')
    }
    const roster = deriveLockedRosterBinding(lockedRoster, pending.teams, current.organizer_user_id, firstTeam, null, null)
    if (!roster.ok) return roomFailure(roster.code, roster.message)
    const gameId = `competition-game-${await semanticFingerprint({
      kind: 'round-initial-game', roundRef, matchId: schedule.matchId, gameNumber: 1,
    })}`
    const binding: CompetitionGameBinding = {
      roundRef, roundStartMs: request.roundStartMs, competitionId: request.competitionId, matchId: schedule.matchId,
      gameNumber: 1, organizerUserId: current.organizer_user_id, expectedMatchRevision: 2,
      startKind: { type: 'ordinaryNewGame' }, ruleVersion: roster.value.ruleVersion, ruleset: roster.value.ruleset,
      enabledRuleModules: roster.value.enabledRuleModules,
      initialHp: pending.teams.map(team => ({ team, hp: roster.value.initialHp })),
      players: roster.value.players, deckSnapshot: roster.value.deckSnapshot, firstPlayer: roster.value.firstPlayer,
      turnOrder: roster.value.turnOrder, deckSeed: crypto.randomUUID(),
    }
    validateCompetitionGameBinding(binding)
    const bindingRef = await competitionGameBindingRefForRequest({ gameId, binding })
    return roomSuccess({ matchId: schedule.matchId, checkpoint: verified.checkpoint, clock,
      matchProjection: verified.projection, clockProjection, nextGame: verified.projection.public.match.nextGame,
      binding, bindingRef })
  }

  private async readRoundPublicationInternal(
    input: CompetitionRoomRoundPublicationReadRequest,
  ): Promise<CompetitionRoomRoundPublicationReadResult> {
    if (!isCompetitionRoomRoundPublicationReadRequest(input)) {
      return roomFailure('invalidRequest', 'round publication query is invalid')
    }
    const request = structuredClone(input)
    const current = this.meta()
    if (!current) return roomFailure('notInitialized', 'competition room is not initialized')
    if (current.competition_id !== request.competitionId) {
      return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
    }
    if (!Number.isSafeInteger(current.revision) || current.revision < 1
      || !safeNonNegativeInteger(current.updated_trusted_time_ms) || !nonEmpty(current.organizer_user_id)) {
      return roomFailure('invalidRoundPublication', 'round authority metadata is malformed')
    }
    const caller = this.acl(request.callerUserId)
    if (!caller || caller.active !== 1 || caller.role !== 'organizer'
      || caller.user_id !== current.organizer_user_id) {
      return roomFailure('forbidden', 'only the active persisted organizer may read round publication')
    }
    const header = this.currentRoundPublication(request.stageId, request.roundNumber)
    if (!header) return roomSuccess({ competitionId: request.competitionId, revision: current.revision, publication: null })
    try {
      const manifest = parseRoundManifest(header, request.competitionId)
      const tournament = await this.readTournamentInternal({
        competitionId: request.competitionId, callerUserId: request.callerUserId,
      })
      if (!tournament.ok || !samePublishedRoundPairs(tournament.value.projection.public, manifest)) {
        throw new Error('publication differs from current authoritative pairings')
      }
      const rows = this.roundRoomLedger(header.round_generation_id)
      if (rows.length !== manifest.rooms.length) throw new Error('round room ledger is incomplete')
      const rooms: CompetitionRoomRoundPublication['rooms'] = []
      for (const expected of manifest.rooms) {
        const row = rows.find(entry => entry.game_id === expected.gameId)
        if (!row || row.match_id !== expected.matchId || row.game_number !== 1 || row.match_revision !== 2
          || !['pending', 'confirmed', 'blocked', 'revoked', 'started'].includes(row.status)
          || !safeNonNegativeInteger(row.claim_version) || !safeNonNegativeInteger(row.attempts)
          || !safeNonNegativeInteger(row.next_attempt_at_ms)
          || (row.claim_id !== null && !nonEmpty(row.claim_id))
          || (row.status === 'pending' && row.ack_confirmed_at_ms !== null)
          || (row.status === 'confirmed' && row.ack_confirmed_at_ms === null)) {
          throw new Error('round room ledger identity is invalid')
        }
        const binding = parseStoredCompetitionGameBinding(row.binding_json)
        const ref: unknown = JSON.parse(row.binding_ref_json)
        const nextGame: unknown = JSON.parse(row.next_game_json)
        const pair = manifest.tournamentRound.matches
        const scheduled = Array.isArray(pair)
          ? pair.find(entry => isJsonObject(entry) && entry.matchId === row.match_id) : undefined
        const firstPlayer = binding.players.find(player => player.player === binding.firstPlayer)
        if (!isCompetitionGameBindingRef(ref) || !sameJson(ref, expected.bindingRef)
          || binding.organizerUserId !== current.organizer_user_id
          || binding.competitionId !== request.competitionId || binding.matchId !== expected.matchId
          || binding.gameNumber !== 1 || binding.expectedMatchRevision !== 2
          || !sameJson(binding.roundRef, manifest.roundRef) || binding.roundStartMs !== manifest.roundStartMs
          || binding.startKind.type !== 'ordinaryNewGame'
          || !isJsonObject(scheduled) || !isTeamPair(scheduled.participants) || !firstPlayer
          || !sameTeamSet([...new Set(binding.players.map(player => player.team))], scheduled.participants)
          || !sameJson(nextGame, { gameNumber: 1, kind: binding.startKind,
            firstPlayer: { type: 'assigned', team: firstPlayer.team } })
          || !sameJson(await competitionGameBindingRefForRequest({ gameId: row.game_id, binding }), expected.bindingRef)) {
          throw new Error('round binding differs from the immutable manifest')
        }
        rooms.push({ matchId: row.match_id, gameId: row.game_id, ackConfirmedAtMs: row.ack_confirmed_at_ms })
      }
      const publication: CompetitionRoomRoundPublication = {
        roundRef: manifest.roundRef, roundStartMs: header.round_start_ms, publicationVersion: header.publication_version,
        status: header.status, releaseDecisionId: header.release_decision_id,
        requiresReferee: header.requires_referee === 1 || (header.release_not_before_ms > 0
          && Date.now() >= header.release_not_before_ms
          && !this.roundHasCompleteTimelyReleaseAcks(header, request.competitionId)),
        rooms,
      }
      if (!isCompetitionRoomRoundPublication(publication)) throw new Error('public round publication is malformed')
      return roomSuccess({ competitionId: request.competitionId, revision: current.revision, publication })
    } catch {
      return roomFailure('invalidRoundPublication', 'round publication checkpoint is malformed')
    }
  }

  private async ingestInitialGameStartedInternal(
    input: CompetitionRoomInitialGameStartedRequest,
  ): Promise<CompetitionRoomInitialGameStartedResult> {
    if (!isCompetitionRoomInitialGameStartedRequest(input)) {
      return roomFailure('invalidRequest', 'initial GameStarted request is invalid')
    }
    const request = structuredClone(input)
    const fingerprint = await initialGameStartedFingerprint(request)

    // 成功收據先於 revision／目前狀態；只允許同一 exact command 恢復原 ACK。
    const receipt = this.commandReceipt(request.commandId)
    if (receipt) {
      if (receipt.receipt_kind !== 'ingestInitialGameStarted'
        || receipt.match_id !== request.matchId || receipt.fingerprint !== fingerprint) {
        return roomFailure('commandIdConflict', 'command id was used with another payload')
      }
      return parseStoredInitialGameStartedReceipt(receipt.response_json, request)
    }
    const priorStart = this.storage.sql.exec<CommandReceiptRow>(
      `SELECT receipt_kind, match_id, fingerprint, response_json
       FROM competition_command_receipt
       WHERE match_id = ? AND receipt_kind = 'ingestInitialGameStarted' LIMIT 1`,
      request.matchId,
    ).toArray()[0]
    if (priorStart) {
      return roomFailure('initialGameStartedConflict', 'this scheduled match already has a first-start receipt')
    }

    const current = this.meta()
    if (!current) return roomFailure('notInitialized', 'competition room is not initialized')
    if (current.competition_id !== request.competitionId) {
      return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
    }
    if (!safeNonNegativeInteger(current.revision) || current.revision < 1
      || !safeNonNegativeInteger(current.updated_trusted_time_ms)
      || !nonEmpty(current.organizer_user_id)) {
      return roomFailure('invalidTournamentCheckpoint', 'initial-start parent authority is malformed')
    }
    const caller = this.acl(request.callerUserId)
    if (!caller || caller.active !== 1 || caller.role !== 'organizer'
      || request.callerUserId !== current.organizer_user_id) {
      return roomFailure('forbidden', 'only the active persisted organizer may ingest initial GameStarted')
    }

    const evidence = request.evidence
    const { roundRef } = evidence.bindingRef
    const headerRows = this.storage.sql.exec<RoundPublicationRow>(
      'SELECT * FROM competition_round_publication WHERE round_generation_id = ?',
      roundRef.roundGenerationId,
    ).toArray()
    if (headerRows.length !== 1) {
      return roomFailure('invalidRoundPublication', 'initial-start round publication is missing or ambiguous')
    }
    const header = headerRows[0]
    if (!header) return roomFailure('invalidRoundPublication', 'initial-start round publication is missing')
    if (header.status === 'retired') {
      const retiredTarget = this.roundRoomLedger(roundRef.roundGenerationId).find(row => (
        row.match_id === request.matchId && row.game_id === evidence.bindingRef.gameId
      ))
      return retiredTarget?.status === 'revoked'
        ? roomFailure('retiredStartContradictsRevocation', 'retired round has a durable revoked proof for this room')
        : roomFailure('roundRetired', 'retired round cannot accept a new initial start')
    }
    if (header.status !== 'released' && header.status !== 'quiescing') {
      return roomFailure('roundNotReleased', 'initial GameStarted has no durable released-round authority')
    }
    if (header.stage_id !== roundRef.stageId || header.round_number !== roundRef.roundNumber
      || !sameJson(this.currentRoundPublication(header.stage_id, header.round_number), header)) {
      return roomFailure('invalidRoundPublication', 'initial-start round is no longer the current publication')
    }

    let manifest: RoundManifest
    try {
      manifest = parseRoundManifest(header, request.competitionId)
    } catch {
      return roomFailure('invalidRoundPublication', 'initial-start round manifest is invalid')
    }
    const expectedRoom = manifest.rooms.find(room => room.matchId === request.matchId
      && room.gameId === evidence.bindingRef.gameId)
    const cutoff = competitionRoundReadyCutoffMs(header.round_start_ms)
    if (!expectedRoom || cutoff === null || header.release_decision_id === null
      || !nonEmpty(header.release_decision_id)
      || evidence.releaseDecisionId !== header.release_decision_id
      || !sameJson(expectedRoom.bindingRef, evidence.bindingRef)) {
      return roomFailure('invalidRoundPublication', 'initial-start evidence differs from the persisted release decision')
    }
    const releaseRequest: CompetitionGameAdmissionRequest = {
      bindingRef: expectedRoom.bindingRef,
      commandId: header.release_decision_id,
    }
    if (!isCompetitionGameInitialStartEvidence(evidence, releaseRequest)) {
      return roomFailure('invalidRequest', 'initial GameStarted evidence is not bound to the persisted release')
    }

    const publicationResult = await this.readRoundPublicationInternal({
      competitionId: request.competitionId,
      stageId: header.stage_id,
      roundNumber: header.round_number,
      callerUserId: request.callerUserId,
    })
    const publication = publicationResult.ok ? publicationResult.value.publication : null
    const publicationCompetitionId = publicationResult.ok ? publicationResult.value.competitionId : null
    if (!publication || publicationCompetitionId !== request.competitionId
      || publication.status !== header.status
      || publication.releaseDecisionId !== header.release_decision_id
      || !sameJson(publication.roundRef, manifest.roundRef)
      || publication.roundStartMs !== manifest.roundStartMs
      || publication.rooms.length !== manifest.rooms.length
      || !manifest.rooms.every(expected => {
        const proven = publication.rooms.find(room => room.matchId === expected.matchId
          && room.gameId === expected.gameId)
        return proven !== undefined && safeNonNegativeInteger(proven.ackConfirmedAtMs)
          && proven.ackConfirmedAtMs <= cutoff
      })) {
      return roomFailure('invalidRoundPublication', 'timely whole-round readiness proof is incomplete')
    }

    const releaseSnapshot = this.roundReleaseRows(roundRef.roundGenerationId)
    if (releaseSnapshot.length !== manifest.rooms.length
      || !manifest.rooms.every(expected => releaseSnapshot.filter(row => row.game_id === expected.gameId).length === 1
        && releaseSnapshot.some(row => row.game_id === expected.gameId
          && row.release_decision_id === header.release_decision_id && validRoundReleaseRow(row)))) {
      return roomFailure('invalidRoundPublication', 'persisted whole-round release decision is incomplete')
    }

    const ledgerSnapshot = this.roundRoomLedger(roundRef.roundGenerationId)
    const matchingLedgerRows = ledgerSnapshot.filter(row => row.match_id === request.matchId
      && row.game_id === evidence.bindingRef.gameId && row.game_number === 1)
    const ledger = matchingLedgerRows[0]
    if (ledgerSnapshot.length !== manifest.rooms.length || matchingLedgerRows.length !== 1
      || !ledger || ledger.status !== 'confirmed'
      || !safeNonNegativeInteger(ledger.ack_confirmed_at_ms)
      || ledger.ack_confirmed_at_ms > cutoff
      || ledger.match_revision !== request.expectedMatchRevision) {
      return roomFailure('initialGameStartedConflict', 'target room is not the exact confirmed first-game linkage')
    }

    const match = this.match(request.matchId)
    if (!match) return roomFailure('matchNotFound', 'scheduled match row does not exist')
    if (match.revision !== request.expectedMatchRevision) {
      return roomFailure('matchRevisionConflict', 'expected revision does not match the pending match')
    }
    if (evidence.startedAtMs < match.last_trusted_time_ms) {
      return roomFailure('trustedTimeRegression', 'child clock sample precedes the match clock cursor')
    }

    let state: CompetitionServerState
    let binding: CompetitionGameBinding
    let bindingRef: unknown
    let nextGame: unknown
    let intentValue: ProvisionIntent | null = null
    let schedule: ScheduledMatchMaterialization
    let checkpoint: JsonObject
    let clock: JsonObject
    let matchProjection: CompetitionRoomMatchProjection | null
    let priorClockProjection: CompetitionRoomMatchProjection | null
    try {
      state = parseServerState(current.server_state_json)
      if (!samePublishedRoundPairs(state.tournament, manifest)) {
        throw new Error('current tournament pairings differ from the immutable round manifest')
      }
      schedule = deriveScheduledMatch(state.tournament, header.stage_id, request.matchId,
        false, false, 'pending')
      const intent = deriveProvisionIntent(match, state, null, null, 'pending')
      if (!intent.ok || intent.value.gameNumber !== 1 || match.stage_id !== header.stage_id
        || !sameJson(intent.value.teams, schedule.teams)
        || !sameJson(intent.value.nextGame, JSON.parse(ledger.next_game_json))) {
        throw new Error('target match is not the persisted pending first game')
      }
      intentValue = intent.value
      checkpoint = parseJsonObject(match.match_checkpoint_json, 'initial match checkpoint')
      clock = parseJsonObject(match.clock_checkpoint_json ?? '', 'initial clock checkpoint')
      matchProjection = parseOptionalProjection(match.match_projection_json)
      priorClockProjection = parseOptionalProjection(match.clock_projection_json)
      binding = parseStoredCompetitionGameBinding(ledger.binding_json)
      bindingRef = JSON.parse(ledger.binding_ref_json)
      nextGame = JSON.parse(ledger.next_game_json)
    } catch {
      return roomFailure('invalidRoundPublication', 'target pending match, binding, or linkage is malformed')
    }
    if (!intentValue) {
      return roomFailure('invalidRoundPublication', 'target pending match intent is unavailable')
    }
    if (match.revision !== ledger.match_revision
      || !isCompetitionGameBindingRef(bindingRef)
      || !sameJson(bindingRef, expectedRoom.bindingRef)
      || binding.competitionId !== request.competitionId
      || binding.organizerUserId !== current.organizer_user_id
      || binding.matchId !== request.matchId || binding.gameNumber !== 1
      || binding.expectedMatchRevision !== request.expectedMatchRevision
      || !sameJson(binding.roundRef, manifest.roundRef)
      || binding.roundStartMs !== manifest.roundStartMs
      || !sameJson(binding.startKind, { type: 'ordinaryNewGame' })
      || !binding.players.some(player => player.player === binding.firstPlayer)
      || (evidence.initialWindow.type === 'action'
        && binding.players.find(player => player.player === binding.firstPlayer)?.team !== evidence.initialWindow.team)
      || !sameJson(nextGame, intentValue.nextGame)
      || !sameTeamSet([...new Set(binding.players.map(player => player.team))], schedule.teams)
      || !sameJson(await competitionGameBindingRefForRequest({ gameId: ledger.game_id, binding }), expectedRoom.bindingRef)
      || !matchProjection || !priorClockProjection || priorClockProjection.operation !== 'clockCreate'
      || !isValidClockCheckpoint(clock, schedule.teams) || !sameJson(clock.config, schedule.clockConfig)
      || clock.preparationStarted !== false || clock.running !== null || clock.paused !== null
      || clock.expiredTeam !== null || clock.doubleExpired !== false || clock.needsDecision !== null
      || validateClockPublicView(priorClockProjection.public, clock, schedule.teams) !== null
      || storedClockProjectionDeadline(priorClockProjection) !== null
      || !safeNonNegativeInteger(match.last_trusted_time_ms)) {
      return roomFailure('invalidRoundPublication', 'target first-game binding or idle clock is inconsistent')
    }

    const parentSample = Math.max(Date.now(), current.updated_trusted_time_ms)
    if (!safeNonNegativeInteger(parentSample)
      || !safeNonNegativeInteger(current.revision + 1)
      || !safeNonNegativeInteger(match.revision + 1)) {
      return roomFailure('trustedTimeRegression', 'parent start sample or revision is invalid')
    }
    const actor = actorFor('organizer', request.callerUserId)
    const startOperation: CompetitionOperation = {
      type: 'tournamentStartMatch', stageId: header.stage_id, matchId: request.matchId,
    }
    const startResponse = await this.computeRoundEngine({
      serverActor: actor,
      trustedTimeMs: parentSample,
      operation: startOperation,
      serverState: state,
    })
    if (!startResponse.ok) {
      return roomFailure(`engine:${startResponse.error.code}`, startResponse.error.message)
    }
    const startedTournament = validateStartedTournamentResponse(
      startResponse, state, header.stage_id, request.matchId,
    )
    if (!startedTournament) {
      return roomFailure('engineInvalidTournamentCheckpoint', 'tournamentStartMatch returned an invalid checkpoint')
    }
    let startedSchedule: ScheduledMatchMaterialization
    try {
      startedSchedule = deriveScheduledMatch(startedTournament, header.stage_id, request.matchId)
    } catch {
      return roomFailure('engineInvalidTournamentCheckpoint', 'started tournament schedule is invalid')
    }
    if (!sameJson(startedSchedule.teams, schedule.teams)
      || !sameJson(startedSchedule.format, schedule.format)
      || !sameJson(startedSchedule.clockConfig, schedule.clockConfig)) {
      return roomFailure('engineInvalidTournamentCheckpoint', 'tournamentStartMatch changed the published pairing')
    }

    const clockOperation: CompetitionRoomMatchOperation = evidence.initialWindow.type === 'preparation'
      ? { type: 'clockBeginPreparation', matchId: request.matchId }
      : { type: 'clockBeginAction', matchId: request.matchId,
          team: evidence.initialWindow.team, turn: evidence.initialWindow.turn }
    const clockResponse = await this.computeRoundEngine({
      serverActor: actor,
      trustedTimeMs: evidence.startedAtMs,
      operation: engineOperation(clockOperation),
      serverState: {
        registration: null, clock, tournament: null, matches: { [request.matchId]: checkpoint },
      },
    })
    if (!clockResponse.ok) {
      return roomFailure(`engine:${clockResponse.error.code}`, clockResponse.error.message)
    }
    if (clockResponse.operation !== clockOperation.type) {
      return roomFailure('engineOperationMismatch', 'competition engine returned another initial clock operation')
    }
    const clockStateError = validateScopedEngineState(
      clockResponse.serverState, request.matchId, checkpoint, clock, clockOperation,
    )
    if (clockStateError) return roomFailure(clockStateError.code, clockStateError.message)
    const nextClock = clockResponse.serverState.clock
    if (!isJsonObject(nextClock) || !isValidClockCheckpoint(nextClock, schedule.teams)
      || !sameJson(nextClock.config, schedule.clockConfig)
      || nextClock.lastObservedMs !== evidence.startedAtMs) {
      return roomFailure('engineInvalidClockCheckpoint', 'initial clock operation returned an invalid checkpoint')
    }
    const parsedClock = parseClockMutationResult(clockResponse.public, nextClock, schedule.teams)
    if (!parsedClock.ok) return roomFailure(parsedClock.error.code, parsedClock.error.message)
    const timeout = await deriveClockTimeoutEvidence(
      parsedClock.value, clock, nextClock, request.competitionId, request.matchId,
      request.commandId, match.revision + 1, schedule.teams,
    )
    if (!timeout.ok) return roomFailure(timeout.error.code, timeout.error.message)
    const nextMatch = clockResponse.serverState.matches?.[request.matchId]
    if (!isJsonObject(nextMatch) || !sameJson(nextMatch, checkpoint)) {
      return roomFailure('engineInvalidMatchCheckpoint', 'initial clock operation changed the match checkpoint')
    }

    const tournamentProjection = stripPrivate(publicProjection(startResponse, 'organizer'))
    const clockProjection = stripPrivate(publicProjection(clockResponse, 'organizer'))
    const matchRevision = match.revision + 1
    const competitionRevision = current.revision + 1
    const result = roomSuccess<CompetitionRoomMatchExecuteValue>({
      competitionId: request.competitionId,
      matchId: request.matchId,
      matchRevision,
      commandId: request.commandId,
      idempotent: false,
      timeoutEvidence: timeout.value,
      nextClockDeadlineMs: parsedClock.value.nextTimeoutDeadlineMs,
      matchProjection,
      clockProjection,
    })

    // evidence 與九欄 ACK 共存於現有 receipt payload；不增 DTO、表格或 canonical event。
    const storedReceipt = { ack: result, evidence: structuredClone(evidence) }
    this.storage.transactionSync(() => {
      const latest = this.meta()
      const latestMatch = this.match(request.matchId)
      const latestHeader = this.storage.sql.exec<RoundPublicationRow>(
        'SELECT * FROM competition_round_publication WHERE round_generation_id = ?',
        roundRef.roundGenerationId,
      ).toArray()[0]
      const activeHeader = this.currentRoundPublication(header.stage_id, header.round_number)
      const latestLedger = this.roundRoomLedger(roundRef.roundGenerationId)
      const latestCaller = this.acl(request.callerUserId)
      const latestReceipt = this.commandReceipt(request.commandId)
      const latestMatchReceipt = this.storage.sql.exec<CommandReceiptRow>(
        `SELECT receipt_kind, match_id, fingerprint, response_json
         FROM competition_command_receipt
         WHERE match_id = ? AND receipt_kind = 'ingestInitialGameStarted' LIMIT 1`,
        request.matchId,
      ).toArray()[0]
      if (!latest || !sameJson(latest, current) || !sameJson(latestMatch, match)
        || !sameJson(latestHeader, header) || !sameJson(activeHeader, header)
        || !sameJson(this.roundReleaseRows(roundRef.roundGenerationId), releaseSnapshot)
        || !sameJson(latestLedger, ledgerSnapshot)
        || !latestCaller || !sameJson(latestCaller, caller)
        || latestReceipt !== undefined || latestMatchReceipt !== undefined) {
        throw new Error('initial GameStarted late CAS failed')
      }
      const updated = parseServerState(latest.server_state_json)
      updated.tournament = startedTournament
      updated.matches = { ...(updated.matches ?? {}), [request.matchId]: nextMatch }
      this.storage.sql.exec(
        `UPDATE competition_matches
         SET match_checkpoint_json = ?, clock_checkpoint_json = ?,
             match_projection_json = ?, clock_projection_json = ?,
             revision = ?, last_trusted_time_ms = ?
         WHERE match_id = ?`,
        JSON.stringify(nextMatch), JSON.stringify(nextClock), JSON.stringify(matchProjection),
        JSON.stringify(clockProjection), matchRevision, evidence.startedAtMs, request.matchId,
      )
      this.storage.sql.exec(
        `UPDATE competition_meta
         SET server_state_json = ?, public_projection_json = ?, revision = ?, updated_trusted_time_ms = ?
         WHERE singleton = 1`,
        JSON.stringify(updated), JSON.stringify(tournamentProjection), competitionRevision, parentSample,
      )
      if (header.status === 'quiescing' && header.requires_referee === 0) {
        if (!safeNonNegativeInteger(header.publication_version + 1)) {
          throw new Error('quiescing publication version overflow')
        }
        this.storage.sql.exec(
          `UPDATE competition_round_publication
           SET requires_referee = 1, publication_version = publication_version + 1
           WHERE round_generation_id = ? AND status = 'quiescing' AND requires_referee = 0`,
          roundRef.roundGenerationId,
        )
      }
      this.insertAudit({
        actorUserId: request.callerUserId,
        role: 'organizer',
        matchId: request.matchId,
        operationType: 'startScheduledMatch',
        commandId: request.commandId,
        revision: competitionRevision,
        outcomeCode: 'success',
        trustedTimeMs: parentSample,
        reason: '首局 GameStarted 已接收',
      })
      this.insertCommandReceipt(
        request.commandId, 'ingestInitialGameStarted', fingerprint, storedReceipt, request.matchId,
      )
    })
    return result
  }

  private currentRoundPublication(stageId: string, roundNumber: number): RoundPublicationRow | undefined {
    return this.storage.sql.exec<RoundPublicationRow>(
      `SELECT * FROM competition_round_publication WHERE stage_id = ? AND round_number = ? AND status != 'retired'`,
      stageId, roundNumber,
    ).toArray()[0]
  }

  private publishedRoundBindingContext(
    matchId: string,
  ): { roundRef: CompetitionRoundRef; roundStartMs: number } | null {
    const current = this.meta()
    if (!current) return null
    const headers = this.storage.sql.exec<RoundPublicationRow>(
      `SELECT * FROM competition_round_publication WHERE status != 'retired' ORDER BY rowid`,
    ).toArray()
    for (const header of headers) {
      try {
        const manifest = parseRoundManifest(header, current.competition_id)
        if (manifest.rooms.some(room => room.matchId === matchId)) {
          return { roundRef: structuredClone(manifest.roundRef), roundStartMs: manifest.roundStartMs }
        }
      } catch {
        // 損壞的其他輪公告不會成為這場比賽的 binding authority。
      }
    }
    return null
  }

  private originalRoundPublication(commandId: string): RoundPublicationRow {
    // 關聯與完整 manifest 同交易持久化；retired 不排除。CASE 避免損壞 JSON 造成非封閉 SQL 例外。
    const rows = this.storage.sql.exec<RoundPublicationRow>(
      `SELECT * FROM competition_round_publication
       WHERE CASE WHEN json_valid(manifest_json)
         THEN json_extract(manifest_json, '$.announcementCommandId') ELSE NULL END = ?`,
      commandId,
    ).toArray()
    const original = rows[0]
    if (!original || rows.length !== 1) throw new Error('original round announcement authority is missing or ambiguous')
    return original
  }

  private roundRoomLedger(roundGenerationId: string): RoundRoomLedgerRow[] {
    return this.storage.sql.exec<RoundRoomLedgerRow>(
      'SELECT * FROM competition_round_room_ledger WHERE round_generation_id = ? ORDER BY rowid', roundGenerationId,
    ).toArray()
  }

  /** 本地完整 authority 快照；所有 cursor 已消費，外部呼叫不能持有可變 SQL view。 */
  private async roundDeliveryAuthority(
    header: RoundPublicationRow,
    pending: boolean,
  ): Promise<RoundDeliveryAuthority> {
    const current = this.meta()
    if (!current || !safeNonNegativeInteger(current.updated_trusted_time_ms)
      || header.requires_referee !== 0 || !['materializing', 'released'].includes(header.status)) {
      throw new Error('round delivery authority is unavailable')
    }
    const manifest = parseRoundManifest(header, current.competition_id)
    const publication = await this.readRoundPublicationInternal({ competitionId: current.competition_id,
      stageId: header.stage_id, roundNumber: header.round_number, callerUserId: current.organizer_user_id })
    if (!publication.ok || !publication.value.publication
      || !sameJson(publication.value.publication.roundRef, manifest.roundRef)
      || !sameJson(this.meta(), current)
      || !sameJson(this.currentRoundPublication(header.stage_id, header.round_number), header)) {
      throw new Error('current round publication is invalid')
    }
    const rows = this.roundRoomLedger(header.round_generation_id)
    const matches: MatchRow[] = []
    if (pending) {
      const state = parseServerState(current.server_state_json)
      const selected = selectPublishableRound(
        state.tournament, header.stage_id, header.round_number,
        match => this.isAuthoritativeNoShowResult(match),
      )
      // 不用單條晉級來源或靜態 standings 冒充整前輪實際完成。
      for (const previous of selected.previousMatches) {
        if (isCoreBye(previous)) continue
        if (this.isAuthoritativeNoShowResult(previous)) continue
        const row = typeof previous.matchId === 'string' ? this.match(previous.matchId) : undefined
        if (!row || row.stage_id !== header.stage_id) throw new Error('previous match authority is missing')
        const checkpoint = parseJsonObject(row.match_checkpoint_json, 'previous match')
        const submission = latestSubmissionFromCheckpoint(checkpoint)
        const scoped: CompetitionServerState = {
          registration: null, clock: null, tournament: null, matches: { [row.match_id]: checkpoint },
        }
        const view = await this.computeRoundEngine({ serverActor: actorFor('organizer', current.organizer_user_id),
          trustedTimeMs: current.updated_trusted_time_ms, operation: { type: 'matchView', matchId: row.match_id },
          serverState: scoped })
        if (!view.ok || !isValidStoredMatchView(view, scoped, row) || !submission
          || !isJsonObject(view.public) || !isJsonObject(view.public.match)
          || !isJsonObject(checkpoint.config) || !isValidMatchFormat(checkpoint.config.format)
          || !Array.isArray(checkpoint.history) || !Number.isSafeInteger(row.revision)
          || row.revision < 1 + checkpoint.history.length || !safeNonNegativeInteger(row.last_trusted_time_ms)
          || row.last_trusted_time_ms > current.updated_trusted_time_ms || checkpoint.config.matchId !== row.match_id
          || !sameJson(checkpoint.config.format, selected.schedules[0]?.format)
          || !sameJson(state.matches?.[row.match_id], checkpoint)
          || !sameJson(checkpoint.config.teams, previous.participants)) throw new Error('previous match is invalid')
        const mapped = mapCompletedMatchResult(view.public.match, checkpoint.config,
          checkpoint.config.format, checkpoint, submission)
        if (!mapped.ok || mapped.value.status !== 'completed' || !sameJson(mapped.value.result, previous.result)) {
          throw new Error('previous round result is not accepted')
        }
        matches.push(row)
      }
      for (const ledger of rows) {
        const row = this.match(ledger.match_id)
        const schedule = selected.schedules.find(entry => entry.matchId === ledger.match_id)
        if (!row || !schedule || row.revision !== 2 || row.revision !== ledger.match_revision
          || typeof row.clock_checkpoint_json !== 'string'
          || !safeNonNegativeInteger(row.last_trusted_time_ms)
          || row.last_trusted_time_ms > current.updated_trusted_time_ms) throw new Error('pending match row is invalid')
        const intent = deriveProvisionIntent(row, state, null, null, 'pending')
        const clock = parseJsonObject(row.clock_checkpoint_json, 'round clock')
        const clockProjection = parseOptionalProjection(row.clock_projection_json)
        const checkpoint = parseJsonObject(row.match_checkpoint_json, 'round match')
        if (!hasExactKeys(checkpoint, ['config', 'history']) || !Array.isArray(checkpoint.history)
          || checkpoint.history.length !== 1) throw new Error('round initial history is invalid')
        const scoped: CompetitionServerState = {
          registration: null, clock: null, tournament: null, matches: { [row.match_id]: checkpoint },
        }
        const view = await this.computeRoundEngine({ serverActor: actorFor('organizer', current.organizer_user_id),
          trustedTimeMs: current.updated_trusted_time_ms, operation: { type: 'matchView', matchId: row.match_id },
          serverState: scoped })
        if (!view.ok || !isValidStoredMatchView(view, scoped, row) || !intent.ok
          || intent.value.gameNumber !== 1 || !sameJson(intent.value.nextGame, JSON.parse(ledger.next_game_json))
          || !isValidClockCheckpoint(clock, schedule.teams) || !sameJson(clock.config, schedule.clockConfig)
          || clock.preparationStarted !== false || clock.running !== null || clock.paused !== null
          || clock.expiredTeam !== null || clock.doubleExpired !== false || clock.needsDecision !== null
          || !clockProjection || clockProjection.operation !== 'clockCreate'
          || validateClockPublicView(clockProjection.public, clock, schedule.teams) !== null
          || storedClockProjectionDeadline(clockProjection) !== null) throw new Error('pending round match is invalid')
        matches.push(row)
      }
    } else {
      // 已持久 release 的重送可遇上真 started；仍驗 row/mirror/clock，不猜新的時計或開局。
      const state = parseServerState(current.server_state_json)
      for (const ledger of rows) {
        const row = this.match(ledger.match_id)
        // release 決定不等於 GameStarted；依持久 ledger 驗證尚未開打或已開始。
        const schedule = findScheduledMatch(state.tournament, ledger.match_id, false, false,
          ledger.status === 'started' ? 'inProgress' : 'pending')
        if (!row || !schedule || row.stage_id !== header.stage_id || !Number.isSafeInteger(row.revision)
          || typeof row.clock_checkpoint_json !== 'string'
          || row.revision < ledger.match_revision) throw new Error('released match authority is missing')
        const checkpoint = parseJsonObject(row.match_checkpoint_json, 'released match')
        const clock = parseJsonObject(row.clock_checkpoint_json, 'released clock')
        const clockProjection = parseOptionalProjection(row.clock_projection_json)
        const clockPublic = clockProjection?.operation === 'clockCreate' ? clockProjection.public
          : isJsonObject(clockProjection?.public) ? clockProjection.public.clock : null
        const scoped: CompetitionServerState = {
          registration: null, clock: null, tournament: null, matches: { [row.match_id]: checkpoint },
        }
        const view = await this.computeRoundEngine({ serverActor: actorFor('organizer', current.organizer_user_id),
          trustedTimeMs: current.updated_trusted_time_ms, operation: { type: 'matchView', matchId: row.match_id },
          serverState: scoped })
        if (!view.ok || !isValidStoredMatchView(view, scoped, row)
          || !hasExactKeys(checkpoint, ['config', 'history']) || !Array.isArray(checkpoint.history)
          || row.revision < 1 + checkpoint.history.length || !sameJson(state.matches?.[row.match_id], checkpoint)
          || !isJsonObject(checkpoint.config) || checkpoint.config.matchId !== row.match_id
          || !sameJson(checkpoint.config.teams, schedule.teams) || !sameJson(checkpoint.config.format, schedule.format)
          || !isValidClockCheckpoint(clock, schedule.teams) || !sameJson(clock.config, schedule.clockConfig)
          || !clockProjection || !clockProjection.operation.startsWith('clock')
          || clockProjection.operation === 'clockView'
          || validateClockPublicView(clockPublic, clock, schedule.teams) !== null) {
          throw new Error('released match checkpoint is invalid')
        }
        matches.push(row)
      }
    }
    const authority = { meta: current, header, manifest, rows, matches }
    this.assertRoundAuthorityUnchanged(authority)
    return authority
  }

  private assertRoundAuthorityUnchanged(authority: RoundDeliveryAuthority): void {
    if (!sameJson(this.meta(), authority.meta)
      || !sameJson(this.currentRoundPublication(authority.header.stage_id, authority.header.round_number), authority.header)
      || !sameJson(this.roundRoomLedger(authority.header.round_generation_id), authority.rows)
      || authority.matches.some(row => !sameJson(this.match(row.match_id), row))) {
      throw new Error('round authority changed before commit')
    }
  }

  private roundReleaseRows(generation: string): RoundReleaseRow[] {
    return this.storage.sql.exec<RoundReleaseRow>(
      'SELECT * FROM competition_round_release_outbox WHERE round_generation_id = ? ORDER BY rowid', generation,
    ).toArray()
  }

  /** 這只是工作候選，不是權威 release 判定；真正提交仍重查完整 checkpoint。 */
  private roundsReadyForRelease(): RoundPublicationRow[] {
    const current = this.meta()
    if (!current) return []
    const now = Date.now()
    return this.storage.sql.exec<RoundPublicationRow>(
      "SELECT * FROM competition_round_publication WHERE status = 'materializing' AND requires_referee = 0",
    ).toArray().filter(header => {
      try {
        // 自動輪次要在公告點前完成整批子房放行；到點才開始派送會壓縮報到窗口。
        return (header.release_not_before_ms === 0 || header.release_not_before_ms > now)
          && header.release_decision_id === null
          && this.roundHasCompleteTimelyCreationAcks(header, current.competition_id)
      } catch { return false }
    })
  }

  private roundsWaitingForReleaseWindow(): RoundPublicationRow[] {
    const current = this.meta()
    if (!current) return []
    const now = Date.now()
    return this.storage.sql.exec<RoundPublicationRow>(
      "SELECT * FROM competition_round_publication WHERE status IN ('materializing', 'released') AND requires_referee = 0",
    ).toArray().filter(header => {
      try {
        return header.release_not_before_ms > now
          && this.roundHasCompleteTimelyCreationAcks(header, current.competition_id)
      } catch { return false }
    })
  }

  private roundHasCompleteTimelyCreationAcks(header: RoundPublicationRow, competitionId: string): boolean {
    const manifest = parseRoundManifest(header, competitionId)
    const cutoff = competitionRoundReadyCutoffMs(header.round_start_ms)
    const rows = this.roundRoomLedger(header.round_generation_id)
    return cutoff !== null && rows.length === manifest.rooms.length
      && manifest.rooms.every(expected => rows.some(row => row.game_id === expected.gameId
        && row.match_id === expected.matchId && row.game_number === 1 && row.status === 'confirmed'
        && safeNonNegativeInteger(row.ack_confirmed_at_ms) && row.ack_confirmed_at_ms <= cutoff))
  }

  private roundHasCompleteTimelyReleaseAcks(header: RoundPublicationRow, competitionId: string): boolean {
    if (header.release_not_before_ms === 0) return true
    if (!nonEmpty(header.release_decision_id)) return false
    const manifest = parseRoundManifest(header, competitionId)
    const rows = this.roundReleaseRows(header.round_generation_id)
    return rows.length === manifest.rooms.length && manifest.rooms.every(expected => rows.some(row => (
      row.game_id === expected.gameId && row.release_decision_id === header.release_decision_id
      && row.status === 'completed' && safeNonNegativeInteger(row.completed_at_ms)
      && row.completed_at_ms <= header.release_not_before_ms
    )))
  }

  private expireMissedRoundReadinessWindows(now: number): void {
    if (!safeNonNegativeInteger(now)) return
    const current = this.meta()
    if (!current) return
    const due = this.storage.sql.exec<RoundPublicationRow>(
      `SELECT * FROM competition_round_publication
       WHERE status IN ('materializing', 'released') AND requires_referee = 0
         AND release_not_before_ms > 0 AND release_not_before_ms <= ?`, now,
    ).toArray()
    for (const header of due) {
      if (header.status === 'released'
        && this.roundHasCompleteTimelyReleaseAcks(header, current.competition_id)) continue
      if (!safeNonNegativeInteger(header.publication_version + 1)) continue
      this.storage.transactionSync(() => {
        const latest = this.currentRoundPublication(header.stage_id, header.round_number)
        if (!sameJson(latest, header)) return
        this.storage.sql.exec(`UPDATE competition_round_publication
          SET requires_referee = 1, publication_version = publication_version + 1
          WHERE round_generation_id = ? AND status = ? AND requires_referee = 0`,
        header.round_generation_id, header.status)
      })
    }
  }

  /** 一次只 claim 一項；lease 僅供重新取工作，不授予 release 或解除 fence。 */
  private async claimRoundDelivery(now: number): Promise<RoundDeliveryClaim | 'decision' | null> {
    if (!safeNonNegativeInteger(now)) return null
    const creates = this.storage.sql.exec<RoundRoomLedgerRow>(
      `SELECT ledger.* FROM competition_round_room_ledger ledger
       JOIN competition_round_publication publication USING (round_generation_id)
       WHERE ledger.status = 'pending' AND ledger.next_attempt_at_ms <= ?
         AND publication.status = 'materializing' AND publication.requires_referee = 0
       ORDER BY ledger.next_attempt_at_ms, ledger.game_id LIMIT 1`, now,
    ).toArray()
    const candidate = creates[0]
    if (candidate) {
      const header = this.storage.sql.exec<RoundPublicationRow>(
        'SELECT * FROM competition_round_publication WHERE round_generation_id = ?', candidate.round_generation_id,
      ).toArray()[0]
      if (!header) return null
      const authority = await this.roundDeliveryAuthority(header, true)
      const expected = authority.manifest.rooms.find(entry => entry.gameId === candidate.game_id)
      if (!expected || !Number.isSafeInteger(candidate.claim_version + 1)
        || !sameJson(authority.rows.find(row => row.game_id === candidate.game_id), candidate)) {
        throw new Error('materialization claim is invalid')
      }
      const row = { ...candidate, claim_id: crypto.randomUUID(), claim_version: candidate.claim_version + 1,
        next_attempt_at_ms: checkedRoundRetryTime(now, 30_000) }
      this.storage.transactionSync(() => {
        this.assertRoundAuthorityUnchanged(authority)
        this.storage.sql.exec(`UPDATE competition_round_room_ledger
          SET claim_id = ?, claim_version = ?, next_attempt_at_ms = ? WHERE game_id = ?`,
        row.claim_id, row.claim_version, row.next_attempt_at_ms, row.game_id)
      })
      return { kind: 'create', authority, row, bindingRef: structuredClone(expected.bindingRef),
        request: { gameId: row.game_id, binding: parseStoredCompetitionGameBinding(row.binding_json) } }
    }
    // 全輪房間 ACK 已證明都在五分鐘屏障內後，立即持久授權；T 是報到截止，不是開局下限。
    const due = this.roundsReadyForRelease()[0]
    if (due) {
      const authority = await this.roundDeliveryAuthority(due, true)
      const releaseAt = Date.now()
      if (!safeNonNegativeInteger(releaseAt) || releaseAt < now
        || releaseAt < authority.meta.updated_trusted_time_ms
        || !safeNonNegativeInteger(due.publication_version + 1)) throw new Error('release sample or version is invalid')
      const decision = crypto.randomUUID()
      this.storage.transactionSync(() => {
        this.assertRoundAuthorityUnchanged(authority)
        if (!this.roundsReadyForRelease().some(header => sameJson(header, due))
          || this.roundReleaseRows(due.round_generation_id).length !== 0) throw new Error('round release CAS failed')
        this.storage.sql.exec(`UPDATE competition_round_publication
          SET status = 'released', publication_version = publication_version + 1, release_decision_id = ?
          WHERE round_generation_id = ?`, decision, due.round_generation_id)
        for (const room of authority.manifest.rooms) {
          this.storage.sql.exec(`INSERT INTO competition_round_release_outbox
            (round_generation_id, game_id, release_decision_id, status, claim_id, claim_version, attempts, next_attempt_at_ms)
            VALUES (?, ?, ?, 'pending', NULL, 0, 0, ?)`, due.round_generation_id, room.gameId, decision, releaseAt)
        }
      })
      return 'decision'
    }
    const release = this.storage.sql.exec<RoundReleaseRow>(
      `SELECT outbox.* FROM competition_round_release_outbox outbox
       JOIN competition_round_publication publication USING (round_generation_id)
       WHERE outbox.status = 'pending' AND outbox.next_attempt_at_ms <= ?
         AND publication.status = 'released' AND publication.requires_referee = 0
       ORDER BY outbox.next_attempt_at_ms, outbox.game_id LIMIT 1`, now,
    ).toArray()[0]
    if (!release) return null
    const header = this.storage.sql.exec<RoundPublicationRow>(
      'SELECT * FROM competition_round_publication WHERE round_generation_id = ?', release.round_generation_id,
    ).toArray()[0]
    if (!header) return null
    const authority = await this.roundDeliveryAuthority(header, false)
    const outbox = this.roundReleaseRows(header.round_generation_id)
    const expected = authority.manifest.rooms.find(entry => entry.gameId === release.game_id)
    if (!expected || !nonEmpty(header.release_decision_id) || outbox.length !== authority.manifest.rooms.length
      || authority.rows.some(row => row.status !== 'confirmed')
      || !Number.isSafeInteger(release.claim_version + 1)
      || !sameJson(outbox.find(entry => entry.game_id === release.game_id), release)
      || !authority.manifest.rooms.every(room => outbox.some(entry => entry.game_id === room.gameId
        && entry.release_decision_id === header.release_decision_id && validRoundReleaseRow(entry)))) {
      throw new Error('complete release outbox is invalid')
    }
    const row = { ...release, claim_id: crypto.randomUUID(), claim_version: release.claim_version + 1,
      next_attempt_at_ms: checkedRoundRetryTime(now, 30_000) }
    this.storage.transactionSync(() => {
      this.assertRoundAuthorityUnchanged(authority)
      if (!sameJson(this.roundReleaseRows(header.round_generation_id), outbox)) throw new Error('release claim changed')
      this.storage.sql.exec(`UPDATE competition_round_release_outbox
        SET claim_id = ?, claim_version = ?, next_attempt_at_ms = ? WHERE round_generation_id = ? AND game_id = ?`,
      row.claim_id, row.claim_version, row.next_attempt_at_ms, row.round_generation_id, row.game_id)
    })
    return { kind: 'release', authority, row,
      request: { bindingRef: structuredClone(expected.bindingRef), commandId: header.release_decision_id } }
  }

  private async finishRoundDelivery(claim: RoundDeliveryClaim, accepted: boolean): Promise<void> {
    const { row } = claim
    const header = this.currentRoundPublication(claim.authority.header.stage_id, claim.authority.header.round_number)
    if (!header || header.round_generation_id !== row.round_generation_id || header.requires_referee !== 0
      || header.status !== (claim.kind === 'create' ? 'materializing' : 'released')
      || header.manifest_json !== claim.authority.header.manifest_json
      || header.round_start_ms !== claim.authority.header.round_start_ms) return
    const latest = claim.kind === 'create'
      ? this.roundRoomLedger(row.round_generation_id).find(entry => entry.game_id === row.game_id)
      : this.roundReleaseRows(row.round_generation_id).find(entry => entry.game_id === row.game_id)
    if (!latest || latest.status !== 'pending' || latest.claim_id !== row.claim_id
      || latest.claim_version !== row.claim_version || !sameJson(latest, row)) return
    const now = Date.now()
    if (!safeNonNegativeInteger(now)) throw new Error('round confirmation sample is invalid')
    if (!accepted) {
      if (!safeNonNegativeInteger(row.attempts + 1)) throw new Error('round retry counter overflow')
      const next = checkedRoundRetryTime(now, Math.min(60_000, 1_000 * 2 ** Math.min(row.attempts + 1, 6)))
      const table = claim.kind === 'create' ? 'competition_round_room_ledger' : 'competition_round_release_outbox'
      this.storage.transactionSync(() => {
        this.storage.sql.exec(`UPDATE ${table} SET claim_id = NULL, attempts = attempts + 1,
          next_attempt_at_ms = ? WHERE round_generation_id = ? AND game_id = ? AND claim_id = ? AND claim_version = ?`,
        next, row.round_generation_id, row.game_id, row.claim_id, row.claim_version)
      })
      return
    }
    const authority = await this.roundDeliveryAuthority(header, claim.kind === 'create')
    // C 是取得 tail 且完成完整 authority 檢查後的實際父 sample，不能使用來件時間。
    const confirmedAt = Date.now()
    if (!safeNonNegativeInteger(confirmedAt) || confirmedAt < now
      || confirmedAt < authority.meta.updated_trusted_time_ms) throw new Error('round confirmation sample regressed')
    if (claim.kind === 'create' && !safeNonNegativeInteger(header.publication_version + 1)) {
      throw new Error('round publication version overflow')
    }
    this.storage.transactionSync(() => {
      this.assertRoundAuthorityUnchanged(authority)
      if (claim.kind === 'create') {
        const current = this.roundRoomLedger(row.round_generation_id).find(entry => entry.game_id === row.game_id)
        if (!sameJson(current, row)) throw new Error('materialization ACK claim changed')
        this.storage.sql.exec(`UPDATE competition_round_room_ledger SET status = 'confirmed',
          ack_confirmed_at_ms = ?, claim_id = NULL WHERE game_id = ?`, confirmedAt, row.game_id)
        this.storage.sql.exec(`UPDATE competition_round_publication SET publication_version = publication_version + 1
          WHERE round_generation_id = ?`, row.round_generation_id)
      } else {
        const current = this.roundReleaseRows(row.round_generation_id).find(entry => entry.game_id === row.game_id)
        if (!sameJson(current, row) || header.release_decision_id !== claim.request.commandId) {
          throw new Error('release ACK claim changed')
        }
        this.storage.sql.exec(`UPDATE competition_round_release_outbox SET status = 'completed', claim_id = NULL
          , completed_at_ms = ? WHERE round_generation_id = ? AND game_id = ?`,
        confirmedAt, row.round_generation_id, row.game_id)
      }
    })
  }

  /** 外部 await 在 operationTail 之外；ACK／更正／pure read 可進本地 critical lane。 */
  private async pumpRoundDeliveries(): Promise<void> {
    for (let count = 0; count < 16; count += 1) {
      const claim = await this.serialized(async () => {
        let next: RoundDeliveryClaim | 'decision' | null
        try {
          this.expireMissedRoundReadinessWindows(Date.now())
          next = await this.claimRoundDelivery(Date.now())
        } catch {
          console.warn('CompetitionRoom retained invalid round delivery authority')
          return null
        }
        if (next !== null) {
          try { await this.scheduleSharedAlarm() } catch {
            console.warn('CompetitionRoom retained round delivery after alarm scheduling failure')
          }
        }
        return next
      })
      if (claim === null) break
      if (claim === 'decision') continue
      let accepted = false
      try {
        const namespace = this.roomEnv.GAME_ROOM
        if (!namespace) throw new Error('round game namespace unavailable')
        const stub = namespace.get(namespace.idFromName(claim.row.game_id))
        if (claim.kind === 'create') {
          const method = stub.createCompetitionGame
          if (typeof method !== 'function') throw new Error('round create capability unavailable')
          const result: unknown = await Reflect.apply(method, stub, [structuredClone(claim.request)])
          accepted = consumeRoundReadyAck(result, claim.bindingRef) !== null
        } else {
          const method = stub.releaseCompetitionGame
          if (typeof method !== 'function') throw new Error('round release capability unavailable')
          const result: unknown = await Reflect.apply(method, stub, [structuredClone(claim.request)])
          accepted = isCompetitionGameReleaseResult(result) && result === true
          // 例外 object 回傳依然釋放窄 transport 資源，但絕不當 boolean 成功。
          if (typeof result === 'object' && result !== null) consumeRoundReadyAck(result, claim.request.bindingRef)
        }
      } catch {
        // 技術失敗只保留 durable 工作；不反射 remote message、private binding 或假的成功 receipt。
      }
      await this.serialized(async () => {
        try { await this.finishRoundDelivery(claim, accepted) } catch {
          console.warn('CompetitionRoom retained round delivery after ACK commit failure')
        }
      })
    }
    await this.serialized(async () => {
      try { await this.scheduleSharedAlarm() } catch {
        console.warn('CompetitionRoom retained round work after alarm scheduling failure')
      }
    })
  }

  private noShowOutbox(matchId: string): NoShowOutboxRow | undefined {
    return this.storage.sql.exec<NoShowOutboxRow>(
      'SELECT * FROM competition_no_show_outbox WHERE match_id = ?', matchId,
    ).toArray()[0]
  }

  /** 晉級只接受父房已提交且能對回子房固定成員判定的 no-show。 */
  private isAuthoritativeNoShowResult(match: JsonObject): boolean {
    if (typeof match.matchId !== 'string' || match.status !== 'completed' || match.started !== false
      || !isTeamPair(match.participants) || !isJsonObject(match.result)) return false
    const row = this.noShowOutbox(match.matchId)
    if (!row || row.status !== 'completed' || !validNoShowOutboxRow(row)
      || !safeNonNegativeInteger(row.result_confirmed_at_ms)
      || row.decision_json === null) return false
    let decision: unknown
    try { decision = JSON.parse(row.decision_json) } catch { return false }
    if (!isCompetitionGameNoShowDecision(decision) || decision.type !== 'noShow') return false
    const expected = noShowTournamentResult(match.participants, decision)
    if (!expected || !sameJson(match.result, expected)) return false
    const header = this.storage.sql.exec<RoundPublicationRow>(
      'SELECT * FROM competition_round_publication WHERE round_generation_id = ?',
      row.round_generation_id,
    ).toArray()[0]
    const current = this.meta()
    if (!header || !current || row.game_id === '' || match.stageId !== header.stage_id) return false
    try {
      const manifest = parseRoundManifest(header, current.competition_id)
      return manifest.roundStartMs === row.deadline_ms
        && manifest.rooms.some(room => room.matchId === row.match_id && room.gameId === row.game_id
          && room.bindingRef.roundRef.roundGenerationId === row.round_generation_id)
    } catch {
      return false
    }
  }

  /** T 到期後只判定在 T 前已完整開放的房間；決策與賽果都透過持久 outbox 重試。 */
  private async claimNoShowRuling(now: number): Promise<NoShowDeliveryClaim | null> {
    if (!safeNonNegativeInteger(now)) return null
    const candidate = this.storage.sql.exec<NoShowOutboxRow>(
      `SELECT * FROM competition_no_show_outbox
       WHERE status = 'pending' AND next_attempt_at_ms <= ?
       ORDER BY next_attempt_at_ms, match_id LIMIT 1`, now,
    ).toArray()[0]
    if (!candidate) return null
    if (!validNoShowOutboxRow(candidate) || now < candidate.deadline_ms) {
      throw new Error('no-show outbox candidate is invalid or early')
    }
    const header = this.storage.sql.exec<RoundPublicationRow>(
      'SELECT * FROM competition_round_publication WHERE round_generation_id = ?',
      candidate.round_generation_id,
    ).toArray()[0]
    if (!header) throw new Error('no-show publication is missing')
    const manifest = parseRoundManifest(header, this.meta()?.competition_id ?? '')
    const room = manifest.rooms.find(entry => entry.matchId === candidate.match_id
      && entry.gameId === candidate.game_id)
    if (!room || manifest.roundStartMs !== candidate.deadline_ms
      || candidate.command_id !== competitionGameNoShowCommandId(room.bindingRef)) {
      throw new Error('no-show outbox does not match its round manifest')
    }
    let decision: CompetitionGameNoShowDecision | null = null
    if (candidate.decision_json !== null) {
      const parsed: unknown = JSON.parse(candidate.decision_json)
      if (!isCompetitionGameNoShowDecision(parsed)) throw new Error('stored no-show decision is invalid')
      decision = parsed
    } else {
      const releaseRows = this.roundReleaseRows(candidate.round_generation_id)
      const ledgerRows = this.roundRoomLedger(candidate.round_generation_id)
      const cutoff = competitionRoundReadyCutoffMs(candidate.deadline_ms)
      const releasedOnTime = header.status === 'released' && header.requires_referee === 0
        && cutoff !== null && ledgerRows.length === manifest.rooms.length
        && manifest.rooms.every(expected => ledgerRows.some(row => row.match_id === expected.matchId
          && row.game_id === expected.gameId && row.status === 'confirmed'
          && safeNonNegativeInteger(row.ack_confirmed_at_ms) && row.ack_confirmed_at_ms <= cutoff))
        && releaseRows.length === manifest.rooms.length
        && manifest.rooms.every(expected => releaseRows.some(row => row.game_id === expected.gameId
          && row.status === 'completed' && validRoundReleaseRow(row)
          && row.completed_at_ms !== null && row.completed_at_ms <= candidate.deadline_ms))
      if (!releasedOnTime) decision = { type: 'unavailable' }
    }
    const nextAttempt = checkedRoundRetryTime(now, 30_000)
    if (!Number.isSafeInteger(candidate.claim_version + 1)) throw new Error('no-show claim version overflow')
    const claimed: NoShowOutboxRow = {
      ...candidate,
      claim_id: crypto.randomUUID(),
      claim_version: candidate.claim_version + 1,
      next_attempt_at_ms: nextAttempt,
    }
    this.storage.transactionSync(() => {
      const latest = this.noShowOutbox(candidate.match_id)
      if (!sameJson(latest, candidate)) throw new Error('no-show candidate changed before claim')
      this.storage.sql.exec(`UPDATE competition_no_show_outbox
        SET claim_id = ?, claim_version = ?, next_attempt_at_ms = ?
        WHERE round_generation_id = ? AND match_id = ? AND status = 'pending'`,
      claimed.claim_id, claimed.claim_version, claimed.next_attempt_at_ms,
      claimed.round_generation_id, claimed.match_id)
    })
    return {
      row: claimed,
      request: { bindingRef: structuredClone(room.bindingRef), commandId: claimed.command_id,
        deadlineMs: claimed.deadline_ms },
      decision,
    }
  }

  private async persistNoShowDecision(
    claim: NoShowDeliveryClaim,
    decision: CompetitionGameNoShowDecision,
  ): Promise<NoShowDeliveryClaim | null> {
    if (!isCompetitionGameNoShowDecision(decision)
      || (decision.type === 'ready' || decision.type === 'noShow')
        && decision.sampledAtMs < claim.row.deadline_ms) return null
    if (claim.row.decision_json !== null) {
      return sameJson(JSON.parse(claim.row.decision_json), decision) ? { ...claim, decision } : null
    }
    if (decision.type === 'noShow') {
      const header = this.storage.sql.exec<RoundPublicationRow>(
        'SELECT * FROM competition_round_publication WHERE round_generation_id = ?',
        claim.row.round_generation_id,
      ).toArray()[0]
      if (!header) return null
      const manifest = parseRoundManifest(header, this.meta()?.competition_id ?? '')
      const scheduledMatches = manifest.tournamentRound.matches
      if (!Array.isArray(scheduledMatches)) return null
      const scheduled = scheduledMatches.find((match): match is JsonObject =>
        isJsonObject(match) && match.matchId === claim.row.match_id)
      if (!scheduled || !isTeamPair(scheduled.participants)
        || !noShowTournamentResult(scheduled.participants, decision)) return null
    }
    this.storage.transactionSync(() => {
      const latest = this.noShowOutbox(claim.row.match_id)
      if (!sameJson(latest, claim.row)) throw new Error('no-show decision claim changed')
      this.storage.sql.exec(`UPDATE competition_no_show_outbox SET decision_json = ?
        WHERE round_generation_id = ? AND match_id = ? AND status = 'pending'
          AND claim_id = ? AND claim_version = ? AND decision_json IS NULL`,
      JSON.stringify(decision), claim.row.round_generation_id, claim.row.match_id,
      claim.row.claim_id, claim.row.claim_version)
    })
    const row = this.noShowOutbox(claim.row.match_id)
    return row && row.decision_json === JSON.stringify(decision)
      ? { ...claim, row, decision }
      : null
  }

  private async applyNoShowTournamentResult(
    claim: NoShowDeliveryClaim,
    decision: Extract<CompetitionGameNoShowDecision, { type: 'noShow' }>,
  ): Promise<'completed' | 'retry'> {
    const row = claim.row
    return this.serialized(async () => {
      const current = this.meta()
      const latestOutbox = this.noShowOutbox(row.match_id)
      if (!current || !latestOutbox || !sameJson(latestOutbox, row)
        || latestOutbox.status !== 'pending' || latestOutbox.claim_id !== row.claim_id
        || latestOutbox.claim_version !== row.claim_version
        || latestOutbox.decision_json !== JSON.stringify(decision)) return 'retry'
      let state: CompetitionServerState
      let schedule: ScheduledMatchMaterialization | null
      try {
        state = parseServerState(current.server_state_json)
        schedule = findScheduledMatch(state.tournament, row.match_id, true, true, 'pending')
      } catch {
        try {
          state = parseServerState(current.server_state_json)
          schedule = findScheduledMatch(state.tournament, row.match_id, true, true, 'inProgress')
        } catch {
          return 'retry'
        }
      }
      const expectedResult = noShowTournamentResult(schedule?.teams, decision)
      if (!schedule || !expectedResult || schedule.stageId !== this.match(row.match_id)?.stage_id) {
        // 已開始或已有其他正式賽果時不再覆蓋該結果。
        if (schedule?.status === 'completed' || schedule?.status === 'inProgress') return 'completed'
        return 'retry'
      }
      if (schedule.status === 'completed') {
        return 'completed'
      }
      if (schedule.status !== 'pending' || schedule.started || schedule.result !== null) return 'completed'
      if (!current.organizer_user_id || !safeNonNegativeInteger(current.revision)
        || !safeNonNegativeInteger(current.updated_trusted_time_ms)) return 'retry'
      const organizer = this.acl(current.organizer_user_id)
      if (!organizer || organizer.active !== 1 || organizer.role !== 'organizer') return 'retry'
      const trustedTimeMs = Math.max(Date.now(), current.updated_trusted_time_ms)
      if (!safeNonNegativeInteger(trustedTimeMs)) return 'retry'
      const request: CompetitionRoomExecuteRequest = {
        competitionId: current.competition_id,
        callerUserId: current.organizer_user_id,
        commandId: row.command_id,
        expectedRevision: current.revision,
        trustedTimeMs,
        operation: { type: 'tournamentSubmitResult', stageId: schedule.stageId,
          matchId: row.match_id, result: expectedResult },
      }
      const fingerprint = await commandFingerprint(request)
      const receipt = this.commandReceipt(request.commandId)
      if (receipt) {
        if (receipt.receipt_kind === 'execute' && receipt.fingerprint === fingerprint) {
          return 'completed'
        }
        return 'retry'
      }
      let response: CompetitionEngineResponse
      try {
        response = await this.engine({
          serverActor: actorFor('organizer', current.organizer_user_id),
          trustedTimeMs,
          operation: request.operation,
          serverState: { registration: null, clock: null,
            tournament: state.tournament, matches: {} },
        })
      } catch {
        return 'retry'
      }
      const submitError = response.ok
        ? validateTournamentSubmitResponse(response, state.tournament, schedule, expectedResult)
        : response.error.code
      if (submitError !== null || !response.ok || response.operation !== 'tournamentSubmitResult'
        || !sameMatches(response.serverState.matches, {})) {
        return 'retry'
      }
      const projection = publicProjection(response, 'organizer')
      const revision = current.revision + 1
      if (!safeNonNegativeInteger(revision)) return 'retry'
      const persistedState: CompetitionServerState = {
        ...state,
        tournament: response.serverState.tournament,
      }
      const result = roomSuccess<CompetitionRoomExecuteValue>({
        competitionId: current.competition_id, revision, commandId: request.commandId,
        idempotent: false, projection,
      })
      this.storage.transactionSync(() => {
        const latest = this.meta()
        const latestAcl = this.acl(current.organizer_user_id)
        if (!sameJson(latest, current) || !sameJson(this.noShowOutbox(row.match_id), row)
          || !latestAcl || latestAcl.active !== 1 || latestAcl.role !== 'organizer'
          || this.commandReceipt(row.command_id)) throw new Error('no-show tournament result CAS failed')
        this.storage.sql.exec(`UPDATE competition_meta
          SET revision = ?, server_state_json = ?, public_projection_json = ?, updated_trusted_time_ms = ?
          WHERE singleton = 1`, revision, JSON.stringify(persistedState),
        JSON.stringify(stripPrivate(projection)), trustedTimeMs)
        this.storage.sql.exec(`UPDATE competition_no_show_outbox
          SET result_confirmed_at_ms = ?
          WHERE round_generation_id = ? AND match_id = ? AND status = 'pending'
            AND claim_id = ? AND claim_version = ?`,
        trustedTimeMs, row.round_generation_id, row.match_id, row.claim_id, row.claim_version)
        this.insertAudit({ actorUserId: current.organizer_user_id, role: 'organizer',
          operationType: 'tournamentSubmitResult', commandId: row.command_id, revision,
          outcomeCode: 'success', trustedTimeMs, reason: '賽前未完成報到自動判負' })
        this.insertCommandReceipt(row.command_id, 'execute', fingerprint, result)
      })
      return 'completed'
    })
  }

  private async finishNoShowRuling(
    claim: NoShowDeliveryClaim,
    complete: boolean,
  ): Promise<void> {
    const latest = this.noShowOutbox(claim.row.match_id)
    if (!latest || latest.status !== 'pending' || latest.claim_id !== claim.row.claim_id
      || latest.claim_version !== claim.row.claim_version || !sameJson(latest, claim.row)
        && latest.decision_json !== claim.row.decision_json) return
    const now = Date.now()
    if (!safeNonNegativeInteger(now)) return
    if (complete) {
      this.storage.sql.exec(`UPDATE competition_no_show_outbox SET status = 'completed', claim_id = NULL
        WHERE round_generation_id = ? AND match_id = ? AND status = 'pending'
          AND claim_id = ? AND claim_version = ?`,
      latest.round_generation_id, latest.match_id, latest.claim_id, latest.claim_version)
      return
    }
    const delay = Math.min(60_000, 1_000 * 2 ** Math.min(latest.attempts + 1, 6))
    this.storage.sql.exec(`UPDATE competition_no_show_outbox SET claim_id = NULL,
      attempts = attempts + 1, next_attempt_at_ms = ?
      WHERE round_generation_id = ? AND match_id = ? AND status = 'pending'
        AND claim_id = ? AND claim_version = ?`,
    checkedRoundRetryTime(now, delay), latest.round_generation_id, latest.match_id,
    latest.claim_id, latest.claim_version)
  }

  private async pumpNoShowRulings(): Promise<void> {
    for (let count = 0; count < 16; count += 1) {
      const claim = await this.serialized(async () => {
        try {
          const next = await this.claimNoShowRuling(Date.now())
          if (next) {
            try { await this.scheduleSharedAlarm() } catch {
              console.warn('CompetitionRoom retained automatic no-show ruling after alarm scheduling failure')
            }
          }
          return next
        } catch {
          console.warn('CompetitionRoom retained invalid automatic no-show authority')
          return null
        }
      })
      if (!claim) break
      let decision = claim.decision
      if (!decision) {
        try {
          const namespace = this.roomEnv.GAME_ROOM
          if (!namespace) throw new Error('no-show game namespace unavailable')
          const stub = namespace.get(namespace.idFromName(claim.row.game_id))
          const method = stub.adjudicateCompetitionNoShow
          if (typeof method !== 'function') throw new Error('no-show adjudication capability unavailable')
          const returned: unknown = await Reflect.apply(method, stub, [structuredClone(claim.request)])
          if (!isCompetitionGameNoShowDecision(returned)) throw new Error('no-show decision is invalid')
          decision = returned
        } catch {
          await this.serialized(() => this.finishNoShowRuling(claim, false))
          continue
        }
      }
      const decided = await this.serialized(() => this.persistNoShowDecision(claim, decision!))
      if (!decided) {
        await this.serialized(() => this.finishNoShowRuling(claim, false))
        continue
      }
      let complete = decision.type !== 'noShow'
      if (decision.type === 'noShow') {
        complete = await this.applyNoShowTournamentResult(decided, decision) === 'completed'
      }
      await this.serialized(() => this.finishNoShowRuling(decided, complete))
    }
    await this.serialized(async () => {
      try { await this.scheduleSharedAlarm() } catch {
        console.warn('CompetitionRoom retained automatic no-show work after alarm scheduling failure')
      }
    })
  }

  private prepareGameProvisionDelivery(
    linked: GameProvisionRow,
  ): { ok: true; binding: CompetitionGameBinding } | { ok: false; errorCode: string } {
    const current = this.meta()
    const row = this.match(linked.match_id)
    if (!current || !row || row.revision !== linked.match_revision) {
      return { ok: false, errorCode: 'staleMatchState' }
    }
    try {
      const binding = parseStoredCompetitionGameBinding(linked.binding_json)
      const state = parseServerState(current.server_state_json)
      const currentIntent = deriveProvisionIntent(row, state, binding.firstPlayer, binding.turnOrder)
      if (!currentIntent.ok
        || binding.gameNumber !== linked.game_number
        || currentIntent.value.gameNumber !== linked.game_number
        || !sameJson(currentIntent.value.nextGame, JSON.parse(linked.next_game_json))
        // 局別及 carry-forward 毫秒必須與權威 intent 完全相同，不能由 binding 改寫。
        || !sameJson(binding.startKind, currentIntent.value.nextGame.kind)
        || binding.expectedMatchRevision !== row.revision) {
        return { ok: false, errorCode: 'staleMatchState' }
      }
      return { ok: true, binding }
    } catch {
      return { ok: false, errorCode: 'invalidStoredBinding' }
    }
  }

  private claimGameProvisionDelivery(now: number): GameProvisionDeliveryClaim | 'blocked' | null {
    if (!safeNonNegativeInteger(now)) return null
    const candidate = this.nextProvisionRow()
    // 損壞 due 保留原 row；不符合非負 safe integer 的值不可比較後 claim。
    if (!candidate || !safeNonNegativeInteger(candidate.next_attempt_at_ms)
      || candidate.next_attempt_at_ms > now) return null
    const prepared = this.prepareGameProvisionDelivery(candidate)
    if (!prepared.ok) {
      this.blockGameProvision(candidate, prepared.errorCode)
      return 'blocked'
    }
    if (!Number.isSafeInteger(candidate.claim_version + 1)) {
      this.blockGameProvision(candidate, 'invalidDeliveryClaim')
      return 'blocked'
    }
    const claim: GameProvisionRow = {
      ...candidate,
      claim_id: crypto.randomUUID(),
      claim_version: candidate.claim_version + 1,
      next_attempt_at_ms: checkedRoundRetryTime(now, 30_000),
    }
    this.storage.transactionSync(() => {
      const current = this.gameProvision(candidate.match_id, candidate.game_number)
      if (!sameJson(current, candidate)) throw new Error('game provision claim source changed')
      this.storage.sql.exec(`UPDATE competition_game_provision
        SET claim_id = ?, claim_version = ?, next_attempt_at_ms = ?
        WHERE match_id = ? AND game_number = ? AND status = 'pending'
          AND claim_version = ? AND claim_id IS ?`,
      claim.claim_id, claim.claim_version, claim.next_attempt_at_ms, claim.match_id,
      claim.game_number, candidate.claim_version, candidate.claim_id)
    })
    const latest = this.gameProvision(claim.match_id, claim.game_number)
    if (!sameJson(latest, claim)) throw new Error('game provision claim was not retained')
    return { row: claim, binding: prepared.binding }
  }

  private deferInvalidClockControl(row: ClockControlRow, now: number): void {
    const next = checkedRoundRetryTime(now, 60_000)
    if (safeNonNegativeInteger(row.attempts + 1)) {
      this.storage.sql.exec(`UPDATE competition_clock_control_outbox
        SET attempts = attempts + 1, next_attempt_at_ms = ?
        WHERE control_id = ? AND status = 'pending' AND claim_version = ? AND claim_id IS ?`,
      next, row.control_id, row.claim_version, row.claim_id)
    } else {
      this.storage.sql.exec(`UPDATE competition_clock_control_outbox SET next_attempt_at_ms = ?
        WHERE control_id = ? AND status = 'pending' AND claim_version = ? AND claim_id IS ?`,
      next, row.control_id, row.claim_version, row.claim_id)
    }
  }

  private claimClockControlDelivery(now: number): ClockControlDeliveryClaim | 'deferred' | null {
    if (!safeNonNegativeInteger(now)) return null
    const candidate = this.nextClockControlRow()
    // 損壞 due 保留原 row；selector 仍讓它阻擋同 GameRoom 後續 revision。
    if (!candidate || !safeNonNegativeInteger(candidate.next_attempt_at_ms)
      || candidate.next_attempt_at_ms > now) return null
    if (!nonEmpty(candidate.control_id) || !nonEmpty(candidate.competition_id)
      || !nonEmpty(candidate.match_id) || !nonEmpty(candidate.game_id)
      || !nonEmpty(candidate.source_command_id) || !safeNonNegativeInteger(candidate.game_number)
      || candidate.game_number < 1 || !safeNonNegativeInteger(candidate.match_revision)
      || candidate.match_revision < 1 || !safeNonNegativeInteger(candidate.trusted_time_ms)
      || (candidate.next_clock_deadline_ms !== null
        && !safeNonNegativeInteger(candidate.next_clock_deadline_ms))
      || !safeNonNegativeInteger(candidate.attempts) || !safeNonNegativeInteger(candidate.claim_version)
      || !Number.isSafeInteger(candidate.claim_version + 1)) {
      this.deferInvalidClockControl(candidate, now)
      return 'deferred'
    }
    const claim: ClockControlRow = {
      ...candidate,
      claim_id: crypto.randomUUID(),
      claim_version: candidate.claim_version + 1,
      next_attempt_at_ms: checkedRoundRetryTime(now, 30_000),
    }
    const request: CompetitionClockControlRequest = {
      controlId: claim.control_id,
      competitionId: claim.competition_id,
      matchId: claim.match_id,
      gameId: claim.game_id,
      gameNumber: claim.game_number,
      sourceCommandId: claim.source_command_id,
      matchRevision: claim.match_revision,
      operation: claim.operation,
      trustedTimeMs: claim.trusted_time_ms,
      nextClockDeadlineMs: claim.next_clock_deadline_ms,
    }
    this.storage.transactionSync(() => {
      const current = this.clockControl(candidate.control_id)
      if (!sameJson(current, candidate)) throw new Error('clock control claim source changed')
      this.storage.sql.exec(`UPDATE competition_clock_control_outbox
        SET claim_id = ?, claim_version = ?, next_attempt_at_ms = ?
        WHERE control_id = ? AND status = 'pending' AND claim_version = ? AND claim_id IS ?`,
      claim.claim_id, claim.claim_version, claim.next_attempt_at_ms, claim.control_id,
      candidate.claim_version, candidate.claim_id)
    })
    const latest = this.clockControl(claim.control_id)
    if (!sameJson(latest, claim)) throw new Error('clock control claim was not retained')
    return { row: claim, request }
  }

  private async pumpGameProvisionDeliveries(batch: LegacyAlarmBatch): Promise<void> {
    for (;;) {
      if (batch.remaining === 0) break
      batch.remaining -= 1
      let claim: GameProvisionDeliveryClaim | 'blocked' | null
      try {
        claim = await this.serialized(async () => {
          try {
            const next = this.claimGameProvisionDelivery(Date.now())
            if (next !== null) {
              try { await this.scheduleSharedAlarm() } catch {
                console.warn('CompetitionRoom retained game provision after alarm scheduling failure')
              }
            }
            return next
          } catch {
            console.warn('CompetitionRoom retained game provision after claim failure')
            return null
          }
        })
      } catch {
        console.warn('CompetitionRoom retained game provision after claim failure')
        break
      }
      if (claim === null) {
        batch.remaining += 1
        break
      }
      if (claim === 'blocked') continue
      let accepted = false
      let errorCode = 'gameRoomDeliveryFailed'
      try {
        const namespace = this.roomEnv.GAME_ROOM
        if (!namespace) throw new Error('game room binding unavailable')
        const stub = namespace.get(namespace.idFromName(claim.row.game_id))
        const ack = await stub.createCompetitionGame({
          gameId: claim.row.game_id,
          binding: structuredClone(claim.binding),
        })
        const bindingRef = await competitionGameBindingRefForRequest({
          gameId: claim.row.game_id,
          binding: claim.binding,
        })
        accepted = isCompetitionRoomReadyAck(ack, bindingRef)
        if (!accepted) errorCode = 'invalidGameRoomAck'
      } catch {
        // 失敗保留同一持久 linkage，晚 ACK 只由原 claim 收斂。
      }
      await this.serialized(async () => {
        try {
          await this.finishGameProvisionDelivery(claim, accepted, errorCode)
        } catch {
          console.warn('CompetitionRoom retained game provision after ACK commit failure')
        }
        try { await this.scheduleSharedAlarm() } catch {
          console.warn('CompetitionRoom retained game provision after alarm scheduling failure')
        }
      })
    }
  }

  private async finishGameProvisionDelivery(
    claim: GameProvisionDeliveryClaim,
    accepted: boolean,
    errorCode: string,
  ): Promise<void> {
    const { row } = claim
    const latest = this.gameProvision(row.match_id, row.game_number)
    if (!latest || latest.status !== 'pending' || latest.claim_id !== row.claim_id
      || latest.claim_version !== row.claim_version || !sameJson(latest, row)) return
    const currentSource = this.prepareGameProvisionDelivery(latest)
    if (!currentSource.ok) {
      this.storage.sql.exec(`UPDATE competition_game_provision
        SET status = 'blocked', error_code = ?, claim_id = NULL
        WHERE match_id = ? AND game_number = ? AND status = 'pending'
          AND claim_id = ? AND claim_version = ?`,
      currentSource.errorCode, row.match_id, row.game_number, row.claim_id, row.claim_version)
      return
    }
    if (accepted && !sameJson(currentSource.binding, claim.binding)) return
    if (!accepted) {
      if (!safeNonNegativeInteger(row.attempts + 1)) throw new Error('game provision retry counter overflow')
      const next = checkedRoundRetryTime(Date.now(), Math.min(60_000, 1_000 * 2 ** Math.min(row.attempts + 1, 6)))
      this.storage.sql.exec(`UPDATE competition_game_provision
        SET claim_id = NULL, attempts = attempts + 1, next_attempt_at_ms = ?, error_code = ?
        WHERE match_id = ? AND game_number = ? AND status = 'pending'
          AND claim_id = ? AND claim_version = ?`,
      next, errorCode, row.match_id, row.game_number, row.claim_id, row.claim_version)
      return
    }
    this.storage.sql.exec(`UPDATE competition_game_provision
      SET status = 'provisioned', error_code = NULL, claim_id = NULL
      WHERE match_id = ? AND game_number = ? AND status = 'pending'
        AND claim_id = ? AND claim_version = ?`,
    row.match_id, row.game_number, row.claim_id, row.claim_version)
  }

  private async pumpClockControlDeliveries(batch: LegacyAlarmBatch): Promise<void> {
    for (;;) {
      if (batch.remaining === 0) break
      batch.remaining -= 1
      let claim: ClockControlDeliveryClaim | 'deferred' | null
      try {
        claim = await this.serialized(async () => {
          try {
            const next = this.claimClockControlDelivery(Date.now())
            if (next !== null) {
              try { await this.scheduleSharedAlarm() } catch {
                console.warn('CompetitionRoom retained clock control after alarm scheduling failure')
              }
            }
            return next
          } catch {
            console.warn('CompetitionRoom retained clock control after claim failure')
            return null
          }
        })
      } catch {
        console.warn('CompetitionRoom retained clock control after claim failure')
        break
      }
      if (claim === null) {
        batch.remaining += 1
        break
      }
      if (claim === 'deferred') continue
      let accepted = false
      let errorCode = 'gameRoomClockControlDeliveryFailed'
      try {
        const namespace = this.roomEnv.GAME_ROOM
        if (!namespace) throw new Error('game room binding unavailable')
        const target = namespace.get(namespace.idFromName(claim.row.game_id))
        const ack = await target.synchronizeCompetitionClock(structuredClone(claim.request))
        accepted = isCompetitionClockControlAck(ack, claim.request)
        if (!accepted) errorCode = 'invalidGameRoomClockControlAck'
      } catch {
        // 失敗保留控制 outbox，下一次由同一 controlId 至少一次重送。
      }
      await this.serialized(async () => {
        try {
          await this.finishClockControlDelivery(claim, accepted, errorCode)
        } catch {
          console.warn('CompetitionRoom retained clock control after ACK commit failure')
        }
        try { await this.scheduleSharedAlarm() } catch {
          console.warn('CompetitionRoom retained clock control after alarm scheduling failure')
        }
      })
    }
  }

  private async finishClockControlDelivery(
    claim: ClockControlDeliveryClaim,
    accepted: boolean,
    errorCode: string,
  ): Promise<void> {
    const { row } = claim
    const latest = this.clockControl(row.control_id)
    if (!latest || latest.status !== 'pending' || latest.claim_id !== row.claim_id
      || latest.claim_version !== row.claim_version || !sameJson(latest, row)) return
    if (accepted) {
      this.storage.sql.exec(`UPDATE competition_clock_control_outbox
        SET status = 'completed', claim_id = NULL
        WHERE control_id = ? AND status = 'pending' AND claim_id = ? AND claim_version = ?`,
      row.control_id, row.claim_id, row.claim_version)
      return
    }
    if (!safeNonNegativeInteger(row.attempts + 1)) throw new Error('clock control retry counter overflow')
    const next = checkedRoundRetryTime(Date.now(), Math.min(60_000, 1_000 * 2 ** Math.min(row.attempts + 1, 6)))
    this.storage.sql.exec(`UPDATE competition_clock_control_outbox
      SET claim_id = NULL, attempts = attempts + 1, next_attempt_at_ms = ?
      WHERE control_id = ? AND status = 'pending' AND claim_id = ? AND claim_version = ?`,
    next, row.control_id, row.claim_id, row.claim_version)
    console.warn('CompetitionRoom retained clock-control delivery', errorCode)
  }

  private async initializeFromRulesInternal(
    request: CompetitionRoomInitializeFromRulesRequest,
  ): Promise<CompetitionRoomResult<CompetitionRoomInitializeValue>> {
    if (!isRulesInitializeRequest(request)) {
      return roomFailure('invalidRequest', 'rules-derived initialization request is invalid')
    }
    // 固定已驗證的來源，後續 await 不再使用呼叫端可變的物件。
    request = structuredClone(request)
    const current = this.meta()
    if (current) {
      return current.competition_id !== request.competitionId
        ? roomFailure('competitionIdMismatch', 'competition id is bound to this room')
        : roomFailure('alreadyInitialized', 'competition room is already initialized')
    }
    const draft = request.announcement
    let response: CompetitionEngineResponse
    const originalState = emptyServerState()
    try {
      response = await this.engine({
        serverActor: { type: 'organizer', playerId: request.callerUserId },
        trustedTimeMs: request.trustedTimeMs,
        operation: {
          type: 'registrationResolveSettings', ruleVersion: draft.ruleVersion, ruleset: draft.ruleset,
          enabledRuleModules: [...draft.enabledRuleModules], teamSize: draft.teamSize,
        },
        serverState: structuredClone(originalState),
      })
      if (!isInitializeEngineResponse(response, 'registrationResolveSettings')) {
        return roomFailure('engineProtocolError', 'rules settings response is invalid')
      }
      if (response.ok && (!isResolvedCompetitionSettings(response.public)
        || !sameJson(response.serverState, originalState))) {
        return roomFailure('engineProtocolError', 'rules settings response is invalid')
      }
    } catch {
      return roomFailure('engineProtocolError', 'competition engine request failed')
    }
    if (!response.ok) return roomFailure(`engine:${response.error.code}`, response.error.message)
    // 此處只使用規則權威生命；直接走私有 initializer，避免 serialized 公開 RPC 等待自己。
    if (!isResolvedCompetitionSettings(response.public)) {
      return roomFailure('engineProtocolError', 'rules settings response is invalid')
    }
    return this.initializeInternal({
      competitionId: request.competitionId, callerUserId: request.callerUserId,
      trustedTimeMs: request.trustedTimeMs, participantUserIds: [],
      announcement: {
        organizer: request.callerUserId, initialHp: response.public.initialHp,
        ruleVersion: draft.ruleVersion, ruleset: draft.ruleset,
        enabledRuleModules: [...draft.enabledRuleModules], teamSize: draft.teamSize,
        registrationMode: structuredClone(draft.registrationMode), personalDecks: draft.personalDecks,
        registrationDeadline: draft.registrationDeadline, checkInDeadline: draft.checkInDeadline,
        plan: structuredClone(draft.plan),
      },
    }, true)
  }

  private async initializeInternal(
    request: CompetitionRoomInitializeRequest,
    rulesDerived = false,
  ): Promise<CompetitionRoomResult<CompetitionRoomInitializeValue>> {
    const validation = validateInitializeRequest(request)
    if (validation) return roomFailure(validation.code, validation.message)

    const current = this.meta()
    if (current) {
      if (current.competition_id !== request.competitionId) {
        return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
      }
      return roomFailure('alreadyInitialized', 'competition room is already initialized')
    }

    const expectedAnnouncement = rulesDerived ? structuredClone(request.announcement) : null
    let response: CompetitionEngineResponse
    try {
      response = await this.engine({
        serverActor: { type: 'organizer', playerId: request.callerUserId },
        trustedTimeMs: request.trustedTimeMs,
        operation: { type: 'registrationCreate', announcement: rulesDerived
          ? structuredClone(request.announcement) : request.announcement },
        serverState: emptyServerState(),
      })
    } catch {
      return roomFailure('engineProtocolError', 'competition engine request failed')
    }

    if (rulesDerived && !isInitializeEngineResponse(response, 'registrationCreate')) {
      return roomFailure('engineProtocolError', 'registration creation response is invalid')
    }
    if (rulesDerived && response.ok && expectedAnnouncement !== null && (
      !sameJson(response.serverState, {
        ...emptyServerState(), registration: { announcement: expectedAnnouncement, commands: [] },
      }) || !sameJson(response.public, {
        announcement: { ...expectedAnnouncement,
          registrationMode: expectedAnnouncement.registrationMode === 'public' ? 'public' : 'inviteOnly' },
        lifecycle: 'draft', revision: 0, teams: [],
      })
    )) {
      return roomFailure('engineProtocolError', 'registration creation response is invalid')
    }
    if (!response.ok) {
      return roomFailure(`engine:${response.error.code}`, response.error.message)
    }
    if (response.operation !== 'registrationCreate') {
      return roomFailure('engineOperationMismatch', 'competition engine returned another operation')
    }

    const projection = publicProjection(response, 'organizer')
    const serverState = response.serverState
    const revision = 1
    const participantIds = uniqueIds(request.participantUserIds)

    this.storage.transactionSync(() => {
      if (this.meta()) {
        throw new Error('competition room was initialized concurrently')
      }
      this.storage.sql.exec(
        `INSERT INTO competition_meta
          (singleton, competition_id, organizer_user_id, revision, server_state_json,
           public_projection_json, updated_trusted_time_ms)
         VALUES (1, ?, ?, ?, ?, ?, ?)`,
        request.competitionId,
        request.callerUserId,
        revision,
        JSON.stringify(serverState),
        JSON.stringify(projection),
        request.trustedTimeMs,
      )
      this.storage.sql.exec(
        `INSERT INTO competition_acl (user_id, role, active, participant)
         VALUES (?, 'organizer', 1, 0)`,
        request.callerUserId,
      )
      for (const participantId of participantIds) {
        if (participantId === request.callerUserId) continue
        this.storage.sql.exec(
          `INSERT INTO competition_acl (user_id, role, active, participant)
           VALUES (?, 'player', 1, 1)`,
          participantId,
        )
      }
      this.insertAudit({
        actorUserId: request.callerUserId,
        role: 'organizer',
        operationType: 'registrationCreate',
        commandId: null,
        revision,
        outcomeCode: 'success',
        trustedTimeMs: request.trustedTimeMs,
      })
    })

    return roomSuccess({
      competitionId: request.competitionId,
      organizerUserId: request.callerUserId,
      revision,
      projection,
    })
  }

  private async executeInternal(
    request: CompetitionRoomExecuteRequest,
  ): Promise<CompetitionRoomExecuteResult> {
    const validation = validateExecuteRequest(request)
    if (validation) return roomFailure(validation.code, validation.message)

    const current = this.meta()
    if (!current) return roomFailure('notInitialized', 'competition room is not initialized')
    if (current.competition_id !== request.competitionId) {
      return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
    }

    if (isMatchScopedOperation(request.operation)) {
      return roomFailure('operationNotAllowed', 'match operations must use executeMatch')
    }
    if (request.operation.type === 'tournamentCreate') {
      return roomFailure('operationNotAllowed', 'tournamentCreate must use createTournamentFromLockedRoster')
    }
    if (request.operation.type === 'tournamentStartMatch') {
      return roomFailure('operationNotAllowed', 'tournamentStartMatch must use startScheduledMatch')
    }
    if (request.operation.type === 'tournamentSubmitResult'
      || request.operation.type === 'tournamentCorrectResult') {
      return roomFailure(
        'operationNotAllowed',
        'tournament results must come from a completed competition match lifecycle',
      )
    }
    if (isClockOperation(request.operation)) {
      return roomFailure('operationNotAllowed', 'clock operations must use executeMatch')
    }

    if (request.operation.type === 'registrationView'
      || request.operation.type === 'registrationExportLockedRoster') {
      return this.executeRegistrationQuery(request, current)
    }

    const fingerprint = await commandFingerprint(request)
    const receipt = this.commandReceipt(request.commandId)
    if (receipt) {
      if (receipt.receipt_kind !== 'execute' || receipt.fingerprint !== fingerprint) {
        return roomFailure('commandIdConflict', 'command id was used with another payload')
      }
      return parseStoredExecuteResult(receipt.response_json)
    }

    const acl = this.acl(request.callerUserId)
    const canAttemptRegistration = isPlayerRegistrationOperation(request.operation)
    if ((!acl || !acl.active) && !canAttemptRegistration) {
      return roomFailure('notParticipant', 'caller is not an active competition participant')
    }
    if (request.expectedRevision !== current.revision) {
      return roomFailure('revisionConflict', 'expected revision does not match room revision')
    }
    if (request.trustedTimeMs < current.updated_trusted_time_ms) {
      return roomFailure('trustedTimeRegression', 'trusted server time cannot move backwards')
    }
    if (!isKnownExecuteOperation(request.operation)) {
      return roomFailure('invalidOperation', 'operation is not valid for competition execution')
    }
    if (request.operation.type === 'registrationExecute'
      && (request.operation.commandId !== request.commandId
        || request.operation.expectedRevision !== request.expectedRevision)) {
      return roomFailure('commandMismatch', 'registration command metadata does not match room request')
    }
    const role: CompetitionRoomRole = acl?.active ? acl.role : 'player'
    if (role === 'player' && isOfficialOperation(request.operation)) {
      return roomFailure('forbidden', 'player actor cannot execute an official operation')
    }

    const serverActor = actorFor(role, request.callerUserId)
    const serverState = parseServerState(current.server_state_json)
    let operation = request.operation
    let registrationRevision: number | null = null
    if (operation.type === 'registrationExecute') {
      if (!hasExactKeys(operation, ['type', 'commandId', 'expectedRevision', 'action'])) {
        return roomFailure('invalidRequest', 'registration command fields are invalid')
      }
      registrationRevision = registrationCheckpointRevision(serverState.registration, current.organizer_user_id)
      if (registrationRevision === null) {
        return roomFailure('invalidRegistrationCheckpoint', 'stored registration checkpoint is invalid')
      }
      // HTTP revision 屬於整個房間；Rust 子命令只使用權威命令歷史推導的 revision。
      operation = { ...operation, expectedRevision: registrationRevision }
    }
    let response: CompetitionEngineResponse
    try {
      response = await this.engine({
        serverActor,
        trustedTimeMs: request.trustedTimeMs,
        operation,
        serverState,
      })
    } catch {
      return this.persistFailedExecution(
        current,
        request,
        role,
        fingerprint,
        'engineProtocolError',
        'competition engine request failed',
      )
    }

    if (!response.ok) {
      return this.persistFailedExecution(
        current,
        request,
        role,
        fingerprint,
        `engine:${response.error.code}`,
        response.error.message,
      )
    }
    if (response.operation !== request.operation.type) {
      return this.persistFailedExecution(
        current,
        request,
        role,
        fingerprint,
        'engineOperationMismatch',
        'competition engine returned another operation',
      )
    }
    if (!sameMatches(response.serverState.matches, serverState.matches)) {
      return this.persistFailedExecution(
        current,
        request,
        role,
        fingerprint,
        'engineCrossScopeMutation',
        'global execution attempted to change match checkpoints',
      )
    }

    if (operation.type === 'registrationExecute' && registrationRevision !== null) {
      const expectedCheckpoint = {
        announcement: serverState.registration!.announcement,
        commands: [...serverState.registration!.commands as JsonValue[], {
          commandId: operation.commandId, expectedRevision: registrationRevision,
          serverActor, action: operation.action, trustedTimeMs: request.trustedTimeMs,
        }],
      }
      if (registrationCheckpointRevision(response.serverState.registration, current.organizer_user_id)
          !== registrationRevision + 1
        || !sameJson(response.serverState.registration, expectedCheckpoint)
        || !sameJson(response.serverState.clock, serverState.clock)
        || !sameJson(response.serverState.tournament, serverState.tournament)
        || !isJsonObject(response.public) || response.public.revision !== registrationRevision + 1) {
        return this.persistFailedExecution(current, request, role, fingerprint,
          'engineProtocolError', 'registration engine returned an invalid checkpoint')
      }
    }

    let initialSeedingIntent: InitialSeedingIntentRow | null = null
    if (operation.type === 'registrationExecute' && operation.action.type === 'lock') {
      const lockedRoster = await this.exportLockedRosterFromState(
        response.serverState,
        current.organizer_user_id,
        request.trustedTimeMs,
      )
      if (!lockedRoster) {
        return roomFailure('engineProtocolError', 'locked roster export returned an invalid response')
      }
      if (lockedRoster.teamIds.length >= 2) {
        const plan = parseCompetitionAnnouncementPlan(lockedRoster.announcement.plan)
        const announcedStage = plan?.stages[0]
        if (!announcedStage || announcedStage.qualification.type !== 'allEligibleTeams') {
          return roomFailure('engineProtocolError', 'first announced stage is unavailable for automatic seeding')
        }
        let drawn: ReturnType<typeof drawInitialSeedTeams>
        try {
          drawn = drawInitialSeedTeams(lockedRoster.teamIds, this.randomUint32)
        } catch {
          return roomFailure('randomSourceFailure', 'trusted random source failed during initial seeding')
        }
        const stage: StageConfig = {
          stageId: announcedStage.stageId,
          format: announcedStage.format,
          teams: drawn.teams,
          randomSeed: drawn.randomSeed,
          matchFormat: announcedStage.matchFormat,
          clockConfig: announcedStage.clockConfig,
          swissRounds: announcedStage.swissRounds,
          swissPolicy: announcedStage.swissPolicy,
        }
        initialSeedingIntent = {
          competition_id: current.competition_id,
          command_id: `initial-seeding:${crypto.randomUUID()}`,
          stage_id: stage.stageId,
          stage_json: JSON.stringify(stage),
          status: 'pending',
          attempts: 0,
          next_attempt_at_ms: 0,
          last_error: null,
          created_trusted_time_ms: request.trustedTimeMs,
        }
      }
    }

    if (operation.type === 'registrationExecute' && operation.action.type === 'open') {
      const deadline = registrationFreezeDeadline(response.serverState.registration, current.organizer_user_id)
      if (deadline === null) {
        return roomFailure('alarmSchedulingFailed', 'registration freeze deadline is unavailable')
      }
      try {
        // 先確保 F 的持久喚醒，再提交成功收據；失敗時讓相同命令可安全重試。
        await this.scheduleSharedAlarm(response.serverState.registration)
      } catch {
        return roomFailure('alarmSchedulingFailed', 'registration freeze alarm could not be scheduled')
      }
    }

    const mayReceivePrivate = role === 'player'
      && (Boolean(acl?.active) || isRosterMutation(request.operation))
    const projected = publicProjection(response, role, mayReceivePrivate)
    const revision = current.revision + 1
    const storedProjection = stripPrivate(projected)
    const result = roomSuccess<CompetitionRoomExecuteValue>({
      competitionId: request.competitionId,
      revision,
      commandId: request.commandId,
      idempotent: false,
      projection: projected,
    })

    this.storage.transactionSync(() => {
      if (registrationRevision !== null) {
        const latest = this.meta()
        if (!latest || latest.revision !== current.revision
          || latest.server_state_json !== current.server_state_json
          || latest.updated_trusted_time_ms !== current.updated_trusted_time_ms) {
          throw new Error('registration state changed before atomic commit')
        }
      }
      this.storage.sql.exec(
        `UPDATE competition_meta
         SET revision = ?, server_state_json = ?, public_projection_json = ?,
             updated_trusted_time_ms = ?
         WHERE singleton = 1`,
        revision,
        JSON.stringify(response.serverState),
        JSON.stringify(storedProjection),
        request.trustedTimeMs,
      )
      if (initialSeedingIntent) {
        this.storage.sql.exec(
          `INSERT INTO competition_initial_seeding
             (competition_id, command_id, stage_id, stage_json, status, attempts,
              next_attempt_at_ms, last_error, created_trusted_time_ms)
           VALUES (?, ?, ?, ?, 'pending', 0, ?, NULL, ?)`,
          initialSeedingIntent.competition_id,
          initialSeedingIntent.command_id,
          initialSeedingIntent.stage_id,
          initialSeedingIntent.stage_json,
          initialSeedingIntent.next_attempt_at_ms,
          initialSeedingIntent.created_trusted_time_ms,
        )
      }
      if (projected.private !== undefined && mayReceivePrivate) {
        this.storage.sql.exec(
          `INSERT INTO competition_private_projection
             (user_id, payload_json, revision, operation_type)
           VALUES (?, ?, ?, ?)
           ON CONFLICT(user_id) DO UPDATE SET
             payload_json = excluded.payload_json,
             revision = excluded.revision,
             operation_type = excluded.operation_type`,
          request.callerUserId,
          JSON.stringify(projected.private),
          revision,
          projected.operation,
        )
      }
      for (const participantId of participantsFromOperation(request.operation, request.callerUserId)) {
        if (participantId === current.organizer_user_id) continue
        this.storage.sql.exec(
          `INSERT INTO competition_acl (user_id, role, active, participant)
           VALUES (?, 'player', 1, 1)
           ON CONFLICT(user_id) DO UPDATE SET
             active = 1,
             participant = 1,
             role = CASE WHEN competition_acl.role = 'referee' THEN 'referee' ELSE 'player' END`,
          participantId,
        )
      }
      this.insertAudit({
        actorUserId: request.callerUserId,
        role,
        operationType: request.operation.type,
        commandId: request.commandId,
        revision,
        outcomeCode: 'success',
        trustedTimeMs: request.trustedTimeMs,
      })
      this.insertCommandReceipt(request.commandId, 'execute', fingerprint, result)
    })

    return result
  }

  private async executeMatchInternal(
    request: CompetitionRoomMatchExecuteRequest,
    clockEligibility?: ClockEligibilityProbeResult,
  ): Promise<MatchExecuteInternalResult> {
    const validation = validateMatchExecuteRequest(request)
    if (validation) return roomFailure(validation.code, validation.message)
    const current = this.meta()
    if (!current) return roomFailure('notInitialized', 'competition room is not initialized')
    if (current.competition_id !== request.competitionId) {
      return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
    }
    const innerMatchId = matchIdFromOperation(request.operation)
    if (innerMatchId !== request.matchId) {
      return roomFailure('matchIdMismatch', 'outer and inner match ids must match')
    }

    if (request.operation.type === 'matchCreate') {
      return roomFailure('operationNotAllowed', 'matchCreate must use startScheduledMatch')
    }
    if (request.operation.type === 'clockCreate') {
      return roomFailure('operationNotAllowed', 'clockCreate must use startScheduledMatch')
    }
    if (request.operation.type === 'matchAssignFirstPlayer') {
      return roomFailure(
        'operationNotAllowed',
        'first-player assignment must use a policy-specific server-only RPC',
      )
    }
    if (request.operation.type === 'matchSubmitGame'
      || request.operation.type === 'clockRuleGame') {
      return roomFailure('operationNotAllowed', 'terminal results must use ingestGameTerminal')
    }

    const fingerprint = await matchCommandFingerprint(request)
    const receipt = this.commandReceipt(request.commandId)
    if (receipt) {
      if (receipt.receipt_kind !== 'matchExecute' || receipt.match_id !== request.matchId
        || receipt.fingerprint !== fingerprint) {
        return roomFailure('commandIdConflict', 'command id was used with another payload')
      }
      return parseStoredMatchExecuteResult(receipt.response_json)
    }

    const acl = this.acl(request.callerUserId)
    if (!acl || !acl.active || (acl.role !== 'organizer' && acl.role !== 'referee')) {
      return roomFailure('forbidden', 'players and unknown users cannot mutate a match')
    }
    if (isPauseResumeOperation(request.operation)
      && (acl?.role !== 'referee' || !acl.active)) {
      return roomFailure('forbidden', 'only an active referee may pause or resume a match clock')
    }
    if (request.operation.type === 'matchCorrectRuling') {
      if (acl?.role !== 'referee' || request.operation.refereeId !== request.callerUserId) {
        return roomFailure('forbidden', 'only the acting referee may correct a ruling')
      }
    }

    const row = this.match(request.matchId)
    if (!row) return roomFailure('matchNotFound', 'match row does not exist')
    const currentRevision = row?.revision ?? 0
    if (request.expectedMatchRevision !== currentRevision) {
      return roomFailure('matchRevisionConflict', 'expected match revision does not match match revision')
    }
    if (row && request.trustedTimeMs < row.last_trusted_time_ms) {
      return roomFailure('trustedTimeRegression', 'trusted server time cannot move backwards for this match')
    }

    if (isPauseResumeOperation(request.operation) || request.operation.type === 'matchCorrectRuling') {
      const refereeError = await this.validateRefereeOwnMatch(current, row, acl, request.trustedTimeMs)
      if (refereeError) return roomFailure(refereeError.code, refereeError.message)
    }

    let linkedGame: GameProvisionRow | undefined
    if (isPauseResumeOperation(request.operation)) {
      const anyLink = this.latestGameProvision(request.matchId)
      if (anyLink) {
        let currentGameNumber: number | null = null
        try {
          currentGameNumber = currentControllableGameNumber(row, parseServerState(current.server_state_json))
        } catch {
          currentGameNumber = null
        }
        linkedGame = currentGameNumber === null
          ? undefined
          : this.gameProvision(request.matchId, currentGameNumber)
      }
      if (clockEligibility) {
        const currentSnapshot = clockEligibilitySnapshot(
          request, current, acl, row, anyLink, linkedGame,
        )
        if (currentSnapshot !== clockEligibility.snapshot) {
          return roomFailure(
            'gameRoomNotProvisioned',
            'the current match game must be active before its clock can be controlled',
          )
        }
      }
      if (anyLink) {
        if (!linkedGame || linkedGame.status !== 'provisioned') {
          return roomFailure(
            'gameRoomNotProvisioned',
            'the current match game must be provisioned before its clock can be controlled',
          )
        }
      }
    }

    const operation = engineOperation(request.operation)
    const inputCheckpoint = parseJsonObject(row.match_checkpoint_json, 'match checkpoint')
    const inputClock = row?.clock_checkpoint_json === null || row?.clock_checkpoint_json === undefined
      ? null
      : parseJsonObject(row.clock_checkpoint_json, 'clock checkpoint')
    let correctionSchedule: ScheduledMatchMaterialization | null = null
    let correctionTournament: JsonObject | null = null
    let correctionBefore: CompletedMatchMapping | null = null
    if (request.operation.type === 'matchCorrectRuling') {
      try {
        const persistedState = parseServerState(current.server_state_json)
        correctionTournament = persistedState.tournament
        correctionSchedule = findScheduledMatch(correctionTournament, request.matchId, true, true)
        const storedMatchProjection = parseOptionalProjection(row.match_projection_json)
        const beforeMatch = storedMatchProjection?.public
        const beforeSubmission = latestSubmissionFromCheckpoint(inputCheckpoint)
        if (!correctionSchedule || !beforeSubmission || !isJsonObject(inputCheckpoint.config)
          || !isValidMatchFormat(inputCheckpoint.config.format)
          || row.stage_id !== correctionSchedule.stageId
          || inputCheckpoint.config.matchId !== request.matchId
          || !sameJson(inputCheckpoint.config.teams, correctionSchedule.teams)
          || !sameJson(inputCheckpoint.config.format, correctionSchedule.format)
          || !sameJson(persistedState.matches?.[request.matchId], inputCheckpoint)
          || !isJsonObject(beforeMatch) || !isJsonObject(beforeMatch.match)) {
          return roomFailure('scheduleMismatch', 'materialized match does not match its tournament schedule')
        }
        const before = mapCompletedMatchResult(
          beforeMatch.match,
          inputCheckpoint.config,
          inputCheckpoint.config.format,
          inputCheckpoint,
          beforeSubmission,
        )
        if (!before.ok) {
          return roomFailure('invalidMatchCheckpoint', REDACTED_MATCH_CORRECTION_FAILURE_MESSAGE)
        }
        correctionBefore = before.value
        if (correctionSchedule.status === 'completed') {
          if (correctionSchedule.result === null || before.value.status !== 'completed'
            || !sameJson(correctionSchedule.result, before.value.result)) {
            return roomFailure('scheduleMismatch', 'materialized match does not match its tournament schedule')
          }
        } else if (correctionSchedule.status !== 'inProgress' || correctionSchedule.result !== null
          || before.value.status === 'completed') {
          return roomFailure('scheduleMismatch', 'materialized match does not match its tournament schedule')
        }
      } catch {
        return roomFailure('invalidTournamentCheckpoint', REDACTED_MATCH_CORRECTION_FAILURE_MESSAGE)
      }
    }
    if (isPauseResumeOperation(request.operation) && inputCheckpoint) {
      const refereeError = validateClockReferee(request.callerUserId, inputCheckpoint)
      if (refereeError) return roomFailure(refereeError.code, refereeError.message)
      if (linkedGame) {
        const snapshot = clockEligibilitySnapshot(
          request, current, acl, row, this.latestGameProvision(request.matchId), linkedGame,
        )
        if (!clockEligibility) {
          return {
            kind: 'clockEligibilityProbe',
            snapshot,
            request: Object.freeze({
              competitionId: current.competition_id,
              matchId: row.match_id,
              gameId: linkedGame.game_id,
              gameNumber: linkedGame.game_number,
            }),
          }
        }
        // 只接受本次 exact probe 的 primitive true；false／缺少方法／throw 一律 fail closed。
        if (!clockEligibility.eligible) {
          return roomFailure(
            'gameRoomNotProvisioned',
            'the current match game must be active before its clock can be controlled',
          )
        }
      }
    }
    let response: CompetitionEngineResponse
    try {
      response = await this.engine({
        serverActor: actorFor(acl?.role ?? 'organizer', request.callerUserId),
        trustedTimeMs: request.trustedTimeMs,
        operation,
        serverState: {
          registration: null,
          clock: inputClock,
          tournament: null,
          matches: { [request.matchId]: inputCheckpoint },
        },
      })
    } catch {
      return this.persistFailedMatchExecution(
        request,
        acl?.role ?? 'organizer',
        fingerprint,
        'engineProtocolError',
        'competition engine request failed',
        row?.revision ?? 0,
      )
    }
    if (!response.ok) {
      return this.persistFailedMatchExecution(
        request,
        acl?.role ?? 'organizer',
        fingerprint,
        `engine:${response.error.code}`,
        request.operation.type === 'matchCorrectRuling'
          ? REDACTED_MATCH_CORRECTION_FAILURE_MESSAGE
          : response.error.message,
        row?.revision ?? 0,
      )
    }
    if (response.operation !== operation.type) {
      return this.persistFailedMatchExecution(
        request,
        acl?.role ?? 'organizer',
        fingerprint,
        'engineOperationMismatch',
        'competition engine returned another operation',
        row?.revision ?? 0,
      )
    }

    if (request.operation.type === 'matchCorrectRuling'
      && (!hasExactKeys(response as unknown as JsonObject, ['ok', 'operation', 'public', 'serverState'])
        || !isJsonObject(response.public) || !hasExactKeys(response.public, ['match'])
        || !isJsonObject(response.serverState)
        || !hasExactKeys(response.serverState, ['registration', 'clock', 'tournament', 'matches']))) {
      return this.persistFailedMatchExecution(
        request, acl?.role ?? 'organizer', fingerprint,
        'engineInvalidMatchCheckpoint', REDACTED_MATCH_CORRECTION_FAILURE_MESSAGE, currentRevision,
      )
    }

    const stateError = validateScopedEngineState(
      response.serverState,
      request.matchId,
      inputCheckpoint,
      inputClock,
      request.operation,
    )
    if (stateError) {
      return this.persistFailedMatchExecution(
        request,
        acl?.role ?? 'organizer',
        fingerprint,
        request.operation.type === 'matchCorrectRuling' ? 'engineInvalidMatchCheckpoint' : stateError.code,
        request.operation.type === 'matchCorrectRuling'
          ? REDACTED_MATCH_CORRECTION_FAILURE_MESSAGE
          : stateError.message,
        row?.revision ?? 0,
      )
    }

    let timeoutEvidence: Extract<GameTerminalEvidence, { type: 'timeout' }> | null = null
    let nextClockDeadlineMs: number | null = null
    if (isClockOperation(request.operation)) {
      const parsedUpdate = parseClockMutationResult(
        response.public,
        response.serverState.clock,
        isJsonObject(inputCheckpoint.config) && Array.isArray(inputCheckpoint.config.teams)
          ? inputCheckpoint.config.teams as string[]
          : [],
      )
      if (!parsedUpdate.ok) {
        return this.persistFailedMatchExecution(
          request,
          acl?.role ?? 'organizer',
          fingerprint,
          parsedUpdate.error.code,
          parsedUpdate.error.message,
          row?.revision ?? 0,
        )
      }
      nextClockDeadlineMs = parsedUpdate.value.nextTimeoutDeadlineMs
      const timeout = await deriveClockTimeoutEvidence(
        parsedUpdate.value,
        inputClock,
        response.serverState.clock,
        request.competitionId,
        request.matchId,
        request.commandId,
        currentRevision + 1,
        isJsonObject(inputCheckpoint.config) && Array.isArray(inputCheckpoint.config.teams)
          && inputCheckpoint.config.teams.every(team => nonEmpty(team))
          ? inputCheckpoint.config.teams as string[]
          : [],
      )
      if (!timeout.ok) {
        return this.persistFailedMatchExecution(
          request,
          acl?.role ?? 'organizer',
          fingerprint,
          timeout.error.code,
          timeout.error.message,
          row?.revision ?? 0,
        )
      }
      timeoutEvidence = timeout.value
    }

    const projection = publicProjection(response, acl?.role ?? 'organizer')
    const nextMatches = response.serverState.matches ?? {}
    const nextMatch = nextMatches[request.matchId]
    if (!isJsonObject(nextMatch)) {
      return this.persistFailedMatchExecution(
        request,
        acl?.role ?? 'organizer',
        fingerprint,
        'engineInvalidMatchCheckpoint',
        'scoped engine returned an invalid match checkpoint',
        row?.revision ?? 0,
      )
    }
    let nextTournament = correctionTournament
    let tournamentProjection: PublicCompetitionSuccess | null = null
    let tournamentRevision: number | null = null
    let tournamentApplyTimeMs: number | null = null
    if (request.operation.type === 'matchCorrectRuling') {
      const afterPublic = isJsonObject(response.public) ? response.public.match : null
      const afterSubmission = latestSubmissionFromCheckpoint(nextMatch)
      if (!correctionSchedule || !correctionTournament || !correctionBefore
        || !isJsonObject(inputCheckpoint.config) || !isValidMatchFormat(inputCheckpoint.config.format)
        || !isJsonObject(afterPublic) || !afterSubmission
        || !isValidMatchCorrectionCheckpoint(inputCheckpoint, nextMatch, request.operation)) {
        return this.persistFailedMatchExecution(
          request, acl?.role ?? 'organizer', fingerprint,
          'engineInvalidMatchResult', REDACTED_MATCH_CORRECTION_FAILURE_MESSAGE, currentRevision,
        )
      }
      const after = mapCompletedMatchResult(
        afterPublic,
        inputCheckpoint.config,
        inputCheckpoint.config.format,
        nextMatch,
        afterSubmission,
      )
      if (!after.ok) {
        return this.persistFailedMatchExecution(
          request, acl?.role ?? 'organizer', fingerprint,
          after.code, REDACTED_MATCH_CORRECTION_FAILURE_MESSAGE, currentRevision,
        )
      }
      if (correctionSchedule.result !== null && after.value.status !== 'completed') {
        return this.persistFailedMatchExecution(
          request, acl?.role ?? 'organizer', fingerprint,
          'correctionWouldReopenScheduledResult', REDACTED_MATCH_CORRECTION_FAILURE_MESSAGE, currentRevision,
        )
      }

      let tournamentOperation: Extract<CompetitionOperation, { type: 'tournamentCorrectResult' | 'tournamentSubmitResult' }> | null = null
      let expectedTournamentResult: TournamentMatchResult | null = null
      let expectedTournamentOperation: 'tournamentCorrectResult' | 'tournamentSubmitResult' | null = null
      if (after.value.status === 'completed') {
        expectedTournamentResult = after.value.result
        expectedTournamentOperation = correctionSchedule.result === null
          ? 'tournamentSubmitResult'
          : 'tournamentCorrectResult'
        tournamentOperation = correctionSchedule.result === null
          ? {
              type: 'tournamentSubmitResult',
              stageId: correctionSchedule.stageId,
              matchId: request.matchId,
              result: after.value.result,
            }
          : {
              type: 'tournamentCorrectResult',
              stageId: correctionSchedule.stageId,
              matchId: request.matchId,
              replacement: after.value.result,
              reason: request.operation.reason,
            }
      }
      if (tournamentOperation && expectedTournamentResult && expectedTournamentOperation) {
        tournamentApplyTimeMs = Math.max(request.trustedTimeMs, current.updated_trusted_time_ms)
        let tournamentResponse: CompetitionEngineResponse
        try {
          tournamentResponse = await this.engine({
            serverActor: actorFor('referee', request.callerUserId),
            trustedTimeMs: tournamentApplyTimeMs,
            operation: tournamentOperation,
            serverState: {
              registration: null,
              clock: null,
              tournament: correctionTournament,
              matches: {},
            },
          })
        } catch {
          return this.persistFailedMatchExecution(
            request, acl?.role ?? 'organizer', fingerprint,
            'engineProtocolError', REDACTED_MATCH_CORRECTION_FAILURE_MESSAGE, currentRevision,
          )
        }
        if (!tournamentResponse.ok) {
          return this.persistFailedMatchExecution(
            request, acl?.role ?? 'organizer', fingerprint,
            `engine:${tournamentResponse.error.code}`, REDACTED_MATCH_CORRECTION_FAILURE_MESSAGE, currentRevision,
          )
        }
        const tournamentError = validateTournamentCorrectionUpdate(
          tournamentResponse,
          correctionTournament,
          correctionSchedule,
          expectedTournamentResult,
          expectedTournamentOperation,
        )
        if (tournamentError) {
          return this.persistFailedMatchExecution(
            request, acl?.role ?? 'organizer', fingerprint,
            tournamentError, REDACTED_MATCH_CORRECTION_FAILURE_MESSAGE, currentRevision,
          )
        }
        nextTournament = tournamentResponse.serverState.tournament
        tournamentProjection = publicProjection(tournamentResponse, acl?.role ?? 'organizer')
        tournamentRevision = current.revision + 1
      }
    }
    const nextClock = response.serverState.clock
    const nextMatchProjection = isMatchMutation(request.operation)
      ? projection
      : parseOptionalProjection(row?.match_projection_json ?? null)
    const nextClockProjection = isClockOperation(request.operation)
      ? projection
      : parseOptionalProjection(row?.clock_projection_json ?? null)
    if (!isClockOperation(request.operation) && nextClockProjection) {
      nextClockDeadlineMs = storedClockProjectionDeadline(nextClockProjection)
    }
    const nextRevision = currentRevision + 1
    const result = roomSuccess<CompetitionRoomMatchExecuteValue>({
      competitionId: request.competitionId,
      matchId: request.matchId,
      matchRevision: nextRevision,
      commandId: request.commandId,
      idempotent: false,
      timeoutEvidence,
      nextClockDeadlineMs,
      matchProjection: nextMatchProjection,
      clockProjection: nextClockProjection,
    })

    const clockControl: CompetitionClockControlRequest | undefined = linkedGame
      && isPauseResumeOperation(request.operation)
      ? {
          controlId: await clockControlIdentity({
            competitionId: request.competitionId,
            matchId: request.matchId,
            gameId: linkedGame.game_id,
            gameNumber: linkedGame.game_number,
            sourceCommandId: request.commandId,
            matchRevision: nextRevision,
          }),
          competitionId: request.competitionId,
          matchId: request.matchId,
          gameId: linkedGame.game_id,
          gameNumber: linkedGame.game_number,
          sourceCommandId: request.commandId,
          matchRevision: nextRevision,
          operation: request.operation.type,
          trustedTimeMs: request.trustedTimeMs,
          nextClockDeadlineMs,
        }
      : undefined

    this.storage.transactionSync(() => {
      const meta = this.meta()
      if (!meta) throw new Error('competition room disappeared during match mutation')
      const globalState = parseServerState(meta.server_state_json)
      const globalMatches = globalState.matches ?? {}
      globalMatches[request.matchId] = nextMatch
      globalState.matches = globalMatches
      if (request.operation.type === 'matchCorrectRuling' && tournamentRevision !== null) {
        globalState.tournament = nextTournament
      }
      this.storage.sql.exec(
        `UPDATE competition_matches
         SET match_checkpoint_json = ?, clock_checkpoint_json = ?,
             match_projection_json = ?, clock_projection_json = ?,
             revision = ?, last_trusted_time_ms = ?,
             tournament_result_confirmed_at_ms = CASE WHEN ? IS NULL
               THEN tournament_result_confirmed_at_ms ELSE ? END
         WHERE match_id = ?`,
        JSON.stringify(nextMatch),
        nextClock ? JSON.stringify(nextClock) : null,
        nextMatchProjection ? JSON.stringify(nextMatchProjection) : null,
        nextClockProjection ? JSON.stringify(nextClockProjection) : null,
        nextRevision,
        request.trustedTimeMs,
        tournamentApplyTimeMs,
        tournamentApplyTimeMs,
        request.matchId,
      )
      this.storage.sql.exec(
        `UPDATE competition_meta
         SET server_state_json = ?,
             revision = CASE WHEN ? IS NULL THEN revision ELSE ? END,
             public_projection_json = CASE WHEN ? IS NULL THEN public_projection_json ELSE ? END,
             updated_trusted_time_ms = CASE WHEN ? IS NULL THEN updated_trusted_time_ms ELSE ? END
         WHERE singleton = 1`,
        JSON.stringify(globalState),
        tournamentRevision,
        tournamentRevision,
        tournamentProjection ? 1 : null,
        tournamentProjection ? JSON.stringify(stripPrivate(tournamentProjection)) : null,
        tournamentRevision,
        tournamentApplyTimeMs,
      )
      this.insertAudit({
        actorUserId: request.callerUserId,
        role: acl?.role ?? 'organizer',
        matchId: request.matchId,
        operationType: request.operation.type,
        commandId: request.commandId,
        revision: nextRevision,
        outcomeCode: 'success',
        trustedTimeMs: request.trustedTimeMs,
        reason: operationReason(request.operation),
      })
      this.insertCommandReceipt(request.commandId, 'matchExecute', fingerprint, result, request.matchId)
      if (clockControl) {
        this.storage.sql.exec(
          `INSERT INTO competition_clock_control_outbox
            (control_id, competition_id, match_id, game_id, game_number, source_command_id,
             match_revision, operation, trusted_time_ms, next_clock_deadline_ms, attempts,
             next_attempt_at_ms, status)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, 'pending')`,
          clockControl.controlId,
          clockControl.competitionId,
          clockControl.matchId,
          clockControl.gameId,
          clockControl.gameNumber,
          clockControl.sourceCommandId,
          clockControl.matchRevision,
          clockControl.operation,
          clockControl.trustedTimeMs,
          clockControl.nextClockDeadlineMs,
          Date.now(),
        )
      }
    })
    return result
  }

  private async ingestGameTerminalInternal(
    request: CompetitionRoomTerminalIngestRequest,
  ): Promise<CompetitionRoomTerminalIngestResult> {
    const validation = validateTerminalIngestRequest(request)
    if (validation) return roomFailure(validation.code, validation.message)

    const current = this.meta()
    if (!current) return roomFailure('notInitialized', 'competition room is not initialized')
    if (current.competition_id !== request.competitionId) {
      return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
    }

    const fingerprint = await terminalIngestFingerprint(request)
    // 收據必須先於 revision、trusted time 與 row 存在性檢查，讓跨重試只回原結果。
    const receipt = this.commandReceipt(request.commandId)
    if (receipt) {
      if (receipt.receipt_kind !== 'ingestGameTerminal'
        || receipt.match_id !== request.matchId
        || receipt.fingerprint !== fingerprint) {
        return roomFailure('commandIdConflict', 'command id was used with another payload')
      }
      const stored = parseStoredMatchExecuteResult(receipt.response_json, true)
      // strict ACK 使用原指令的身分與提交後 revision，不可信任損壞的收據值。
      if (stored.ok && (stored.value.competitionId !== request.competitionId
        || stored.value.matchId !== request.matchId
        || stored.value.commandId !== request.commandId
        || stored.value.matchRevision !== request.expectedMatchRevision + 1)) {
        throw new Error('stored terminal receipt contradicts its request')
      }
      if (stored.ok) await this.validateAutomaticDrawProvisionLinkage(request, stored.value)
      return stored
    }

    const caller = this.acl(request.callerUserId)
    if (!caller || !caller.active || caller.role !== 'organizer') {
      return roomFailure('forbidden', 'only the active organizer may ingest a game terminal')
    }

    const row = this.match(request.matchId)
    if (!row) return roomFailure('matchNotFound', 'match row does not exist')
    if (request.expectedMatchRevision !== row.revision) {
      return roomFailure('matchRevisionConflict', 'expected match revision does not match match revision')
    }
    if (request.trustedTimeMs < row.last_trusted_time_ms) {
      return roomFailure('trustedTimeRegression', 'trusted server time cannot move backwards for this match')
    }
    const inputCheckpoint = parseJsonObject(row.match_checkpoint_json, 'match checkpoint')
    const inputClock = row.clock_checkpoint_json === null || row.clock_checkpoint_json === undefined
      ? null
      : parseJsonObject(row.clock_checkpoint_json, 'clock checkpoint')
    const structureError = validateTerminalIngestStructure(request, inputCheckpoint, inputClock)
    if (structureError) return roomFailure(structureError.code, structureError.message)

    const currentServerState = parseServerState(current.server_state_json)
    let scheduledMatch: ScheduledMatchMaterialization | null
    try {
      scheduledMatch = findScheduledMatch(currentServerState.tournament, request.matchId)
    } catch {
      return roomFailure('invalidTournamentCheckpoint', 'tournament checkpoint is malformed')
    }
    if (!scheduledMatch
      || row.stage_id !== scheduledMatch.stageId
      || scheduledMatch.status !== 'inProgress'
      || !scheduledMatch.started
      || !isJsonObject(inputCheckpoint.config)
      || inputCheckpoint.config.matchId !== request.matchId
      || !Array.isArray(inputCheckpoint.config.teams)
      || !sameJson(inputCheckpoint.config.teams, scheduledMatch.teams)
      || !sameJson(inputCheckpoint.config.format, scheduledMatch.format)
      || !sameJson(currentServerState.matches?.[request.matchId], inputCheckpoint)) {
      return roomFailure('scheduleMismatch', 'materialized match does not match its tournament schedule')
    }

    const matchFormat = inputCheckpoint.config as JsonObject
    const format = matchFormat.format
    const matchTeams = matchFormat.teams as string[]
    const mode: GameResultMode = isJsonObject(format) && format.type === 'fixedGames'
      ? 'retainDraw'
      : 'elimination'
    let ruling: CompetitionRuling
    let nextClock: JsonObject | null = inputClock
    let nextClockProjection = parseOptionalProjection(row.clock_projection_json)

    if (request.terminalEvidence.type === 'canonicalGameEnded') {
      const clockOperation: CompetitionRoomMatchOperation = {
        type: 'clockRuleGame',
        matchId: request.matchId,
        conclusion: request.terminalEvidence.conclusion,
        mode,
      }
      let clockResponse: CompetitionEngineResponse
      try {
        clockResponse = await this.engine({
          serverActor: actorFor('organizer', request.callerUserId),
          trustedTimeMs: request.trustedTimeMs,
          operation: engineOperation(clockOperation),
          serverState: {
            registration: null,
            clock: inputClock,
            tournament: null,
            matches: { [request.matchId]: inputCheckpoint },
          },
        })
      } catch {
        // adapter 的暫時性協定錯誤不可寫入收據，否則同 commandId 的重試會被黏住。
        return roomFailure('engineProtocolError', 'competition engine request failed')
      }
      if (!clockResponse.ok) {
        return this.persistFailedTerminalIngest(
          request,
          fingerprint,
          `engine:${clockResponse.error.code}`,
          row.revision,
        )
      }
      if (clockResponse.operation !== 'clockRuleGame') {
        return this.persistFailedTerminalIngest(
          request,
          fingerprint,
          'engineOperationMismatch',
          row.revision,
        )
      }
      const clockStateError = validateScopedEngineState(
        clockResponse.serverState,
        request.matchId,
        inputCheckpoint,
        inputClock,
        clockOperation,
      )
      if (clockStateError) {
        return this.persistFailedTerminalIngest(
          request,
          fingerprint,
          clockStateError.code,
          row.revision,
        )
      }
      const clockPublicError = isJsonObject(clockResponse.public)
        ? validateClockPublicView(clockResponse.public.clock, clockResponse.serverState.clock, matchTeams)
        : { code: 'engineInvalidRuling', message: 'clockRuleGame public result is missing' }
      if (clockPublicError) {
        return this.persistFailedTerminalIngest(
          request,
          fingerprint,
          clockPublicError.code,
          row.revision,
        )
      }
      const publicResult = parseClockRuleGameResult(clockResponse.public, matchTeams)
      if (!publicResult.ok) {
        return this.persistFailedTerminalIngest(
          request,
          fingerprint,
          publicResult.error.code,
          row.revision,
        )
      }
      ruling = publicResult.value
      nextClock = clockResponse.serverState.clock
      nextClockProjection = publicProjection(clockResponse, 'organizer')
    } else {
      const timeoutResult = await deriveTimeoutRuling(request.terminalEvidence, inputClock as JsonObject)
      if (!timeoutResult.ok) return roomFailure(timeoutResult.error.code, timeoutResult.error.message)
      ruling = timeoutResult.value
    }

    const submission = {
      submissionId: request.submissionId,
      gameRecordId: request.gameRecordId,
      rulingRecordId: request.rulingRecordId,
      startKind: request.startKind,
      firstPlayer: request.firstPlayer,
      terminalEvidence: request.terminalEvidence,
      ruling,
    }
    const submitOperation: CompetitionRoomMatchOperation = {
      type: 'matchSubmitGame',
      matchId: request.matchId,
      submission,
    }
    let submitResponse: CompetitionEngineResponse
    try {
      submitResponse = await this.engine({
        serverActor: actorFor('organizer', request.callerUserId),
        trustedTimeMs: request.trustedTimeMs,
        operation: engineOperation(submitOperation),
        serverState: {
          registration: null,
          clock: nextClock,
          tournament: null,
          matches: { [request.matchId]: inputCheckpoint },
        },
      })
    } catch {
      // 第二段 adapter 的暫時性協定錯誤同樣不可形成永久失敗收據。
      return roomFailure('engineProtocolError', 'competition engine request failed')
    }
    if (!submitResponse.ok) {
      return this.persistFailedTerminalIngest(
        request,
        fingerprint,
        `engine:${submitResponse.error.code}`,
        row.revision,
      )
    }
    if (submitResponse.operation !== 'matchSubmitGame') {
      return this.persistFailedTerminalIngest(
        request,
        fingerprint,
        'engineOperationMismatch',
        row.revision,
      )
    }
    const submitStateError = validateScopedEngineState(
      submitResponse.serverState,
      request.matchId,
      inputCheckpoint,
      nextClock,
      submitOperation,
    )
    if (submitStateError) {
      return this.persistFailedTerminalIngest(
        request,
        fingerprint,
        submitStateError.code,
        row.revision,
      )
    }
    const nextMatch = submitResponse.serverState.matches?.[request.matchId]
    if (!isJsonObject(nextMatch) || !isJsonObject(submitResponse.public)
      || !isJsonObject(submitResponse.public.match)) {
      return this.persistFailedTerminalIngest(
        request,
        fingerprint,
        'engineInvalidMatchCheckpoint',
        row.revision,
      )
    }
    const matchProjection = publicProjection(submitResponse, 'organizer')
    const mapped = mapCompletedMatchResult(
      submitResponse.public.match,
      inputCheckpoint.config,
      scheduledMatch.format,
      nextMatch,
      submission,
    )
    if (!mapped.ok) {
      return this.persistFailedTerminalIngest(
        request,
        fingerprint,
        mapped.code,
        row.revision,
      )
    }

    let nextTournament = currentServerState.tournament
    let tournamentProjection: PublicCompetitionSuccess | null = null
    let nextTournamentRevision: number | null = null
    let tournamentApplyTimeMs: number | null = null
    if (mapped.value.status === 'completed') {
      // outbox 的 match 時間可能早於其他全域操作；套用賽程時只使用不倒退的伺服器時間。
      tournamentApplyTimeMs = Math.max(request.trustedTimeMs, current.updated_trusted_time_ms)
      let tournamentResponse: CompetitionEngineResponse
      try {
        tournamentResponse = await this.engine({
          serverActor: actorFor('organizer', request.callerUserId),
          trustedTimeMs: tournamentApplyTimeMs,
          operation: {
            type: 'tournamentSubmitResult',
            stageId: scheduledMatch.stageId,
            matchId: request.matchId,
            result: mapped.value.result,
          },
          serverState: {
            registration: null,
            clock: null,
            tournament: currentServerState.tournament,
            matches: {},
          },
        })
      } catch {
        return roomFailure('engineProtocolError', 'competition engine request failed')
      }
      if (!tournamentResponse.ok) {
        return this.persistFailedTerminalIngest(
          request,
          fingerprint,
          `engine:${tournamentResponse.error.code}`,
          row.revision,
        )
      }
      const tournamentError = validateTournamentSubmitResponse(
        tournamentResponse,
        currentServerState.tournament,
        scheduledMatch,
        mapped.value.result,
      )
      if (tournamentError) {
        return this.persistFailedTerminalIngest(
          request,
          fingerprint,
          tournamentError,
          row.revision,
        )
      }
      nextTournament = tournamentResponse.serverState.tournament
      tournamentProjection = publicProjection(tournamentResponse, 'organizer')
      nextTournamentRevision = current.revision + 1
    }
    const nextRevision = row.revision + 1
    const result = roomSuccess<CompetitionRoomMatchExecuteValue>({
      competitionId: request.competitionId,
      matchId: request.matchId,
      matchRevision: nextRevision,
      commandId: request.commandId,
      idempotent: false,
      timeoutEvidence: null,
      nextClockDeadlineMs: null,
      matchProjection,
      clockProjection: nextClockProjection,
    })

    const automaticProvision = await this.prepareAutomaticDrawProvision({
      request,
      current,
      row,
      currentServerState,
      nextMatch,
      matchProjection,
      nextClock,
      nextClockProjection,
      nextRevision,
      nextTournament,
    })
    if (!automaticProvision.ok) return automaticProvision.result

    this.storage.transactionSync(() => {
      const meta = this.meta()
      const latestRow = this.match(request.matchId)
      if (!meta || meta.competition_id !== current.competition_id
        || meta.organizer_user_id !== current.organizer_user_id
        || meta.revision !== current.revision
        || meta.updated_trusted_time_ms !== current.updated_trusted_time_ms
        || meta.server_state_json !== current.server_state_json
        || !latestRow || latestRow.revision !== row.revision
        || latestRow.last_trusted_time_ms !== row.last_trusted_time_ms
        || latestRow.match_checkpoint_json !== row.match_checkpoint_json
        || latestRow.match_projection_json !== row.match_projection_json
        || latestRow.clock_checkpoint_json !== row.clock_checkpoint_json
        || latestRow.clock_projection_json !== row.clock_projection_json) {
        throw new Error('competition state changed during terminal ingest')
      }
      if (automaticProvision.value) {
        const prior = this.gameProvision(request.matchId, automaticProvision.value.gameNumber - 1)
        if (!prior || this.gameProvision(request.matchId, automaticProvision.value.gameNumber)) {
          throw new Error('automatic next-game linkage changed before commit')
        }
      }
      const globalState = parseServerState(meta.server_state_json)
      const globalMatches = globalState.matches ?? {}
      globalMatches[request.matchId] = nextMatch
      globalState.matches = globalMatches
      globalState.tournament = nextTournament
      this.storage.sql.exec(
        `UPDATE competition_matches
         SET match_checkpoint_json = ?, clock_checkpoint_json = ?,
             match_projection_json = ?, clock_projection_json = ?,
             revision = ?, last_trusted_time_ms = ?,
             tournament_result_confirmed_at_ms = CASE WHEN ? IS NULL
               THEN tournament_result_confirmed_at_ms ELSE ? END
         WHERE match_id = ?`,
        JSON.stringify(nextMatch),
        nextClock ? JSON.stringify(nextClock) : null,
        JSON.stringify(matchProjection),
        nextClockProjection ? JSON.stringify(nextClockProjection) : null,
        nextRevision,
        request.trustedTimeMs,
        tournamentApplyTimeMs,
        tournamentApplyTimeMs,
        request.matchId,
      )
      this.storage.sql.exec(
        `UPDATE competition_meta
         SET server_state_json = ?,
             revision = CASE WHEN ? IS NULL THEN revision ELSE ? END,
             public_projection_json = CASE WHEN ? IS NULL THEN public_projection_json ELSE ? END,
             updated_trusted_time_ms = CASE WHEN ? IS NULL THEN updated_trusted_time_ms ELSE ? END
         WHERE singleton = 1`,
        JSON.stringify(globalState),
        nextTournamentRevision,
        nextTournamentRevision,
        tournamentProjection ? 1 : null,
        tournamentProjection ? JSON.stringify(stripPrivate(tournamentProjection)) : null,
        nextTournamentRevision,
        tournamentApplyTimeMs,
      )
      this.insertAudit({
        actorUserId: request.callerUserId,
        role: 'organizer',
        matchId: request.matchId,
        operationType: 'ingestGameTerminal',
        commandId: request.commandId,
        revision: nextRevision,
        outcomeCode: 'success',
        trustedTimeMs: request.trustedTimeMs,
      })
      this.insertCommandReceipt(
        request.commandId,
        'ingestGameTerminal',
        fingerprint,
        result,
        request.matchId,
      )
      if (automaticProvision.value) {
        this.insertScheduledGameProvisionLinkage({
          matchId: request.matchId,
          gameNumber: automaticProvision.value.gameNumber,
          gameId: automaticProvision.value.gameId,
          binding: automaticProvision.value.binding,
          matchRevision: nextRevision,
          nextGame: automaticProvision.value.nextGame,
          commandId: automaticProvision.value.commandId,
          fingerprint: automaticProvision.value.fingerprint,
          trustedTimeMs: request.trustedTimeMs,
        })
        this.insertAudit({
          actorUserId: current.organizer_user_id,
          role: 'organizer',
          matchId: request.matchId,
          operationType: 'provisionAssignedScheduledGame',
          commandId: automaticProvision.value.commandId,
          revision: nextRevision,
          outcomeCode: 'pending',
          trustedTimeMs: request.trustedTimeMs,
          reason: automaticProvision.value.kind === 'ordinaryDraw'
            ? '普通局和局後自動建立下一局'
            : '淘汰賽加賽和局後自動建立下一局',
        })
      }
    })
    return result
  }

  private async prepareAutomaticDrawProvision(input: {
    request: CompetitionRoomTerminalIngestRequest
    current: CompetitionMetaRow
    row: MatchRow
    currentServerState: CompetitionServerState
    nextMatch: JsonObject
    matchProjection: CompetitionRoomPublicProjection
    nextClock: JsonObject | null
    nextClockProjection: CompetitionRoomPublicProjection | null
    nextRevision: number
    nextTournament: JsonObject | null
  }): Promise<{ ok: true; value: AutomaticDrawProvision | null }
    | { ok: false; result: CompetitionRoomTerminalIngestResult }> {
    const projectionPublic = input.matchProjection.public
    if (!isJsonObject(projectionPublic) || !isJsonObject(projectionPublic.match)) {
      return { ok: false, result: roomFailure('engineInvalidMatchResult', 'match projection is invalid') }
    }
    const continuation = classifyAutomaticContinuation(
      input.request,
      input.nextMatch,
      input.matchProjection,
    )
    if (continuation === false) return { ok: true, value: null }
    if (continuation === null) {
      return { ok: false, result: roomFailure('engineInvalidMatchResult', 'automatic continuation is invalid') }
    }

    const rawPublicMatch = projectionPublic.match
    if (!isJsonObject(rawPublicMatch) || !isJsonObject(rawPublicMatch.nextGame)) {
      return { ok: false, result: roomFailure('engineInvalidMatchResult', 'match result is invalid') }
    }
    const gameNumber = rawPublicMatch.nextGame.gameNumber
    if (typeof gameNumber !== 'number' || !Number.isSafeInteger(gameNumber) || gameNumber < 2) {
      return { ok: false, result: roomFailure('engineInvalidMatchResult', 'match continuation is invalid') }
    }
    if (!this.gameProvision(input.request.matchId, gameNumber - 1)) {
      return { ok: false, result: roomFailure('gameNotProvisionable', 'previous scheduled game is not linked') }
    }
    if (this.gameProvision(input.request.matchId, gameNumber)) {
      return { ok: false, result: roomFailure('gameAlreadyLinked', 'next scheduled game is already linked') }
    }

    const nextRow: MatchRow = {
      ...input.row,
      match_checkpoint_json: JSON.stringify(input.nextMatch),
      clock_checkpoint_json: input.nextClock ? JSON.stringify(input.nextClock) : null,
      match_projection_json: JSON.stringify(input.matchProjection),
      clock_projection_json: input.nextClockProjection ? JSON.stringify(input.nextClockProjection) : null,
      revision: input.nextRevision,
      last_trusted_time_ms: input.request.trustedTimeMs,
    }
    const nextMatches = { ...(input.currentServerState.matches ?? {}), [input.request.matchId]: input.nextMatch }
    const nextState = { ...input.currentServerState, matches: nextMatches, tournament: input.nextTournament }
    const intent = deriveProvisionIntent(nextRow, nextState, null, null)
    const expectedKind: GameStartKind = continuation.kind === 'ordinaryDraw'
      ? { type: 'ordinaryNewGame' }
      : { type: 'eliminationDrawContinuation', continuation: continuation.continuation }
    if (!intent.ok || intent.value.gameNumber !== gameNumber
      || !sameJson(intent.value.startKind, expectedKind)) {
      return { ok: false, result: roomFailure('engineInvalidMatchResult', 'automatic continuation kind is invalid') }
    }

    let exportResponse: CompetitionEngineResponse
    try {
      exportResponse = await this.engine({
        serverActor: actorFor('organizer', input.request.callerUserId),
        trustedTimeMs: input.request.trustedTimeMs,
        operation: { type: 'registrationExportLockedRoster' },
        serverState: nextState,
      })
    } catch {
      return { ok: false, result: roomFailure('engineProtocolError', 'locked roster export request failed') }
    }
    if (!exportResponse.ok || exportResponse.operation !== 'registrationExportLockedRoster'
      || !hasExactKeys(exportResponse as unknown as JsonObject, ['ok', 'operation', 'public', 'private', 'serverState'])
      || !sameJson(exportResponse.serverState, nextState)
      || !isJsonObject(exportResponse.public)
      || !hasExactKeys(exportResponse.public, ['exported'])
      || exportResponse.public.exported !== true) {
      return { ok: false, result: roomFailure('engineProtocolError', 'locked roster export returned an invalid response') }
    }
    const roundContext = this.publishedRoundBindingContext(input.request.matchId)
    if (!roundContext) {
      return { ok: false, result: roomFailure('roundNotPublishable', 'the scheduled match has no published round binding') }
    }
    const provision = await deriveScheduledGameProvision(
      exportResponse.private,
      input.request.competitionId,
      input.request.matchId,
      input.current.organizer_user_id,
      input.nextRevision,
      intent.value,
      roundContext,
    )
    if (!provision.ok) {
      return { ok: false, result: roomFailure(provision.code, provision.message) }
    }
    const fingerprint = await automaticContinuationProvisionFingerprint({
      terminalCommandId: input.request.commandId,
      competitionId: input.request.competitionId,
      matchId: input.request.matchId,
      gameNumber,
      kind: continuation.kind,
    })
    return {
      ok: true,
      value: {
        gameNumber,
        kind: continuation.kind,
        gameId: provision.value.gameId,
        binding: provision.value.binding,
        nextGame: intent.value.nextGame,
        commandId: `auto-draw-provision:${fingerprint}`,
        fingerprint,
      },
    }
  }

  private async validateAutomaticDrawProvisionLinkage(
    request: CompetitionRoomTerminalIngestRequest,
    result: CompetitionRoomMatchExecuteValue,
  ): Promise<void> {
    if (!result.matchProjection || !isJsonObject(result.matchProjection.public)
      || !isJsonObject(result.matchProjection.public.match)) return
    const continuation = classifyAutomaticContinuation(
        request,
        result.matchProjection.public.match,
        result.matchProjection,
      )
    if (continuation === false) return
    if (continuation === null) throw new Error('stored automatic continuation is malformed')
    const match = result.matchProjection.public.match
    if (!isJsonObject(match.nextGame) || typeof match.nextGame.gameNumber !== 'number') {
      throw new Error('stored automatic draw continuation is malformed')
    }
    const gameNumber = match.nextGame.gameNumber
    const fingerprint = await automaticContinuationProvisionFingerprint({
      terminalCommandId: request.commandId,
      competitionId: request.competitionId,
      matchId: request.matchId,
      gameNumber,
      kind: continuation.kind,
    })
    const linkage = this.gameProvision(request.matchId, gameNumber)
    if (!linkage || linkage.command_id !== `auto-draw-provision:${fingerprint}`
      || linkage.fingerprint !== fingerprint || linkage.match_revision !== result.matchRevision) {
      throw new Error('stored automatic draw linkage is missing or inconsistent')
    }
  }

  private persistFailedTerminalIngest(
    request: CompetitionRoomTerminalIngestRequest,
    fingerprint: string,
    outcomeCode: string,
    revision: number,
  ): CompetitionRoomTerminalIngestResult {
    const result = roomFailure(outcomeCode, REDACTED_TERMINAL_ENGINE_FAILURE_MESSAGE)
    this.storage.transactionSync(() => {
      this.insertAudit({
        actorUserId: request.callerUserId,
        role: 'organizer',
        matchId: request.matchId,
        operationType: 'ingestGameTerminal',
        commandId: request.commandId,
        revision,
        outcomeCode,
        trustedTimeMs: request.trustedTimeMs,
      })
      this.insertCommandReceipt(
        request.commandId,
        'ingestGameTerminal',
        fingerprint,
        result,
        request.matchId,
      )
    })
    return result
  }

  private async startScheduledMatchInternal(
    request: CompetitionRoomStartScheduledMatchRequest,
  ): Promise<CompetitionRoomStartScheduledMatchResult> {
    const validation = validateStartScheduledMatchRequest(request)
    if (validation) return roomFailure(validation.code, validation.message)

    const fingerprint = await startScheduledMatchFingerprint(request)
    const receipt = this.commandReceipt(request.commandId)
    if (receipt) {
      if (receipt.receipt_kind !== 'startScheduledMatch'
        || receipt.match_id !== request.matchId
        || receipt.fingerprint !== fingerprint) {
        return roomFailure('commandIdConflict', 'command id was used with another payload')
      }
      return parseStoredStartScheduledMatchResult(receipt.response_json, request)
    }

    const current = this.meta()
    if (!current) return roomFailure('notInitialized', 'competition room is not initialized')
    if (current.competition_id !== request.competitionId) {
      return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
    }

    const caller = this.acl(request.callerUserId)
    if (!caller || !caller.active || caller.role !== 'organizer') {
      return roomFailure('forbidden', 'only the active organizer may start a scheduled match')
    }
    if (request.expectedRevision !== current.revision) {
      return roomFailure('revisionConflict', 'expected revision does not match tournament snapshot')
    }
    if (request.trustedTimeMs < current.updated_trusted_time_ms) {
      return roomFailure('trustedTimeRegression', 'trusted server time cannot move backwards')
    }
    if (this.match(request.matchId)) {
      return roomFailure('alreadyExists', 'match row already exists')
    }

    const currentState = parseServerState(current.server_state_json)
    let pending: ScheduledMatchMaterialization
    try {
      pending = deriveScheduledMatch(
        currentState.tournament,
        request.stageId,
        request.matchId,
        false,
        false,
        'pending',
      )
    } catch {
      return roomFailure('invalidTournamentCheckpoint', 'tournament checkpoint is malformed')
    }

    const startOperation: CompetitionOperation = {
      type: 'tournamentStartMatch', stageId: request.stageId, matchId: request.matchId,
    }
    const serverActor = actorFor('organizer', request.callerUserId)
    let startResponse: CompetitionEngineResponse
    try {
      startResponse = await this.engine({
        serverActor,
        trustedTimeMs: request.trustedTimeMs,
        operation: startOperation,
        serverState: currentState,
      })
    } catch {
      return roomFailure('engineProtocolError', 'competition engine request failed')
    }
    if (!startResponse.ok) {
      return roomFailure(`engine:${startResponse.error.code}`, startResponse.error.message)
    }
    const startedTournament = validateStartedTournamentResponse(
      startResponse, currentState, request.stageId, request.matchId,
    )
    if (!startedTournament) {
      return roomFailure('engineInvalidTournamentCheckpoint', 'tournamentStartMatch returned an invalid checkpoint')
    }
    let derived: ScheduledMatchMaterialization
    try {
      derived = deriveScheduledMatch(startedTournament, request.stageId, request.matchId)
    } catch {
      return roomFailure('engineInvalidTournamentCheckpoint', 'tournamentStartMatch returned an invalid checkpoint')
    }
    if (!sameJson(derived.teams, pending.teams)
      || !sameJson(derived.format, pending.format)
      || !sameJson(derived.clockConfig, pending.clockConfig)) {
      return roomFailure('engineInvalidTournamentCheckpoint', 'tournamentStartMatch changed stage settings')
    }

    if (request.referees.some(referee => {
      const entry = this.acl(referee.refereeId)
      return !entry || !entry.active || entry.role !== 'referee'
    })) {
      return roomFailure('invalidRefereeAcl', 'every configured referee must be an active referee')
    }

    const matchOperation: CompetitionRoomMatchOperation = {
      type: 'matchCreate',
      config: {
        matchId: request.matchId,
        teams: derived.teams,
        format: derived.format,
        firstGame: { type: 'pending', reason: '等待可信隨機先手' },
        referees: request.referees,
      },
    }
    let matchResponse: CompetitionEngineResponse
    try {
      matchResponse = await this.engine({
        serverActor,
        trustedTimeMs: request.trustedTimeMs,
        operation: matchOperation,
        serverState: { registration: null, clock: null, tournament: null, matches: {} },
      })
    } catch {
      return roomFailure('engineProtocolError', 'competition engine request failed')
    }
    if (!matchResponse.ok) {
      return roomFailure(`engine:${matchResponse.error.code}`, matchResponse.error.message)
    }
    if (matchResponse.operation !== 'matchCreate') {
      return roomFailure('engineOperationMismatch', 'competition engine returned another operation')
    }
    const matchStateError = validateScopedEngineState(
      matchResponse.serverState,
      request.matchId,
      null,
      null,
      matchOperation,
    )
    if (matchStateError) return roomFailure(matchStateError.code, matchStateError.message)
    const nextMatch = matchResponse.serverState.matches?.[request.matchId]
    if (!isJsonObject(nextMatch)) {
      return roomFailure('engineInvalidMatchCheckpoint', 'matchCreate returned no match checkpoint')
    }
    const matchProjection = publicProjection(matchResponse, 'organizer')

    const clockOperation: CompetitionRoomMatchOperation = {
      type: 'clockCreate',
      matchId: request.matchId,
      teams: derived.teams,
      config: derived.clockConfig,
    }
    let clockResponse: CompetitionEngineResponse
    try {
      clockResponse = await this.engine({
        serverActor,
        trustedTimeMs: request.trustedTimeMs,
        operation: engineOperation(clockOperation),
        serverState: {
          registration: null,
          clock: null,
          tournament: null,
          matches: { [request.matchId]: nextMatch },
        },
      })
    } catch {
      return roomFailure('engineProtocolError', 'competition engine request failed')
    }
    if (!clockResponse.ok) {
      return roomFailure(`engine:${clockResponse.error.code}`, clockResponse.error.message)
    }
    if (clockResponse.operation !== 'clockCreate') {
      return roomFailure('engineOperationMismatch', 'competition engine returned another operation')
    }
    const clockStateError = validateScopedEngineState(
      clockResponse.serverState,
      request.matchId,
      nextMatch,
      null,
      clockOperation,
    )
    if (clockStateError) return roomFailure(clockStateError.code, clockStateError.message)
    const nextClock = clockResponse.serverState.clock
    if (!nextClock || !isValidClockCheckpoint(nextClock, derived.teams)
      || !isJsonObject(nextClock.config)
      || !sameJson(nextClock.config, clockOperation.config)) {
      return roomFailure('engineInvalidClockCheckpoint', 'clockCreate returned an invalid clock checkpoint')
    }
    const clockPublicError = validateClockPublicView(clockResponse.public, nextClock, derived.teams)
    if (clockPublicError) return roomFailure(clockPublicError.code, clockPublicError.message)
    const clockProjection = publicProjection(clockResponse, 'organizer')
    const revision = current.revision + 1
    const tournamentProjection = publicProjection(startResponse, 'organizer')
    const result = roomSuccess<CompetitionRoomMaterializeScheduledMatchValue>({
      competitionId: request.competitionId,
      stageId: request.stageId,
      matchId: request.matchId,
      revision,
      matchRevision: 1,
      commandId: request.commandId,
      idempotent: false,
      tournamentProjection,
      matchProjection,
      clockProjection,
    })

    this.storage.transactionSync(() => {
      const meta = this.meta()
      if (!meta || meta.revision !== request.expectedRevision
        || request.trustedTimeMs < meta.updated_trusted_time_ms) {
        throw new Error('competition tournament snapshot changed during scheduled start')
      }
      if (this.match(request.matchId)) throw new Error('match row already exists')
      const globalState = parseServerState(meta.server_state_json)
      const globalMatches = globalState.matches ?? {}
      if (globalMatches[request.matchId] !== undefined) {
        throw new Error('global match mirror already exists')
      }
      globalState.tournament = startedTournament
      globalMatches[request.matchId] = nextMatch
      globalState.matches = globalMatches
      this.storage.sql.exec(
        `INSERT INTO competition_matches
           (match_id, stage_id, match_checkpoint_json, clock_checkpoint_json,
            match_projection_json, clock_projection_json, revision,
            last_trusted_time_ms)
         VALUES (?, ?, ?, ?, ?, ?, 1, ?)`,
        request.matchId,
        derived.stageId,
        JSON.stringify(nextMatch),
        JSON.stringify(nextClock),
        JSON.stringify(matchProjection),
        JSON.stringify(clockProjection),
        request.trustedTimeMs,
      )
      this.storage.sql.exec(
        `UPDATE competition_meta
         SET revision = ?, server_state_json = ?, public_projection_json = ?,
             updated_trusted_time_ms = ?
         WHERE singleton = 1`,
        revision,
        JSON.stringify(globalState),
        JSON.stringify(stripPrivate(tournamentProjection)),
        request.trustedTimeMs,
      )
      this.insertAudit({
        actorUserId: request.callerUserId,
        role: 'organizer',
        matchId: request.matchId,
        operationType: 'startScheduledMatch',
        commandId: request.commandId,
        revision,
        outcomeCode: 'success',
        trustedTimeMs: request.trustedTimeMs,
      })
      this.insertCommandReceipt(
        request.commandId,
        'startScheduledMatch',
        fingerprint,
        result,
        request.matchId,
      )
    })
    return result
  }

  private async createTournamentFromLockedRosterInternal(
    request: CompetitionRoomCreateTournamentRequest,
  ): Promise<CompetitionRoomCreateTournamentResult> {
    const validation = validateCreateTournamentRequest(request)
    if (validation) return roomFailure(validation.code, validation.message)

    const fingerprint = await createTournamentFingerprint(request)
    const receipt = this.commandReceipt(request.commandId)
    if (receipt) {
      if (receipt.receipt_kind !== 'createTournamentFromLockedRoster'
        || receipt.match_id !== null || receipt.fingerprint !== fingerprint) {
        return roomFailure('commandIdConflict', 'command id was used with another payload')
      }
      return parseStoredCreateTournamentResult(receipt.response_json)
    }

    const current = this.meta()
    if (!current) return roomFailure('notInitialized', 'competition room is not initialized')
    if (current.competition_id !== request.competitionId) {
      return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
    }
    const caller = this.acl(request.callerUserId)
    if (!caller || !caller.active || caller.role !== 'organizer'
      || request.callerUserId !== current.organizer_user_id) {
      return roomFailure('forbidden', 'only the active organizer may create a tournament')
    }
    if (request.expectedRevision !== current.revision) {
      return roomFailure('revisionConflict', 'expected revision does not match room revision')
    }
    if (request.trustedTimeMs < current.updated_trusted_time_ms) {
      return roomFailure('trustedTimeRegression', 'trusted server time cannot move backwards')
    }

    const seedingIntent = this.initialSeedingIntent(request.competitionId)
    let persistedStage: unknown
    try {
      persistedStage = seedingIntent ? JSON.parse(seedingIntent.stage_json) as unknown : null
    } catch {
      return roomFailure('invalidStageConfig', 'persisted initial seeding config is invalid')
    }
    if (!seedingIntent || seedingIntent.status !== 'pending'
      || request.commandId !== seedingIntent.command_id
      || seedingIntent.stage_id !== request.stage.stageId
      || !sameJson(persistedStage, request.stage)) {
      return roomFailure('operationNotAllowed', 'tournament creation must use the persisted initial draw')
    }

    const serverState = parseServerState(current.server_state_json)
    if (!serverState.registration) {
      return roomFailure('registrationNotLocked', 'registration must be locked before tournament creation')
    }
    if (serverState.tournament !== null) {
      return roomFailure('tournamentAlreadyExists', 'tournament already exists')
    }
    const requestedTeamIds = request.stage.teams.map(team => team.teamId)
    if (!isUniqueNonEmptyStringArray(requestedTeamIds)) {
      return roomFailure('invalidStageTeams', 'stage teams must have unique non-empty ids')
    }

    let exportResponse: CompetitionEngineResponse
    try {
      exportResponse = await this.engine({
        serverActor: actorFor('organizer', request.callerUserId),
        trustedTimeMs: request.trustedTimeMs,
        operation: { type: 'registrationExportLockedRoster' },
        serverState,
      })
    } catch {
      return roomFailure('engineProtocolError', 'locked roster export failed')
    }
    if (!exportResponse.ok) {
      return roomFailure('engineRejected', 'locked roster export was rejected')
    }
    const lockedRoster = parseLockedRoster(exportResponse.private, current.organizer_user_id)
    if (exportResponse.operation !== 'registrationExportLockedRoster'
      || !hasExactKeys(exportResponse as unknown as JsonObject,
        ['ok', 'operation', 'public', 'private', 'serverState'])
      || !isJsonObject(exportResponse.public)
      || !hasExactKeys(exportResponse.public, ['exported'])
      || exportResponse.public.exported !== true
      || !sameJson(exportResponse.serverState, serverState)
      || !lockedRoster) {
      return roomFailure('engineProtocolError', 'locked roster export returned an invalid response')
    }
    if (!sameTeamSet(requestedTeamIds, lockedRoster.teamIds)) {
      return roomFailure('stageTeamMismatch', 'stage teams must exactly match the locked roster')
    }

    const operation: CompetitionOperation = {
      type: 'tournamentCreate',
      config: { tournamentId: request.competitionId, stage: request.stage },
    }
    let createResponse: CompetitionEngineResponse
    try {
      createResponse = await this.engine({
        serverActor: actorFor('organizer', request.callerUserId),
        trustedTimeMs: request.trustedTimeMs,
        operation,
        serverState,
      })
    } catch {
      return roomFailure('engineProtocolError', 'tournament creation failed')
    }
    if (!createResponse.ok) {
      return roomFailure('engineRejected', 'tournament creation was rejected')
    }
    if (createResponse.operation !== 'tournamentCreate'
      || !hasExactKeys(createResponse as unknown as JsonObject,
        ['ok', 'operation', 'public', 'serverState'])
      || !isJsonObject(createResponse.serverState)
      || !hasExactKeys(createResponse.serverState,
        ['registration', 'clock', 'tournament', 'matches'])
      || !sameJson(createResponse.serverState.registration, serverState.registration)
      || !sameJson(createResponse.serverState.clock, serverState.clock)
      || !sameJson(createResponse.serverState.matches, serverState.matches ?? {})
      || !isJsonObject(createResponse.serverState.tournament)
      || !sameJson(createResponse.public, createResponse.serverState.tournament)
      || !validCreatedTournament(createResponse.serverState.tournament, request.stage,
        request.competitionId)
      || containsPrivateRosterFields(createResponse.public)) {
      return roomFailure('engineProtocolError', 'tournament creation returned an invalid response')
    }

    let projection: PublicCompetitionSuccess
    try {
      projection = stripPrivate(publicProjection(createResponse, 'organizer'))
    } catch {
      return roomFailure('engineProtocolError', 'tournament projection is invalid')
    }
    if (projection.operation !== 'tournamentCreate' || 'private' in projection) {
      return roomFailure('engineProtocolError', 'tournament projection is invalid')
    }
    const revision = current.revision + 1
    const result = roomSuccess<CompetitionRoomCreateTournamentValue>({
      competitionId: request.competitionId,
      revision,
      commandId: request.commandId,
      idempotent: false,
      projection,
    })
    this.storage.transactionSync(() => {
      const latest = this.meta()
      if (!latest || latest.competition_id !== current.competition_id
        || latest.revision !== current.revision
        || latest.server_state_json !== current.server_state_json) {
        throw new Error('competition state changed during tournament creation')
      }
      const nextState: CompetitionServerState = {
        ...serverState,
        tournament: createResponse.serverState.tournament,
      }
      this.storage.sql.exec(
        `UPDATE competition_meta
         SET revision = ?, server_state_json = ?, public_projection_json = ?,
             updated_trusted_time_ms = ?
         WHERE singleton = 1`,
        revision,
        JSON.stringify(nextState),
        JSON.stringify(stripPrivate(projection)),
        request.trustedTimeMs,
      )
      this.storage.sql.exec(
        `UPDATE competition_initial_seeding
         SET status = 'completed', last_error = NULL
         WHERE competition_id = ? AND command_id = ? AND status = 'pending'`,
        request.competitionId,
        request.commandId,
      )
      this.insertAudit({
        actorUserId: request.callerUserId,
        role: 'organizer',
        operationType: 'tournamentCreate',
        commandId: request.commandId,
        revision,
        outcomeCode: 'success',
        trustedTimeMs: request.trustedTimeMs,
      })
      this.insertCommandReceipt(
        request.commandId,
        'createTournamentFromLockedRoster',
        fingerprint,
        result,
      )
    })
    return result
  }

  private async exportLockedRosterFromState(
    serverState: CompetitionServerState,
    organizerUserId: string,
    trustedTimeMs: number,
  ): Promise<{ teamIds: string[], announcement: JsonObject } | null> {
    let response: CompetitionEngineResponse
    try {
      response = await this.engine({
        serverActor: actorFor('organizer', organizerUserId),
        trustedTimeMs,
        operation: { type: 'registrationExportLockedRoster' },
        serverState,
      })
    } catch {
      return null
    }
    if (!response.ok || response.operation !== 'registrationExportLockedRoster'
      || !hasExactKeys(response as unknown as JsonObject, ['ok', 'operation', 'public', 'private', 'serverState'])
      || !isJsonObject(response.public) || !hasExactKeys(response.public, ['exported'])
      || response.public.exported !== true || !sameJson(response.serverState, serverState)) return null
    const roster = parseLockedRoster(response.private, organizerUserId)
    return roster ? { teamIds: roster.teamIds, announcement: roster.announcement } : null
  }

  private initialSeedingIntent(competitionId: string): InitialSeedingIntentRow | undefined {
    const row = this.storage.sql.exec<InitialSeedingIntentRow>(
      'SELECT * FROM competition_initial_seeding WHERE competition_id = ?',
      competitionId,
    ).toArray()[0]
    return row && (row.status === 'pending' || row.status === 'completed')
      && safeNonNegativeInteger(row.attempts)
      && safeNonNegativeInteger(row.next_attempt_at_ms)
      && safeNonNegativeInteger(row.created_trusted_time_ms)
      ? row
      : undefined
  }

  private async ensureInitialSeedingIntentForLockedRoom(
    serverState: CompetitionServerState,
    current: CompetitionTournamentReadMetaRow,
  ): Promise<void> {
    if (serverState.tournament !== null || !isLockedRegistrationCheckpoint(
      serverState.registration, current.organizer_user_id,
    ) || this.initialSeedingIntent(current.competition_id)) return
    const lockedRoster = await this.exportLockedRosterFromState(
      serverState,
      current.organizer_user_id,
      current.updated_trusted_time_ms,
    )
    if (!lockedRoster || lockedRoster.teamIds.length < 2) return
    const plan = parseCompetitionAnnouncementPlan(lockedRoster.announcement.plan)
    const announcedStage = plan?.stages[0]
    if (!announcedStage || announcedStage.qualification.type !== 'allEligibleTeams') return
    let drawn: ReturnType<typeof drawInitialSeedTeams>
    try {
      drawn = drawInitialSeedTeams(lockedRoster.teamIds, this.randomUint32)
    } catch {
      return
    }
    const stage: StageConfig = {
      stageId: announcedStage.stageId,
      format: announcedStage.format,
      teams: drawn.teams,
      randomSeed: drawn.randomSeed,
      matchFormat: announcedStage.matchFormat,
      clockConfig: announcedStage.clockConfig,
      swissRounds: announcedStage.swissRounds,
      swissPolicy: announcedStage.swissPolicy,
    }
    const intent: InitialSeedingIntentRow = {
      competition_id: current.competition_id,
      command_id: `initial-seeding:${crypto.randomUUID()}`,
      stage_id: stage.stageId,
      stage_json: JSON.stringify(stage),
      status: 'pending',
      attempts: 0,
      next_attempt_at_ms: 0,
      last_error: null,
      created_trusted_time_ms: current.updated_trusted_time_ms,
    }
    this.storage.transactionSync(() => {
      const latest = this.meta()
      if (!latest || latest.competition_id !== current.competition_id
        || latest.revision !== current.revision
        || latest.server_state_json !== current.server_state_json) return
      this.storage.sql.exec(
        `INSERT OR IGNORE INTO competition_initial_seeding
           (competition_id, command_id, stage_id, stage_json, status, attempts,
            next_attempt_at_ms, last_error, created_trusted_time_ms)
         VALUES (?, ?, ?, ?, 'pending', 0, ?, NULL, ?)`,
        intent.competition_id,
        intent.command_id,
        intent.stage_id,
        intent.stage_json,
        intent.next_attempt_at_ms,
        intent.created_trusted_time_ms,
      )
    })
  }

  private async pumpInitialSeedingInternal(): Promise<void> {
    try {
      const row = this.storage.sql.exec<InitialSeedingIntentRow>(
        `SELECT * FROM competition_initial_seeding
         WHERE status = 'pending' OR (status = 'completed' AND last_error IS NOT NULL
           AND last_error != 'roundNotPublishable')
         ORDER BY created_trusted_time_ms LIMIT 1`,
      ).toArray()[0]
      if (!row || !safeNonNegativeInteger(row.attempts) || !safeNonNegativeInteger(row.next_attempt_at_ms)) return
      const now = Date.now()
      if (row.next_attempt_at_ms > now) return
      if (row.status === 'completed') {
        try { await this.publishAutomaticInitialRoundIfNeeded(row) } catch {
          this.deferAutomaticRoundPublication(row, 'automaticRoundPublicationFailure', Date.now())
        }
        return
      }
      const current = this.meta()
      if (!current || current.competition_id !== row.competition_id) return
      let stage: StageConfig
      try {
        const parsed: unknown = JSON.parse(row.stage_json)
        if (!isJsonObject(parsed) || !hasExactKeys(parsed, [
          'stageId', 'format', 'teams', 'randomSeed', 'matchFormat', 'clockConfig', 'swissRounds', 'swissPolicy',
        ]) || parsed.stageId !== row.stage_id) throw new Error('invalid saved stage')
        stage = parsed as unknown as StageConfig
      } catch {
        this.deferInitialSeeding(row, 'invalidPersistedConfig', now)
        return
      }
      const result = await this.createTournamentFromLockedRosterInternal({
        competitionId: row.competition_id,
        callerUserId: current.organizer_user_id,
        commandId: row.command_id,
        expectedRevision: current.revision,
        trustedTimeMs: Math.max(now, current.updated_trusted_time_ms),
        stage,
      })
      if (!result.ok) this.deferInitialSeeding(row, result.error.code, now)
      else {
        try { await this.publishAutomaticInitialRoundIfNeeded(row) } catch {
          this.deferAutomaticRoundPublication(row, 'automaticRoundPublicationFailure', Date.now())
        }
      }
    } catch {
      console.warn('CompetitionRoom retained automatic initial seeding after an attempt failure')
    }
  }

  /** 前輪權威完成後，預留最多五分鐘備妥下一輪，公告點後再留五分鐘報到。 */
  private async publishNextAutomaticRoundIfReady(): Promise<'announced' | 'retry' | 'none'> {
    const current = this.meta()
    if (!current || !safeNonNegativeInteger(current.updated_trusted_time_ms)) return 'none'
    let state: CompetitionServerState
    try { state = parseServerState(current.server_state_json) } catch { return 'none' }
    const tournament = state.tournament
    if (!isJsonObject(tournament) || !Array.isArray(tournament.stages)) {
      return 'none'
    }
    const activeStages = tournament.stages.filter(stage => isJsonObject(stage) && stage.status === 'active')
    if (activeStages.length !== 1) return 'none'
    const stage = activeStages[0]
    if (!isJsonObject(stage) || !isJsonObject(stage.config)
      || typeof stage.config.stageId !== 'string' || !Array.isArray(stage.rounds)) return 'none'
    const stageIndex = tournament.stages.indexOf(stage)
    if (stageIndex < 0 || tournament.stages.slice(0, stageIndex).some(previous =>
      !isJsonObject(previous) || previous.status !== 'completed')) return 'none'
    const stageId = stage.config.stageId
    const rounds = stage.rounds.filter((round): round is JsonObject => isJsonObject(round)
      && safeNonNegativeInteger(round.roundNumber) && round.roundNumber > 0).sort((left, right) =>
      Number(left.roundNumber) - Number(right.roundNumber))
    for (const round of rounds) {
      const roundNumber = round.roundNumber as number
      if ((roundNumber === 1 && stageIndex === 0) || this.currentRoundPublication(stageId, roundNumber)) continue
      let selected: ReturnType<typeof selectPublishableRound>
      try {
        selected = selectPublishableRound(tournament, stageId, roundNumber,
          match => this.isAuthoritativeNoShowResult(match))
      } catch { continue }
      if (roundNumber > 1 && selected.previousMatches.length === 0) continue

      const completedRounds = roundNumber > 1
        ? rounds.filter(entry => Number(entry.roundNumber) < roundNumber)
        : tournament.stages.slice(0, stageIndex).flatMap(previousStage =>
            isJsonObject(previousStage) && Array.isArray(previousStage.rounds) ? previousStage.rounds : [])
      const previousMatches: JsonObject[] = []
      for (const completedRound of completedRounds) {
        if (!isJsonObject(completedRound) || !Array.isArray(completedRound.matches)) return 'retry'
        for (const match of completedRound.matches) {
          if (!isJsonObject(match) || match.status !== 'completed' || match.result === null
            || match.cancellationReason !== null) return 'retry'
          if (isCoreBye(match)) {
            previousMatches.push(match)
            continue
          }
          const played = Array.isArray(match.participants) && match.participants.length === 2
            && match.started === true
          const noShow = Array.isArray(match.participants) && match.participants.length === 2
            && match.started === false && this.isAuthoritativeNoShowResult(match)
          if (!played && !noShow) return 'retry'
          previousMatches.push(match)
        }
      }
      const playedResults = previousMatches.filter(match => !isCoreBye(match))
      const previousMatchTimes = playedResults.flatMap(match => {
        if (typeof match.matchId !== 'string') return []
        if (this.isAuthoritativeNoShowResult(match)) {
          const ruling = this.noShowOutbox(match.matchId)
          return ruling && safeNonNegativeInteger(ruling.result_confirmed_at_ms)
            ? [ruling.result_confirmed_at_ms] : []
        }
        const row = this.match(match.matchId)
        if (!row) return []
        const confirmedAt = row.tournament_result_confirmed_at_ms ?? row.last_trusted_time_ms
        return safeNonNegativeInteger(confirmedAt) ? [confirmedAt] : []
      })
      if (previousMatchTimes.length !== playedResults.length) return 'retry'
      const completedAtMs = previousMatchTimes.length > 0
        ? Math.max(...previousMatchTimes)
        : current.updated_trusted_time_ms
      const announceAtMs = floorCompetitionMinute(checkedAdd(completedAtMs, 5 * 60_000))
      const roundStartMs = announceAtMs === null ? null : checkedAdd(announceAtMs, 5 * 60_000)
      const now = Date.now()
      if (announceAtMs === null || roundStartMs === null || !safeNonNegativeInteger(now)) return 'none'
      const trustedTimeMs = Math.max(now, current.updated_trusted_time_ms)
      if (!safeNonNegativeInteger(trustedTimeMs)) return 'none'
      const missedPreparationWindow = trustedTimeMs >= announceAtMs
      const commandDigest = await semanticFingerprint({
        kind: 'automatic-next-round', competitionId: current.competition_id,
        stageId, roundNumber, completedAtMs,
      })
      const result = await this.announceRoundInternal({
        competitionId: current.competition_id,
        stageId,
        roundNumber,
        callerUserId: current.organizer_user_id,
        commandId: `automatic-next-round-${commandDigest}`,
        expectedRevision: current.revision,
        trustedTimeMs,
        roundStartMs,
        refereeAssignments: [],
      }, announceAtMs, missedPreparationWindow)
      if (result.ok) return result.value.idempotent ? 'none' : 'announced'
      return isRetryableAutomaticRoundAdvancementError(result.error.code) ? 'retry' : 'none'
    }
    return 'none'
  }

  /** 首輪沿用公告 T；不因開房延誤而改寫時間或略過五分鐘窗口。 */
  private async publishAutomaticInitialRoundIfNeeded(row: InitialSeedingIntentRow): Promise<void> {
    const current = this.meta()
    if (!current || current.competition_id !== row.competition_id) {
      this.markAutomaticInitialRoundNotPublishable(row)
      return
    }
    if (this.currentRoundPublication(row.stage_id, 1)) {
      this.storage.sql.exec(
        `UPDATE competition_initial_seeding SET last_error = NULL, attempts = 0, next_attempt_at_ms = 0
         WHERE competition_id = ? AND command_id = ? AND status = 'completed'`,
        row.competition_id, row.command_id,
      )
      return
    }
    let state: CompetitionServerState
    try { state = parseServerState(current.server_state_json) } catch {
      this.markAutomaticInitialRoundNotPublishable(row)
      return
    }
    const registration = state.registration
    const announcement = isJsonObject(registration) && isJsonObject(registration.announcement)
      ? registration.announcement : null
    const roundStartMs = announcement?.checkInDeadline
    if (!safeNonNegativeInteger(roundStartMs) || roundStartMs % 60_000 !== 0) {
      this.markAutomaticInitialRoundNotPublishable(row)
      return
    }
    const cutoff = competitionRoundReadyCutoffMs(roundStartMs)
    const trustedTimeMs = Math.max(Date.now(), current.updated_trusted_time_ms)
    if (cutoff === null || !safeNonNegativeInteger(trustedTimeMs)) {
      this.markAutomaticInitialRoundNotPublishable(row)
      return
    }
    const missedPreparationWindow = trustedTimeMs > cutoff
    const commandDigest = await semanticFingerprint({
      kind: 'automatic-initial-round', competitionId: row.competition_id,
      stageId: row.stage_id, roundNumber: 1,
    })
    const result = await this.announceRoundInternal({
      competitionId: row.competition_id,
      stageId: row.stage_id,
      roundNumber: 1,
      callerUserId: current.organizer_user_id,
      commandId: `automatic-round-${commandDigest}`,
      expectedRevision: current.revision,
      trustedTimeMs,
      roundStartMs,
      refereeAssignments: [],
    }, cutoff, missedPreparationWindow)
    if (result.ok) {
      this.storage.sql.exec(
        `UPDATE competition_initial_seeding SET last_error = NULL, attempts = 0, next_attempt_at_ms = 0
         WHERE competition_id = ? AND command_id = ? AND status = 'completed'`,
        row.competition_id, row.command_id,
      )
      return
    }
    if (result.error.code === 'roundNotPublishable') {
      this.markAutomaticInitialRoundNotPublishable(row)
      return
    }
    this.deferAutomaticRoundPublication(row, result.error.code, Date.now())
  }

  private markAutomaticInitialRoundNotPublishable(row: InitialSeedingIntentRow): void {
    this.storage.sql.exec(
      `UPDATE competition_initial_seeding SET last_error = 'roundNotPublishable', next_attempt_at_ms = 0
       WHERE competition_id = ? AND command_id = ? AND status = 'completed'`,
      row.competition_id, row.command_id,
    )
  }

  private deferAutomaticRoundPublication(row: InitialSeedingIntentRow, errorCode: string, now: number): void {
    if (!safeNonNegativeInteger(row.attempts + 1)) return
    const nextAttempt = checkedRoundRetryTime(now, Math.min(60_000, 1_000 * 2 ** Math.min(row.attempts + 1, 6)))
    this.storage.sql.exec(
      `UPDATE competition_initial_seeding SET attempts = attempts + 1, next_attempt_at_ms = ?, last_error = ?
       WHERE competition_id = ? AND command_id = ? AND status = 'completed'`,
      nextAttempt, errorCode, row.competition_id, row.command_id,
    )
  }

  private deferInitialSeeding(row: InitialSeedingIntentRow, errorCode: string, now: number): void {
    if (!safeNonNegativeInteger(row.attempts + 1)) return
    const nextAttempt = checkedRoundRetryTime(now, Math.min(60_000, 1_000 * 2 ** Math.min(row.attempts + 1, 6)))
    this.storage.sql.exec(
      `UPDATE competition_initial_seeding
       SET attempts = attempts + 1, next_attempt_at_ms = ?, last_error = ?
       WHERE competition_id = ? AND command_id = ? AND status = 'pending'`,
      nextAttempt,
      errorCode,
      row.competition_id,
      row.command_id,
    )
  }

  private async provisionScheduledGameInternal(
    request: CompetitionRoomProvisionScheduledGameRequest,
  ): Promise<CompetitionRoomProvisionScheduledGameResult> {
    const validation = validateProvisionScheduledGameRequest(request)
    if (validation) return roomFailure(validation.code, validation.message)

    const current = this.meta()
    if (!current) return roomFailure('notInitialized', 'competition room is not initialized')
    if (current.competition_id !== request.competitionId) {
      return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
    }

    const fingerprint = await provisionScheduledGameFingerprint(request)
    const receipt = this.commandReceipt(request.commandId)
    if (receipt) {
      if (receipt.receipt_kind !== 'provisionScheduledGame'
        || receipt.match_id !== request.matchId
        || receipt.fingerprint !== fingerprint) {
        return roomFailure('commandIdConflict', 'command id was used with another payload')
      }
      const linked = this.gameProvisionByCommand(request.commandId)
      if (!linked) throw new Error('game provision receipt has no linkage row')
      return roomSuccess(this.gameProvisionValue(current.competition_id, linked, true))
    }

    const caller = this.acl(request.callerUserId)
    if (!caller || !caller.active || caller.role !== 'organizer') {
      return roomFailure('forbidden', 'only the active organizer may provision a scheduled game')
    }
    const row = this.match(request.matchId)
    if (!row) return roomFailure('matchNotFound', 'match row does not exist')
    if (row.revision !== request.expectedMatchRevision) {
      return roomFailure('matchRevisionConflict', 'expected match revision does not match match revision')
    }
    if (request.trustedTimeMs < row.last_trusted_time_ms
      || request.trustedTimeMs < current.updated_trusted_time_ms) {
      return roomFailure('trustedTimeRegression', 'trusted server time cannot move backwards')
    }

    const serverState = parseServerState(current.server_state_json)
    const intent = deriveProvisionIntent(row, serverState, request.firstPlayer, request.turnOrder)
    if (!intent.ok) return roomFailure(intent.code, intent.message)

    let exportResponse: CompetitionEngineResponse
    try {
      exportResponse = await this.engine({
        serverActor: actorFor('organizer', request.callerUserId),
        trustedTimeMs: request.trustedTimeMs,
        operation: { type: 'registrationExportLockedRoster' },
        serverState,
      })
    } catch {
      return roomFailure('engineProtocolError', 'competition engine request failed')
    }
    if (!exportResponse.ok) {
      return roomFailure(`engine:${exportResponse.error.code}`, exportResponse.error.message)
    }
    if (exportResponse.operation !== 'registrationExportLockedRoster'
      || !sameJson(exportResponse.serverState, serverState)
      || !isJsonObject(exportResponse.public)
      || !hasExactKeys(exportResponse.public, ['exported'])
      || exportResponse.public.exported !== true) {
      return roomFailure('engineProtocolError', 'locked roster export returned an invalid response')
    }
    const roster = deriveLockedRosterBinding(
      exportResponse.private,
      intent.value.teams,
      current.organizer_user_id,
      intent.value.assignedFirstPlayerTeam,
      request.firstPlayer,
      request.turnOrder,
    )
    if (!roster.ok) return roomFailure(roster.code, roster.message)

    const gameId = `competition-game-${await semanticFingerprint({
      kind: 'competition-game',
      competitionId: request.competitionId,
      matchId: request.matchId,
      gameNumber: intent.value.gameNumber,
    })}`
    const roundContext = this.publishedRoundBindingContext(request.matchId)
    if (!roundContext) return roomFailure('roundNotPublishable', 'the scheduled match has no published round binding')
    const binding: CompetitionGameBinding = {
      roundRef: roundContext.roundRef,
      roundStartMs: roundContext.roundStartMs,
      competitionId: request.competitionId,
      matchId: request.matchId,
      gameNumber: intent.value.gameNumber,
      organizerUserId: current.organizer_user_id,
      expectedMatchRevision: row.revision,
      startKind: intent.value.startKind,
      ruleVersion: roster.value.ruleVersion,
      ruleset: roster.value.ruleset,
      enabledRuleModules: roster.value.enabledRuleModules,
      initialHp: intent.value.teams.map(team => ({ team, hp: roster.value.initialHp })),
      players: roster.value.players,
      deckSnapshot: roster.value.deckSnapshot,
      firstPlayer: request.firstPlayer,
      turnOrder: [...request.turnOrder],
      deckSeed: crypto.randomUUID(),
    }
    try {
      validateCompetitionGameBinding(binding)
    } catch {
      return roomFailure('invalidLockedRoster', 'locked roster cannot produce a valid game binding')
    }

    const existing = this.gameProvision(request.matchId, intent.value.gameNumber)
    if (existing) {
      return roomFailure('gameAlreadyLinked', 'the scheduled game already has a room linkage')
    }
    const initial = roomSuccess<CompetitionRoomProvisionScheduledGameValue>({
      competitionId: request.competitionId,
      matchId: request.matchId,
      gameNumber: intent.value.gameNumber,
      gameId,
      commandId: request.commandId,
      idempotent: false,
      status: 'pending',
      binding,
    })
    this.storage.transactionSync(() => {
      const latest = this.match(request.matchId)
      if (!latest || latest.revision !== row.revision
        || latest.match_checkpoint_json !== row.match_checkpoint_json
        || latest.match_projection_json !== row.match_projection_json) {
        throw new Error('match changed during game provision')
      }
      if (this.gameProvision(request.matchId, intent.value.gameNumber)) {
        throw new Error('game linkage was created concurrently')
      }
      this.storage.sql.exec(
        `INSERT INTO competition_game_provision
          (match_id, game_number, game_id, binding_json, match_revision, next_game_json,
           command_id, fingerprint, status, attempts, next_attempt_at_ms, error_code,
           created_trusted_time_ms)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'pending', 0, ?, NULL, ?)`,
        request.matchId,
        intent.value.gameNumber,
        gameId,
        JSON.stringify(binding),
        row.revision,
        JSON.stringify(intent.value.nextGame),
        request.commandId,
        fingerprint,
        Date.now(),
        request.trustedTimeMs,
      )
      this.insertAudit({
        actorUserId: request.callerUserId,
        role: 'organizer',
        matchId: request.matchId,
        operationType: 'provisionScheduledGame',
        commandId: request.commandId,
        revision: row.revision,
        outcomeCode: 'pending',
        trustedTimeMs: request.trustedTimeMs,
      })
      this.insertCommandReceipt(
        request.commandId,
        'provisionScheduledGame',
        fingerprint,
        initial,
        request.matchId,
      )
    })

    const linked = this.gameProvision(request.matchId, intent.value.gameNumber)
    if (!linked) throw new Error('game linkage disappeared after commit')
    return roomSuccess(this.gameProvisionValue(request.competitionId, linked, false))
  }

  private async assignNextFirstPlayerByCaptainInternal(
    request: CompetitionRoomAssignNextFirstPlayerByCaptainRequest,
  ): Promise<CompetitionRoomAssignNextFirstPlayerByCaptainResult> {
    const validation = validateAssignNextFirstPlayerByCaptainRequest(request)
    if (validation) return roomFailure(validation.code, validation.message)

    const fingerprint = await assignNextFirstPlayerByCaptainFingerprint(request)
    const receipt = this.commandReceipt(request.commandId)
    if (receipt) {
      if (receipt.receipt_kind !== 'assignNextFirstPlayerByCaptain'
        || receipt.match_id !== request.matchId || receipt.fingerprint !== fingerprint) {
        return roomFailure('commandIdConflict', 'command id was used with another payload')
      }
      return roomSuccess(parseStoredCaptainFirstPlayerResult(receipt.response_json, request))
    }

    const current = this.meta()
    if (!current) return roomFailure('notInitialized', 'competition room is not initialized')
    if (current.competition_id !== request.competitionId) {
      return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
    }
    const caller = this.acl(request.callerUserId)
    if (!caller || !caller.active || caller.role !== 'player' || caller.participant !== 1) {
      return roomFailure('forbidden', 'only an active participant captain may assign the next first player')
    }
    const row = this.match(request.matchId)
    if (!row) return roomFailure('matchNotFound', 'match row does not exist')
    const storedProjection = parseOptionalProjection(row.match_projection_json)
    const storedMatch = isJsonObject(storedProjection?.public)
      && isJsonObject(storedProjection.public.match)
      ? storedProjection.public.match : null
    const storedNextGame = isJsonObject(storedMatch?.nextGame) ? storedMatch.nextGame : null
    if (typeof storedNextGame?.gameNumber === 'number'
      && this.gameProvision(request.matchId, storedNextGame.gameNumber)) {
      return roomFailure('gameAlreadyLinked', 'the next game already has a provisioned room')
    }
    if (row.revision !== request.expectedMatchRevision) {
      return roomFailure('matchRevisionConflict', 'expected match revision does not match match revision')
    }
    if (request.trustedTimeMs < row.last_trusted_time_ms
      || request.trustedTimeMs < current.updated_trusted_time_ms) {
      return roomFailure('trustedTimeRegression', 'trusted server time cannot move backwards')
    }

    let serverState: CompetitionServerState
    try {
      serverState = parseServerState(current.server_state_json)
    } catch {
      return roomFailure('matchNotProvisionable', 'persisted competition match state is invalid')
    }
    let view: CompetitionEngineResponse
    try {
      view = await this.engine({
        serverActor: actorFor('organizer', current.organizer_user_id),
        trustedTimeMs: request.trustedTimeMs,
        operation: { type: 'matchView', matchId: request.matchId },
        serverState,
      })
    } catch {
      return roomFailure('engineProtocolError', 'match checkpoint replay failed')
    }
    if (!isValidStoredMatchView(view, serverState, row)) {
      return roomFailure('engineInvalidMatchCheckpoint', 'match checkpoint replay failed')
    }

    const eligible = deriveCaptainAssignmentIntent(row, serverState, request.selectedFirstPlayerTeam)
    if (!eligible.ok) return roomFailure(eligible.code, eligible.message)
    let exportResponse: CompetitionEngineResponse
    try {
      exportResponse = await this.engine({
        serverActor: actorFor('organizer', current.organizer_user_id),
        trustedTimeMs: request.trustedTimeMs,
        operation: { type: 'registrationExportLockedRoster' },
        serverState,
      })
    } catch {
      return roomFailure('engineProtocolError', 'locked roster export request failed')
    }
    if (!exportResponse.ok || exportResponse.operation !== 'registrationExportLockedRoster'
      || !hasExactKeys(exportResponse as unknown as JsonObject, ['ok', 'operation', 'public', 'private', 'serverState'])
      || !sameJson(exportResponse.serverState, serverState)
      || !isJsonObject(exportResponse.public)
      || !hasExactKeys(exportResponse.public, ['exported'])
      || exportResponse.public.exported !== true) {
      return roomFailure('engineProtocolError', 'locked roster export returned an invalid response')
    }
    const roster = deriveLockedRosterBinding(
      exportResponse.private,
      eligible.value.teams,
      current.organizer_user_id,
      request.selectedFirstPlayerTeam,
      null,
      null,
    )
    if (!roster.ok) return roomFailure(roster.code, roster.message)
    const rawRoster = exportResponse.private
    const losingTeam = eligible.value.losingTeam
    const captain = isJsonObject(rawRoster) && Array.isArray(rawRoster.teams)
      ? rawRoster.teams.find(team => isJsonObject(team) && team.teamId === losingTeam)
      : undefined
    if (!isJsonObject(captain) || captain.captain !== request.callerUserId
      || !Array.isArray(captain.members) || !captain.members.includes(request.callerUserId)) {
      return roomFailure('forbidden', 'only the active losing-team captain may assign the next first player')
    }

    let response: CompetitionEngineResponse
    try {
      response = await this.engine({
        serverActor: actorFor('organizer', current.organizer_user_id),
        trustedTimeMs: request.trustedTimeMs,
        operation: {
          type: 'matchAssignFirstPlayer',
          matchId: request.matchId,
          team: request.selectedFirstPlayerTeam,
          reason: CAPTAIN_FIRST_PLAYER_REASON,
        },
        serverState,
      })
    } catch {
      return roomFailure('engineProtocolError', 'first-player assignment failed')
    }
    const validated = validateCaptainFirstPlayerResponse(
      response, request.matchId, serverState, eligible.value.checkpoint,
      eligible.value.publicMatch, request.selectedFirstPlayerTeam,
    )
    if (!validated) return roomFailure('engineProtocolError', 'first-player assignment returned an invalid response')
    const revision = row.revision + 1
    const result = roomSuccess<CompetitionRoomAssignNextFirstPlayerByCaptainValue>({
      competitionId: request.competitionId,
      matchId: request.matchId,
      matchRevision: revision,
      commandId: request.commandId,
      idempotent: false,
      matchProjection: validated.projection,
    })

    this.storage.transactionSync(() => {
      const latestMeta = this.meta()
      const latestRow = this.match(request.matchId)
      if (!latestMeta || latestMeta.competition_id !== current.competition_id
        || latestMeta.organizer_user_id !== current.organizer_user_id
        || latestMeta.revision !== current.revision
        || latestMeta.updated_trusted_time_ms !== current.updated_trusted_time_ms
        || latestMeta.server_state_json !== current.server_state_json
        || !latestRow || latestRow.revision !== row.revision
        || latestRow.last_trusted_time_ms !== row.last_trusted_time_ms
        || latestRow.match_checkpoint_json !== row.match_checkpoint_json
        || latestRow.match_projection_json !== row.match_projection_json
        || latestRow.clock_checkpoint_json !== row.clock_checkpoint_json
        || latestRow.clock_projection_json !== row.clock_projection_json) {
        throw new Error('match state changed before captain assignment commit')
      }
      this.storage.sql.exec(
        `UPDATE competition_matches
         SET match_checkpoint_json = ?, match_projection_json = ?, revision = ?, last_trusted_time_ms = ?
         WHERE match_id = ?`,
        JSON.stringify(validated.checkpoint),
        JSON.stringify(validated.projection),
        revision,
        request.trustedTimeMs,
        request.matchId,
      )
      this.storage.sql.exec(
        'UPDATE competition_meta SET server_state_json = ? WHERE singleton = 1',
        JSON.stringify(validated.serverState),
      )
      this.insertAudit({
        actorUserId: request.callerUserId,
        role: 'player',
        matchId: request.matchId,
        operationType: 'assignNextFirstPlayerByCaptain',
        commandId: request.commandId,
        revision,
        outcomeCode: 'success',
        trustedTimeMs: request.trustedTimeMs,
        reason: CAPTAIN_FIRST_PLAYER_REASON,
      })
      this.insertCommandReceipt(
        request.commandId,
        'assignNextFirstPlayerByCaptain',
        fingerprint,
        result,
        request.matchId,
      )
    })
    return result
  }

  private async provisionAssignedScheduledGameInternal(
    request: CompetitionRoomProvisionAssignedScheduledGameRequest,
  ): Promise<CompetitionRoomProvisionAssignedScheduledGameResult> {
    const validation = validateProvisionAssignedScheduledGameRequest(request)
    if (validation) return roomFailure(validation.code, validation.message)

    // trustedTimeMs 不屬於語意指紋；expectedMatchRevision 則是 command 的語意欄位。
    const fingerprint = await provisionAssignedScheduledGameFingerprint(request)
    const receipt = this.commandReceipt(request.commandId)
    if (receipt) {
      if (receipt.receipt_kind !== 'provisionAssignedScheduledGame'
        || receipt.match_id !== request.matchId || receipt.fingerprint !== fingerprint) {
        return roomFailure('commandIdConflict', 'command id was used with another payload')
      }
      const stored = await parseStoredProvisionAssignedScheduledGameResult(receipt.response_json, request)
      const linked = this.gameProvisionByCommand(request.commandId)
      if (!linked || linked.match_id !== request.matchId
        || linked.game_number !== stored.gameNumber
        || linked.game_id !== stored.gameId
        || linked.binding_json !== JSON.stringify(stored.binding)
        || linked.match_revision !== stored.matchRevision) {
        throw new Error('assigned game receipt does not match its durable linkage')
      }
      const latest = this.gameProvision(linked.match_id, linked.game_number)
      if (!latest) throw new Error('assigned game linkage disappeared during retry')
      return roomSuccess({
        ...stored,
        idempotent: true,
        status: latest.status,
        binding: parseStoredCompetitionGameBinding(latest.binding_json),
      })
    }

    const current = this.meta()
    if (!current) return roomFailure('notInitialized', 'competition room is not initialized')
    if (current.competition_id !== request.competitionId) {
      return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
    }
    const caller = this.acl(request.callerUserId)
    if (!caller || !caller.active || caller.role !== 'organizer'
      || request.callerUserId !== current.organizer_user_id) {
      return roomFailure('forbidden', 'only the active persisted organizer may provision a scheduled game')
    }
    const row = this.match(request.matchId)
    if (!row) return roomFailure('matchNotFound', 'match row does not exist')
    if (row.revision !== request.expectedMatchRevision) {
      return roomFailure('matchRevisionConflict', 'expected match revision does not match match revision')
    }
    if (request.trustedTimeMs < row.last_trusted_time_ms
      || request.trustedTimeMs < current.updated_trusted_time_ms) {
      return roomFailure('trustedTimeRegression', 'trusted server time cannot move backwards')
    }
    const serverState = parseServerState(current.server_state_json)
    let matchViewResponse: CompetitionEngineResponse
    try {
      matchViewResponse = await this.engine({
        serverActor: actorFor('organizer', request.callerUserId),
        trustedTimeMs: request.trustedTimeMs,
        operation: { type: 'matchView', matchId: request.matchId },
        serverState,
      })
    } catch {
      return roomFailure('engineProtocolError', 'match checkpoint replay failed')
    }
    if (!isValidStoredMatchView(matchViewResponse, serverState, row)) {
      return roomFailure('engineInvalidMatchCheckpoint', 'match checkpoint replay failed')
    }
    const intent = deriveProvisionIntent(row, serverState, null, null)
    if (!intent.ok) return roomFailure(intent.code, intent.message)
    if (intent.value.gameNumber === 1) {
      return roomFailure(
        'initialGameRequiresTrustedAssignment',
        'the first scheduled game requires the trusted initial provision RPC',
      )
    }
    if (this.gameProvision(request.matchId, intent.value.gameNumber)) {
      return roomFailure('gameAlreadyLinked', 'the scheduled game already has a game room linkage')
    }

    let exportResponse: CompetitionEngineResponse
    try {
      exportResponse = await this.engine({
        serverActor: actorFor('organizer', request.callerUserId),
        trustedTimeMs: request.trustedTimeMs,
        operation: { type: 'registrationExportLockedRoster' },
        serverState,
      })
    } catch {
      return roomFailure('engineProtocolError', 'locked roster export request failed')
    }
    if (!exportResponse.ok || exportResponse.operation !== 'registrationExportLockedRoster'
      || !sameJson(exportResponse.serverState, serverState)
      || !isJsonObject(exportResponse.public)
      || !hasExactKeys(exportResponse.public, ['exported'])
      || exportResponse.public.exported !== true) {
      return roomFailure('engineProtocolError', 'locked roster export returned an invalid response')
    }
    const roundContext = this.publishedRoundBindingContext(request.matchId)
    if (!roundContext) return roomFailure('roundNotPublishable', 'the scheduled match has no published round binding')
    const provision = await deriveScheduledGameProvision(
      exportResponse.private,
      request.competitionId,
      request.matchId,
      current.organizer_user_id,
      row.revision,
      intent.value,
      roundContext,
    )
    if (!provision.ok) return roomFailure(provision.code, provision.message)
    const { gameId, binding } = provision.value

    const initial = roomSuccess<CompetitionRoomProvisionAssignedScheduledGameValue>({
      competitionId: request.competitionId,
      matchId: request.matchId,
      matchRevision: row.revision,
      gameNumber: intent.value.gameNumber,
      gameId,
      commandId: request.commandId,
      idempotent: false,
      status: 'pending',
      binding,
    })
    this.persistScheduledGameProvision({
      current,
      row,
      matchId: request.matchId,
      gameNumber: intent.value.gameNumber,
      gameId,
      binding,
      nextGame: intent.value.nextGame,
      commandId: request.commandId,
      fingerprint,
      trustedTimeMs: request.trustedTimeMs,
      actorUserId: request.callerUserId,
      role: 'organizer',
      operationType: 'provisionAssignedScheduledGame',
      receiptKind: 'provisionAssignedScheduledGame',
      receiptResponse: initial,
    })

    const linked = this.gameProvision(request.matchId, intent.value.gameNumber)
    if (!linked) throw new Error('assigned game linkage disappeared after commit')
    return roomSuccess(this.gameProvisionAssignedValue(request.competitionId, linked, false))
  }

  private async provisionCaptainAssignedScheduledGameInternal(
    request: CompetitionRoomProvisionCaptainAssignedScheduledGameRequest,
  ): Promise<CompetitionRoomProvisionCaptainAssignedScheduledGameResult> {
    const validation = validateProvisionCaptainAssignedScheduledGameRequest(request)
    if (validation) return roomFailure(validation.code, validation.message)

    const fingerprint = await provisionCaptainAssignedScheduledGameFingerprint(request)
    const receipt = this.commandReceipt(request.commandId)
    if (receipt) {
      if (receipt.receipt_kind !== 'provisionCaptainAssignedScheduledGame'
        || receipt.match_id !== request.matchId || receipt.fingerprint !== fingerprint) {
        return roomFailure('commandIdConflict', 'command id was used with another payload')
      }
      const stored = await parseStoredProvisionCaptainAssignedScheduledGameResult(
        receipt.response_json, request,
      )
      const linked = this.gameProvisionByCommand(request.commandId)
      if (!linked || linked.match_id !== request.matchId
        || linked.game_number !== stored.gameNumber || linked.game_id !== stored.gameId
        || linked.match_revision !== stored.matchRevision || linked.fingerprint !== fingerprint) {
        throw new Error('captain game receipt does not match its durable linkage')
      }
      const binding = parseStoredCompetitionGameBinding(linked.binding_json)
      if (binding.competitionId !== request.competitionId || binding.matchId !== request.matchId
        || binding.gameNumber !== stored.gameNumber
        || binding.expectedMatchRevision !== stored.matchRevision) {
        throw new Error('captain game linkage binding does not match its receipt')
      }
      const latest = this.gameProvision(linked.match_id, linked.game_number)
      if (!latest) throw new Error('captain game linkage disappeared during retry')
      return roomSuccess({ ...stored, idempotent: true, status: latest.status })
    }

    const current = this.meta()
    if (!current) return roomFailure('notInitialized', 'competition room is not initialized')
    if (current.competition_id !== request.competitionId) {
      return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
    }
    const caller = this.acl(request.callerUserId)
    if (!caller || !caller.active || caller.role !== 'player' || caller.participant !== 1) {
      return roomFailure('forbidden', 'only an active participant captain may provision this game')
    }
    const row = this.match(request.matchId)
    if (!row) return roomFailure('matchNotFound', 'match row does not exist')
    if (row.revision !== request.expectedMatchRevision) {
      return roomFailure('matchRevisionConflict', 'expected match revision does not match match revision')
    }
    if (request.trustedTimeMs < row.last_trusted_time_ms
      || request.trustedTimeMs < current.updated_trusted_time_ms) {
      return roomFailure('trustedTimeRegression', 'trusted server time cannot move backwards')
    }

    let serverState: CompetitionServerState
    try {
      serverState = parseServerState(current.server_state_json)
    } catch {
      return roomFailure('matchNotProvisionable', 'persisted competition match state is invalid')
    }
    let view: CompetitionEngineResponse
    try {
      view = await this.engine({
        serverActor: actorFor('organizer', current.organizer_user_id),
        trustedTimeMs: request.trustedTimeMs,
        operation: { type: 'matchView', matchId: request.matchId },
        serverState,
      })
    } catch {
      return roomFailure('engineProtocolError', 'match checkpoint replay failed')
    }
    if (!isValidStoredMatchView(view, serverState, row)) {
      return roomFailure('engineInvalidMatchCheckpoint', 'match checkpoint replay failed')
    }
    const intent = deriveCaptainProvisionIntent(row, serverState)
    if (!intent.ok) return roomFailure(intent.code, intent.message)
    const currentNextGame = intent.value.publicMatch.nextGame
    if (!isJsonObject(currentNextGame) || !safeNonNegativeInteger(currentNextGame.gameNumber)) {
      return roomFailure('matchNotProvisionable', 'captain assignment is not a provisionable later game')
    }
    const gameNumber = currentNextGame.gameNumber
    const assignedFirstPlayerTeam = intent.value.selectedFirstPlayerTeam
    if (gameNumber <= 1) {
      return roomFailure('matchNotProvisionable', 'captain assignment is not a provisionable later game')
    }
    if (this.gameProvision(request.matchId, gameNumber)) {
      return roomFailure('gameAlreadyLinked', 'the scheduled game already has a game room linkage')
    }

    const assignmentReceipt = this.commandReceipt(request.assignmentCommandId)
    const assignmentRequest: CompetitionRoomAssignNextFirstPlayerByCaptainRequest = {
      competitionId: request.competitionId,
      matchId: request.matchId,
      callerUserId: request.callerUserId,
      commandId: request.assignmentCommandId,
      expectedMatchRevision: request.expectedMatchRevision - 1,
      trustedTimeMs: request.trustedTimeMs,
      selectedFirstPlayerTeam: intent.value.selectedFirstPlayerTeam,
    }
    const assignmentFingerprint = await assignNextFirstPlayerByCaptainFingerprint(assignmentRequest)
    if (!assignmentReceipt || assignmentReceipt.receipt_kind !== 'assignNextFirstPlayerByCaptain'
      || assignmentReceipt.match_id !== request.matchId
      || assignmentReceipt.fingerprint !== assignmentFingerprint
      || request.assignmentCommandId === request.commandId) {
      return roomFailure('assignmentNotCurrent', 'captain assignment receipt is not current for this match')
    }
    let assignment
    try {
      assignment = parseStoredCaptainFirstPlayerResult(assignmentReceipt.response_json, assignmentRequest)
    } catch {
      return roomFailure('assignmentNotCurrent', 'captain assignment receipt is invalid')
    }
    const currentProjection = parseOptionalProjection(row.match_projection_json)
    if (!currentProjection || assignment.matchRevision !== row.revision
      || !sameJson(assignment.matchProjection, currentProjection)) {
      return roomFailure('assignmentNotCurrent', 'captain assignment receipt does not match the current match')
    }
    const checkpoint = parseJsonObject(row.match_checkpoint_json, 'match checkpoint')
    const lastHistoryEvent = Array.isArray(checkpoint.history)
      ? checkpoint.history[checkpoint.history.length - 1]
      : undefined
    if (!isJsonObject(lastHistoryEvent)
      || !hasExactKeys(lastHistoryEvent, ['type', 'team', 'reason'])
      || lastHistoryEvent.type !== 'assignFirstPlayer'
      || lastHistoryEvent.team !== intent.value.selectedFirstPlayerTeam
      || lastHistoryEvent.reason !== CAPTAIN_FIRST_PLAYER_REASON) {
      return roomFailure('assignmentNotCurrent', 'current match history does not contain the captain assignment')
    }
    const assignmentAudits = this.storage.sql.exec<AuditRow>(
      `SELECT id, match_id, actor_user_id, role, operation_type, command_id, revision,
              outcome_code, trusted_time_ms, reason
       FROM competition_audit WHERE command_id = ?`,
      request.assignmentCommandId,
    ).toArray()
    if (assignmentAudits.length !== 1) {
      return roomFailure('assignmentNotCurrent', 'captain assignment audit record is missing or ambiguous')
    }
    const assignmentAudit = assignmentAudits[0]!
    if (!isValidCaptainAssignmentAudit(
      assignmentAudit, request, row.revision, row.last_trusted_time_ms,
    )) {
      return roomFailure('assignmentNotCurrent', 'captain assignment audit record does not match')
    }

    let exportResponse: CompetitionEngineResponse
    try {
      exportResponse = await this.engine({
        serverActor: actorFor('organizer', current.organizer_user_id),
        trustedTimeMs: request.trustedTimeMs,
        operation: { type: 'registrationExportLockedRoster' },
        serverState,
      })
    } catch {
      return roomFailure('engineProtocolError', 'locked roster export request failed')
    }
    if (!exportResponse.ok || exportResponse.operation !== 'registrationExportLockedRoster'
      || !hasExactKeys(exportResponse as unknown as JsonObject, ['ok', 'operation', 'public', 'private', 'serverState'])
      || !sameJson(exportResponse.serverState, serverState)
      || !isJsonObject(exportResponse.public)
      || !hasExactKeys(exportResponse.public, ['exported'])
      || exportResponse.public.exported !== true) {
      return roomFailure('engineProtocolError', 'locked roster export returned an invalid response')
    }
    const rawRoster = exportResponse.private
    const roster = deriveLockedRosterBinding(
      rawRoster,
      intent.value.teams,
      current.organizer_user_id,
      intent.value.selectedFirstPlayerTeam,
      null,
      null,
    )
    if (!roster.ok) return roomFailure(roster.code, roster.message)
    const losingTeamRoster = isJsonObject(rawRoster) && Array.isArray(rawRoster.teams)
      ? rawRoster.teams.find(team => isJsonObject(team) && team.teamId === intent.value.losingTeam)
      : undefined
    if (!isJsonObject(losingTeamRoster)
      || losingTeamRoster.captain !== request.callerUserId
      || !Array.isArray(losingTeamRoster.members)
      || !losingTeamRoster.members.includes(request.callerUserId)) {
      return roomFailure('forbidden', 'only the active losing-team captain may provision this game')
    }

    const provisionIntent: ProvisionIntent = {
      teams: intent.value.teams,
      gameNumber,
      startKind: currentNextGame.kind as GameStartKind,
      assignedFirstPlayerTeam,
      nextGame: currentNextGame,
    }
    const roundContext = this.publishedRoundBindingContext(request.matchId)
    if (!roundContext) return roomFailure('roundNotPublishable', 'the scheduled match has no published round binding')
    const provision = await deriveScheduledGameProvision(
      rawRoster,
      request.competitionId,
      request.matchId,
      current.organizer_user_id,
      row.revision,
      provisionIntent,
      roundContext,
    )
    if (!provision.ok) return roomFailure(provision.code, provision.message)
    const { gameId, binding } = provision.value
    const initial = roomSuccess<CompetitionRoomProvisionCaptainAssignedScheduledGameValue>({
      competitionId: request.competitionId,
      matchId: request.matchId,
      matchRevision: row.revision,
      gameNumber,
      gameId,
      commandId: request.commandId,
      idempotent: false,
      status: 'pending',
    })
    this.persistScheduledGameProvision({
      current,
      row,
      matchId: request.matchId,
      gameNumber,
      gameId,
      binding,
      nextGame: provisionIntent.nextGame,
      commandId: request.commandId,
      fingerprint,
      trustedTimeMs: request.trustedTimeMs,
      actorUserId: request.callerUserId,
      role: 'player',
      operationType: 'provisionCaptainAssignedScheduledGame',
      receiptKind: 'provisionCaptainAssignedScheduledGame',
      receiptResponse: initial,
      reason: '已驗證隊長先手指派',
      additionalCheck: () => {
        const latestAssignmentReceipt = this.commandReceipt(request.assignmentCommandId)
        const latestAssignmentAudit = this.storage.sql.exec<AuditRow>(
          `SELECT id, match_id, actor_user_id, role, operation_type, command_id, revision,
                  outcome_code, trusted_time_ms, reason
           FROM competition_audit WHERE command_id = ?`,
          request.assignmentCommandId,
        ).toArray()
        if (!latestAssignmentReceipt
          || latestAssignmentReceipt.receipt_kind !== 'assignNextFirstPlayerByCaptain'
          || latestAssignmentReceipt.match_id !== request.matchId
          || latestAssignmentReceipt.fingerprint !== assignmentFingerprint
          || latestAssignmentReceipt.response_json !== assignmentReceipt.response_json
          || latestAssignmentAudit.length !== 1) return false
        try {
          const latestAssignment = parseStoredCaptainFirstPlayerResult(
            latestAssignmentReceipt.response_json, assignmentRequest,
          )
          const latestAudit = latestAssignmentAudit[0]!
          return latestAssignment.matchRevision === row.revision
            && sameJson(latestAssignment.matchProjection, currentProjection)
            && sameCaptainAssignmentAudit(latestAudit, assignmentAudit)
            && isValidCaptainAssignmentAudit(
              latestAudit, request, row.revision, row.last_trusted_time_ms,
            )
        } catch {
          return false
        }
      },
    })
    const linked = this.gameProvision(request.matchId, gameNumber)
    if (!linked) throw new Error('captain game linkage disappeared after commit')
    return roomSuccess({ ...initial.value, status: linked.status })
  }

  private async provisionInitialScheduledGameInternal(
    request: CompetitionRoomProvisionInitialScheduledGameRequest,
  ): Promise<CompetitionRoomProvisionInitialScheduledGameResult> {
    const validation = validateProvisionInitialScheduledGameRequest(request)
    if (validation) return roomFailure(validation.code, validation.message)

    const fingerprint = await provisionInitialScheduledGameFingerprint(request)
    const receipt = this.commandReceipt(request.commandId)
    if (receipt) {
      if (receipt.receipt_kind !== 'provisionInitialScheduledGame'
        || receipt.match_id !== request.matchId || receipt.fingerprint !== fingerprint) {
        return roomFailure('commandIdConflict', 'command id was used with another payload')
      }
      const stored = await parseStoredProvisionInitialScheduledGameResult(
        receipt.response_json,
        request,
      )
      const linked = this.gameProvisionByCommand(request.commandId)
      if (!linked || linked.match_id !== request.matchId || linked.game_number !== 1
        || linked.game_id !== stored.gameId || linked.binding_json !== JSON.stringify(stored.binding)
        || linked.match_revision !== stored.matchRevision) {
        throw new Error('initial game receipt does not match its durable linkage')
      }
      const latest = this.gameProvision(request.matchId, 1)
      if (!latest) throw new Error('initial game linkage disappeared during retry')
      return roomSuccess({
        ...stored,
        idempotent: true,
        status: latest.status,
        binding: parseStoredCompetitionGameBinding(latest.binding_json),
      })
    }

    const current = this.meta()
    if (!current) return roomFailure('notInitialized', 'competition room is not initialized')
    if (current.competition_id !== request.competitionId) {
      return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
    }
    const caller = this.acl(request.callerUserId)
    if (!caller || !caller.active || caller.role !== 'organizer'
      || request.callerUserId !== current.organizer_user_id) {
      return roomFailure('forbidden', 'only the active persisted organizer may provision the first game')
    }
    const row = this.match(request.matchId)
    if (!row) return roomFailure('matchNotFound', 'match row does not exist')
    if (row.revision !== request.expectedMatchRevision) {
      return roomFailure('matchRevisionConflict', 'expected match revision does not match match revision')
    }
    if (request.trustedTimeMs < row.last_trusted_time_ms
      || request.trustedTimeMs < current.updated_trusted_time_ms) {
      return roomFailure('trustedTimeRegression', 'trusted server time cannot move backwards')
    }
    if (this.gameProvision(request.matchId, 1) || this.latestGameProvision(request.matchId)) {
      return roomFailure('gameAlreadyLinked', 'the scheduled match already has a game room linkage')
    }

    let pending: InitialScheduledGameState
    try {
      pending = deriveInitialScheduledGameState(current, row, request)
    } catch {
      return roomFailure('matchNotProvisionable', 'first game state cannot be safely provisioned')
    }
    let randomIndex: number
    try {
      randomIndex = this.randomBit()
    } catch {
      return roomFailure('randomSourceFailure', 'trusted random source failed')
    }
    if (randomIndex !== 0 && randomIndex !== 1) {
      return roomFailure('randomSourceFailure', 'trusted random source returned an invalid bit')
    }
    const firstPlayerTeam = pending.teams[randomIndex]
    const otherTeam = pending.teams[1 - randomIndex]
    if (!firstPlayerTeam || !otherTeam) {
      return roomFailure('invalidTournamentCheckpoint', 'scheduled teams are invalid')
    }
    let assignedCheckpoint: JsonObject
    let assignedProjection: CompetitionRoomMatchProjection
    try {
      const response = await this.engine({
        serverActor: actorFor('organizer', request.callerUserId),
        trustedTimeMs: request.trustedTimeMs,
        operation: engineOperation({
          type: 'matchAssignFirstPlayer',
          matchId: request.matchId,
          team: firstPlayerTeam,
          reason: INITIAL_FIRST_PLAYER_REASON,
        }),
        serverState: {
          registration: null,
          clock: pending.clock,
          tournament: null,
          matches: { [request.matchId]: pending.checkpoint },
        },
      })
      const validated = validateInitialFirstPlayerResponse(
        response,
        request.matchId,
        pending,
        firstPlayerTeam,
        INITIAL_FIRST_PLAYER_REASON,
      )
      if (!validated) return roomFailure('engineInvalidMatchCheckpoint', 'first-player assignment returned an invalid checkpoint')
      assignedCheckpoint = validated.checkpoint
      assignedProjection = validated.projection
    } catch {
      return roomFailure('engineProtocolError', 'first-player assignment request failed')
    }

    let exportResponse: CompetitionEngineResponse
    try {
      exportResponse = await this.engine({
        serverActor: actorFor('organizer', request.callerUserId),
        trustedTimeMs: request.trustedTimeMs,
        operation: { type: 'registrationExportLockedRoster' },
        serverState: parseServerState(current.server_state_json),
      })
    } catch {
      return roomFailure('engineProtocolError', 'locked roster export request failed')
    }
    if (!exportResponse.ok || exportResponse.operation !== 'registrationExportLockedRoster'
      || !sameJson(exportResponse.serverState, parseServerState(current.server_state_json))
      || !isJsonObject(exportResponse.public)
      || !hasExactKeys(exportResponse.public, ['exported'])
      || exportResponse.public.exported !== true) {
      return roomFailure('engineProtocolError', 'locked roster export returned an invalid response')
    }
    const roster = deriveLockedRosterBinding(
      exportResponse.private,
      pending.teams,
      current.organizer_user_id,
      firstPlayerTeam,
      null,
      null,
    )
    if (!roster.ok) return roomFailure(roster.code, roster.message)

    const gameId = `competition-game-${await semanticFingerprint({
      kind: 'competition-game',
      competitionId: request.competitionId,
      matchId: request.matchId,
      gameNumber: 1,
    })}`
    const roundContext = this.publishedRoundBindingContext(request.matchId)
    if (!roundContext) return roomFailure('roundNotPublishable', 'the scheduled match has no published round binding')
    const binding: CompetitionGameBinding = {
      roundRef: roundContext.roundRef,
      roundStartMs: roundContext.roundStartMs,
      competitionId: request.competitionId,
      matchId: request.matchId,
      gameNumber: 1,
      organizerUserId: current.organizer_user_id,
      expectedMatchRevision: 2,
      startKind: { type: 'ordinaryNewGame' },
      ruleVersion: roster.value.ruleVersion,
      ruleset: roster.value.ruleset,
      enabledRuleModules: roster.value.enabledRuleModules,
      initialHp: pending.teams.map(team => ({ team, hp: roster.value.initialHp })),
      players: roster.value.players,
      deckSnapshot: roster.value.deckSnapshot,
      firstPlayer: roster.value.firstPlayer,
      turnOrder: roster.value.turnOrder,
      deckSeed: crypto.randomUUID(),
    }
    try {
      validateCompetitionGameBinding(binding)
    } catch {
      return roomFailure('invalidLockedRoster', 'locked roster cannot produce a valid game binding')
    }
    if (!sameJson(assignedCheckpoint.config, pending.checkpoint.config)
      || !sameJson(assignedCheckpoint.history, [
        ...(pending.checkpoint.history as JsonValue[]),
        { type: 'assignFirstPlayer', team: firstPlayerTeam, reason: INITIAL_FIRST_PLAYER_REASON },
      ])) {
      return roomFailure('engineInvalidMatchCheckpoint', 'first-player assignment changed unrelated match data')
    }

    const revision = row.revision + 1
    const initial = roomSuccess<CompetitionRoomProvisionInitialScheduledGameValue>({
      competitionId: request.competitionId,
      matchId: request.matchId,
      matchRevision: revision,
      gameNumber: 1,
      gameId,
      commandId: request.commandId,
      idempotent: false,
      status: 'pending',
      binding,
    })
    const globalState = parseServerState(current.server_state_json)
    const updatedGlobalState: CompetitionServerState = {
      ...globalState,
      matches: { ...(globalState.matches ?? {}), [request.matchId]: assignedCheckpoint },
    }
    this.storage.transactionSync(() => {
      const latestMeta = this.meta()
      const latestRow = this.match(request.matchId)
      if (!latestMeta || latestMeta.competition_id !== current.competition_id
        || latestMeta.organizer_user_id !== current.organizer_user_id
        || latestMeta.revision !== current.revision
        || latestMeta.updated_trusted_time_ms !== current.updated_trusted_time_ms
        || latestMeta.server_state_json !== current.server_state_json
        || !latestRow || latestRow.revision !== row.revision
        || latestRow.stage_id !== row.stage_id
        || latestRow.last_trusted_time_ms !== row.last_trusted_time_ms
        || latestRow.match_checkpoint_json !== row.match_checkpoint_json
        || latestRow.clock_checkpoint_json !== row.clock_checkpoint_json
        || latestRow.match_projection_json !== row.match_projection_json
        || latestRow.clock_projection_json !== row.clock_projection_json
        || this.gameProvision(request.matchId, 1) || this.latestGameProvision(request.matchId)) {
        throw new Error('initial scheduled game state changed before atomic commit')
      }
      this.storage.sql.exec(
        `UPDATE competition_matches
         SET match_checkpoint_json = ?, match_projection_json = ?, revision = ?, last_trusted_time_ms = ?
         WHERE match_id = ? AND revision = 1`,
        JSON.stringify(assignedCheckpoint),
        JSON.stringify(assignedProjection),
        revision,
        request.trustedTimeMs,
        request.matchId,
      )
      this.storage.sql.exec(
        'UPDATE competition_meta SET server_state_json = ? WHERE singleton = 1',
        JSON.stringify(updatedGlobalState),
      )
      this.storage.sql.exec(
        `INSERT INTO competition_game_provision
          (match_id, game_number, game_id, binding_json, match_revision, next_game_json,
           command_id, fingerprint, status, attempts, next_attempt_at_ms, error_code,
           created_trusted_time_ms)
         VALUES (?, 1, ?, ?, ?, ?, ?, ?, 'pending', 0, ?, NULL, ?)`,
        request.matchId,
        gameId,
        JSON.stringify(binding),
        revision,
        JSON.stringify({ gameNumber: 1, kind: { type: 'ordinaryNewGame' }, firstPlayer: { type: 'assigned', team: firstPlayerTeam } }),
        request.commandId,
        fingerprint,
        Date.now(),
        request.trustedTimeMs,
      )
      this.insertAudit({
        actorUserId: request.callerUserId,
        role: 'organizer',
        matchId: request.matchId,
        operationType: 'provisionInitialScheduledGame',
        commandId: request.commandId,
        revision,
        outcomeCode: 'pending',
        trustedTimeMs: request.trustedTimeMs,
        reason: INITIAL_FIRST_PLAYER_REASON,
      })
      this.insertCommandReceipt(
        request.commandId,
        'provisionInitialScheduledGame',
        fingerprint,
        initial,
        request.matchId,
      )
    })

    const delivered = this.gameProvision(request.matchId, 1)
    if (!delivered) throw new Error('initial game linkage disappeared after commit')
    return roomSuccess({
      ...initial.value,
      status: delivered.status,
      binding,
    })
  }

  private async readProvisionedGameInternal(
    request: CompetitionRoomGameProvisionReadRequest,
  ): Promise<CompetitionRoomResult<CompetitionRoomGameProvisionReadValue>> {
    if (!isJsonObject(request)
      || !hasExactKeys(request as unknown as JsonObject, ['competitionId', 'matchId', 'callerUserId'])
      || !nonEmpty(request.competitionId) || !nonEmpty(request.matchId)
      || !nonEmpty(request.callerUserId)) {
      return roomFailure('invalidRequest', 'competition id, match id and caller are required')
    }
    const current = this.meta()
    if (!current) return roomFailure('notInitialized', 'competition room is not initialized')
    if (current.competition_id !== request.competitionId) {
      return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
    }
    const caller = this.acl(request.callerUserId)
    if (!caller || !caller.active || caller.role !== 'organizer') {
      return roomFailure('forbidden', 'only the active organizer may read a game linkage')
    }
    const linked = this.latestGameProvision(request.matchId)
    if (!linked) return roomFailure('gameNotLinked', 'the match has no game room linkage')
    const value = this.gameProvisionValue(current.competition_id, linked, false)
    return roomSuccess({
      competitionId: value.competitionId,
      matchId: value.matchId,
      gameNumber: value.gameNumber,
      gameId: value.gameId,
      status: value.status,
      binding: value.binding,
    })
  }

  private async readParticipantGameInternal(
    request: CompetitionRoomParticipantGameReadRequest,
  ): Promise<CompetitionRoomParticipantGameReadResult> {
    if (!isJsonObject(request)
      || !hasExactKeys(request, ['competitionId', 'matchId', 'callerUserId'])
      || !nonEmpty(request.competitionId) || !nonEmpty(request.matchId)
      || !nonEmpty(request.callerUserId)) {
      return roomFailure('invalidRequest', 'competition id, match id and caller are required')
    }
    const current = this.meta()
    if (!current) return roomFailure('notInitialized', 'competition room is not initialized')
    if (current.competition_id !== request.competitionId) {
      return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
    }
    const caller = this.acl(request.callerUserId)
    if (!caller || caller.active !== 1 || caller.role !== 'player' || caller.participant !== 1) {
      return roomFailure('forbidden', 'caller is not an active match participant')
    }
    const organizer = this.acl(current.organizer_user_id)
    if (!organizer || organizer.active !== 1 || organizer.role !== 'organizer'
      || organizer.participant !== 0) {
      return roomFailure('invalidMatchCheckpoint', 'participant game state is unavailable')
    }
    const row = this.match(request.matchId)
    if (!row) return roomFailure('matchNotFound', 'match row does not exist')
    if (!Number.isSafeInteger(row.revision) || row.revision < 1
      || !safeNonNegativeInteger(row.last_trusted_time_ms)
      || !safeNonNegativeInteger(current.updated_trusted_time_ms)) {
      return roomFailure('invalidMatchCheckpoint', 'participant game state is unavailable')
    }

    const serverState = parseParticipantServerState(current.server_state_json)
    if (!serverState
      || registrationCheckpointRevision(serverState.registration, current.organizer_user_id) === null) {
      return roomFailure('invalidMatchCheckpoint', 'participant game state is unavailable')
    }
    let matchView: CompetitionEngineResponse
    try {
      matchView = await this.engine({
        serverActor: actorFor('organizer', current.organizer_user_id),
        trustedTimeMs: current.updated_trusted_time_ms,
        operation: { type: 'matchView', matchId: request.matchId },
        serverState,
      })
    } catch {
      return roomFailure('invalidMatchCheckpoint', 'participant game state is unavailable')
    }
    const matchTeams = participantMatchTeams(row, serverState)
    if (!isValidStoredMatchView(matchView, serverState, row) || !matchTeams) {
      return roomFailure('invalidMatchCheckpoint', 'participant game state is unavailable')
    }

    let rosterResponse: CompetitionEngineResponse
    try {
      rosterResponse = await this.engine({
        serverActor: actorFor('organizer', current.organizer_user_id),
        trustedTimeMs: current.updated_trusted_time_ms,
        operation: { type: 'registrationExportLockedRoster' },
        serverState,
      })
    } catch {
      return roomFailure('invalidLockedRoster', 'participant game roster is unavailable')
    }
    const rawRosterResponse = rosterResponse as unknown as JsonObject
    if (!isJsonObject(rawRosterResponse)
      || !hasExactKeys(rawRosterResponse, ['ok', 'operation', 'public', 'private', 'serverState'])
      || rawRosterResponse.ok !== true
      || rawRosterResponse.operation !== 'registrationExportLockedRoster'
      || !isJsonObject(rawRosterResponse.public)
      || !hasExactKeys(rawRosterResponse.public, ['exported'])
      || rawRosterResponse.public.exported !== true
      || !sameJson(rawRosterResponse.serverState, serverState)
      || !parseLockedRoster(rawRosterResponse.private, current.organizer_user_id)) {
      return roomFailure('invalidLockedRoster', 'participant game roster is unavailable')
    }
    const rawRoster = rawRosterResponse.private
    if (!isJsonObject(rawRoster) || !Array.isArray(rawRoster.teams)) {
      return roomFailure('invalidLockedRoster', 'participant game roster is unavailable')
    }
    const firstTeamRoster = rawRoster.teams.find(team => isJsonObject(team)
      && team.teamId === matchTeams[0])
    const secondTeamRoster = rawRoster.teams.find(team => isJsonObject(team)
      && team.teamId === matchTeams[1])
    if (!isJsonObject(firstTeamRoster) || !Array.isArray(firstTeamRoster.members)
      || !isJsonObject(secondTeamRoster) || !Array.isArray(secondTeamRoster.members)) {
      return roomFailure('invalidLockedRoster', 'participant game roster is unavailable')
    }
    if (![...firstTeamRoster.members, ...secondTeamRoster.members].includes(request.callerUserId)) {
      return roomFailure('forbidden', 'caller is not an active match participant')
    }

    return this.readVerifiedCurrentGame(current, row, serverState, rawRoster)
  }

  /** 共用既有純 linkage 驗證；ACL 與 locked roster export 必須先由各專用入口核實。 */
  private async readVerifiedCurrentGame(
    current: CompetitionMetaRow,
    row: MatchRow,
    serverState: CompetitionServerState,
    rawRoster: JsonObject,
  ): Promise<CompetitionRoomParticipantGameReadResult> {
    const request = { competitionId: current.competition_id, matchId: row.match_id }
    const projection = parseOptionalProjection(row.match_projection_json)
    const match = isJsonObject(projection?.public) && isJsonObject(projection.public.match)
      ? projection.public.match : null
    if (!match) return roomFailure('invalidMatchCheckpoint', 'participant game state is unavailable')
    if (match.status === 'completed' || match.status === 'awaitingReferee') {
      return roomSuccess({
        competitionId: request.competitionId,
        matchId: request.matchId,
        matchRevision: row.revision,
        game: null,
      })
    }

    const intent = deriveProvisionIntent(row, serverState, null, null)
    if (!intent.ok) {
      if (intent.code === 'firstPlayerPending') {
        return roomSuccess({
          competitionId: request.competitionId,
          matchId: request.matchId,
          matchRevision: row.revision,
          game: null,
        })
      }
      return roomFailure('invalidMatchCheckpoint', 'participant game state is unavailable')
    }
    const rosterBinding = deriveLockedRosterBinding(
      rawRoster,
      intent.value.teams,
      current.organizer_user_id,
      intent.value.assignedFirstPlayerTeam,
      null,
      null,
    )
    if (!rosterBinding.ok) {
      return roomFailure('invalidLockedRoster', 'participant game roster is unavailable')
    }
    const linked = this.gameProvision(request.matchId, intent.value.gameNumber)
      ?? await this.confirmedRoundParticipantLink(request.matchId, intent.value.gameNumber)
    if (!linked) {
      return roomSuccess({
        competitionId: request.competitionId,
        matchId: request.matchId,
        matchRevision: row.revision,
        game: null,
      })
    }

    let binding: CompetitionGameBinding
    let storedNextGame: unknown
    try {
      const parsedBinding = JSON.parse(linked.binding_json) as unknown
      if (!isJsonObject(parsedBinding) || !hasExactKeys(parsedBinding, [
        'roundRef', 'roundStartMs', 'competitionId', 'matchId', 'gameNumber',
        'organizerUserId', 'expectedMatchRevision',
        'startKind', 'ruleVersion', 'ruleset', 'enabledRuleModules', 'initialHp', 'players',
        'deckSnapshot', 'firstPlayer', 'turnOrder', 'deckSeed',
      ])) throw new Error('invalid participant game binding')
      validateCompetitionGameBinding(parsedBinding)
      binding = parsedBinding
      storedNextGame = JSON.parse(linked.next_game_json) as unknown
    } catch {
      return roomFailure('invalidGameProvision', 'participant game linkage is unavailable')
    }
    const expectedGameIds = await Promise.all([
      semanticFingerprint({
        kind: 'competition-game',
        competitionId: request.competitionId,
        matchId: request.matchId,
        gameNumber: intent.value.gameNumber,
      }),
      semanticFingerprint({
        kind: 'round-initial-game',
        roundRef: binding.roundRef,
        matchId: request.matchId,
        gameNumber: intent.value.gameNumber,
      }),
    ])
    const expectedGameIdsWithPrefix = expectedGameIds.map(value => `competition-game-${value}`)
    if (!['pending', 'provisioned', 'blocked'].includes(linked.status)
      || linked.match_id !== request.matchId
      || linked.game_number !== intent.value.gameNumber
      || !nonEmpty(linked.game_id)
      || !expectedGameIdsWithPrefix.includes(linked.game_id)
      || !nonEmpty(linked.command_id)
      || !/^[0-9a-f]{64}$/.test(linked.fingerprint)
      || !safeNonNegativeInteger(linked.attempts)
      || !safeNonNegativeInteger(linked.next_attempt_at_ms)
      || !safeNonNegativeInteger(linked.created_trusted_time_ms)
      || (linked.error_code !== null && !nonEmpty(linked.error_code))
      || !Number.isSafeInteger(linked.match_revision)
      || linked.match_revision !== binding.expectedMatchRevision
      || binding.expectedMatchRevision > row.revision
      || binding.competitionId !== request.competitionId
      || binding.matchId !== request.matchId
      || binding.gameNumber !== intent.value.gameNumber
      || binding.organizerUserId !== current.organizer_user_id
      || !sameJson(binding.startKind, intent.value.startKind)
      || binding.ruleVersion !== rosterBinding.value.ruleVersion
      || binding.ruleset !== rosterBinding.value.ruleset
      || !sameJson(binding.enabledRuleModules, rosterBinding.value.enabledRuleModules)
      || !sameJson(binding.initialHp, intent.value.teams.map(team => ({
        team,
        hp: rosterBinding.value.initialHp,
      })))
      || !sameJson(binding.players, rosterBinding.value.players)
      || !sameJson(binding.deckSnapshot, rosterBinding.value.deckSnapshot)
      || binding.firstPlayer !== rosterBinding.value.firstPlayer
      || !sameJson(binding.turnOrder, rosterBinding.value.turnOrder)
      || !isJsonObject(storedNextGame)
      || !sameJson(storedNextGame, intent.value.nextGame)) {
      return roomFailure('invalidGameProvision', 'participant game linkage is unavailable')
    }
    return roomSuccess({
      competitionId: request.competitionId,
      matchId: request.matchId,
      matchRevision: row.revision,
      game: linked.status === 'provisioned'
        ? { gameId: linked.game_id, gameNumber: intent.value.gameNumber }
        : null,
    })
  }

  /** 只將目前有效輪次中已取得實體房 ACK 的 ledger 投影為玩家可進入連結。 */
  private async confirmedRoundParticipantLink(
    matchId: string,
    gameNumber: number,
  ): Promise<GameProvisionRow | undefined> {
    if (gameNumber !== 1) return undefined
    const ledgers = this.storage.sql.exec<RoundRoomLedgerRow>(
      `SELECT * FROM competition_round_room_ledger
       WHERE match_id = ? AND game_number = 1 ORDER BY rowid DESC`, matchId,
    ).toArray()
    for (const ledger of ledgers) {
      if (!['confirmed', 'started'].includes(ledger.status)
        || !safeNonNegativeInteger(ledger.ack_confirmed_at_ms)) continue
      const header = this.storage.sql.exec<RoundPublicationRow>(
        'SELECT * FROM competition_round_publication WHERE round_generation_id = ?',
        ledger.round_generation_id,
      ).toArray()[0]
      const release = this.storage.sql.exec<RoundReleaseRow>(
        `SELECT * FROM competition_round_release_outbox
         WHERE round_generation_id = ? AND game_id = ?`,
        ledger.round_generation_id, ledger.game_id,
      ).toArray()[0]
      const current = this.meta()
      if (!header || !release || release.status !== 'completed' || !current || header.status !== 'released'
        || header.requires_referee !== 0
        || (header.release_not_before_ms > 0 && (Date.now() < header.release_not_before_ms
          || !this.roundHasCompleteTimelyCreationAcks(header, current.competition_id)
          || !this.roundHasCompleteTimelyReleaseAcks(header, current.competition_id)))
        || !sameJson(this.currentRoundPublication(header.stage_id, header.round_number), header)) continue
      try {
        const manifest = parseRoundManifest(header, current.competition_id)
        const expected = manifest.rooms.find(room => room.matchId === matchId && room.gameId === ledger.game_id)
        const binding: unknown = JSON.parse(ledger.binding_json)
        const bindingRef: unknown = JSON.parse(ledger.binding_ref_json)
        if (!expected || !isCompetitionGameBindingRef(bindingRef)
          || !sameJson(bindingRef, expected.bindingRef)
          || !isJsonObject(binding)) continue
        validateCompetitionGameBinding(binding)
        if (binding.competitionId !== current.competition_id || binding.matchId !== matchId
          || binding.gameNumber !== 1 || binding.roundStartMs !== manifest.roundStartMs
          || !sameJson(binding.roundRef, manifest.roundRef)) continue
        const fingerprint = await semanticFingerprint({
          kind: 'round-participant-game-link', roundRef: manifest.roundRef,
          matchId, gameId: ledger.game_id,
        })
        return {
          match_id: ledger.match_id,
          game_number: ledger.game_number,
          game_id: ledger.game_id,
          binding_json: ledger.binding_json,
          match_revision: ledger.match_revision,
          next_game_json: ledger.next_game_json,
          command_id: manifest.announcementCommandId,
          fingerprint,
          status: 'provisioned',
          attempts: 0,
          next_attempt_at_ms: 0,
          error_code: null,
          created_trusted_time_ms: header.announced_trusted_time_ms,
          claim_id: null,
          claim_version: 0,
        }
      } catch {
        continue
      }
    }
    return undefined
  }

  private async readMatchClockInternal(
    request: CompetitionRoomMatchClockReadRequest,
  ): Promise<CompetitionRoomMatchClockReadResult> {
    if (!isJsonObject(request)
      || !hasExactClockQueryKeys(request, ['competitionId', 'matchId', 'callerUserId', 'trustedTimeMs'])
      || !nonEmpty(request.competitionId) || !nonEmpty(request.matchId)
      || !nonEmpty(request.callerUserId) || !safeNonNegativeInteger(request.trustedTimeMs)) {
      return roomFailure('invalidRequest', 'clock query identity and trusted time are required')
    }
    const current = this.meta()
    if (!current) return roomFailure('notInitialized', 'competition room is not initialized')
    if (current.competition_id !== request.competitionId) {
      return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
    }
    const caller = this.acl(request.callerUserId)
    if (!caller || caller.active !== 1
      || !['organizer', 'referee', 'player'].includes(caller.role)
      || (caller.role === 'organizer' && caller.user_id !== current.organizer_user_id)
      || (caller.role === 'player' && caller.participant !== 1)) {
      return roomFailure('forbidden', 'caller cannot read this match clock')
    }
    const organizer = this.acl(current.organizer_user_id)
    if (!organizer || organizer.active !== 1 || organizer.role !== 'organizer' || organizer.participant !== 0) {
      return roomFailure('invalidMatchCheckpoint', 'clock query match state is unavailable')
    }
    const row = this.match(request.matchId)
    if (!row) return roomFailure('matchNotFound', 'match row does not exist')
    const serverState = parseParticipantServerState(current.server_state_json)
    if (!serverState || !Number.isSafeInteger(current.revision) || current.revision < 1
      || !serverState.tournament || !isTournamentForCompetition(serverState.tournament, current.competition_id)
      || !safeNonNegativeInteger(current.updated_trusted_time_ms)
      || !Number.isSafeInteger(row.revision) || row.revision < 1
      || !safeNonNegativeInteger(row.last_trusted_time_ms)
      || registrationCheckpointRevision(serverState.registration, current.organizer_user_id) === null) {
      return roomFailure('invalidMatchCheckpoint', 'clock query match state is unavailable')
    }
    const teams = participantMatchTeams(row, serverState)
    const checkpointJson = row.match_checkpoint_json
    const clockJson = row.clock_checkpoint_json
    // 持久欄位先做執行期驗證與型別收斂；缺少 scoped 時計不可退回 global 時計。
    if (typeof checkpointJson !== 'string') {
      return roomFailure('invalidMatchCheckpoint', 'clock query match state is unavailable')
    }
    if (typeof clockJson !== 'string') {
      return roomFailure('invalidClockCheckpoint', 'clock query checkpoint is unavailable')
    }
    let checkpoint: JsonObject
    let clock: JsonObject
    let projectionDeadline: number | null
    let matchStatus: CompetitionRoomMatchClockReadValue['matchStatus']
    try {
      checkpoint = parseJsonObject(checkpointJson, 'match checkpoint')
      clock = parseJsonObject(clockJson, 'clock checkpoint')
      const projection = parseOptionalProjection(row.match_projection_json)
      const publicMatch = isJsonObject(projection?.public) && isJsonObject(projection.public.match)
        ? projection.public.match : null
      const rawMatchProjection = row.match_projection_json === null ? null : JSON.parse(row.match_projection_json) as unknown
      if (!isJsonObject(rawMatchProjection)
        || !hasExactClockQueryKeys(rawMatchProjection, ['ok', 'operation', 'public'])
        || !teams || !Array.isArray(checkpoint.history) || row.revision < 1 + checkpoint.history.length
        || !publicMatch || !['open', 'awaitingFirstPlayer', 'awaitingReferee', 'completed'].includes(publicMatch.status as string)) {
        return roomFailure('invalidMatchCheckpoint', 'clock query match state is unavailable')
      }
      matchStatus = publicMatch.status as CompetitionRoomMatchClockReadValue['matchStatus']
      if (!isJsonObject(checkpoint.config) || !Array.isArray(checkpoint.config.referees)
        || checkpoint.config.referees.some(referee => !isJsonObject(referee)
          || !hasExactKeys(referee, ['refereeId', 'conflictedTeam']) || !nonEmpty(referee.refereeId)
          || (referee.conflictedTeam !== null && !nonEmpty(referee.conflictedTeam)))) {
        return roomFailure('invalidMatchCheckpoint', 'clock query match state is unavailable')
      }
      // 顯示權限只需 assigned；有利益衝突不代表可裁判，亦不借用 pause 的裁判授權。
      if (caller.role === 'referee' && !checkpoint.config.referees.some(referee => (
        isJsonObject(referee) && referee.refereeId === request.callerUserId
      ))) return roomFailure('forbidden', 'caller cannot read this match clock')
      const schedule = findScheduledMatch(serverState.tournament, request.matchId, true, true)
      const clockProjection = parseOptionalProjection(row.clock_projection_json)
      const rawClockProjection = row.clock_projection_json === null ? null : JSON.parse(row.clock_projection_json) as unknown
      const clockPublic = clockProjection?.operation === 'clockCreate'
        ? clockProjection.public
        : isJsonObject(clockProjection?.public) ? clockProjection.public.clock : null
      if (!isJsonObject(rawClockProjection)
        || !hasExactClockQueryKeys(rawClockProjection, ['ok', 'operation', 'public'])
        || !schedule || !isValidClockCheckpoint(clock, teams)
        || !sameJson(clock.config, schedule.clockConfig)
        || !Array.isArray(clock.teams)
        || !sameJson(clock.teams.map(team => isJsonObject(team) ? team.team : null), teams)
        || !safeNonNegativeInteger(clock.lastObservedMs) || clock.lastObservedMs > row.last_trusted_time_ms
        || !clockProjection || !clockProjection.operation.startsWith('clock')
        || clockProjection.operation === 'clockView'
        || validateClockPublicView(clockPublic ?? null, clock, teams) !== null) {
        return roomFailure('invalidClockCheckpoint', 'clock query checkpoint is unavailable')
      }
      projectionDeadline = storedClockProjectionDeadline(clockProjection)
    } catch {
      return roomFailure('invalidClockCheckpoint', 'clock query checkpoint is unavailable')
    }
    if (request.trustedTimeMs < Math.max(current.updated_trusted_time_ms, row.last_trusted_time_ms)) {
      return roomFailure('trustedTimeRegression', 'trusted server time cannot move backwards')
    }
    let matchView: CompetitionEngineResponse
    try {
      matchView = await this.engine({
        serverActor: actorFor(caller.role, request.callerUserId), trustedTimeMs: request.trustedTimeMs,
        operation: { type: 'matchView', matchId: request.matchId }, serverState: structuredClone(serverState),
      })
    } catch {
      return roomFailure('invalidMatchCheckpoint', 'clock query match state is unavailable')
    }
    if (!isJsonObject(matchView)
      || !hasExactClockQueryKeys(matchView, ['ok', 'operation', 'public', 'serverState'])
      || !isValidStoredMatchView(matchView, serverState, row)) {
      return roomFailure('invalidMatchCheckpoint', 'clock query match state is unavailable')
    }
    let rosterResponse: CompetitionEngineResponse
    try {
      rosterResponse = await this.engine({
        serverActor: actorFor('organizer', current.organizer_user_id), trustedTimeMs: request.trustedTimeMs,
        operation: { type: 'registrationExportLockedRoster' }, serverState: structuredClone(serverState),
      })
    } catch {
      return roomFailure('invalidLockedRoster', 'clock query roster is unavailable')
    }
    const rawRosterResponse = rosterResponse as unknown as JsonObject
    if (!isJsonObject(rawRosterResponse)
      || !hasExactClockQueryKeys(rawRosterResponse, ['ok', 'operation', 'public', 'private', 'serverState'])
      || rawRosterResponse.ok !== true || rawRosterResponse.operation !== 'registrationExportLockedRoster'
      || !isJsonObject(rawRosterResponse.public) || !hasExactClockQueryKeys(rawRosterResponse.public, ['exported'])
      || rawRosterResponse.public.exported !== true || !sameJson(rawRosterResponse.serverState, serverState)
      || !isJsonObject(rawRosterResponse.private)
      || !parseLockedRoster(rawRosterResponse.private, current.organizer_user_id)) {
      return roomFailure('invalidLockedRoster', 'clock query roster is unavailable')
    }
    const roster = rawRosterResponse.private
    const rosterTeams = roster.teams
    if (!Array.isArray(rosterTeams) || !teams.every(teamId => rosterTeams.some(team => (
      isJsonObject(team) && team.teamId === teamId && Array.isArray(team.members)
    )))) return roomFailure('invalidLockedRoster', 'clock query roster is unavailable')
    if (caller.role === 'player' && !rosterTeams.some(team => (
      isJsonObject(team) && teams.includes(team.teamId as string)
      && Array.isArray(team.members) && team.members.includes(request.callerUserId)
    ))) return roomFailure('forbidden', 'caller cannot read this match clock')
    // 尚未有 linkage 也須驗證鎖定名單／牌組 DTO 與已知規則；參數只用於純驗證，不指派先手。
    if (!deriveLockedRosterBinding(roster, teams, current.organizer_user_id, teams[0], null, null).ok) {
      return roomFailure('invalidLockedRoster', 'clock query roster is unavailable')
    }
    const game = await this.readVerifiedCurrentGame(current, row, serverState, roster)
    if (!game.ok) return game

    // scoped 時計從 row 還原，global 時計可能是另一場；其他 domain 原形必須保持不變。
    const scopedState: CompetitionServerState = { ...serverState, clock }
    let display: ClockDisplay
    try {
      // 期限只取已驗證 Rust checkpoint 的純 getter；不抄 TS 計時計算或呼叫 observe。
      const metadata = await this.engine({
        serverActor: actorFor(caller.role, request.callerUserId), trustedTimeMs: request.trustedTimeMs,
        operation: { type: 'clockCheckpointMetadata' }, serverState: structuredClone(scopedState),
      })
      if (!isJsonObject(metadata)
        || !hasExactClockQueryKeys(metadata, ['ok', 'operation', 'public', 'serverState'])
        || metadata.ok !== true || metadata.operation !== 'clockCheckpointMetadata'
        || !sameJson(metadata.serverState, scopedState)) {
        return roomFailure('engineProtocolError', 'clock checkpoint query failed')
      }
      const parsedMetadata = parseCompetitionEngineResponse(metadata)
      if (!parsedMetadata.ok || !isClockCheckpointMetadata(parsedMetadata.public)) {
        return roomFailure('engineProtocolError', 'clock checkpoint query failed')
      }
      if (parsedMetadata.public.nextTimeoutDeadlineMs !== projectionDeadline) {
        return roomFailure('invalidClockCheckpoint', 'clock query checkpoint is unavailable')
      }
      const raw = await this.engine({
        serverActor: actorFor(caller.role, request.callerUserId), trustedTimeMs: request.trustedTimeMs,
        operation: { type: 'clockView' }, serverState: structuredClone(scopedState),
      })
      if (!isJsonObject(raw) || !hasExactClockQueryKeys(raw, ['ok', 'operation', 'public', 'serverState'])
        || raw.ok !== true || raw.operation !== 'clockView' || !sameJson(raw.serverState, scopedState)) {
        return roomFailure('engineProtocolError', 'clock display query failed')
      }
      const parsed = parseCompetitionEngineResponse(raw)
      if (!parsed.ok || !isJsonObject(parsed.public) || parsed.public.sampledAtMs !== request.trustedTimeMs
        || !Array.isArray(parsed.public.teams)
        || !sameJson(parsed.public.teams.map(team => isJsonObject(team) ? team.team : null), teams)) {
        return roomFailure('engineProtocolError', 'clock display query failed')
      }
      // bridge 已逐層 exact 驗證 ClockDisplay，不另抄取樣或扣時計算。
      display = parsed.public as unknown as ClockDisplay
    } catch {
      return roomFailure('engineProtocolError', 'clock display query failed')
    }
    return roomSuccess({
      competitionId: current.competition_id, matchId: row.match_id, matchRevision: row.revision,
      matchStatus, game: game.value.game, clock: display,
    })
  }

  private async readTournamentInternal(
    request: CompetitionRoomTournamentReadRequest,
  ): Promise<CompetitionRoomTournamentReadResult> {
    if (!isJsonObject(request)
      || !hasExactKeys(request, ['competitionId', 'callerUserId'])
      || !nonEmpty(request.competitionId) || !nonEmpty(request.callerUserId)) {
      return roomFailure('invalidRequest', 'competition id and caller user id are required')
    }
    let current = this.storage.sql.exec<CompetitionTournamentReadMetaRow>(
      `SELECT competition_id, organizer_user_id, revision, server_state_json, updated_trusted_time_ms
       FROM competition_meta WHERE singleton = 1`,
    ).toArray()[0]
    if (!current) return roomFailure('notInitialized', 'competition room is not initialized')
    if (current.competition_id !== request.competitionId) {
      return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
    }
    if (!Number.isSafeInteger(current.revision) || current.revision < 1
      || !safeNonNegativeInteger(current.updated_trusted_time_ms)
      || !nonEmpty(current.organizer_user_id)) {
      return roomFailure('invalidTournamentCheckpoint', 'tournament checkpoint is malformed')
    }
    const caller = this.acl(request.callerUserId)
    if (!caller || caller.active !== 1
      || !['organizer', 'referee', 'player'].includes(caller.role)) {
      return roomFailure('notParticipant', 'caller is not an active competition participant')
    }

    let serverState = parseParticipantServerState(current.server_state_json)
    if (!serverState
      || registrationCheckpointRevision(serverState.registration, current.organizer_user_id) === null) {
      return roomFailure('invalidTournamentCheckpoint', 'tournament checkpoint is malformed')
    }
    if (serverState.tournament === null) {
      if (isLockedRegistrationCheckpoint(serverState.registration, current.organizer_user_id)) {
        await this.ensureInitialSeedingIntentForLockedRoom(serverState, current)
        await this.pumpInitialSeedingInternal()
        const latest = this.meta()
        if (latest && latest.competition_id === current.competition_id) {
          current = {
            competition_id: latest.competition_id,
            organizer_user_id: latest.organizer_user_id,
            revision: latest.revision,
            server_state_json: latest.server_state_json,
            updated_trusted_time_ms: latest.updated_trusted_time_ms,
          }
          serverState = parseParticipantServerState(latest.server_state_json)
        }
      }
      if (!serverState || serverState.tournament === null) {
        return roomFailure('notReady', 'tournament is not ready')
      }
    }
    if (!isTournamentForCompetition(serverState.tournament, current.competition_id)) {
      return roomFailure('invalidTournamentCheckpoint', 'tournament checkpoint is malformed')
    }

    let response: CompetitionEngineResponse
    try {
      response = await this.engine({
        serverActor: actorFor(caller.role, request.callerUserId),
        trustedTimeMs: current.updated_trusted_time_ms,
        operation: { type: 'tournamentView' },
        serverState,
      })
    } catch {
      return roomFailure('engineProtocolError', 'tournament query failed')
    }

    const raw = response as unknown as JsonObject
    if (!isJsonObject(raw)
      || !hasExactKeys(raw, ['ok', 'operation', 'public', 'serverState'])
      || raw.ok !== true || raw.operation !== 'tournamentView'
      || !isJsonObject(raw.public)
      || !isTournamentForCompetition(raw.public, current.competition_id)
      || !sameJson(raw.public, serverState.tournament)
      || !sameJson(raw.serverState, serverState)
      || containsPrivateRosterFields(raw.public)) {
      return roomFailure('invalidTournamentCheckpoint', 'tournament checkpoint is malformed')
    }

    return roomSuccess<CompetitionRoomTournamentReadValue>({
      competitionId: current.competition_id,
      revision: current.revision,
      projection: {
        ok: true,
        operation: 'tournamentView',
        public: raw.public,
      },
    })
  }

  private blockGameProvision(linked: GameProvisionRow, errorCode: string): void {
    this.storage.sql.exec(
      `UPDATE competition_game_provision
       SET status = 'blocked', error_code = ?, claim_id = NULL
       WHERE match_id = ? AND game_number = ? AND status = 'pending'`,
      errorCode,
      linked.match_id,
      linked.game_number,
    )
  }

  private async scheduleSharedAlarm(
    registrationOverride?: unknown,
    additionalDueTimes: readonly number[] = [],
  ): Promise<void> {
    const provision = this.nextProvisionRow()
    const control = this.nextClockControlRow()
    const initialSeeding = this.storage.sql.exec<{
      [key: string]: ArrayBuffer | string | number | null
      due: number
    }>(
      `SELECT MIN(next_attempt_at_ms) AS due FROM competition_initial_seeding
       WHERE status = 'pending' OR (status = 'completed' AND last_error IS NOT NULL
         AND last_error != 'roundNotPublishable')`,
    ).one().due
    const now = Date.now()
    const current = this.meta()
    let registrationDeadline: number | null = null
    if (registrationOverride !== undefined) {
      registrationDeadline = registrationFreezeDeadline(registrationOverride, current?.organizer_user_id ?? '')
    } else if (current) {
      try {
        registrationDeadline = registrationFreezeDeadline(
          parseServerState(current.server_state_json).registration,
          current.organizer_user_id,
        )
      } catch {
        // 壞掉的報名 checkpoint 不應阻止其他持久工作重排。
      }
    }
    const roundJobs = this.storage.sql.exec<{ [key: string]: ArrayBuffer | string | number | null; due: number }>(
      `SELECT ledger.next_attempt_at_ms AS due FROM competition_round_room_ledger ledger
       JOIN competition_round_publication publication USING (round_generation_id)
       WHERE ledger.status = 'pending' AND publication.status = 'materializing' AND publication.requires_referee = 0
       UNION ALL
       SELECT outbox.next_attempt_at_ms AS due FROM competition_round_release_outbox outbox
       JOIN competition_round_publication publication USING (round_generation_id)
       WHERE outbox.status = 'pending' AND publication.status = 'released' AND publication.requires_referee = 0`,
    ).toArray().map(row => row.due).filter(safeNonNegativeInteger)
    const missedRoundWindows = this.storage.sql.exec<RoundPublicationRow>(
      `SELECT * FROM competition_round_publication
       WHERE status IN ('materializing', 'released') AND requires_referee = 0
         AND release_not_before_ms > 0 AND release_not_before_ms <= ?`, now,
    ).toArray().filter(header => {
      if (header.status !== 'released' || !current) return true
      try { return !this.roundHasCompleteTimelyReleaseAcks(header, current.competition_id) } catch {
        return true
      }
    })
    const noShowJobs = this.storage.sql.exec<{ [key: string]: ArrayBuffer | string | number | null; due: number }>(
      `SELECT next_attempt_at_ms AS due FROM competition_no_show_outbox WHERE status = 'pending'`,
    ).toArray().map(row => row.due).filter(safeNonNegativeInteger)
    const dueTimes = [
      provision?.next_attempt_at_ms,
      control?.next_attempt_at_ms,
      safeNonNegativeInteger(initialSeeding)
        ? initialSeeding > now ? initialSeeding : checkedRoundRetryTime(now, 1_000)
        : undefined,
      registrationDeadline === null
        ? undefined
        : registrationDeadline > Math.max(now, current?.updated_trusted_time_ms ?? 0)
          ? registrationDeadline
          : checkedRoundRetryTime(now, 1_000),
      // 過期工作以有界 retry 再排，不讓到 T 但 authority 損壞形成即刻空轉。
      ...roundJobs.map(due => due > now ? due : checkedRoundRetryTime(now, 1_000)),
      ...missedRoundWindows.map(() => checkedRoundRetryTime(now, 1_000)),
      ...noShowJobs.map(due => due > now ? due : checkedRoundRetryTime(now, 1_000)),
      ...this.roundsWaitingForReleaseWindow().map(header => header.release_not_before_ms),
      ...this.roundsReadyForRelease().map(() => checkedRoundRetryTime(now, 1_000)),
      ...additionalDueTimes,
    ].filter(safeNonNegativeInteger)
    if (dueTimes.length) {
      const due = Math.min(...dueTimes)
      let lastError: unknown
      for (let attempt = 0; attempt < 2; attempt += 1) {
        try {
          await this.storage.setAlarm(due)
          return
        } catch (error) {
          lastError = error
        }
      }
      throw lastError
    }
    await this.storage.deleteAlarm()
  }

  /** 由伺服器接受時間或 Durable Object alarm 將已到 F 的報名提交為固定版本。 */
  private async freezeRegistrationIfDue(trustedTimeMs: number): Promise<boolean> {
    if (!safeNonNegativeInteger(trustedTimeMs)) return false
    const current = this.meta()
    if (!current) return false
    let serverState: CompetitionServerState
    try {
      serverState = parseServerState(current.server_state_json)
    } catch {
      return false
    }
    const registrationRevision = registrationCheckpointRevision(
      serverState.registration,
      current.organizer_user_id,
    )
    const deadline = registrationFreezeDeadline(serverState.registration, current.organizer_user_id)
    if (registrationRevision === null || deadline === null || trustedTimeMs < deadline) return false

    const commandId = `registration-freeze:${crypto.randomUUID()}`
    const effectiveTime = Math.max(trustedTimeMs, current.updated_trusted_time_ms, deadline)
    await this.executeInternal({
      competitionId: current.competition_id,
      callerUserId: current.organizer_user_id,
      commandId,
      expectedRevision: current.revision,
      trustedTimeMs: effectiveTime,
      operation: {
        type: 'registrationExecute',
        commandId,
        expectedRevision: current.revision,
        action: { type: 'lock' },
      },
    })
    return true
  }

  private async readMatchInternal(
    request: CompetitionRoomMatchReadRequest,
  ): Promise<CompetitionRoomResult<CompetitionRoomMatchReadValue>> {
    if (!nonEmpty(request.competitionId) || !nonEmpty(request.matchId) || !nonEmpty(request.callerUserId)) {
      return roomFailure('invalidRequest', 'competition id, match id and caller user id are required')
    }
    const current = this.meta()
    if (!current) return roomFailure('notInitialized', 'competition room is not initialized')
    if (current.competition_id !== request.competitionId) {
      return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
    }
    const row = this.match(request.matchId)
    if (!row) return roomFailure('matchNotFound', 'match row does not exist')
    return roomSuccess({
      competitionId: request.competitionId,
      matchId: request.matchId,
      matchRevision: row.revision,
      matchProjection: parseOptionalProjection(row.match_projection_json),
      clockProjection: parseOptionalProjection(row.clock_projection_json),
    })
  }

  private persistFailedMatchExecution(
    request: CompetitionRoomMatchExecuteRequest,
    role: CompetitionRoomRole,
    fingerprint: string,
    outcomeCode: string,
    message: string,
    revision: number,
  ): CompetitionRoomResult<CompetitionRoomMatchExecuteValue> {
    const result = roomFailure(outcomeCode, message)
    this.storage.transactionSync(() => {
      this.insertAudit({
        actorUserId: request.callerUserId,
        role,
        matchId: request.matchId,
        operationType: request.operation.type,
        commandId: request.commandId,
        revision,
      outcomeCode,
      trustedTimeMs: request.trustedTimeMs,
      reason: operationReason(request.operation),
      })
      this.insertCommandReceipt(request.commandId, 'matchExecute', fingerprint, result, request.matchId)
    })
    return result
  }

  /** 自隊限制依鎖定 members 核實；成功 receipt 已先返回，純匯出不產生持久副作用。 */
  private async validateRefereeOwnMatch(
    current: CompetitionMetaRow,
    row: MatchRow,
    caller: AclRow,
    trustedTimeMs: number,
  ): Promise<{ code: string; message: string } | null> {
    // 正常報名同步 participant；獨立裁判不需讀取整份私有名單。
    if (caller.participant === 0) return null
    const unavailable = { code: 'invalidLockedRoster', message: 'referee match membership cannot be verified' }
    if (caller.participant !== 1) return unavailable
    const organizer = this.acl(current.organizer_user_id)
    const serverState = parseParticipantServerState(current.server_state_json)
    if (!organizer || organizer.active !== 1 || organizer.role !== 'organizer' || organizer.participant !== 0
      || !serverState || !serverState.tournament
      || !isTournamentForCompetition(serverState.tournament, current.competition_id)
      || registrationCheckpointRevision(serverState.registration, current.organizer_user_id) === null) {
      return unavailable
    }
    const teams = participantMatchTeams(row, serverState)
    if (!teams) return { code: 'invalidMatchCheckpoint', message: 'referee match teams cannot be verified' }
    let response: unknown
    try {
      response = await this.engine({
        serverActor: actorFor('organizer', current.organizer_user_id), trustedTimeMs,
        operation: { type: 'registrationExportLockedRoster' }, serverState: structuredClone(serverState),
      })
    } catch {
      return unavailable
    }
    if (!isExactCompetitionJson(response) || !isJsonObject(response)
      || !hasExactKeys(response, ['ok', 'operation', 'public', 'private', 'serverState'])
      || response.ok !== true || response.operation !== 'registrationExportLockedRoster'
      || !isJsonObject(response.public) || !hasExactKeys(response.public, ['exported'])
      || response.public.exported !== true || !sameJson(response.serverState, serverState)
      || !parseLockedRoster(response.private, current.organizer_user_id)
      || !isJsonObject(response.private) || !Array.isArray(response.private.teams)
      || !sameJson(response.private.announcement, serverState.registration?.announcement)) {
      return unavailable
    }
    const matchRosters: JsonObject[] = []
    for (const teamId of teams) {
      const team = response.private.teams.find(entry => isJsonObject(entry) && entry.teamId === teamId)
      if (!isJsonObject(team) || !Array.isArray(team.members)) return unavailable
      matchRosters.push(team)
    }
    if (matchRosters.some(team => Array.isArray(team.members) && team.members.includes(caller.user_id))) {
      return { code: 'forbidden', message: 'a referee cannot adjudicate their own match' }
    }
    return null
  }

  private async executeRegistrationQuery(
    request: CompetitionRoomExecuteRequest,
    current: CompetitionMetaRow,
  ): Promise<CompetitionRoomExecuteResult> {
    const acl = this.acl(request.callerUserId)
    if (acl && !acl.active) return roomFailure('notParticipant', 'caller is not an active competition participant')

    const isExport = request.operation.type === 'registrationExportLockedRoster'
    if (isExport && !acl) {
      return roomFailure('notParticipant', 'caller is not an active competition participant')
    }
    if (isExport && acl?.role === 'player') {
      return roomFailure('forbidden', 'players cannot export the locked roster')
    }

    const role: CompetitionRoomRole = acl?.active ? acl.role : 'player'
    const allowPrivate = isExport
      ? role === 'organizer'
      : role === 'player' && Boolean(acl?.active)
    const serverState = parseServerState(current.server_state_json)
    if (registrationCheckpointRevision(serverState.registration, current.organizer_user_id) === null) {
      return roomFailure('invalidRegistrationCheckpoint', 'stored registration checkpoint is invalid')
    }
    let response: CompetitionEngineResponse
    try {
      response = await this.engine({
        serverActor: actorFor(role, request.callerUserId),
        trustedTimeMs: request.trustedTimeMs,
        operation: request.operation,
        serverState,
      })
    } catch {
      return roomFailure('engineProtocolError', 'competition engine request failed')
    }
    if (!response.ok) {
      return roomFailure(`engine:${response.error.code}`, response.error.message)
    }
    if (response.operation !== request.operation.type) {
      return roomFailure('engineOperationMismatch', 'competition engine returned another operation')
    }

    return roomSuccess({
      competitionId: request.competitionId,
      revision: current.revision,
      commandId: request.commandId,
      idempotent: false,
      projection: publicProjection(response, role, allowPrivate),
    })
  }

  private persistFailedExecution(
    current: CompetitionMetaRow,
    request: CompetitionRoomExecuteRequest,
    role: CompetitionRoomRole,
    fingerprint: string,
    outcomeCode: string,
    message: string,
  ): CompetitionRoomExecuteResult {
    const result = roomFailure(outcomeCode, message)
    this.storage.transactionSync(() => {
      this.insertAudit({
        actorUserId: request.callerUserId,
        role,
        operationType: request.operation.type,
        commandId: request.commandId,
        revision: current.revision,
        outcomeCode,
        trustedTimeMs: request.trustedTimeMs,
      })
      this.insertCommandReceipt(request.commandId, 'execute', fingerprint, result)
    })
    return result
  }

  private async readInternal(
    request: CompetitionRoomReadRequest,
  ): Promise<CompetitionRoomResult<CompetitionRoomReadValue>> {
    if (!nonEmpty(request.competitionId) || !nonEmpty(request.callerUserId)) {
      return roomFailure('invalidRequest', 'competition id and caller user id are required')
    }
    const current = this.meta()
    if (!current) return roomFailure('notInitialized', 'competition room is not initialized')
    if (current.competition_id !== request.competitionId) {
      return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
    }
    const acl = this.acl(request.callerUserId)
    if (!acl || !acl.active) {
      return roomFailure('notParticipant', 'caller is not an active competition participant')
    }
    const projection = parseStoredProjection(current.public_projection_json)
    const canReadPrivate = (acl.role === 'player' && projection.operation.startsWith('registration'))
      || (acl.role === 'organizer' && projection.operation === 'registrationExportLockedRoster')
    if (!canReadPrivate) {
      return roomSuccess({
        competitionId: current.competition_id,
        revision: current.revision,
        projection,
      })
    }

    const privateRow = this.storage.sql.exec<PrivateProjectionRow>(
      `SELECT payload_json, revision, operation_type
       FROM competition_private_projection
       WHERE user_id = ? AND revision = ? AND operation_type = ?`,
      request.callerUserId,
      current.revision,
      projection.operation,
    ).toArray()[0]
    if (!privateRow) {
      return roomSuccess({
        competitionId: current.competition_id,
        revision: current.revision,
        projection,
      })
    }
    const privatePayload = parseJsonValue(privateRow.payload_json)
    return roomSuccess({
      competitionId: current.competition_id,
      revision: current.revision,
      projection: { ...projection, private: privatePayload },
    })
  }

  private async changeRefereeInternal(
    request: CompetitionRoomRefereeRequest,
    operationType: 'grantReferee' | 'revokeReferee',
  ): Promise<CompetitionRoomResult<CompetitionRoomAclValue>> {
    const validation = validateRefereeRequest(request)
    if (validation) return roomFailure(validation.code, validation.message)
    const current = this.meta()
    if (!current) return roomFailure('notInitialized', 'competition room is not initialized')
    if (current.competition_id !== request.competitionId) {
      return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
    }
    const fingerprint = await refereeFingerprint(request, operationType)
    const receipt = this.commandReceipt(request.commandId)
    if (receipt) {
      if (receipt.receipt_kind !== operationType || receipt.fingerprint !== fingerprint) {
        return roomFailure('commandIdConflict', 'command id was used with another payload')
      }
      return parseStoredAclResult(receipt.response_json)
    }
    const caller = this.acl(request.callerUserId)
    if (!caller || !caller.active) return roomFailure('notParticipant', 'caller is not active')
    if (caller.role !== 'organizer') return roomFailure('forbidden', 'only the organizer may manage referees')
    if (request.targetUserId === current.organizer_user_id) {
      return roomFailure('organizerCannotBeReferee', 'organizer cannot be granted or revoked as referee')
    }
    if (request.expectedRevision !== current.revision) {
      return roomFailure('revisionConflict', 'expected revision does not match room revision')
    }
    if (request.trustedTimeMs < current.updated_trusted_time_ms) {
      return roomFailure('trustedTimeRegression', 'trusted server time cannot move backwards')
    }

    const target = this.acl(request.targetUserId)
    if (operationType === 'grantReferee' && target?.role === 'referee' && target.active) {
      return roomFailure('alreadyReferee', 'user is already an active referee')
    }
    if (operationType === 'revokeReferee' && (!target || !target.active || target.role !== 'referee')) {
      return roomFailure('refereeNotFound', 'user is not an active referee')
    }

    const revision = current.revision + 1
    const targetRole: CompetitionRoomRole = operationType === 'grantReferee'
      ? 'referee'
      : target?.participant ? 'player' : 'player'
    const active = operationType === 'grantReferee'
    const result = roomSuccess<CompetitionRoomAclValue>({
      competitionId: request.competitionId,
      revision,
      targetUserId: request.targetUserId,
      role: targetRole,
      active,
      idempotent: false,
    })
    this.storage.transactionSync(() => {
      if (operationType === 'grantReferee') {
        this.storage.sql.exec(
          `INSERT INTO competition_acl (user_id, role, active, participant)
           VALUES (?, 'referee', 1, ?)
           ON CONFLICT(user_id) DO UPDATE SET role = 'referee', active = 1`,
          request.targetUserId,
          target?.participant ? 1 : 0,
        )
      } else if (target?.participant) {
        this.storage.sql.exec(
          `UPDATE competition_acl SET role = 'player', active = 1 WHERE user_id = ?`,
          request.targetUserId,
        )
      } else {
        this.storage.sql.exec(
          'DELETE FROM competition_acl WHERE user_id = ?',
          request.targetUserId,
        )
      }
      this.storage.sql.exec(
        'UPDATE competition_meta SET revision = ?, updated_trusted_time_ms = ? WHERE singleton = 1',
        revision,
        request.trustedTimeMs,
      )
      this.insertAudit({
        actorUserId: request.callerUserId,
        role: 'organizer',
        operationType,
        commandId: request.commandId,
        revision,
        outcomeCode: 'success',
        trustedTimeMs: request.trustedTimeMs,
      })
      this.insertCommandReceipt(request.commandId, operationType, fingerprint, result)
    })

    return result
  }

  private async auditInternal(
    request: CompetitionRoomAuditQuery,
  ): Promise<CompetitionRoomResult<CompetitionRoomAuditValue>> {
    if (!nonEmpty(request.competitionId) || !nonEmpty(request.callerUserId)) {
      return roomFailure('invalidRequest', 'competition id and caller user id are required')
    }
    const current = this.meta()
    if (!current) return roomFailure('notInitialized', 'competition room is not initialized')
    if (current.competition_id !== request.competitionId) {
      return roomFailure('competitionIdMismatch', 'competition id is bound to this room')
    }
    const caller = this.acl(request.callerUserId)
    if (!caller || !caller.active) return roomFailure('notParticipant', 'caller is not active')
    if (caller.role !== 'organizer' && caller.role !== 'referee') {
      return roomFailure('forbidden', 'players cannot read competition audit records')
    }
    const limit = Math.min(Math.max(Math.floor(request.limit ?? 50), 1), 100)
    const rows = this.storage.sql.exec<AuditRow>(
      `SELECT id, match_id, actor_user_id, role, operation_type, command_id, revision,
              outcome_code, trusted_time_ms, reason
       FROM competition_audit ORDER BY id DESC LIMIT ?`,
      limit,
    ).toArray()
    return roomSuccess({
      competitionId: current.competition_id,
      records: rows.map(row => ({
        sequence: row.id,
        matchId: row.match_id,
        actorUserId: row.actor_user_id,
        role: row.role,
        operationType: row.operation_type,
        commandId: row.command_id,
        revision: row.revision,
        outcomeCode: row.outcome_code,
        trustedTimeMs: row.trusted_time_ms,
        reason: row.reason,
      })),
    })
  }

  private meta(): CompetitionMetaRow | undefined {
    return this.storage.sql.exec<CompetitionMetaRow>(
      `SELECT competition_id, organizer_user_id, revision, server_state_json,
              public_projection_json, updated_trusted_time_ms
       FROM competition_meta WHERE singleton = 1`,
    ).toArray()[0]
  }

  private match(matchId: string): MatchRow | undefined {
    return this.storage.sql.exec<MatchRow>(
      `SELECT match_id, match_checkpoint_json, clock_checkpoint_json,
              stage_id, match_projection_json, clock_projection_json, revision,
              last_trusted_time_ms
       FROM competition_matches WHERE match_id = ?`,
      matchId,
    ).toArray()[0]
  }

  private gameProvision(matchId: string, gameNumber: number): GameProvisionRow | undefined {
    return this.storage.sql.exec<GameProvisionRow>(
      `SELECT match_id, game_number, game_id, binding_json, match_revision,
              next_game_json, command_id, fingerprint, status, attempts,
              next_attempt_at_ms, error_code, created_trusted_time_ms, claim_id, claim_version
       FROM competition_game_provision WHERE match_id = ? AND game_number = ?`,
      matchId,
      gameNumber,
    ).toArray()[0]
  }

  private gameProvisionByCommand(commandId: string): GameProvisionRow | undefined {
    return this.storage.sql.exec<GameProvisionRow>(
      `SELECT match_id, game_number, game_id, binding_json, match_revision,
              next_game_json, command_id, fingerprint, status, attempts,
              next_attempt_at_ms, error_code, created_trusted_time_ms, claim_id, claim_version
       FROM competition_game_provision WHERE command_id = ?`,
      commandId,
    ).toArray()[0]
  }

  private latestGameProvision(matchId: string): GameProvisionRow | undefined {
    return this.storage.sql.exec<GameProvisionRow>(
      `SELECT match_id, game_number, game_id, binding_json, match_revision,
              next_game_json, command_id, fingerprint, status, attempts,
              next_attempt_at_ms, error_code, created_trusted_time_ms, claim_id, claim_version
       FROM competition_game_provision WHERE match_id = ?
       ORDER BY game_number DESC LIMIT 1`,
      matchId,
    ).toArray()[0]
  }

  private nextProvisionRow(): GameProvisionRow | undefined {
    return this.storage.sql.exec<GameProvisionRow>(
      `SELECT match_id, game_number, game_id, binding_json, match_revision,
              next_game_json, command_id, fingerprint, status, attempts,
              next_attempt_at_ms, error_code, created_trusted_time_ms, claim_id, claim_version
       FROM competition_game_provision WHERE status = 'pending'
         -- 在 LIMIT 1 前排除壞 due，避免它遮蔽其他 match 的有效派送工作。
         AND typeof(next_attempt_at_ms) = 'integer'
         AND next_attempt_at_ms BETWEEN 0 AND 9007199254740991
       ORDER BY next_attempt_at_ms, match_id, game_number LIMIT 1`,
    ).toArray()[0]
  }

  private clockControl(controlId: string): ClockControlRow | undefined {
    return this.storage.sql.exec<ClockControlRow>(
      `SELECT control_id, competition_id, match_id, game_id, game_number,
              source_command_id, match_revision, operation, trusted_time_ms,
              next_clock_deadline_ms, attempts, next_attempt_at_ms, status, claim_id, claim_version
       FROM competition_clock_control_outbox WHERE control_id = ?`,
      controlId,
    ).toArray()[0]
  }

  private nextClockControlRow(): ClockControlRow | undefined {
    // 每個 GameRoom 依 match revision 消化控制；已 claim 或 retry 中的低 revision 仍是 stream 首筆。
    return this.storage.sql.exec<ClockControlRow>(
      `SELECT control.control_id, control.competition_id, control.match_id,
              control.game_id, control.game_number, control.source_command_id,
              control.match_revision, control.operation, control.trusted_time_ms,
              control.next_clock_deadline_ms, control.attempts,
              control.next_attempt_at_ms, control.status, control.claim_id,
              control.claim_version
       FROM competition_clock_control_outbox control
       WHERE control.status = 'pending'
         -- 只選有效時間；下面的 NOT EXISTS 仍檢查全部 pending stream head。
         AND typeof(control.next_attempt_at_ms) = 'integer'
         AND control.next_attempt_at_ms BETWEEN 0 AND 9007199254740991
         AND NOT EXISTS (
           SELECT 1 FROM competition_clock_control_outbox earlier
           WHERE earlier.status = 'pending' AND earlier.game_id = control.game_id
             AND (earlier.match_revision < control.match_revision
               OR (earlier.match_revision = control.match_revision
                 AND earlier.control_id < control.control_id))
         )
       ORDER BY control.next_attempt_at_ms, control.match_id,
                control.game_number, control.match_revision, control.control_id
       LIMIT 1`,
    ).toArray()[0]
  }

  private gameProvisionValue(
    competitionId: string,
    row: GameProvisionRow,
    idempotent: boolean,
  ): CompetitionRoomProvisionScheduledGameValue {
    return {
      competitionId,
      matchId: row.match_id,
      gameNumber: row.game_number,
      gameId: row.game_id,
      commandId: row.command_id,
      idempotent,
      status: row.status,
      binding: parseStoredCompetitionGameBinding(row.binding_json),
    }
  }

  private gameProvisionAssignedValue(
    competitionId: string,
    row: GameProvisionRow,
    idempotent: boolean,
  ): CompetitionRoomProvisionAssignedScheduledGameValue {
    return {
      competitionId,
      matchId: row.match_id,
      matchRevision: row.match_revision,
      gameNumber: row.game_number,
      gameId: row.game_id,
      commandId: row.command_id,
      idempotent,
      status: row.status,
      binding: parseStoredCompetitionGameBinding(row.binding_json),
    }
  }

  private persistScheduledGameProvision(input: {
    current: CompetitionMetaRow
    row: MatchRow
    matchId: string
    gameNumber: number
    gameId: string
    binding: CompetitionGameBinding
    nextGame: JsonObject
    commandId: string
    fingerprint: string
    trustedTimeMs: number
    actorUserId: string
    role: CompetitionRoomRole
    operationType: 'provisionAssignedScheduledGame' | 'provisionCaptainAssignedScheduledGame'
    receiptKind: 'provisionAssignedScheduledGame' | 'provisionCaptainAssignedScheduledGame'
    receiptResponse: unknown
    reason?: string
    additionalCheck?: () => boolean
  }): void {
    this.storage.transactionSync(() => {
      const latestMeta = this.meta()
      const latestRow = this.match(input.matchId)
      if (!latestMeta || latestMeta.competition_id !== input.current.competition_id
        || latestMeta.organizer_user_id !== input.current.organizer_user_id
        || latestMeta.revision !== input.current.revision
        || latestMeta.updated_trusted_time_ms !== input.current.updated_trusted_time_ms
        || latestMeta.server_state_json !== input.current.server_state_json
        || !latestRow || latestRow.revision !== input.row.revision
        || latestRow.last_trusted_time_ms !== input.row.last_trusted_time_ms
        || latestRow.match_checkpoint_json !== input.row.match_checkpoint_json
        || latestRow.match_projection_json !== input.row.match_projection_json
        || latestRow.clock_checkpoint_json !== input.row.clock_checkpoint_json
        || latestRow.clock_projection_json !== input.row.clock_projection_json
        || this.gameProvision(input.matchId, input.gameNumber)
        || (input.additionalCheck && !input.additionalCheck())) {
        throw new Error('scheduled game state changed before atomic provision commit')
      }
      this.insertScheduledGameProvisionLinkage({
        matchId: input.matchId,
        gameNumber: input.gameNumber,
        gameId: input.gameId,
        binding: input.binding,
        matchRevision: input.row.revision,
        nextGame: input.nextGame,
        commandId: input.commandId,
        fingerprint: input.fingerprint,
        trustedTimeMs: input.trustedTimeMs,
      })
      this.insertAudit({
        actorUserId: input.actorUserId,
        role: input.role,
        matchId: input.matchId,
        operationType: input.operationType,
        commandId: input.commandId,
        revision: input.row.revision,
        outcomeCode: 'pending',
        trustedTimeMs: input.trustedTimeMs,
        reason: input.reason,
      })
      this.insertCommandReceipt(
        input.commandId,
        input.receiptKind,
        input.fingerprint,
        input.receiptResponse,
        input.matchId,
      )
    })
  }

  private insertScheduledGameProvisionLinkage(input: {
    matchId: string
    gameNumber: number
    gameId: string
    binding: CompetitionGameBinding
    matchRevision: number
    nextGame: JsonObject
    commandId: string
    fingerprint: string
    trustedTimeMs: number
  }): void {
    this.storage.sql.exec(
      `INSERT INTO competition_game_provision
        (match_id, game_number, game_id, binding_json, match_revision, next_game_json,
         command_id, fingerprint, status, attempts, next_attempt_at_ms, error_code,
         created_trusted_time_ms)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'pending', 0, ?, NULL, ?)`,
      input.matchId,
      input.gameNumber,
      input.gameId,
      JSON.stringify(input.binding),
      input.matchRevision,
      JSON.stringify(input.nextGame),
      input.commandId,
      input.fingerprint,
      Date.now(),
      input.trustedTimeMs,
    )
  }

  private acl(userId: string): AclRow | undefined {
    return this.storage.sql.exec<AclRow>(
      'SELECT user_id, role, active, participant FROM competition_acl WHERE user_id = ?',
      userId,
    ).toArray()[0]
  }

  private commandReceipt(commandId: string): CommandReceiptRow | undefined {
    return this.storage.sql.exec<CommandReceiptRow>(
      `SELECT receipt_kind, match_id, fingerprint, response_json
       FROM competition_command_receipt WHERE command_id = ?`,
      commandId,
    ).toArray()[0]
  }

  private insertCommandReceipt(
    commandId: string,
    receiptKind: ReceiptKind,
    fingerprint: string,
    result: unknown,
    matchId: string | null = null,
  ): void {
    this.storage.sql.exec(
      `INSERT INTO competition_command_receipt
         (command_id, receipt_kind, match_id, fingerprint, response_json)
       VALUES (?, ?, ?, ?, ?)`,
      commandId,
      receiptKind,
      matchId,
      fingerprint,
      JSON.stringify(result),
    )
  }

  private insertAudit(record: {
    actorUserId: string
    role: CompetitionRoomRole
    matchId?: string | null
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
    reason?: string | null
  }): void {
    this.storage.sql.exec(
      `INSERT INTO competition_audit
        (match_id, actor_user_id, role, operation_type, command_id, revision, outcome_code, trusted_time_ms, reason)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      record.matchId ?? null,
      record.actorUserId,
      record.role,
      record.operationType,
      record.commandId,
      record.revision,
      record.outcomeCode,
      record.trustedTimeMs,
      record.reason ?? null,
    )
  }
}

function checkedRoundRetryTime(now: number, delay: number): number {
  const due = now + delay
  if (!safeNonNegativeInteger(now) || !safeNonNegativeInteger(delay) || !safeNonNegativeInteger(due)) {
    throw new Error('round work retry time is invalid')
  }
  return due
}

function validRoundReleaseRow(row: RoundReleaseRow): boolean {
  return nonEmpty(row.round_generation_id) && nonEmpty(row.game_id) && nonEmpty(row.release_decision_id)
    && ['pending', 'completed'].includes(row.status)
    && (row.claim_id === null || nonEmpty(row.claim_id)) && safeNonNegativeInteger(row.claim_version)
    && safeNonNegativeInteger(row.attempts) && safeNonNegativeInteger(row.next_attempt_at_ms)
    && (row.status === 'pending' ? row.completed_at_ms === null
      : row.completed_at_ms === null || safeNonNegativeInteger(row.completed_at_ms))
}

function validNoShowOutboxRow(row: NoShowOutboxRow): boolean {
  if (!nonEmpty(row.round_generation_id) || !nonEmpty(row.match_id) || !nonEmpty(row.game_id)
    || !safeNonNegativeInteger(row.deadline_ms) || !nonEmpty(row.command_id)
    || !['pending', 'completed'].includes(row.status)
    || (row.claim_id !== null && !nonEmpty(row.claim_id))
    || !safeNonNegativeInteger(row.claim_version) || !safeNonNegativeInteger(row.attempts)
    || !safeNonNegativeInteger(row.next_attempt_at_ms)
    || (row.result_confirmed_at_ms !== null && !safeNonNegativeInteger(row.result_confirmed_at_ms))) return false
  if (row.decision_json !== null) {
    try {
      if (!isCompetitionGameNoShowDecision(JSON.parse(row.decision_json))) return false
    } catch {
      return false
    }
  }
  return row.status === 'pending' || (row.claim_id === null && row.decision_json !== null)
}

function noShowTournamentResult(
  teams: unknown,
  decision: CompetitionGameNoShowDecision,
): TournamentMatchResult | null {
  if (decision.type !== 'noShow' || !isTeamPair(teams)
    || decision.absentTeams.length < 1 || decision.absentTeams.length > 2
    || new Set(decision.absentTeams).size !== decision.absentTeams.length
    || decision.absentTeams.some(team => !teams.includes(team))) return null
  if (decision.absentTeams.length === 2) {
    if (teams.some(team => !decision.absentTeams.includes(team))) return null
    return { type: 'noShow', absentTeams: [...decision.absentTeams], winnerId: null }
  }
  const winnerId = teams.find(team => !decision.absentTeams.includes(team))
  return winnerId
    ? { type: 'noShow', absentTeams: [...decision.absentTeams], winnerId }
    : null
}

/** RPC disposer 是 transport 資源；只抽 exact 三個 data 欄位，不放寬 shared JSON guard。 */
function consumeRoundReadyAck(value: unknown, expected: CompetitionGameBindingRef): CompetitionRoomReadyAck | null {
  let disposer: ((...args: never[]) => unknown) | null = null
  let snapshot: CompetitionRoomReadyAck | null = null
  try {
    if ((typeof value !== 'object' && typeof value !== 'function') || value === null) return null
    const resource = Object.getOwnPropertyDescriptor(value, Symbol.dispose)
    if (resource && ('value' in resource) && typeof resource.value === 'function') disposer = resource.value
    if (Array.isArray(value) || typeof value === 'function') throw new Error('round ACK must be a data object')
    const keys = Reflect.ownKeys(value)
    const fields = ['gameId', 'bindingDigest', 'status']
    if ((resource && (!('value' in resource) || typeof resource.value !== 'function' || resource.enumerable))
      || keys.length !== fields.length + (resource ? 1 : 0)
      || keys.some(key => key !== Symbol.dispose && (typeof key !== 'string' || !fields.includes(key)))) {
      throw new Error('round ACK transport is invalid')
    }
    const semantic: Record<string, unknown> = {}
    for (const key of fields) {
      const data = Object.getOwnPropertyDescriptor(value, key)
      if (!data || !('value' in data) || !data.enumerable) throw new Error('round ACK data is invalid')
      semantic[key] = data.value
    }
    if (isCompetitionRoomReadyAck(semantic, expected)) snapshot = semantic
  } catch {
    snapshot = null
  } finally {
    if (disposer) {
      try {
        if (Reflect.apply(disposer, value, []) !== undefined) snapshot = null
      } catch { snapshot = null }
    }
  }
  return snapshot
}

function emptyServerState(): CompetitionServerState {
  return { registration: null, clock: null, tournament: null, matches: {} }
}

function actorFor(role: CompetitionRoomRole, userId: string): CompetitionServerActor {
  return { type: role, playerId: userId }
}

function isRetryableAutomaticRoundAdvancementError(code: string): boolean {
  return code === 'engineProtocolError' || code === 'revisionConflict'
    || code === 'internalFailure' || code === 'randomSourceFailure'
}

function isValidRoundTournamentView(
  value: unknown,
  state: CompetitionServerState,
  competitionId: string,
): boolean {
  return isExactCompetitionJson(value) && isJsonObject(value)
    && hasExactKeys(value, ['ok', 'operation', 'public', 'serverState'])
    && value.ok === true && value.operation === 'tournamentView'
    && isJsonObject(value.public) && isTournamentForCompetition(value.public, competitionId)
    && !containsPrivateRosterFields(value.public)
    && sameJson(value.public, state.tournament) && sameJson(value.serverState, state)
}

function isCoreBye(match: JsonObject): boolean {
  return match.status === 'completed' && match.started === false
    && Array.isArray(match.participants) && match.participants.length === 1
    && nonEmpty(match.participants[0]) && isJsonObject(match.result)
    && hasExactKeys(match.result, ['type', 'teamId']) && match.result.type === 'bye'
    && match.result.teamId === match.participants[0] && match.cancellationReason === null
}

function selectPublishableRound(
  tournament: JsonObject | null,
  stageId: string,
  roundNumber: number,
  isAuthoritativeNoShow: (match: JsonObject) => boolean,
): { round: JsonObject; schedules: ScheduledMatchMaterialization[]; previousMatches: JsonObject[] } {
  if (!isJsonObject(tournament) || !Array.isArray(tournament.stages)) {
    throw new Error('current stage prerequisites cannot be established')
  }
  const activeStages = tournament.stages.filter(entry => isJsonObject(entry) && entry.status === 'active')
  const stage = activeStages.find(entry => isJsonObject(entry) && isJsonObject(entry.config)
    && entry.config.stageId === stageId)
  if (activeStages.length !== 1 || !isJsonObject(stage) || !isJsonObject(stage.config)
    || stage.config.stageId !== stageId || !Array.isArray(stage.rounds)) throw new Error('active stage is missing')
  const round = stage.rounds.find(entry => isJsonObject(entry) && entry.roundNumber === roundNumber)
  if (!isJsonObject(round) || !Array.isArray(round.matches) || round.matches.length === 0) {
    throw new Error('current round is missing')
  }
  const previousMatches: JsonObject[] = []
  for (const previous of stage.rounds) {
    if (!isJsonObject(previous) || typeof previous.roundNumber !== 'number'
      || !Array.isArray(previous.matches)) throw new Error('round snapshot is malformed')
    if (previous.roundNumber >= roundNumber) continue
    for (const match of previous.matches) {
      if (!isJsonObject(match) || match.status !== 'completed' || match.result === null
        || match.cancellationReason !== null) throw new Error('the complete previous round is unfinished')
      if (isCoreBye(match)) {
        previousMatches.push(match)
        continue
      }
      const played = Array.isArray(match.participants) && match.participants.length === 2
        && match.started === true
      const authoritativeNoShow = Array.isArray(match.participants) && match.participants.length === 2
        && match.started === false && isAuthoritativeNoShow(match)
      if (!played && !authoritativeNoShow) throw new Error('the complete previous round is unfinished')
      previousMatches.push(match)
    }
  }
  const schedules: ScheduledMatchMaterialization[] = []
  for (const match of round.matches) {
    if (!isJsonObject(match)) throw new Error('scheduled match is malformed')
    if (isCoreBye(match)) continue
    if (!nonEmpty(match.matchId)) throw new Error('scheduled match identity is missing')
    schedules.push(deriveScheduledMatch(tournament, stageId, match.matchId, false, false, 'pending'))
  }
  if (schedules.length === 0) throw new Error('an empty room set cannot establish round readiness')
  return { round, schedules, previousMatches }
}

function parseRoundManifest(
  header: RoundPublicationRow,
  competitionId: string,
  purpose: 'current' | 'announcementReceipt' = 'current',
): RoundManifest {
  const raw: unknown = JSON.parse(header.manifest_json)
  const cutoff = competitionRoundReadyCutoffMs(header.round_start_ms)
  if (!isExactCompetitionJson(raw) || !isJsonObject(raw)
    || !hasExactKeys(raw, ['announcementCommandId', 'roundRef', 'roundStartMs', 'tournamentRound', 'rooms'])
    || !nonEmpty(raw.announcementCommandId) || raw.announcementCommandId.trim() !== raw.announcementCommandId
    || !isCompetitionRoundRef(raw.roundRef) || raw.roundRef.competitionId !== competitionId
    || raw.roundRef.roundGenerationId !== header.round_generation_id
    || raw.roundRef.stageId !== header.stage_id || raw.roundRef.roundNumber !== header.round_number
    || raw.roundStartMs !== header.round_start_ms
    || cutoff === null
    || !safeNonNegativeInteger(header.release_not_before_ms)
    || (header.release_not_before_ms !== 0 && header.release_not_before_ms !== cutoff)
    // receipt 只讀不可變公告身分；後來的操作版本／status／ACK 不決定原成功是否可恢復。
    || (purpose === 'current' && (!Number.isSafeInteger(header.publication_version) || header.publication_version < 1
      || !['materializing', 'released', 'quiescing', 'retired'].includes(header.status)
      || ![0, 1].includes(header.requires_referee)))
    || !safeNonNegativeInteger(header.announced_trusted_time_ms)
    || (header.announced_trusted_time_ms > cutoff && header.requires_referee !== 1)
    || !isJsonObject(raw.tournamentRound)
    || !hasExactKeys(raw.tournamentRound, ['roundNumber', 'kind', 'matches'])
    || raw.tournamentRound.roundNumber !== header.round_number
    || !isValidRoundKind(raw.tournamentRound.kind) || !Array.isArray(raw.tournamentRound.matches)
    || !Array.isArray(raw.rooms) || raw.rooms.length === 0) throw new Error('round manifest is invalid')
  const matchIds = new Set<string>()
  for (const match of raw.tournamentRound.matches) {
    if (!isJsonObject(match) || !hasExactScheduledMatchKeys(match)
      || !isValidScheduledMatchPurpose(match, 'singleElimination', raw.tournamentRound.kind)
      || !nonEmpty(match.matchId) || match.stageId !== header.stage_id
      || match.roundNumber !== header.round_number || match.kind !== raw.tournamentRound.kind
      || !safeNonNegativeInteger(match.sequence) || !Array.isArray(match.participants)) {
      throw new Error('manifest schedule snapshot is invalid')
    }
    if (isCoreBye(match)) continue
    if (matchIds.has(match.matchId) || match.status !== 'pending' || match.started !== false
      || match.result !== null || match.cancellationReason !== null
      || !isTeamPair(match.participants)) throw new Error('manifest is not a complete pending round')
    matchIds.add(match.matchId)
  }
  const rooms: RoundManifest['rooms'] = []
  const gameIds = new Set<string>()
  for (const room of raw.rooms) {
    if (!isJsonObject(room) || !hasExactKeys(room, ['matchId', 'gameId', 'bindingRef'])
      || !nonEmpty(room.matchId) || !nonEmpty(room.gameId) || !matchIds.delete(room.matchId)
      || gameIds.has(room.gameId) || !isCompetitionGameBindingRef(room.bindingRef)
      || room.bindingRef.matchId !== room.matchId || room.bindingRef.gameId !== room.gameId
      || room.bindingRef.gameNumber !== 1 || !sameJson(room.bindingRef.roundRef, raw.roundRef)
      || room.bindingRef.roundStartMs !== raw.roundStartMs) throw new Error('manifest room identity is invalid')
    gameIds.add(room.gameId)
    rooms.push({ matchId: room.matchId, gameId: room.gameId, bindingRef: room.bindingRef })
  }
  if (matchIds.size !== 0) throw new Error('manifest omits a current room')
  return { announcementCommandId: raw.announcementCommandId,
    roundRef: raw.roundRef, roundStartMs: header.round_start_ms, tournamentRound: raw.tournamentRound, rooms }
}

function samePublishedRoundPairs(tournament: JsonValue, manifest: RoundManifest): boolean {
  if (!isJsonObject(tournament) || !Array.isArray(tournament.stages)) return false
  const stage = tournament.stages.find(entry => isJsonObject(entry) && isJsonObject(entry.config)
    && entry.config.stageId === manifest.roundRef.stageId)
  if (!isJsonObject(stage) || !isJsonObject(stage.config) || !Array.isArray(stage.rounds)) return false
  const stageFormat = stage.config.format
  const round = stage.rounds.find(entry => isJsonObject(entry) && entry.roundNumber === manifest.roundRef.roundNumber)
  if (!isJsonObject(round) || round.kind !== manifest.tournamentRound.kind
    || !Array.isArray(round.matches) || !Array.isArray(manifest.tournamentRound.matches)
    || round.matches.length !== manifest.tournamentRound.matches.length) return false
  const matches = round.matches
  return manifest.tournamentRound.matches.every(original => isJsonObject(original)
    && isValidScheduledMatchPurpose(original, stageFormat, manifest.tournamentRound.kind)
    && matches.some(current => isJsonObject(current)
      && ['matchId', 'stageId', 'roundNumber', 'sequence', 'kind', 'participants']
        .every(key => sameJson(current[key], original[key]))
      && sameJson(current.purpose ?? null, original.purpose ?? null)))
}

function publicProjection(
  response: Extract<CompetitionEngineResponse, { ok: true }>,
  role: CompetitionRoomRole,
  allowPrivate = false,
): PublicCompetitionSuccess {
  const projected = projectCompetitionResponse(response, {
    type: allowPrivate ? 'authorizedPrivate' : 'public',
  })
  if (!projected.ok) throw new Error('successful engine response projected as failure')
  if (!allowPrivate || (role === 'player' && !response.operation.startsWith('registration'))) {
    return stripPrivate(projected)
  }
  return projected
}

function stripPrivate(projection: PublicCompetitionSuccess): CompetitionRoomPublicProjection {
  return {
    ok: true,
    operation: projection.operation,
    public: projection.public,
  }
}

function roomSuccess<T>(value: T): RoomSuccess<T> {
  return { ok: true, value }
}

function roomFailure(code: string, message: string): RoomFailure {
  return { ok: false, error: { code, message } }
}

function validateTerminalIngestRequest(
  request: CompetitionRoomTerminalIngestRequest,
): { code: string; message: string } | null {
  if (!isJsonObject(request)) {
    return { code: 'invalidRequest', message: 'terminal ingest request must be an object' }
  }
  const allowed = new Set([
    'competitionId', 'matchId', 'callerUserId', 'commandId', 'expectedMatchRevision',
    'trustedTimeMs', 'submissionId', 'gameRecordId', 'rulingRecordId', 'startKind',
    'firstPlayer', 'terminalEvidence',
  ])
  if (Object.keys(request).some(key => !allowed.has(key))) {
    return { code: 'invalidRequest', message: 'terminal ingest request contains unsupported fields' }
  }
  if (!nonEmpty(request.competitionId) || !nonEmpty(request.matchId)
    || !nonEmpty(request.callerUserId) || !nonEmpty(request.commandId)
    || !nonEmpty(request.submissionId) || !nonEmpty(request.gameRecordId)
    || !nonEmpty(request.rulingRecordId) || !nonEmpty(request.firstPlayer)) {
    return { code: 'invalidRequest', message: 'terminal ingest identity fields are required' }
  }
  if (!Number.isInteger(request.expectedMatchRevision) || request.expectedMatchRevision < 1) {
    return { code: 'invalidRequest', message: 'expected match revision must be a positive integer' }
  }
  if (!Number.isFinite(request.trustedTimeMs)) {
    return { code: 'invalidRequest', message: 'trusted time must be finite' }
  }
  if (!isGameStartKind(request.startKind) || !isGameTerminalEvidence(request.terminalEvidence)) {
    return { code: 'invalidRequest', message: 'terminal evidence or game start kind is invalid' }
  }
  return null
}

function validateTerminalIngestStructure(
  request: CompetitionRoomTerminalIngestRequest,
  matchCheckpoint: JsonObject,
  clockCheckpoint: JsonObject | null,
): { code: string; message: string } | null {
  if (!isJsonObject(matchCheckpoint.config)
    || matchCheckpoint.config.matchId !== request.matchId
    || !Array.isArray(matchCheckpoint.config.teams)
    || matchCheckpoint.config.teams.length !== 2
    || !matchCheckpoint.config.teams.every(team => typeof team === 'string' && nonEmpty(team))
    || !isValidMatchFormat(matchCheckpoint.config.format)) {
    return { code: 'engineInvalidMatchCheckpoint', message: 'match checkpoint config is invalid' }
  }
  const teams = matchCheckpoint.config.teams as string[]
  if (teams[0] === teams[1] || !teams.includes(request.firstPlayer)) {
    return { code: 'invalidRequest', message: 'first player must be one of the match teams' }
  }
  if (!Array.isArray(matchCheckpoint.history) || !matchCheckpoint.history.every(isJsonValue)) {
    return { code: 'engineInvalidMatchCheckpoint', message: 'match checkpoint history is invalid' }
  }
  if (!isValidClockCheckpoint(clockCheckpoint, teams)) {
    return { code: 'engineInvalidClockCheckpoint', message: 'clock checkpoint is invalid' }
  }
  if (request.terminalEvidence.type === 'timeout') {
    if (request.terminalEvidence.expiries.some(expiry => !teams.includes(expiry.team))) {
      return { code: 'invalidRequest', message: 'timeout evidence contains an unknown team' }
    }
  }
  return null
}

function isGameStartKind(value: unknown): value is GameStartKind {
  if (!isJsonObject(value) || !('type' in value)) return false
  if (value.type === 'ordinaryNewGame') return Object.keys(value).length === 1
  return value.type === 'eliminationDrawContinuation'
    && Object.keys(value).length === 2
    && isValidEliminationContinuation(value.continuation)
}

function isValidEliminationContinuation(
  value: unknown,
  expectedTeams?: [string, string],
): boolean {
  if (!isJsonObject(value) || !hasExactKeys(value, ['teams'])
    || !Array.isArray(value.teams) || value.teams.length !== 2) return false
  const seen = new Set<string>()
  for (const entry of value.teams) {
    if (!isJsonObject(entry) || !hasExactKeys(entry, ['team', 'mainTimeRemainingMs'])
      || !nonEmpty(entry.team) || seen.has(entry.team)
      || (expectedTeams && !expectedTeams.includes(entry.team))
      || (entry.mainTimeRemainingMs !== null
        && !safeNonNegativeInteger(entry.mainTimeRemainingMs))) return false
    seen.add(entry.team)
  }
  return !expectedTeams || expectedTeams.every(team => seen.has(team))
}

function isGameTerminalEvidence(value: unknown): value is GameTerminalEvidence {
  if (!isJsonObject(value) || typeof value.type !== 'string') return false
  if (value.type === 'canonicalGameEnded') {
    return Object.keys(value).length === 2 && isGameConclusion(value.conclusion)
  }
  if (value.type !== 'timeout'
    || Object.keys(value).length !== 5
    || !nonEmpty(value.timeoutId)
    || !Array.isArray(value.expiries)
    || (typeof value.observedAtMonotonicMs !== 'number'
      || !Number.isSafeInteger(value.observedAtMonotonicMs)
      || value.observedAtMonotonicMs < 0)
    || !nonEmpty(value.clockCheckpointDigest)) return false
  if (value.expiries.length < 1 || value.expiries.length > 2) return false
  const expiries = value.expiries
  return expiries.every(isValidTimeoutExpiry)
    && new Set(expiries.map(expiry => expiry.team)).size === expiries.length
}

function isValidTimeoutExpiry(value: JsonValue): value is JsonObject & {
  team: string
  deadlineMonotonicMs: number
} {
  return isJsonObject(value)
    && Object.keys(value).length === 2
    && nonEmpty(value.team)
    && typeof value.deadlineMonotonicMs === 'number'
    && Number.isSafeInteger(value.deadlineMonotonicMs)
    && value.deadlineMonotonicMs >= 0
}

function isGameConclusion(value: unknown): boolean {
  if (!isJsonObject(value) || Object.keys(value).length !== 2
    || !isJsonObject(value.outcome) || !Array.isArray(value.causes)) return false
  const outcome = value.outcome
  const validOutcome = (outcome.type === 'draw' && Object.keys(outcome).length === 1)
    || (outcome.type === 'winner' && Object.keys(outcome).length === 2 && nonEmpty(outcome.team))
  return validOutcome && value.causes.every(isGameEndCause)
}

function isGameEndCause(value: unknown): boolean {
  if (!isJsonObject(value) || typeof value.type !== 'string') return false
  if (value.type === 'teamHpDepleted') {
    return Object.keys(value).length === 2
      && Array.isArray(value.teams)
      && value.teams.length > 0
      && value.teams.every(team => nonEmpty(team))
  }
  return value.type === 'directVictory'
    && Object.keys(value).length === 3
    && nonEmpty(value.rule)
    && nonEmpty(value.team)
}

function isValidClockCheckpoint(value: JsonObject | null, teams: string[]): boolean {
  if (!value || Object.keys(value).some(key => ![
    'preparationStarted', 'doubleExpired', 'version', 'config', 'teams', 'running',
    'paused', 'lastObservedMs', 'nextByoyomiEntrySequence', 'expiredTeam', 'needsDecision',
  ].includes(key))) return false
  if ((value.version !== 1 && value.version !== 2)
    || (value.version === 1 && 'paused' in value)
    || (value.version === 2 && !('paused' in value))
    || Object.keys(value).length !== (value.version === 1 ? 10 : 11)
    || typeof value.preparationStarted !== 'boolean'
    || typeof value.doubleExpired !== 'boolean'
    || !isValidClockConfig(value.config)
    || !isExactClockConfig(value.config)
    || !Array.isArray(value.teams) || value.teams.length !== 2
    || value.teams.some(team => !isJsonObject(team)
      || Object.keys(team).some(key => ![
        'team', 'mainTimeRemainingMs', 'actionTurn', 'actionTimeRemainingMs',
        'actionByoyomiStarted', 'enteredByoyomi', 'byoyomiEntrySequence',
      ].includes(key))
      || !nonEmpty(team.team)
      || (team.mainTimeRemainingMs !== null && !validNonNegativeInteger(team.mainTimeRemainingMs))
      || (team.actionTurn !== null && !validNonNegativeInteger(team.actionTurn))
      || (team.actionTimeRemainingMs !== null && !validNonNegativeInteger(team.actionTimeRemainingMs))
      || typeof team.actionByoyomiStarted !== 'boolean'
      || typeof team.enteredByoyomi !== 'boolean'
      || (team.byoyomiEntrySequence !== null && !validNonNegativeInteger(team.byoyomiEntrySequence)))
    || !value.teams.every(team => isJsonObject(team))) return false
  const clockTeams = value.teams as JsonObject[]
  if (new Set(clockTeams.map(team => team.team)).size !== 2
    || !clockTeams.every(team => teams.includes(team.team as string))
    || !isValidCheckpointWindow(value.running, teams)
    || !isValidCheckpointWindow(value.paused, teams)
    || (value.running !== null && value.running !== undefined
      && value.paused !== null && value.paused !== undefined)) return false
  if (!validNonNegativeInteger(value.lastObservedMs)
    || !validNonNegativeInteger(value.nextByoyomiEntrySequence)
    || (value.expiredTeam !== null && !nonEmpty(value.expiredTeam))
    || (value.needsDecision !== null && ![
      'zeroTimeControl', 'simultaneousByoyomiEntry', 'unlimitedEliminationDraw', 'zeroTimeEliminationDraw',
    ].includes(value.needsDecision as string))) return false
  if (value.doubleExpired && value.expiredTeam !== null) return false
  if (value.expiredTeam !== null && !teams.includes(value.expiredTeam as string)) return false
  if (value.expiredTeam !== null && value.needsDecision !== null) return false
  if (value.doubleExpired && value.needsDecision !== null) return false

  const config = value.config as JsonObject
  const activeWindow = value.running ?? value.paused
  const preparationTeams = isJsonObject(activeWindow) && activeWindow.type === 'preparation'
    && Array.isArray(activeWindow.teams)
    ? activeWindow.teams as string[]
    : null
  if (isJsonObject(value.running) && value.running.type === 'preparation'
    && value.preparationStarted !== true) return false
  if (value.needsDecision !== null && value.running === null) return false
  if (value.paused !== null
    && (value.expiredTeam !== null || value.doubleExpired || value.needsDecision !== null)) return false

  for (const team of clockTeams) {
    const main = team.mainTimeRemainingMs
    const action = team.actionTimeRemainingMs
    const entered = team.enteredByoyomi === true
    const started = team.actionByoyomiStarted === true
    const sequence = team.byoyomiEntrySequence
    const mainNumber = typeof main === 'number' ? main : null
    const sequenceNumber = typeof sequence === 'number' ? sequence : null
    if (!validConfiguredRemaining(config.mainTimeMs, main)
      || (sequenceNumber !== null && sequenceNumber >= (value.nextByoyomiEntrySequence as number))
      || entered !== (sequence !== null)
      || (mainNumber !== null && mainNumber > 0 && (entered || started || action !== null))
      || (action !== null && !started)
      || (started && !entered)
      || (started && !validConfiguredRemaining(config.actionTimeMs, action))
      || (started && team.actionTurn === null
        && !(preparationTeams?.includes(team.team as string) ?? false))
      || (team.actionTurn === null && action !== null
        && !(preparationTeams?.includes(team.team as string) ?? false))) {
      return false
    }
  }

  const running = isJsonObject(value.running) ? value.running : null
  if (running) {
    if (running.type === 'action') {
      const team = clockTeams.find(entry => entry.team === running.team)
      if (!team || team.actionTurn !== running.turn) return false
    } else if (running.type === 'outOfTurnChoice'
      && !validConfiguredRemaining(config.outOfTurnChoiceTimeMs, running.remainingMs)) {
      return false
    }
  }
  if (value.paused !== null && isJsonObject(value.paused)
    && value.paused.type === 'outOfTurnChoice'
    && !validConfiguredRemaining(config.outOfTurnChoiceTimeMs, value.paused.remainingMs)) {
    return false
  }

  if (value.doubleExpired) {
    if (!running || running.type !== 'preparation'
      || !Array.isArray(running.teams)
      || running.teams.length !== 2
      || !clockTeams.every(team => (running.teams as string[]).includes(team.team as string)
        && team.mainTimeRemainingMs === 0
        && team.actionByoyomiStarted === true
        && team.actionTimeRemainingMs === 0)) return false
  }
  if (value.expiredTeam !== null) {
    const expired = clockTeams.find(team => team.team === value.expiredTeam)
    if (!expired) return false
    if (!running) return false
    if (running.type === 'outOfTurnChoice') {
      return running.team === value.expiredTeam && running.remainingMs === 0
    }
    const exhausted = expired.mainTimeRemainingMs === 0
      && expired.actionByoyomiStarted === true
      && expired.actionTimeRemainingMs === 0
    if (!exhausted) return false
    if (running.type === 'action'
      && running.team !== value.expiredTeam) return false
    if (running.type === 'preparation'
      && (!Array.isArray(running.teams) || !running.teams.includes(value.expiredTeam))) return false
  }
  return true
}

function validConfiguredRemaining(configured: JsonValue | undefined, remaining: JsonValue | undefined): boolean {
  if (configured === null) return remaining === null
  return typeof configured === 'number'
    && Number.isSafeInteger(configured)
    && configured >= 0
    && typeof remaining === 'number'
    && Number.isSafeInteger(remaining)
    && remaining >= 0
    && remaining <= configured
}

function isExactClockConfig(value: unknown): boolean {
  return isJsonObject(value)
    && Object.keys(value).length === 3
    && ['mainTimeMs', 'actionTimeMs', 'outOfTurnChoiceTimeMs'].every(key => key in value)
}

function isValidCheckpointWindow(value: JsonValue | undefined, teams: string[]): boolean {
  if (value === null || value === undefined) return true
  if (!isJsonObject(value) || typeof value.type !== 'string') return false
  if (value.type === 'preparation') {
    return Object.keys(value).length === 2
      && Array.isArray(value.teams)
      && value.teams.length > 0
      && value.teams.every(team => nonEmpty(team))
      && new Set(value.teams).size === value.teams.length
      && value.teams.every(team => teams.includes(team as string))
  }
  if (value.type === 'action') {
    return Object.keys(value).length === 3
      && nonEmpty(value.team)
      && teams.includes(value.team)
      && validNonNegativeInteger(value.turn)
  }
  return value.type === 'outOfTurnChoice'
    && Object.keys(value).length === 4
    && nonEmpty(value.team)
    && teams.includes(value.team)
    && nonEmpty(value.choiceId)
    && (value.remainingMs === null || validNonNegativeInteger(value.remainingMs))
}

function validNonNegativeInteger(value: unknown): value is number {
  return safeNonNegativeInteger(value)
}

async function deriveTimeoutRuling(
  evidence: Extract<GameTerminalEvidence, { type: 'timeout' }>,
  checkpoint: JsonObject,
): Promise<{ ok: true; value: CompetitionRuling } | RoomFailure> {
  if (evidence.observedAtMonotonicMs !== checkpoint.lastObservedMs) {
    return roomFailure('invalidTimeoutEvidence', 'timeout observation does not match clock checkpoint')
  }
  if (await clockCheckpointDigest(checkpoint) !== evidence.clockCheckpointDigest) {
    return roomFailure('invalidTimeoutEvidence', 'timeout clock checkpoint digest does not match')
  }
  if (checkpoint.needsDecision !== null) {
    return roomFailure('invalidTimeoutEvidence', 'timeout checkpoint still needs a decision')
  }
  if (evidence.expiries.length === 1) {
    const expiry = evidence.expiries[0]
    if (!expiry || checkpoint.doubleExpired !== false || checkpoint.expiredTeam !== expiry.team) {
      return roomFailure('invalidTimeoutEvidence', 'single timeout flags do not match the evidence')
    }
    return { ok: true, value: { type: 'timeoutLoss', loser: expiry.team } }
  }
  const expiredTeams = evidence.expiries.map(expiry => expiry.team)
  const checkpointTeams = (checkpoint.teams as JsonObject[]).map(team => team.team as string)
  if (checkpoint.doubleExpired !== true || checkpoint.expiredTeam !== null
    || expiredTeams.length !== 2
    || new Set(expiredTeams).size !== 2
    || !checkpointTeams.every(team => expiredTeams.includes(team))) {
    return roomFailure('invalidTimeoutEvidence', 'double timeout flags do not match the evidence')
  }
  return { ok: true, value: { type: 'doubleTimeoutLoss', teams: [expiredTeams[0]!, expiredTeams[1]!] } }
}

type ParsedClockMutationResult =
  | ({ type: 'unchanged' | 'advanced' | 'paused' | 'resumed' | 'newGame' | 'needsDecision'; reason?: string }
    | { type: 'enteredByoyomi'; team: string }
    | { type: 'timedOut'; team: string }
    | { type: 'doubleTimedOut'; teams: [string, string] }) & {
      nextTimeoutDeadlineMs: number | null
    }

function parseClockMutationResult(
  publicValue: JsonValue,
  postClock: JsonObject | null,
  teams: string[],
): { ok: true; value: ParsedClockMutationResult } | RoomFailure {
  if (!isJsonObject(publicValue)
    || Object.keys(publicValue).length !== 2
    || !isJsonObject(publicValue.result)
    || !('clock' in publicValue)
    || !isJsonObject(publicValue.clock)) {
    return roomFailure('engineInvalidClockResult', 'clock mutation public result is invalid')
  }
  const clockError = validateClockPublicView(publicValue.clock, postClock, teams)
  if (clockError) return roomFailure(clockError.code, clockError.message)
  const nextTimeoutDeadlineMs = publicValue.clock.nextTimeoutDeadlineMs as number | null
  const result = publicValue.result
  if (typeof result.type !== 'string') {
    return roomFailure('engineInvalidClockResult', 'clock mutation update is missing its type')
  }
  switch (result.type) {
    case 'unchanged':
    case 'advanced':
    case 'paused':
    case 'resumed':
    case 'newGame':
      return Object.keys(result).length === 1
        ? { ok: true, value: { type: result.type, nextTimeoutDeadlineMs } }
        : roomFailure('engineInvalidClockResult', 'clock mutation update contains unsupported fields')
    case 'enteredByoyomi':
    case 'timedOut':
      return Object.keys(result).length === 2 && nonEmpty(result.team)
        ? { ok: true, value: { type: result.type, team: result.team, nextTimeoutDeadlineMs } }
        : roomFailure('engineInvalidClockResult', 'clock mutation update has an invalid team')
    case 'doubleTimedOut': {
      if (Object.keys(result).length !== 2 || !Array.isArray(result.teams)
        || result.teams.length !== 2
        || result.teams.some(team => !nonEmpty(team as unknown))) {
        return roomFailure('engineInvalidClockResult', 'double timeout update is invalid')
      }
      const teams = result.teams as string[]
      return new Set(teams).size === 2
        ? {
            ok: true,
            value: { type: result.type, teams: [teams[0]!, teams[1]!], nextTimeoutDeadlineMs },
          }
        : roomFailure('engineInvalidClockResult', 'double timeout update contains duplicate teams')
    }
    case 'needsDecision':
      return Object.keys(result).length === 2
        && typeof result.reason === 'string'
        && [
          'zeroTimeControl', 'simultaneousByoyomiEntry', 'unlimitedEliminationDraw',
          'zeroTimeEliminationDraw',
        ].includes(result.reason)
        ? { ok: true, value: { type: result.type, reason: result.reason, nextTimeoutDeadlineMs } }
        : roomFailure('engineInvalidClockResult', 'needs-decision update is invalid')
    default:
      return roomFailure('engineInvalidClockResult', 'clock mutation returned an unknown update')
  }
}

function validateClockPublicView(
  value: unknown,
  checkpoint: JsonObject | null,
  teams: string[],
): { code: string; message: string } | null {
  if (!isJsonObject(value)) {
    return { code: 'engineInvalidClockResult', message: 'clock public view is not an object' }
  }
  const allowed = new Set([
    'config', 'teams', 'activeWindow', 'pausedWindow', 'expiredTeam', 'needsDecision',
    'timeoutRuling', 'byoyomiEntryOrder', 'lastObservedMs', 'nextTimeoutDeadlineMs',
  ])
  if (Object.keys(value).length !== allowed.size || Object.keys(value).some(key => !allowed.has(key))) {
    return { code: 'engineInvalidClockResult', message: 'clock public view contains unsupported fields' }
  }
  if (!isJsonObject(checkpoint)
    || !isValidClockCheckpoint(checkpoint, teams)
    || !isExactClockConfig(value.config)
    || !isValidClockConfig(value.config)
    || !Array.isArray(value.teams)
    || !value.teams.every(isJsonObject)
    || !isValidPublicWindow(value.activeWindow, checkpoint.running)
    || !isValidPublicWindow(value.pausedWindow, checkpoint.paused ?? null)
    || (value.nextTimeoutDeadlineMs !== null && !safeNonNegativeInteger(value.nextTimeoutDeadlineMs))
    || !validNonNegativeInteger(value.lastObservedMs)
    || !isValidByoyomiEntryOrder(value.byoyomiEntryOrder)
    || !isValidPublicTimeoutRuling(value.timeoutRuling, checkpoint, teams)) {
    return { code: 'engineInvalidClockResult', message: 'clock public view is invalid' }
  }
  const deadline = value.nextTimeoutDeadlineMs as number | null
  const hasRunningWindow = checkpoint.running !== null && checkpoint.running !== undefined
  const hasPausedWindow = checkpoint.paused !== null && checkpoint.paused !== undefined
  const deadlineForbidden = !hasRunningWindow
    || hasPausedWindow
    || checkpoint.expiredTeam !== null
    || checkpoint.doubleExpired
    || checkpoint.needsDecision !== null
  if (deadline !== null
    && (deadlineForbidden || deadline < (checkpoint.lastObservedMs as number))) {
    return { code: 'engineInvalidClockResult', message: 'clock public view deadline contradicts its checkpoint' }
  }
  if (!sameJson(value.config, checkpoint.config)
    || !sameJson(value.teams, checkpoint.teams)
    || value.lastObservedMs !== checkpoint.lastObservedMs
    || value.expiredTeam !== checkpoint.expiredTeam
    || value.needsDecision !== checkpoint.needsDecision) {
    return { code: 'engineInvalidClockResult', message: 'clock public view contradicts its checkpoint' }
  }
  return null
}

function isValidPublicWindow(
  value: JsonValue | undefined,
  checkpoint: JsonValue | null | undefined,
): boolean {
  if (checkpoint === null || checkpoint === undefined) return value === null
  if (!isJsonObject(checkpoint) || !isJsonObject(value) || typeof checkpoint.type !== 'string') return false
  if (checkpoint.type === 'preparation') {
    return Object.keys(value).length === 2 && value.type === 'preparation'
      && sameJson(value.teams, checkpoint.teams)
  }
  if (checkpoint.type === 'action') {
    return Object.keys(value).length === 3 && value.type === 'action'
      && value.team === checkpoint.team && value.turn === checkpoint.turn
  }
  return checkpoint.type === 'outOfTurnChoice'
    && Object.keys(value).length === 3
    && value.type === 'outOfTurnChoice'
    && value.team === checkpoint.team
    && value.choiceId === checkpoint.choiceId
}

function isValidByoyomiEntryOrder(value: unknown): boolean {
  return value === 'none' || value === 'simultaneous'
    || (isJsonObject(value) && Object.keys(value).length === 1 && nonEmpty(value.first))
}

function isValidPublicTimeoutRuling(
  value: unknown,
  checkpoint: JsonObject,
  teams: string[],
): boolean {
  if (value === null) return checkpoint.expiredTeam === null && checkpoint.doubleExpired === false
  if (!isJsonObject(value) || !Array.isArray(checkpoint.teams)) return false
  if (checkpoint.doubleExpired) {
    return Object.keys(value).length === 2 && value.type === 'doubleTimeoutLoss'
      && isTeamPair(value.teams) && sameTeamSet(value.teams, teams)
  }
  return Object.keys(value).length === 2 && value.type === 'timeoutLoss'
    && nonEmpty(value.loser) && value.loser === checkpoint.expiredTeam && teams.includes(value.loser)
}

async function deriveClockTimeoutEvidence(
  update: ParsedClockMutationResult,
  preClock: JsonObject | null,
  postClock: JsonObject | null,
  competitionId: string,
  matchId: string,
  commandId: string,
  revision: number,
  matchTeams: string[] = [],
): Promise<{
  ok: true
  value: Extract<GameTerminalEvidence, { type: 'timeout' }> | null
} | RoomFailure> {
  if (update.type !== 'timedOut' && update.type !== 'doubleTimedOut') {
    return { ok: true, value: null }
  }
  if (!preClock || !postClock || matchTeams.length !== 2
    || new Set(matchTeams).size !== 2
    || !isValidClockCheckpoint(preClock, matchTeams)
    || !isValidClockCheckpoint(postClock, matchTeams)
    || preClock.running === null || preClock.running === undefined
    || preClock.paused !== null && preClock.paused !== undefined
    || preClock.expiredTeam !== null
    || preClock.doubleExpired !== false
    || preClock.needsDecision !== null) {
    return roomFailure('invalidTimeoutEvidence', 'timeout evidence input checkpoint is invalid')
  }
  if (!safeNonNegativeInteger(preClock.lastObservedMs)
    || !safeNonNegativeInteger(postClock.lastObservedMs)
    || postClock.lastObservedMs < preClock.lastObservedMs) {
    return roomFailure('invalidTimeoutEvidence', 'timeout checkpoint observation is invalid')
  }

  const running = preClock.running
  if (!isJsonObject(running) || typeof running.type !== 'string') {
    return roomFailure('invalidTimeoutEvidence', 'timeout checkpoint has no active window')
  }
  const deadlines: Array<{ team: string; deadline: number }> = []
  const addDeadline = (team: string, snapshot: JsonObject): RoomFailure | null => {
    const deadline = timeoutDeadline(preClock.lastObservedMs as number, preClock.config ?? null, snapshot)
    if (!deadline.ok) return roomFailure('invalidTimeoutEvidence', deadline.message)
    if (deadline.value !== null) deadlines.push({ team, deadline: deadline.value })
    return null
  }
  if (running.type === 'preparation') {
    if (Object.keys(running).length !== 2 || !Array.isArray(running.teams)
      || running.teams.length < 1 || running.teams.length > 2
      || running.teams.some(team => !nonEmpty(team as unknown))) {
      return roomFailure('invalidTimeoutEvidence', 'preparation timeout window is invalid')
    }
    const teamSnapshots = preClock.teams as JsonObject[]
    for (const rawTeam of running.teams) {
      const team = rawTeam as string
      const snapshot = teamSnapshots.find(entry => entry.team === team)
      if (!snapshot) return roomFailure('invalidTimeoutEvidence', 'timeout team snapshot is missing')
      const error = addDeadline(team, snapshot)
      if (error) return error
    }
  } else if (running.type === 'action') {
    if (Object.keys(running).length !== 3 || !nonEmpty(running.team)
      || !safeNonNegativeInteger(running.turn)) {
      return roomFailure('invalidTimeoutEvidence', 'action timeout window is invalid')
    }
    const snapshot = (preClock.teams as JsonObject[]).find(entry => entry.team === running.team)
    if (!snapshot) return roomFailure('invalidTimeoutEvidence', 'timeout team snapshot is missing')
    const error = addDeadline(running.team, snapshot)
    if (error) return error
  } else if (running.type === 'outOfTurnChoice') {
    if (Object.keys(running).length !== 4 || !nonEmpty(running.team)
      || !nonEmpty(running.choiceId)
      || (running.remainingMs !== null && !safeNonNegativeInteger(running.remainingMs))) {
      return roomFailure('invalidTimeoutEvidence', 'choice timeout window is invalid')
    }
    if (running.remainingMs !== null) {
      const deadline = checkedAdd(preClock.lastObservedMs as number, running.remainingMs as number)
      if (deadline === null) return roomFailure('invalidTimeoutEvidence', 'timeout deadline arithmetic overflowed')
      deadlines.push({ team: running.team, deadline })
    }
  } else {
    return roomFailure('invalidTimeoutEvidence', 'timeout checkpoint has an unknown active window')
  }

  const earliest = deadlines.length === 0
    ? null
    : Math.min(...deadlines.map(entry => entry.deadline))
  if (earliest === null || earliest > (postClock.lastObservedMs as number)) {
    return roomFailure('invalidTimeoutEvidence', 'timeout deadline is not observed by the post checkpoint')
  }
  const earliestTeams = deadlines.filter(entry => entry.deadline === earliest).map(entry => entry.team)
  let expiryTeams: string[]
  if (update.type === 'timedOut') {
    if (earliestTeams.length !== 1 || update.team !== earliestTeams[0]) {
      return roomFailure('invalidTimeoutEvidence', 'single timeout update does not match the earliest deadline')
    }
    if (postClock.doubleExpired !== false || postClock.expiredTeam !== update.team) {
      return roomFailure('invalidTimeoutEvidence', 'single timeout flags do not match the update')
    }
    expiryTeams = [update.team]
  } else {
    if (running.type !== 'preparation' || earliestTeams.length !== 2
      || !sameTeamSet(update.teams, earliestTeams)
      || postClock.doubleExpired !== true || postClock.expiredTeam !== null) {
      return roomFailure('invalidTimeoutEvidence', 'double timeout does not match the earliest deadlines')
    }
    expiryTeams = update.teams
  }
  const timeoutId = await clockTimeoutId({ competitionId, matchId, commandId, revision })
  return {
    ok: true,
    value: {
      type: 'timeout',
      timeoutId,
      expiries: expiryTeams.map(team => ({ team, deadlineMonotonicMs: earliest })),
      observedAtMonotonicMs: postClock.lastObservedMs as number,
      clockCheckpointDigest: await clockCheckpointDigest(postClock),
    },
  }
}

function timeoutDeadline(
  lastObservedMs: number,
  config: JsonValue,
  snapshot: JsonObject,
): { ok: true; value: number | null } | { ok: false; message: string } {
  if (!isJsonObject(config) || !safeNonNegativeInteger(lastObservedMs)
    || !safeNonNegativeInteger(snapshot.mainTimeRemainingMs)
      && snapshot.mainTimeRemainingMs !== null) {
    return { ok: false, message: 'timeout clock values are invalid' }
  }
  const main = snapshot.mainTimeRemainingMs as number | null
  if (main === null) return { ok: true, value: null }
  const allowance = snapshot.actionByoyomiStarted === true
    ? snapshot.actionTimeRemainingMs
    : config.actionTimeMs
  if (allowance === null) return { ok: true, value: null }
  if (!safeNonNegativeInteger(allowance)) return { ok: false, message: 'timeout clock allowance is invalid' }
  const remaining = snapshot.actionByoyomiStarted === true
    ? allowance
    : checkedAdd(main, allowance)
  if (remaining === null) return { ok: false, message: 'timeout deadline arithmetic overflowed' }
  const deadline = checkedAdd(lastObservedMs, remaining)
  return deadline === null
    ? { ok: false, message: 'timeout deadline arithmetic overflowed' }
    : { ok: true, value: deadline }
}

function checkedAdd(left: number, right: number): number | null {
  if (!safeNonNegativeInteger(left) || !safeNonNegativeInteger(right)
    || left > Number.MAX_SAFE_INTEGER - right) return null
  return left + right
}

function floorCompetitionMinute(value: number | null): number | null {
  if (!safeNonNegativeInteger(value)) return null
  return value - value % 60_000
}

function safeNonNegativeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
}

function parseClockRuleGameResult(
  publicValue: JsonValue,
  teams: string[],
): { ok: true; value: CompetitionRuling } | RoomFailure {
  if (!isJsonObject(publicValue)
    || Object.keys(publicValue).length !== 2
    || !isJsonObject(publicValue.result)
    || !isJsonObject(publicValue.clock)) {
    return roomFailure('engineInvalidRuling', 'clockRuleGame public result is missing')
  }
  const ruling = parseCompetitionRuling(publicValue.result, teams)
  if (!ruling) return roomFailure('engineInvalidRuling', 'clockRuleGame returned an invalid ruling')
  return { ok: true, value: ruling }
}

function parseCompetitionRuling(value: unknown, teams: string[]): CompetitionRuling | null {
  if (!isJsonObject(value) || typeof value.type !== 'string') return null
  switch (value.type) {
    case 'timeoutLoss':
      return Object.keys(value).length === 2 && nonEmpty(value.loser)
        && teams.includes(value.loser)
        ? { type: 'timeoutLoss', loser: value.loser }
        : null
    case 'doubleTimeoutLoss':
      return Object.keys(value).length === 2 && isTeamPair(value.teams)
        && sameTeamSet(value.teams, teams)
        ? { type: 'doubleTimeoutLoss', teams: value.teams }
        : null
    case 'gameAccepted': {
      if (Object.keys(value).length !== 2) return null
      if (value.outcome === 'draw') return { type: 'gameAccepted', outcome: 'draw' }
      if (isJsonObject(value.outcome) && Object.keys(value.outcome).length === 1
        && nonEmpty(value.outcome.winner)
        && teams.includes(value.outcome.winner)) {
        return { type: 'gameAccepted', outcome: { winner: value.outcome.winner } }
      }
      return null
    }
    case 'eliminationDrawLoss':
      return Object.keys(value).length === 3 && nonEmpty(value.loser)
        && teams.includes(value.loser)
        && value.reason === 'enteredByoyomiFirst'
        ? { type: 'eliminationDrawLoss', loser: value.loser, reason: 'enteredByoyomiFirst' }
        : null
    case 'needsTiebreak':
      return Object.keys(value).length === 2 && isJsonObject(value.continuation)
        && Array.isArray(value.continuation.teams)
        && value.continuation.teams.length === 2
        && value.continuation.teams.every(team => isJsonObject(team)
          && Object.keys(team).length === 2 && nonEmpty(team.team)
          && teams.includes(team.team)
          && (team.mainTimeRemainingMs === null || validNonNegativeInteger(team.mainTimeRemainingMs)))
        && sameTeamSet(value.continuation.teams.map(team => (team as JsonObject).team as string), teams)
        ? { type: 'needsTiebreak', continuation: {
            teams: value.continuation.teams.map(team => ({
              team: (team as JsonObject).team as string,
              mainTimeRemainingMs: (team as JsonObject).mainTimeRemainingMs as number | null,
            })),
          } }
        : null
    case 'needsDecision':
      return Object.keys(value).length === 2 && [
        'zeroTimeControl', 'simultaneousByoyomiEntry', 'unlimitedEliminationDraw', 'zeroTimeEliminationDraw',
      ].includes(value.reason as string)
        ? { type: 'needsDecision', reason: value.reason as
            'zeroTimeControl' | 'simultaneousByoyomiEntry' | 'unlimitedEliminationDraw' | 'zeroTimeEliminationDraw' }
        : null
    default:
      return null
  }
}

function isTeamPair(value: unknown): value is [string, string] {
  return Array.isArray(value) && value.length === 2
    && value.every(team => nonEmpty(team)) && value[0] !== value[1]
}

function sameTeamSet(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every(team => right.includes(team))
}

/** 正常建立鏈的 engine 邊界不得忽略私密欄位或不合法 success／error envelope。 */
function isInitializeEngineResponse(value: unknown, operation: string): value is CompetitionEngineResponse {
  if (!isExactCompetitionJson(value) || !isJsonObject(value)) return false
  if (value.ok === false) {
    return hasExactClockQueryKeys(value, ['ok', 'error']) && isJsonObject(value.error)
      && hasExactClockQueryKeys(value.error, ['code', 'layer', 'message'])
      && typeof value.error.code === 'string' && typeof value.error.layer === 'string'
      && typeof value.error.message === 'string'
  }
  return value.ok === true && value.operation === operation
    && hasExactClockQueryKeys(value, ['ok', 'operation', 'public', 'serverState'])
    && isJsonObject(value.serverState)
}

function isRulesInitializeRequest(value: unknown): value is CompetitionRoomInitializeFromRulesRequest {
  if (!isExactCompetitionJson(value) || !isJsonObject(value)
    || !hasExactClockQueryKeys(value, ['competitionId', 'callerUserId', 'announcement', 'trustedTimeMs'])
    || !nonEmpty(value.competitionId) || !nonEmpty(value.callerUserId)
    || !safeNonNegativeInteger(value.trustedTimeMs) || !isJsonObject(value.announcement)) return false
  const draft = value.announcement
  if (!hasExactClockQueryKeys(draft, [
    'ruleVersion', 'ruleset', 'enabledRuleModules', 'teamSize', 'registrationMode',
    'personalDecks', 'registrationDeadline', 'checkInDeadline', 'plan',
  ]) || (draft.ruleVersion !== '5.16' && draft.ruleVersion !== '5.17')
    || !nonEmpty(draft.ruleset) || !isUniqueNonEmptyStringArray(draft.enabledRuleModules)
    || (draft.teamSize !== 1 && draft.teamSize !== 2)
    || !safeNonNegativeInteger(draft.registrationDeadline) || draft.registrationDeadline % 60_000 !== 0
    || !safeNonNegativeInteger(draft.checkInDeadline) || draft.checkInDeadline % 60_000 !== 0
    || draft.registrationDeadline > draft.checkInDeadline
    || !parseCompetitionAnnouncementPlan(draft.plan)
    || draft.personalDecks !== (draft.enabledRuleModules.includes('personal-deck') ? 'required' : 'disabled')) return false
  return draft.registrationMode === 'public'
    || (isJsonObject(draft.registrationMode) && hasExactClockQueryKeys(draft.registrationMode, ['inviteOnly'])
      && isJsonObject(draft.registrationMode.inviteOnly)
      && hasExactClockQueryKeys(draft.registrationMode.inviteOnly, ['invitedCaptains'])
      && isUniqueNonEmptyStringArray(draft.registrationMode.inviteOnly.invitedCaptains)
      && draft.registrationMode.inviteOnly.invitedCaptains.length > 0)
}

function validateInitializeRequest(
  request: CompetitionRoomInitializeRequest,
): { code: string; message: string } | null {
  if (!nonEmpty(request.competitionId) || !nonEmpty(request.callerUserId)) {
    return { code: 'invalidRequest', message: 'competition id and caller user id are required' }
  }
  if (!Number.isFinite(request.trustedTimeMs)) {
    return { code: 'invalidRequest', message: 'trusted time must be finite' }
  }
  if (request.announcement.organizer !== request.callerUserId) {
    return { code: 'organizerMismatch', message: 'announcement organizer must be the logged-in organizer' }
  }
  if (request.announcement.registrationDeadline % 60_000 !== 0
    || request.announcement.checkInDeadline % 60_000 !== 0
    || request.announcement.registrationDeadline > request.announcement.checkInDeadline
    || !parseCompetitionAnnouncementPlan(request.announcement.plan)) {
    return { code: 'invalidRequest', message: 'announcement must contain a complete minute-based plan' }
  }
  if (!Array.isArray(request.participantUserIds)
    || request.participantUserIds.some(userId => !nonEmpty(userId))) {
    return { code: 'invalidRequest', message: 'participant user ids must be non-empty strings' }
  }
  return null
}

function validateExecuteRequest(
  request: CompetitionRoomExecuteRequest,
): { code: string; message: string } | null {
  if (!nonEmpty(request.competitionId)
    || !nonEmpty(request.callerUserId)
    || !nonEmpty(request.commandId)) {
    return { code: 'invalidRequest', message: 'competition id, caller and command id are required' }
  }
  if (!Number.isInteger(request.expectedRevision) || request.expectedRevision < 1) {
    return { code: 'invalidRequest', message: 'expected revision must be a positive integer' }
  }
  if (!Number.isFinite(request.trustedTimeMs)) {
    return { code: 'invalidRequest', message: 'trusted time must be finite' }
  }
  if (!request.operation || typeof request.operation !== 'object') {
    return { code: 'invalidRequest', message: 'operation is required' }
  }
  const rawOperation: unknown = request.operation
  if (isJsonObject(rawOperation) && rawOperation.type === 'registrationCreate') {
    return { code: 'operationNotAllowed', message: 'registrationCreate is only available during initialization' }
  }
  return null
}

function validateMatchExecuteRequest(
  request: CompetitionRoomMatchExecuteRequest,
): { code: string; message: string } | null {
  if (!nonEmpty(request.competitionId)
    || !nonEmpty(request.matchId)
    || !nonEmpty(request.callerUserId)
    || !nonEmpty(request.commandId)) {
    return { code: 'invalidRequest', message: 'competition id, match id, caller and command id are required' }
  }
  if (!Number.isInteger(request.expectedMatchRevision) || request.expectedMatchRevision < 0) {
    return { code: 'invalidRequest', message: 'expected match revision must be a non-negative integer' }
  }
  if (!Number.isFinite(request.trustedTimeMs)) {
    return { code: 'invalidRequest', message: 'trusted time must be finite' }
  }
  if (!request.operation || typeof request.operation !== 'object') {
    return { code: 'invalidRequest', message: 'match operation is required' }
  }
  const rawOperation: unknown = request.operation
  if (!isJsonObject(rawOperation) || typeof rawOperation.type !== 'string'
    || !['matchCreate', 'matchAssignFirstPlayer', 'matchSubmitGame', 'matchCorrectRuling',
      'clockCreate', 'clockBeginPreparation', 'clockFinishPreparation', 'clockBeginAction',
      'clockBeginOutOfTurnChoice', 'clockObserve', 'clockEndWindow', 'clockPause',
      'clockResume', 'clockNewGame',
      'clockRuleGame'].includes(rawOperation.type)) {
    return { code: 'invalidOperation', message: 'operation is not a supported match or clock operation' }
  }
  if (rawOperation.type === 'matchCreate') {
    if (!isJsonObject(rawOperation.config) || typeof rawOperation.config.matchId !== 'string') {
      return { code: 'invalidOperation', message: 'matchCreate config must contain matchId' }
    }
  } else if (typeof rawOperation.matchId !== 'string') {
    return { code: 'invalidOperation', message: 'match operation must contain matchId' }
  }
  if ((rawOperation.type === 'clockPause' || rawOperation.type === 'clockResume')
    && (typeof rawOperation.reason !== 'string' || rawOperation.reason.trim().length === 0)) {
    return { code: 'invalidReason', message: 'pause/resume reason must be non-empty' }
  }
  return null
}

function validateStartScheduledMatchRequest(
  request: CompetitionRoomStartScheduledMatchRequest,
): { code: string; message: string } | null {
  if (!isJsonObject(request as unknown) || !hasExactKeys(request as unknown as JsonObject, [
    'competitionId', 'stageId', 'matchId', 'callerUserId', 'commandId',
    'expectedRevision', 'trustedTimeMs', 'referees',
  ]) || !nonEmpty(request.competitionId) || !nonEmpty(request.stageId)
    || !nonEmpty(request.matchId) || !nonEmpty(request.callerUserId)
    || !nonEmpty(request.commandId)) {
    return { code: 'invalidRequest', message: 'scheduled match start request is invalid' }
  }
  if (!Number.isSafeInteger(request.expectedRevision) || request.expectedRevision < 1) {
    return { code: 'invalidRequest', message: 'expected revision must be a positive integer' }
  }
  if (!Number.isSafeInteger(request.trustedTimeMs) || request.trustedTimeMs < 0) {
    return { code: 'invalidRequest', message: 'trusted time must be finite' }
  }
  if (!Array.isArray(request.referees) || request.referees.some(referee => {
    return !isJsonObject(referee) || !hasExactKeys(referee, ['refereeId', 'conflictedTeam'])
      || !nonEmpty(referee.refereeId)
      || (referee.conflictedTeam !== null && !nonEmpty(referee.conflictedTeam))
  })) {
    return { code: 'invalidRequest', message: 'referee assignments are invalid' }
  }
  const refereeIds = request.referees.map(referee => referee.refereeId)
  if (new Set(refereeIds).size !== refereeIds.length) {
    return { code: 'invalidRequest', message: 'referee assignments must be unique' }
  }
  return null
}

function validateCreateTournamentRequest(
  request: CompetitionRoomCreateTournamentRequest,
): { code: string; message: string } | null {
  if (!isJsonObject(request as unknown)
    || !hasExactKeys(request as unknown as JsonObject, [
      'competitionId', 'callerUserId', 'commandId', 'expectedRevision', 'trustedTimeMs', 'stage',
    ])
    || !nonEmpty(request.competitionId) || !nonEmpty(request.callerUserId)
    || !nonEmpty(request.commandId)) {
    return { code: 'invalidRequest', message: 'tournament creation request is invalid' }
  }
  if (!Number.isSafeInteger(request.expectedRevision) || request.expectedRevision < 1
    || !Number.isSafeInteger(request.trustedTimeMs) || request.trustedTimeMs < 0) {
    return { code: 'invalidRequest', message: 'revision or trusted time is invalid' }
  }
  if (!isValidStageConfigEnvelope(request.stage)) {
    return { code: 'invalidStageConfig', message: 'stage configuration is malformed' }
  }
  return null
}

function isValidStageConfigEnvelope(value: unknown): value is StageConfig {
  if (!isJsonObject(value) || !hasExactKeys(value, [
    'stageId', 'format', 'teams', 'randomSeed', 'matchFormat', 'clockConfig', 'swissRounds', 'swissPolicy',
  ]) || !nonEmpty(value.stageId) || !isValidTournamentFormat(value.format)
    || !Number.isSafeInteger(value.randomSeed) || (value.randomSeed as number) < 0
    || !Array.isArray(value.teams) || value.teams.length < 2 || !isValidMatchFormat(value.matchFormat)
    || !isExactClockConfig(value.clockConfig) || !isValidClockConfig(value.clockConfig)
    || (value.swissRounds !== null && (!Number.isSafeInteger(value.swissRounds)
      || (value.swissRounds as number) < 0 || (value.swissRounds as number) > 0xffff_ffff))) return false
  if (value.teams.some(team => !isJsonObject(team) || !hasExactKeys(team, ['teamId', 'seed'])
    || !nonEmpty(team.teamId) || !Number.isSafeInteger(team.seed) || (team.seed as number) <= 0
    || (team.seed as number) > 0xffff_ffff)) {
    return false
  }
  const teamIds = value.teams.map(team => (team as JsonObject).teamId)
  const seeds = value.teams.map(team => (team as JsonObject).seed)
  if (new Set(teamIds).size !== teamIds.length || new Set(seeds).size !== seeds.length) return false
  if (value.swissPolicy !== null && !isValidSwissPolicyEnvelope(value.swissPolicy)) return false
  if (value.format === 'swiss'
    && (!Number.isSafeInteger(value.swissRounds) || (value.swissRounds as number) <= 0
      || value.swissPolicy === null || !isValidSwissPolicyEnvelope(value.swissPolicy, true))) return false
  return isCompatibleStageMatchFormat(value.format, value.matchFormat as unknown as MatchFormat)
}

function isValidSwissPolicyEnvelope(value: unknown, validateRules = false): boolean {
  const points = isJsonObject(value) && isJsonObject(value.matchPoints) ? value.matchPoints : {}
  if (!isJsonObject(value) || !hasExactKeys(value, [
    'matchPoints', 'drawCredit', 'opponentWinRateFloorPercent',
    'opponentWinRateCapPercent', 'excludeByesFromOpponentWinRate', 'tieBreakers',
  ]) || !isJsonObject(value.matchPoints)
    || !hasExactKeys(value.matchPoints, ['win', 'draw', 'loss', 'bye'])
    || !['win', 'draw', 'loss', 'bye'].every(key => Number.isSafeInteger(points[key])
      && (points[key] as number) >= 0 && (points[key] as number) <= 0xffff_ffff)
    || !isJsonObject(value.drawCredit) || !hasExactKeys(value.drawCredit, ['numerator', 'denominator'])
    || !Number.isSafeInteger(value.drawCredit.numerator) || (value.drawCredit.numerator as number) < 0
    || (value.drawCredit.numerator as number) > 0xffff_ffff
    || !Number.isSafeInteger(value.drawCredit.denominator) || (value.drawCredit.denominator as number) < 0
    || (value.drawCredit.denominator as number) > 0xffff_ffff
    || ![value.opponentWinRateFloorPercent, value.opponentWinRateCapPercent].every(field =>
      field === null || (Number.isSafeInteger(field) && (field as number) >= 0
        && (field as number) <= 0xff))
    || typeof value.excludeByesFromOpponentWinRate !== 'boolean'
    || !Array.isArray(value.tieBreakers) || !value.tieBreakers.every(item => [
      'matchPoints', 'headToHeadMatchPoints', 'opponentWinRate', 'netGameWins', 'seedOrder',
    ].includes(item as string))) return false
  if (!validateRules) return true
  if (value.drawCredit.denominator === 0
    || (value.drawCredit.numerator as number) > (value.drawCredit.denominator as number)) return false
  if (value.opponentWinRateFloorPercent !== null && value.opponentWinRateCapPercent !== null
    && (value.opponentWinRateFloorPercent as number) > (value.opponentWinRateCapPercent as number)) return false
  return ['opponentWinRateFloorPercent', 'opponentWinRateCapPercent'].every(key => {
    const field = value[key]
    return field === null || (Number.isSafeInteger(field) && (field as number) >= 0 && (field as number) <= 100)
  })
}

function isRegistrationAnnouncement(value: unknown, organizerUserId: string): value is JsonObject {
  if (!isJsonObject(value)) return false
  const announcement = value
  return hasExactKeys(announcement, [
    'organizer', 'ruleVersion', 'ruleset', 'enabledRuleModules', 'teamSize', 'initialHp',
    'registrationMode', 'personalDecks', 'registrationDeadline', 'checkInDeadline', 'plan',
  ]) && announcement.organizer === organizerUserId
    && (announcement.ruleVersion === '5.16' || announcement.ruleVersion === '5.17')
    && nonEmpty(announcement.ruleset) && isUniqueNonEmptyStringArray(announcement.enabledRuleModules)
    && Number.isSafeInteger(announcement.teamSize) && (announcement.teamSize as number) > 0
    && Number.isSafeInteger(announcement.initialHp) && (announcement.initialHp as number) > 0
    && (announcement.registrationMode === 'public'
      || (isJsonObject(announcement.registrationMode)
        && hasExactKeys(announcement.registrationMode, ['inviteOnly'])
        && isJsonObject(announcement.registrationMode.inviteOnly)
        && hasExactKeys(announcement.registrationMode.inviteOnly, ['invitedCaptains'])
        && isUniqueNonEmptyStringArray(announcement.registrationMode.inviteOnly.invitedCaptains)
        && announcement.registrationMode.inviteOnly.invitedCaptains.length > 0))
    && announcement.personalDecks === (announcement.enabledRuleModules.includes('personal-deck') ? 'required' : 'disabled')
    && safeNonNegativeInteger(announcement.registrationDeadline)
    && safeNonNegativeInteger(announcement.checkInDeadline)
    && announcement.registrationDeadline % 60_000 === 0
    && announcement.checkInDeadline % 60_000 === 0
    && announcement.registrationDeadline <= announcement.checkInDeadline
    && parseCompetitionAnnouncementPlan(announcement.plan) !== null
}

function registrationCheckpointRevision(value: unknown, organizerUserId: string): number | null {
  if (!isJsonObject(value) || !hasExactKeys(value, ['announcement', 'commands'])
    || !isRegistrationAnnouncement(value.announcement, organizerUserId)
    || !Array.isArray(value.commands)) return null
  const actionKeys: Record<string, string[]> = {
    open: ['type'], lock: ['type'], registerTeam: ['type', 'teamId', 'captain', 'members'],
    replaceRoster: ['type', 'teamId', 'expectedRosterVersion', 'members'],
    confirmMembership: ['type', 'teamId', 'rosterVersion'],
    submitDeck: ['type', 'teamId', 'rosterVersion', 'name', 'cards'],
    checkInTeam: ['type', 'teamId', 'rosterVersion'], withdrawTeam: ['type', 'teamId'],
  }
  const commandIds = new Set<string>()
  let previousTime = 0
  for (const [index, command] of value.commands.entries()) {
    if (!isJsonObject(command)
      || !hasExactKeys(command, ['commandId', 'expectedRevision', 'serverActor', 'action', 'trustedTimeMs'])
      || !nonEmpty(command.commandId) || commandIds.has(command.commandId)
      || command.expectedRevision !== index || !safeNonNegativeInteger(command.trustedTimeMs)
      || command.trustedTimeMs < previousTime
      || !isJsonObject(command.serverActor) || !hasExactKeys(command.serverActor, ['type', 'playerId'])
      || !nonEmpty(command.serverActor.playerId)
      || !['organizer', 'player'].includes(command.serverActor.type as string)
      || (command.serverActor.type === 'organizer' && command.serverActor.playerId !== organizerUserId)
      || !isJsonObject(command.action) || typeof command.action.type !== 'string'
      || !Object.hasOwn(actionKeys, command.action.type)
      || !hasExactKeys(command.action, actionKeys[command.action.type]!)) return null
    commandIds.add(command.commandId)
    previousTime = command.trustedTimeMs
  }
  // 每筆唯一成功命令恰好推進一次；動作語意仍由 Rust 在執行時重播驗證。
  return value.commands.length
}

function isLockedRegistrationCheckpoint(value: unknown, organizerUserId: string): boolean {
  if (registrationCheckpointRevision(value, organizerUserId) === null
    || !isJsonObject(value) || !Array.isArray(value.commands) || value.commands.length === 0) return false
  const last = value.commands[value.commands.length - 1]
  return isJsonObject(last) && isJsonObject(last.action) && last.action.type === 'lock'
}

function registrationFreezeDeadline(value: unknown, organizerUserId: string): number | null {
  if (registrationCheckpointRevision(value, organizerUserId) === null || !isJsonObject(value)
    || !isJsonObject(value.announcement) || !Array.isArray(value.commands)) return null
  // 報名生命週期由已接受的 registration 命令重播，不另存第二份狀態。
  let opened = false
  let locked = false
  for (const command of value.commands) {
    if (!isJsonObject(command) || !isJsonObject(command.action)) return null
    if (command.action.type === 'open') opened = true
    if (command.action.type === 'lock') locked = true
  }
  const deadline = value.announcement.registrationDeadline
  return opened && !locked && safeNonNegativeInteger(deadline) ? deadline : null
}

function parseLockedRoster(
  value: unknown,
  organizerUserId: string,
): { teamIds: string[], announcement: JsonObject } | null {
  if (!isJsonObject(value) || !hasExactKeys(value, ['announcement', 'teams'])
    || !isRegistrationAnnouncement(value.announcement, organizerUserId) || !Array.isArray(value.teams)) return null
  const announcement = value.announcement
  const teamIds: string[] = []
  const allMembers = new Set<string>()
  for (const team of value.teams) {
    if (!isJsonObject(team) || !hasExactKeys(team, ['teamId', 'captain', 'members', 'rosterVersion', 'decks'])
      || !nonEmpty(team.teamId) || !nonEmpty(team.captain)
      || !isUniqueNonEmptyStringArray(team.members)
      || team.members.length !== announcement.teamSize
      || !(team.members as string[]).includes(team.captain)
      || !Number.isSafeInteger(team.rosterVersion) || (team.rosterVersion as number) <= 0
      || !Array.isArray(team.decks)) return null
    for (const member of team.members) {
      if (allMembers.has(member)) return null
      allMembers.add(member)
    }
    for (const deck of team.decks) {
      if (!isJsonObject(deck) || !hasExactKeys(deck, ['player', 'name', 'cards'])
        || !nonEmpty(deck.player) || !nonEmpty(deck.name) || !isStringArray(deck.cards)) return null
    }
    const members = team.members as string[]
    const decks = team.decks as JsonObject[]
    if (announcement.personalDecks === 'disabled' && decks.length !== 0) return null
    if (announcement.personalDecks === 'required') {
      const deckPlayers = decks.map(deck => deck.player as string)
      if (!isUniqueNonEmptyStringArray(deckPlayers) || !sameTeamSet(deckPlayers, members)) return null
    }
    teamIds.push(team.teamId)
  }
  return isUniqueNonEmptyStringArray(teamIds) ? { teamIds, announcement } : null
}

function isTournamentForCompetition(value: JsonObject, competitionId: string): boolean {
  return hasExactKeys(value, ['tournamentId', 'stages'])
    && value.tournamentId === competitionId
    && Array.isArray(value.stages)
    && value.stages.length > 0
    && value.stages.every(isJsonValue)
}

function validCreatedTournament(
  tournament: JsonObject,
  requestedStage: StageConfig,
  tournamentId: string,
): boolean {
  if (!hasExactKeys(tournament, ['tournamentId', 'stages'])
    || tournament.tournamentId !== tournamentId || !Array.isArray(tournament.stages)
    || tournament.stages.length !== 1) return false
  const stage = tournament.stages[0]
  if (!isJsonObject(stage) || !hasExactKeys(stage, [
    'config', 'status', 'rounds', 'standings', 'corrections', 'lastScheduleError',
    'scheduleGeneration', 'supersededRounds', 'teams', 'doubleState',
  ]) || stage.status !== 'active' || !Array.isArray(stage.rounds)
    || !Array.isArray(stage.standings) || !Array.isArray(stage.corrections)
    || stage.lastScheduleError !== null || !Number.isSafeInteger(stage.scheduleGeneration)
    || stage.scheduleGeneration !== 0 || !Array.isArray(stage.supersededRounds)
    || !isJsonObject(stage.teams)
    || !sameJson(stage.config, requestedStage)) return false
  const doubleElimination = requestedStage.format === 'doubleElimination'
  if (doubleElimination
    ? !validInitialDoubleState(stage.doubleState, requestedStage.teams
      .slice().sort((left, right) => left.seed - right.seed).map(team => team.teamId))
    : stage.doubleState !== null) return false
  const configured = requestedStage.teams
  const expectedTeamIds = configured.map(team => team.teamId)
  const initialRounds = stage.rounds as JsonValue[]
  if (!hasExactKeys(stage.teams, configured.map(team => team.teamId))) return false
  const teamEntries = stage.teams as JsonObject
  const entriesValid = configured.every(team => {
    const entry = teamEntries[team.teamId]
    return isJsonObject(entry) && hasExactKeys(entry, ['seed', 'dropped', 'hadBye'])
      && entry.seed === team.seed && entry.dropped === false && typeof entry.hadBye === 'boolean'
  })
  const seenMatchIds = new Set<string>()
  return entriesValid
    && initialRounds.length > 0
    && validInitialRoundSequence(initialRounds, requestedStage, expectedTeamIds, seenMatchIds)
    && configured.every(team => {
      const entry = teamEntries[team.teamId] as JsonObject
      return entry.hadBye === initialRounds.some(round => isJsonObject(round)
        && Array.isArray(round.matches) && round.matches.some(match => isJsonObject(match)
          && isJsonObject(match.result) && match.result.type === 'bye'
          && match.result.teamId === team.teamId))
    })
    && validInitialStandings(stage.standings, requestedStage, expectedTeamIds, initialRounds)
    && stage.supersededRounds.length === 0
    && stage.corrections.length === 0
    && !containsPrivateRosterFields(tournament)
}

function validInitialStandings(
  value: JsonValue[],
  config: StageConfig,
  teamIds: string[],
  rounds: JsonValue[],
): boolean {
  if (value.length !== teamIds.length) return false
  const byeTeams = new Set<string>()
  for (const round of rounds) {
    if (!isJsonObject(round) || !Array.isArray(round.matches)) return false
    for (const match of round.matches) {
      if (isJsonObject(match) && isJsonObject(match.result)
        && match.result.type === 'bye' && nonEmpty(match.result.teamId)) byeTeams.add(match.result.teamId)
    }
  }
  const seen = new Set<string>()
  const rowsValid = value.every(row => isJsonObject(row)
    && hasExactKeys(row, [
      'teamId', 'matchPoints', 'wins', 'draws', 'losses', 'gameWins', 'gameLosses',
      'netGameWins', 'opponentWinRateBasisPoints', 'dropped', 'rank',
    ])
    && nonEmpty(row.teamId) && teamIds.includes(row.teamId) && !seen.has(row.teamId)
    && (seen.add(row.teamId), true)
    && ['wins', 'draws', 'losses', 'gameWins', 'gameLosses']
      .every(key => Number.isSafeInteger(row[key]) && (row[key] as number) >= 0)
    && row.wins === 0 && row.draws === 0 && row.losses === 0
    && row.gameWins === 0 && row.gameLosses === 0 && row.netGameWins === 0
    && row.dropped === false
    && isU64DecimalString(row.matchPoints)
    && row.matchPoints === String(config.format === 'swiss' && byeTeams.has(row.teamId)
      ? config.swissPolicy?.matchPoints.bye ?? 0 : 0)
    && Number.isSafeInteger(row.netGameWins)
    && row.opponentWinRateBasisPoints === null
    && typeof row.dropped === 'boolean'
    && (row.rank === null || (Number.isSafeInteger(row.rank) && (row.rank as number) > 0)))
    && seen.size === teamIds.length
  if (!rowsValid) return false
  if (config.format !== 'swiss') return value.every(row => isJsonObject(row) && row.rank === 1)
  const byePoints = config.swissPolicy?.matchPoints.bye ?? 0
  const useSeedOrder = config.swissPolicy?.tieBreakers.includes('seedOrder') ?? false
  const matchPointsFor = (teamId: string): number => byeTeams.has(teamId) ? byePoints : 0
  const ordered = [...config.teams].sort((left, right) => {
    const points = matchPointsFor(right.teamId) - matchPointsFor(left.teamId)
    return points || (useSeedOrder ? left.seed - right.seed : 0)
  })
  let previousKey = ''
  let expectedRank = 0
  const ranks = new Map<string, number>()
  ordered.forEach((team, index) => {
    const key = `${matchPointsFor(team.teamId)}:${useSeedOrder ? team.seed : ''}`
    if (key !== previousKey) expectedRank = index + 1
    ranks.set(team.teamId, expectedRank)
    previousKey = key
  })
  return value.every(row => isJsonObject(row) && ranks.get(row.teamId as string) === row.rank)
}

function validInitialRoundSequence(
  rounds: JsonValue[],
  config: StageConfig,
  teamIds: string[],
  seenMatchIds: Set<string>,
): boolean {
  const expectedCount = config.format === 'roundRobin'
    ? (teamIds.length % 2 === 0 ? teamIds.length - 1 : teamIds.length)
    : 1
  if (rounds.length !== expectedCount || !rounds.every((round, index) => validInitialRound(
      round, config, teamIds, seenMatchIds, index + 1,
    ))) return false
  const byeTeams: string[] = []
  const opponentPairs = new Set<string>()
  for (const round of rounds) {
    if (!isJsonObject(round) || !Array.isArray(round.matches)) return false
    for (const match of round.matches) {
      if (!isJsonObject(match) || !Array.isArray(match.participants)) return false
      if (match.participants.length === 1) byeTeams.push(match.participants[0] as string)
      if (match.participants.length === 2) {
        const participants = match.participants as string[]
        const pair = [...participants].sort().join('\u0000')
        if (opponentPairs.has(pair)) return false
        opponentPairs.add(pair)
      }
    }
  }
  const expectedByes = config.format === 'singleElimination' || config.format === 'doubleElimination'
    ? 2 ** Math.ceil(Math.log2(teamIds.length)) - teamIds.length
    : config.format === 'roundRobin'
      ? (teamIds.length % 2 === 1 ? teamIds.length : 0)
      : teamIds.length % 2
  if (byeTeams.length !== expectedByes) return false
  if (config.format === 'roundRobin'
    && (opponentPairs.size !== teamIds.length * (teamIds.length - 1) / 2
      || (teamIds.length % 2 === 1 && new Set(byeTeams).size !== teamIds.length))) return false
  return true
}

function validInitialRound(
  value: JsonValue,
  config: StageConfig,
  teamIds: string[],
  seenMatchIds: Set<string>,
  expectedRoundNumber: number,
): boolean {
  if (!isJsonObject(value) || !hasExactKeys(value, ['roundNumber', 'kind', 'matches'])
    || value.roundNumber !== expectedRoundNumber
    || value.kind !== (config.format === 'doubleElimination' ? 'winners' : 'main')
    || !Array.isArray(value.matches) || value.matches.length === 0) return false
  const seenTeams = new Set<string>()
  const bracketSize = 2 ** Math.ceil(Math.log2(teamIds.length))
  const expectedMatches = config.format === 'singleElimination' || config.format === 'doubleElimination'
    ? bracketSize / 2 : Math.ceil(teamIds.length / 2)
  if (value.matches.length !== expectedMatches) return false
  const matchesValid = value.matches.every((raw, index) => isJsonObject(raw)
    && hasExactKeys(raw, [
      'matchId', 'stageId', 'roundNumber', 'sequence', 'kind', 'participants', 'status',
      'result', 'cancellationReason', 'started',
    ])
    && nonEmpty(raw.matchId) && !seenMatchIds.has(raw.matchId)
    && (seenMatchIds.add(raw.matchId), true)
    && raw.stageId === config.stageId && raw.roundNumber === value.roundNumber
    && raw.sequence === index + 1
    && raw.matchId === `${config.stageId}-g0-r${expectedRoundNumber}-m${index + 1}`
    && raw.kind === value.kind && Array.isArray(raw.participants)
    && raw.participants.length >= 1 && raw.participants.length <= 2
    && new Set(raw.participants).size === raw.participants.length
    && raw.participants.every(team => nonEmpty(team) && teamIds.includes(team)
      && !seenTeams.has(team) && (seenTeams.add(team), true))
    && raw.cancellationReason === null && raw.started === false
    && (raw.participants.length === 2
      ? raw.status === 'pending' && raw.result === null
      : raw.status === 'completed' && isJsonObject(raw.result)
        && validInitialMatchResult(raw.result, teamIds)
        && validResultParticipants(raw.result, raw.participants as string[])))
  return matchesValid && seenTeams.size === teamIds.length
}

function validResultParticipants(result: JsonObject, participants: string[]): boolean {
  if (result.type === 'bye') return participants.length === 1 && result.teamId === participants[0]
  if (result.type === 'win' || result.type === 'forfeit') {
    return participants.length === 2 && participants.includes(result.winnerId as string)
      && participants.includes(result.loserId as string)
  }
  if (result.type === 'draw') return participants.length === 2
  return result.type === 'doubleLoss' && participants.length === 2
}

function validInitialMatchResult(value: JsonValue, teamIds: string[]): boolean {
  return isJsonObject(value) && value.type === 'bye'
    && hasExactKeys(value, ['type', 'teamId'])
    && nonEmpty(value.teamId) && teamIds.includes(value.teamId)
}

function validInitialDoubleState(value: unknown, teamIds: string[]): boolean {
  return isJsonObject(value)
    && hasExactKeys(value, [
      'phase', 'winnersPool', 'losersWaiting', 'lowerPool', 'winnersChampion',
      'losersChampion', 'grandFinalWinner',
    ])
    && value.phase === 'winners'
    && isUniqueNonEmptyStringArray(value.winnersPool)
    && value.winnersPool.length === teamIds.length
    && value.winnersPool.every((teamId, index) => teamId === teamIds[index])
    && Array.isArray(value.losersWaiting) && value.losersWaiting.length === 0
    && Array.isArray(value.lowerPool) && value.lowerPool.length === 0
    && value.winnersChampion === null && value.losersChampion === null
    && value.grandFinalWinner === null
}

function containsPrivateRosterFields(value: JsonValue): boolean {
  if (Array.isArray(value)) return value.some(containsPrivateRosterFields)
  if (!isJsonObject(value)) return false
  if (Object.keys(value).some(key => [
    'announcement', 'captain', 'members', 'decks', 'rosterVersion', 'lockedRoster',
  ].includes(key))) return true
  return Object.values(value).some(containsPrivateRosterFields)
}

function validateRefereeRequest(
  request: CompetitionRoomRefereeRequest,
): { code: string; message: string } | null {
  if (!nonEmpty(request.competitionId)
    || !nonEmpty(request.callerUserId)
    || !nonEmpty(request.targetUserId)
    || !nonEmpty(request.commandId)) {
    return { code: 'invalidRequest', message: 'competition id, caller, target and command id are required' }
  }
  if (!Number.isInteger(request.expectedRevision) || request.expectedRevision < 1) {
    return { code: 'invalidRequest', message: 'expected revision must be a positive integer' }
  }
  if (!Number.isFinite(request.trustedTimeMs)) {
    return { code: 'invalidRequest', message: 'trusted time must be finite' }
  }
  return null
}

function nonEmpty(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every(entry => typeof entry === 'string')
}

function isUniqueNonEmptyStringArray(value: unknown): value is string[] {
  return Array.isArray(value)
    && value.every(nonEmpty)
    && new Set(value).size === value.length
}

function deriveScheduledMatch(
  tournament: JsonObject | null,
  stageId: string,
  matchId: string,
  allowCompleted = false,
  allowFinishedStage = false,
  expectedStatus: 'pending' | 'inProgress' = 'inProgress',
): ScheduledMatchMaterialization {
  if (!isJsonObject(tournament) || !hasExactKeys(tournament, ['tournamentId', 'stages'])
    || typeof tournament.tournamentId !== 'string'
    || !Array.isArray(tournament.stages) || tournament.stages.length === 0) {
    throw new Error('tournament checkpoint is malformed')
  }

  const stageIds = new Set<string>()
  const matchIds = new Set<string>()
  let selectedStage: JsonObject | null = null
  let selectedMatch: JsonObject | null = null

  for (const rawStage of tournament.stages) {
    if (!isJsonObject(rawStage) || !hasExactKeys(rawStage, [
      'config', 'status', 'rounds', 'standings', 'corrections', 'lastScheduleError',
      'scheduleGeneration', 'supersededRounds', 'teams', 'doubleState',
    ]) || !isJsonObject(rawStage.config) || !hasExactKeys(rawStage.config, [
      'stageId', 'format', 'teams', 'randomSeed', 'matchFormat', 'clockConfig', 'swissRounds', 'swissPolicy',
    ])
      || typeof rawStage.config.stageId !== 'string'
      || !nonEmpty(rawStage.config.stageId)
      || !('status' in rawStage)
      || !Array.isArray(rawStage.config.teams)
      || !safeNonNegativeInteger(rawStage.config.randomSeed)
      || !('format' in rawStage.config)
      || !('matchFormat' in rawStage.config)
      || !('clockConfig' in rawStage.config)
      || !('swissRounds' in rawStage.config)
      || !('swissPolicy' in rawStage.config)
      || !Array.isArray(rawStage.rounds)) {
      throw new Error('tournament stage checkpoint is malformed')
    }
    const currentStageId = rawStage.config.stageId
    if (!stageIds.add(currentStageId)) throw new Error('duplicate tournament stage')
    if (!isValidTournamentFormat(rawStage.config.format)
      || !isValidStageStatus(rawStage.status)
      || !validStageTeams(rawStage.config.teams)
      || !isValidMatchFormat(rawStage.config.matchFormat)
      || !isValidClockConfig(rawStage.config.clockConfig)
      || !isCompatibleStageMatchFormat(rawStage.config.format, rawStage.config.matchFormat)) {
      throw new Error('tournament stage config is malformed')
    }
    if (currentStageId === stageId) selectedStage = rawStage

    for (const rawRound of rawStage.rounds) {
      if (!isJsonObject(rawRound) || !hasExactKeys(rawRound, ['roundNumber', 'kind', 'matches'])
        || !safeNonNegativeInteger(rawRound.roundNumber)
        || !isValidRoundKind(rawRound.kind) || !Array.isArray(rawRound.matches)
        || !isRoundKindAllowedForFormat(rawStage.config.format, rawRound.kind)) {
        throw new Error('tournament round checkpoint is malformed')
      }
      for (const rawMatch of rawRound.matches) {
        if (!isJsonObject(rawMatch) || !hasExactScheduledMatchKeys(rawMatch)
          || !isValidScheduledMatchPurpose(rawMatch, rawStage.config.format, rawRound.kind)
          || typeof rawMatch.matchId !== 'string'
          || !nonEmpty(rawMatch.matchId) || rawMatch.stageId !== currentStageId
          || !safeNonNegativeInteger(rawMatch.roundNumber) || rawMatch.roundNumber !== rawRound.roundNumber
          || !safeNonNegativeInteger(rawMatch.sequence) || !isValidRoundKind(rawMatch.kind)
          || rawMatch.kind !== rawRound.kind || !Array.isArray(rawMatch.participants)
          || !isValidMatchStatus(rawMatch.status) || !('result' in rawMatch)
          || !('cancellationReason' in rawMatch) || typeof rawMatch.started !== 'boolean') {
          throw new Error('scheduled match checkpoint is malformed')
        }
        if ((rawMatch.status === 'pending' || rawMatch.status === 'inProgress')
          && (rawMatch.result !== null || rawMatch.cancellationReason !== null)) {
          throw new Error('unfinished scheduled match cannot contain a result or cancellation')
        }
        if (!matchIds.add(rawMatch.matchId)) throw new Error('duplicate tournament match')
        if (currentStageId === stageId && rawMatch.matchId === matchId) selectedMatch = rawMatch
      }
    }
    if (!isValidBronzeSchedule(rawStage.rounds, rawStage.config.format)) {
      throw new Error('bronze schedule is malformed')
    }
  }

  if (!selectedStage) throw new Error('stage was not found')
  if (!selectedMatch) throw new Error('match was not found')
  if (selectedMatch.status !== expectedStatus
    && !(allowCompleted && selectedMatch.status === 'completed')) {
    throw new Error('scheduled match is not in progress')
  }
  if (selectedMatch.started !== (expectedStatus === 'inProgress')) {
    throw new Error('scheduled match started flag is inconsistent')
  }
  if (selectedStage.status !== 'active' && !(allowFinishedStage && selectedStage.status === 'finished')) {
    throw new Error('in-progress match cannot belong to a finished stage')
  }
  const selectedConfig = selectedStage.config
  const participants = selectedMatch.participants
  const participantA = Array.isArray(participants) ? participants[0] : undefined
  const participantB = Array.isArray(participants) ? participants[1] : undefined
  if (!isJsonObject(selectedConfig) || !Array.isArray(selectedConfig.teams)) {
    throw new Error('selected stage config is malformed')
  }
  if (!('matchFormat' in selectedConfig) || !('clockConfig' in selectedConfig)
    || !isValidMatchFormat(selectedConfig.matchFormat)
    || !isValidClockConfig(selectedConfig.clockConfig)) {
    throw new Error('selected stage config is malformed')
  }
  if (!Array.isArray(participants) || participants.length !== 2
    || participants.some(team => !nonEmpty(team)) || participantA === participantB) {
    throw new Error('scheduled match must contain two different teams')
  }
  const configuredTeams = (selectedConfig.teams as JsonValue[])
    .map(team => (isJsonObject(team) && typeof team.teamId === 'string' ? team.teamId : null))
  if (!participants.every(team => typeof team === 'string' && configuredTeams.includes(team))) {
    throw new Error('scheduled match contains an unknown team')
  }

  return {
    matchId,
    stageId,
    teams: [participantA as string, participantB as string],
    format: selectedConfig.matchFormat,
    clockConfig: selectedConfig.clockConfig,
    status: selectedMatch.status as ScheduledMatchMaterialization['status'],
    started: selectedMatch.started as boolean,
    result: selectedMatch.result as JsonValue,
  }
}

function validateStartedTournamentResponse(
  response: CompetitionEngineResponse,
  previousState: CompetitionServerState,
  stageId: string,
  matchId: string,
): JsonObject | null {
  const rawResponse = response as unknown as JsonObject
  if (!hasExactKeys(rawResponse, ['ok', 'operation', 'public', 'serverState'])
    || rawResponse.ok !== true || rawResponse.operation !== 'tournamentStartMatch'
    || !isJsonObject(rawResponse.public)
    || !hasExactKeys(rawResponse.public, ['result', 'tournament'])
    || !isJsonObject(rawResponse.public.result)
    || !hasExactKeys(rawResponse.public.result, ['type'])
    || rawResponse.public.result.type !== 'started'
    || !isJsonObject(rawResponse.serverState)
    || !hasExactKeys(rawResponse.serverState, ['registration', 'clock', 'tournament', 'matches'])
    || !isJsonObject(rawResponse.serverState.tournament)
    || !isJsonObject(previousState.tournament)) {
    return null
  }

  const expected = JSON.parse(JSON.stringify(previousState.tournament)) as unknown
  if (!isJsonObject(expected) || !Array.isArray(expected.stages)) return null
  let found = 0
  for (const stage of expected.stages) {
    if (!isJsonObject(stage) || !Array.isArray(stage.rounds)) return null
    for (const round of stage.rounds) {
      if (!isJsonObject(round) || !Array.isArray(round.matches)) return null
      for (const match of round.matches) {
        if (!isJsonObject(match) || match.matchId !== matchId || match.stageId !== stageId) continue
        found += 1
        if (match.status !== 'pending' || match.started !== false) return null
        match.status = 'inProgress'
        match.started = true
      }
    }
  }
  if (found !== 1
    || !sameJson(rawResponse.serverState.tournament, expected)
    || !sameJson(rawResponse.public.tournament, expected)
    || !sameJson(rawResponse.serverState.registration, previousState.registration)
    || !sameJson(rawResponse.serverState.clock, previousState.clock)
    || !sameJson(rawResponse.serverState.matches, previousState.matches ?? {})) {
    return null
  }
  return expected
}

function findScheduledMatch(
  tournament: JsonObject | null,
  matchId: string,
  allowCompleted = false,
  allowFinishedStage = false,
  expectedStatus: 'pending' | 'inProgress' = 'inProgress',
): ScheduledMatchMaterialization | null {
  if (!isJsonObject(tournament) || !Array.isArray(tournament.stages)) {
    throw new Error('tournament checkpoint is malformed')
  }
  let locatedStageId: string | null = null
  for (const stage of tournament.stages) {
    if (!isJsonObject(stage) || !isJsonObject(stage.config)
      || typeof stage.config.stageId !== 'string' || !Array.isArray(stage.rounds)) {
      throw new Error('tournament stage checkpoint is malformed')
    }
    for (const round of stage.rounds) {
      if (!isJsonObject(round) || !Array.isArray(round.matches)) {
        throw new Error('tournament round checkpoint is malformed')
      }
      for (const scheduled of round.matches) {
        if (!isJsonObject(scheduled) || typeof scheduled.matchId !== 'string') {
          throw new Error('scheduled match checkpoint is malformed')
        }
        if (scheduled.matchId === matchId) {
          if (locatedStageId !== null) throw new Error('scheduled match id is not unique')
          locatedStageId = stage.config.stageId
        }
      }
    }
  }
  if (locatedStageId === null) return null
  return deriveScheduledMatch(tournament, locatedStageId, matchId, allowCompleted, allowFinishedStage, expectedStatus)
}

type TournamentMatchResult =
  | { type: 'noShow'; absentTeams: string[]; winnerId: string | null }
  | { type: 'win'; winnerId: string; loserId: string; winnerGames: number; loserGames: number }
  | { type: 'draw'; firstGames: number; secondGames: number }

type CompletedMatchMapping =
  | { status: 'inProgress' }
  | { status: 'pendingReferee'; reason: string }
  | { status: 'completed'; result: TournamentMatchResult }

type CompetitionTerminalSubmission = Pick<
  CompetitionRoomTerminalIngestRequest,
  'submissionId' | 'gameRecordId' | 'rulingRecordId' | 'startKind' | 'firstPlayer' | 'terminalEvidence'
> & { ruling: CompetitionRuling }

function mapCompletedMatchResult(
  rawMatch: JsonObject,
  rawConfig: JsonValue,
  format: MatchFormat,
  checkpoint: JsonObject,
  expectedSubmission: CompetitionTerminalSubmission,
): { ok: true; value: CompletedMatchMapping } | { ok: false; code: string } {
  const requiredKeys = [
    'config', 'status', 'score', 'games', 'rulings', 'corrections', 'outcome',
    'nextFirstPlayer', 'nextGame',
  ]
  if (!hasExactKeys(rawMatch, requiredKeys)
    || !sameJson(rawMatch.config, rawConfig)
    || !['awaitingFirstPlayer', 'open', 'awaitingReferee', 'completed'].includes(String(rawMatch.status))
    || !Array.isArray(rawMatch.score) || rawMatch.score.length !== 2
    || !rawMatch.score.every(safeNonNegativeInteger)
    || !Array.isArray(rawMatch.games) || !Array.isArray(rawMatch.rulings)
    || !Array.isArray(rawMatch.corrections)
    || !matchCheckpointMatchesPublic(rawMatch, checkpoint, expectedSubmission)) {
    return { ok: false, code: 'engineInvalidMatchResult' }
  }

  if (rawMatch.status === 'awaitingReferee') {
    const outcome = rawMatch.outcome
    if (!isJsonObject(outcome) || (outcome.type !== 'doubleLoss' && outcome.type !== 'pendingReferee')) {
      return { ok: false, code: 'engineInvalidMatchResult' }
    }
    const teams = isJsonObject(rawMatch.config) && Array.isArray(rawMatch.config.teams)
      ? rawMatch.config.teams as string[] : []
    const reason = outcome.type === 'doubleLoss'
      ? hasExactKeys(outcome, ['type', 'teams', 'progression'])
        && isTeamPair(outcome.teams) && outcome.teams.every(team => teams.includes(team))
        && isJsonObject(outcome.progression)
        && hasExactKeys(outcome.progression, ['type', 'reason'])
        && outcome.progression.type === 'pendingReferee'
        ? outcome.progression.reason
        : null
      : hasExactKeys(outcome, ['type', 'gameRecordId', 'reason'])
        && nonEmpty(outcome.gameRecordId)
        && rawMatch.games.some(game => isJsonObject(game) && game.gameRecordId === outcome.gameRecordId)
        ? String(outcome.reason)
        : null
    if (typeof reason !== 'string' || !nonEmpty(reason)
      || rawMatch.nextGame !== null || rawMatch.nextFirstPlayer !== null) {
      return { ok: false, code: 'engineInvalidMatchResult' }
    }
    return { ok: true, value: { status: 'pendingReferee', reason } }
  }

  if (rawMatch.status !== 'completed') {
    if (rawMatch.outcome !== null || !isValidOpenMatchContinuation(rawMatch)) {
      return { ok: false, code: 'engineInvalidMatchResult' }
    }
    return { ok: true, value: { status: 'inProgress' } }
  }
  if (rawMatch.nextGame !== null || rawMatch.nextFirstPlayer !== null
    || rawMatch.outcome === null || rawMatch.games.length === 0) {
    return { ok: false, code: 'engineInvalidMatchResult' }
  }

  const teams = isJsonObject(rawConfig) && Array.isArray(rawConfig.teams)
    ? rawConfig.teams
    : null
  if (!teams || teams.length !== 2 || !teams.every(nonEmpty) || teams[0] === teams[1]) {
    return { ok: false, code: 'engineInvalidMatchResult' }
  }
  const rulingMap = new Map<string, JsonObject>()
  for (const rawRuling of rawMatch.rulings) {
    if (!isJsonObject(rawRuling) || !hasExactKeys(rawRuling, ['rulingRecordId', 'gameRecordId', 'ruling'])
      || !nonEmpty(rawRuling.rulingRecordId) || !nonEmpty(rawRuling.gameRecordId)
      || !isJsonObject(rawRuling.ruling) || rulingMap.has(rawRuling.rulingRecordId)) {
      return { ok: false, code: 'engineInvalidMatchResult' }
    }
    rulingMap.set(rawRuling.rulingRecordId, rawRuling)
  }

  const scores = new Map<string, number>([[teams[0] as string, 0], [teams[1] as string, 0]])
  const gameIds = new Set<string>()
  for (let index = 0; index < rawMatch.games.length; index += 1) {
    const game = rawMatch.games[index]
    if (!isJsonObject(game)
      || !hasExactKeys(game, ['gameNumber', 'gameRecordId', 'startKind', 'firstPlayer', 'terminalEvidence', 'rulingRecordId'])
      || game.gameNumber !== index + 1 || !nonEmpty(game.gameRecordId)
      || !nonEmpty(game.rulingRecordId) || !nonEmpty(game.firstPlayer)
      || !teams.includes(game.firstPlayer)
      || !isJsonValue(game.startKind) || !isJsonObject(game.terminalEvidence)
      || gameIds.has(game.gameRecordId)) {
      return { ok: false, code: 'engineInvalidMatchResult' }
    }
    gameIds.add(game.gameRecordId)
    const rulingRecord = rulingMap.get(game.rulingRecordId as string)
    if (!rulingRecord || rulingRecord.gameRecordId !== game.gameRecordId) {
      return { ok: false, code: 'engineInvalidMatchResult' }
    }
    const ruling = rulingRecord.ruling as JsonObject
    const winner = winnerFromRuling(ruling, teams as [string, string])
    if (winner === 'pending' || winner === 'doubleLoss') {
      return { ok: false, code: 'engineInvalidMatchResult' }
    }
    if (winner !== 'draw') scores.set(winner, (scores.get(winner) ?? 0) + 1)
  }
  if (rulingMap.size < rawMatch.games.length) return { ok: false, code: 'engineInvalidMatchResult' }
  const firstScore = scores.get(teams[0] as string) ?? 0
  const secondScore = scores.get(teams[1] as string) ?? 0
  if (rawMatch.score[0] !== firstScore || rawMatch.score[1] !== secondScore) {
    return { ok: false, code: 'engineInvalidMatchResult' }
  }

  const outcome = rawMatch.outcome
  if (!isJsonObject(outcome) || !Array.isArray(outcome.score)
    || !sameJson(outcome.score, rawMatch.score)) {
    return { ok: false, code: 'engineInvalidMatchResult' }
  }
  if (outcome.type === 'winner' && hasExactKeys(outcome, ['type', 'winnerId', 'loserId', 'score'])
    && nonEmpty(outcome.winnerId) && nonEmpty(outcome.loserId)
    && teams.includes(outcome.winnerId) && teams.includes(outcome.loserId)
    && outcome.winnerId !== outcome.loserId) {
    const winnerScore = scores.get(outcome.winnerId) ?? 0
    const loserScore = scores.get(outcome.loserId) ?? 0
    if (winnerScore <= loserScore) return { ok: false, code: 'engineInvalidMatchResult' }
    if (format.type === 'firstToWins'
      && (winnerScore !== format.value || loserScore >= format.value)) {
      return { ok: false, code: 'engineInvalidMatchResult' }
    }
    if (format.type === 'fixedGames' && rawMatch.games.length !== format.value) {
      return { ok: false, code: 'engineInvalidMatchResult' }
    }
    return {
      ok: true,
      value: {
        status: 'completed',
        result: {
          type: 'win', winnerId: outcome.winnerId, loserId: outcome.loserId,
          winnerGames: winnerScore, loserGames: loserScore,
        },
      },
    }
  }
  if (outcome.type === 'draw' && hasExactKeys(outcome, ['type', 'score']) && format.type === 'fixedGames'
    && firstScore === secondScore && rawMatch.games.length === format.value) {
    return {
      ok: true,
      value: { status: 'completed', result: { type: 'draw', firstGames: firstScore, secondGames: secondScore } },
    }
  }
  return { ok: false, code: 'engineInvalidMatchResult' }
}

function isValidOpenMatchContinuation(rawMatch: JsonObject): boolean {
  if (!Array.isArray(rawMatch.games) || !isJsonObject(rawMatch.config)
    || !Array.isArray(rawMatch.config.teams) || !isTeamPair(rawMatch.config.teams)
    || !isJsonObject(rawMatch.nextGame)
    || !hasExactKeys(rawMatch.nextGame, ['gameNumber', 'kind', 'firstPlayer'])
    || rawMatch.nextGame.gameNumber !== rawMatch.games.length + 1
    || !isJsonObject(rawMatch.nextGame.kind)
    || !isJsonObject(rawMatch.nextGame.firstPlayer)) return false
  const kind = rawMatch.nextGame.kind
  if (!(hasExactKeys(kind, ['type']) && kind.type === 'ordinaryNewGame')
    && !(hasExactKeys(kind, ['type', 'continuation'])
      && kind.type === 'eliminationDrawContinuation'
      && isValidEliminationContinuation(kind.continuation, rawMatch.config.teams as [string, string]))) return false
  const intent = rawMatch.nextGame.firstPlayer
  if (hasExactKeys(intent, ['type', 'reason']) && intent.type === 'pending'
    && nonEmpty(intent.reason)) return rawMatch.nextFirstPlayer === null
  return hasExactKeys(intent, ['type', 'team']) && intent.type === 'assigned'
    && nonEmpty(intent.team) && rawMatch.config.teams.includes(intent.team)
    && rawMatch.nextFirstPlayer === intent.team
}

function winnerFromRuling(
  ruling: JsonObject,
  teams: [string, string],
): string | 'draw' | 'pending' | 'doubleLoss' {
  switch (ruling.type) {
    case 'gameAccepted':
      if (!hasExactKeys(ruling, ['type', 'outcome'])) return 'pending'
      if (ruling.outcome === 'draw') return 'draw'
      if (isJsonObject(ruling.outcome) && hasExactKeys(ruling.outcome, ['winner'])
        && teams.includes(ruling.outcome.winner as string)) return ruling.outcome.winner as string
      return 'pending'
    case 'timeoutLoss':
      if (!hasExactKeys(ruling, ['type', 'loser'])) return 'pending'
      return nonEmpty(ruling.loser) && teams.includes(ruling.loser)
        ? teams[0] === ruling.loser ? teams[1] : teams[0]
        : 'pending'
    case 'eliminationDrawLoss':
      if (!hasExactKeys(ruling, ['type', 'loser', 'reason'])
        || ruling.reason !== 'enteredByoyomiFirst') return 'pending'
      return nonEmpty(ruling.loser) && teams.includes(ruling.loser)
        ? teams[0] === ruling.loser ? teams[1] : teams[0]
        : 'pending'
    case 'doubleTimeoutLoss':
      return hasExactKeys(ruling, ['type', 'teams']) && isTeamPair(ruling.teams)
        && ruling.teams.every(team => teams.includes(team)) ? 'doubleLoss' : 'pending'
    case 'needsDecision':
      if (!hasExactKeys(ruling, ['type', 'reason']) || ![
        'zeroTimeControl', 'simultaneousByoyomiEntry', 'unlimitedEliminationDraw', 'zeroTimeEliminationDraw',
      ].includes(String(ruling.reason))) return 'pending'
      return 'pending'
    case 'needsTiebreak':
      return hasExactKeys(ruling, ['type', 'continuation'])
        && isJsonObject(ruling.continuation)
        && hasExactKeys(ruling.continuation, ['teams'])
        && Array.isArray(ruling.continuation.teams)
        && ruling.continuation.teams.length === 2
        && ruling.continuation.teams.every(team => isJsonObject(team)
          && hasExactKeys(team, ['team', 'mainTimeRemainingMs'])
          && nonEmpty(team.team) && teams.includes(team.team)
          && (team.mainTimeRemainingMs === null || safeNonNegativeInteger(team.mainTimeRemainingMs)))
        ? 'draw' : 'pending'
    default:
      return 'pending'
  }
}

function matchCheckpointMatchesPublic(
  publicMatch: JsonObject,
  checkpoint: JsonObject,
  expectedSubmission: CompetitionTerminalSubmission,
): boolean {
  if (!sameJson(checkpoint.config, publicMatch.config) || !Array.isArray(checkpoint.history)) return false
  const submissions: JsonObject[] = []
  const expectedCorrections: JsonObject[] = []
  const expectedRulings: JsonObject[] = []
  const activeRulings = new Map<string, { rulingRecordId: string; ruling: JsonObject }>()
  for (const event of checkpoint.history) {
    if (!isJsonObject(event)) return false
    if (event.type === 'submitGame') {
      if (!isJsonObject(event.submission)
        || !nonEmpty(event.submission.gameRecordId)
        || !nonEmpty(event.submission.rulingRecordId)
        || !isJsonObject(event.submission.ruling)
        || activeRulings.has(event.submission.gameRecordId)) return false
      const submission = event.submission
      submissions.push(submission)
      const gameRecordId = submission.gameRecordId as string
      const rulingRecordId = submission.rulingRecordId as string
      const ruling = submission.ruling as JsonObject
      activeRulings.set(gameRecordId, {
        rulingRecordId,
        ruling,
      })
      expectedRulings.push({
        rulingRecordId,
        gameRecordId,
        ruling,
      })
    } else if (event.type === 'correctRuling') {
      if (!hasExactKeys(event, [
        'type', 'correctionId', 'refereeId', 'gameRecordId', 'replacementRulingRecordId',
        'replacementRuling', 'reason',
      ]) || !nonEmpty(event.correctionId) || !nonEmpty(event.refereeId)
        || !nonEmpty(event.gameRecordId) || !nonEmpty(event.replacementRulingRecordId)
        || !isJsonObject(event.replacementRuling) || !nonEmpty(event.reason)) return false
      const active = activeRulings.get(event.gameRecordId)
      if (!active) return false
      expectedCorrections.push({
        correctionId: event.correctionId,
        refereeId: event.refereeId,
        gameRecordId: event.gameRecordId,
        previousRulingRecordId: active.rulingRecordId,
        replacementRulingRecordId: event.replacementRulingRecordId,
        replacementRuling: event.replacementRuling,
        reason: event.reason,
      })
      activeRulings.set(event.gameRecordId, {
        rulingRecordId: event.replacementRulingRecordId,
        ruling: event.replacementRuling,
      })
      expectedRulings.push({
        rulingRecordId: event.replacementRulingRecordId,
        gameRecordId: event.gameRecordId,
        ruling: event.replacementRuling,
      })
    } else if (event.type !== 'assignFirstPlayer') {
      return false
    }
  }
  if (!Array.isArray(publicMatch.games) || submissions.length !== publicMatch.games.length
    || !Array.isArray(publicMatch.rulings) || !sameJson(publicMatch.rulings, expectedRulings)
    || !Array.isArray(publicMatch.corrections) || !sameJson(publicMatch.corrections, expectedCorrections)) return false
  const expectedGames = submissions.map((submission, index) => {
    const active = activeRulings.get(submission.gameRecordId as string)
    return {
      gameNumber: index + 1,
      gameRecordId: submission.gameRecordId,
      startKind: submission.startKind,
      firstPlayer: submission.firstPlayer,
      terminalEvidence: submission.terminalEvidence,
      rulingRecordId: active?.rulingRecordId,
    }
  })
  if (!sameJson(publicMatch.games, expectedGames)) return false
  const latest = submissions.at(-1)
  const finalGame = publicMatch.games.at(-1)
  return Boolean(latest && sameJson(latest, expectedSubmission)
    && isJsonObject(finalGame)
    && finalGame.gameRecordId === expectedSubmission.gameRecordId
    && finalGame.rulingRecordId === activeRulings.get(expectedSubmission.gameRecordId)?.rulingRecordId
    && finalGame.firstPlayer === expectedSubmission.firstPlayer
    && sameJson(finalGame.startKind, expectedSubmission.startKind)
    && sameJson(finalGame.terminalEvidence, expectedSubmission.terminalEvidence))
}

function latestSubmissionFromCheckpoint(
  checkpoint: JsonObject,
): CompetitionTerminalSubmission | null {
  if (!Array.isArray(checkpoint.history)) return null
  let latest: JsonObject | null = null
  for (const event of checkpoint.history) {
    if (!isJsonObject(event)) return null
    if (event.type === 'submitGame') {
      if (!isJsonObject(event.submission)) return null
      latest = event.submission
    }
  }
  if (!latest || !hasExactKeys(latest, [
    'submissionId', 'gameRecordId', 'rulingRecordId', 'startKind', 'firstPlayer', 'terminalEvidence', 'ruling',
  ]) || !nonEmpty(latest.submissionId) || !nonEmpty(latest.gameRecordId)
    || !nonEmpty(latest.rulingRecordId) || !nonEmpty(latest.firstPlayer)
    || !isJsonObject(latest.ruling) || !isJsonObject(latest.terminalEvidence)
    || !isJsonValue(latest.startKind)) return null
  return latest as unknown as CompetitionTerminalSubmission
}

function isValidMatchCorrectionCheckpoint(
  previous: JsonObject,
  next: JsonObject,
  operation: Extract<MatchExecuteOperation, { type: 'matchCorrectRuling' }>,
): boolean {
  if (!Array.isArray(previous.history) || !Array.isArray(next.history)
    || next.history.length !== previous.history.length + 1
    || !sameJson(next.history.slice(0, -1), previous.history)) return false
  const finalEvent = next.history.at(-1)
  return isJsonObject(finalEvent)
    && hasExactKeys(finalEvent, [
      'type', 'correctionId', 'refereeId', 'gameRecordId', 'replacementRulingRecordId',
      'replacementRuling', 'reason',
    ])
    && sameJson(finalEvent, {
      type: 'correctRuling',
      correctionId: operation.correctionId,
      refereeId: operation.refereeId,
      gameRecordId: operation.gameRecordId,
      replacementRulingRecordId: operation.replacementRulingRecordId,
      replacementRuling: operation.replacementRuling,
      reason: operation.reason,
    })
}

function validateTournamentCorrectionUpdate(
  response: Extract<CompetitionEngineResponse, { ok: true }>,
  previousTournament: JsonObject,
  previousSchedule: ScheduledMatchMaterialization,
  expectedResult: TournamentMatchResult,
  operation: 'tournamentCorrectResult' | 'tournamentSubmitResult',
): string | null {
  if (!hasExactKeys(response as unknown as JsonObject, ['ok', 'operation', 'public', 'serverState'])
    || !isJsonObject(response.public) || !hasExactKeys(response.public, ['result', 'tournament'])
    || !isJsonObject(response.serverState)
    || !hasExactKeys(response.serverState, ['registration', 'clock', 'tournament', 'matches'])) {
    return 'engineInvalidTournamentCheckpoint'
  }
  if (operation === 'tournamentSubmitResult') {
    return validateTournamentSubmitResponse(
      response,
      previousTournament,
      previousSchedule,
      expectedResult,
    )
  }
  if (response.operation !== 'tournamentCorrectResult') return 'engineOperationMismatch'
  if (response.serverState.registration !== null || response.serverState.clock !== null
    || !isJsonObject(response.serverState.tournament)
    || !isJsonObject(response.serverState.matches) || Object.keys(response.serverState.matches).length !== 0
    || !sameJson(response.public.tournament, response.serverState.tournament)
    || !isJsonObject(response.public.result)
    || !hasExactKeys(response.public.result, ['type'])
    || response.public.result.type !== 'corrected') {
    return 'engineInvalidTournamentCheckpoint'
  }
  if (previousSchedule.result === null) return 'scheduleMismatch'
  try {
    const after = deriveScheduledMatch(
      response.serverState.tournament,
      previousSchedule.stageId,
      previousSchedule.matchId,
      true,
      true,
    )
    if (after.status !== 'completed' || after.started !== (expectedResult.type !== 'noShow')
      || !sameJson(after.teams, previousSchedule.teams)
      || !sameJson(after.format, previousSchedule.format)
      || !sameJson(after.result, expectedResult as unknown as JsonValue)) {
      return 'engineInvalidTournamentCheckpoint'
    }
  } catch {
    return 'engineInvalidTournamentCheckpoint'
  }
  return null
}

function validateTournamentSubmitResponse(
  response: Extract<CompetitionEngineResponse, { ok: true }>,
  previousTournament: JsonObject | null,
  previousSchedule: ScheduledMatchMaterialization,
  expectedResult: TournamentMatchResult,
): string | null {
  if (response.operation !== 'tournamentSubmitResult') return 'engineOperationMismatch'
  if (response.serverState.registration !== null || response.serverState.clock !== null
    || Object.keys(response.serverState.matches ?? {}).length !== 0
    || !isJsonObject(response.serverState.tournament)
    || !isJsonObject(response.public)
    || !hasExactKeys(response.public, ['result', 'tournament'])
    || !sameJson(response.public.tournament, response.serverState.tournament)) {
    return 'engineInvalidTournamentCheckpoint'
  }
  const update = response.public.result
  if (!isJsonObject(update) || typeof update.type !== 'string'
    || !['applied', 'appliedWithScheduleError'].includes(update.type)
    || (update.type === 'applied' && !hasExactKeys(update, ['type']))
    || (update.type === 'appliedWithScheduleError'
      && (!hasExactKeys(update, ['type', 'scheduleError']) || !nonEmpty(update.scheduleError)))) {
    return 'engineInvalidTournamentResult'
  }
  if (!previousTournament || !previousSchedule.stageId) return 'invalidTournamentCheckpoint'
  try {
    const after = deriveScheduledMatch(
      response.serverState.tournament,
      previousSchedule.stageId,
      previousSchedule.matchId,
      true,
      true,
      expectedResult.type === 'noShow' ? 'pending' : 'inProgress',
    )
    if (after.status !== 'completed' || after.started !== (expectedResult.type !== 'noShow')
      || !sameJson(after.teams, previousSchedule.teams)
      || !sameJson(after.format, previousSchedule.format)
      || !sameJson(after.result as JsonValue, expectedResult as unknown as JsonValue)) {
      return 'engineInvalidTournamentCheckpoint'
    }
  } catch {
    return 'engineInvalidTournamentCheckpoint'
  }
  return null
}

/** 新時計 query 不允許 transport 或 Symbol 額外鍵混入內部語意 request／engine 回應。 */
function hasExactClockQueryKeys(value: JsonObject, keys: string[]): boolean {
  return Reflect.ownKeys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key))
}

function isU64DecimalString(value: JsonValue | undefined): value is string {
  return typeof value === 'string'
    && /^(0|[1-9][0-9]*)$/.test(value)
    && (value.length < 20 || (value.length === 20 && value <= '18446744073709551615'))
}

function hasExactKeys(value: JsonObject, keys: string[]): boolean {
  return Object.keys(value).length === keys.length && keys.every(key => key in value)
}

function isValidTournamentFormat(value: unknown): boolean {
  return value === 'singleElimination' || value === 'roundRobin'
    || value === 'doubleElimination' || value === 'swiss'
}

function isValidStageStatus(value: unknown): value is 'active' | 'finished' {
  return value === 'active' || value === 'finished'
}

function isValidMatchStatus(value: unknown): value is 'pending' | 'inProgress' | 'completed' | 'cancelled' {
  return value === 'pending' || value === 'inProgress' || value === 'completed' || value === 'cancelled'
}

function isValidRoundKind(value: unknown): value is 'main' | 'winners' | 'losers' | 'grandFinal' | 'grandFinalReset' {
  return value === 'main' || value === 'winners' || value === 'losers'
    || value === 'grandFinal' || value === 'grandFinalReset'
}

function isRoundKindAllowedForFormat(
  format: unknown,
  kind: 'main' | 'winners' | 'losers' | 'grandFinal' | 'grandFinalReset',
): boolean {
  if (format === 'doubleElimination') return kind !== 'main'
  return kind === 'main'
}

function hasExactScheduledMatchKeys(value: JsonObject): boolean {
  const keys = [
    'matchId', 'stageId', 'roundNumber', 'sequence', 'kind', 'participants', 'status',
    'result', 'cancellationReason', 'started',
  ]
  return hasExactKeys(value, Object.hasOwn(value, 'purpose') ? [...keys, 'purpose'] : keys)
}

function isValidScheduledMatchPurpose(
  value: JsonObject,
  format: unknown,
  roundKind: unknown,
): boolean {
  return !Object.hasOwn(value, 'purpose')
    || (value.purpose === 'bronze' && format === 'singleElimination' && roundKind === 'main')
}

function isValidBronzeSchedule(rounds: JsonValue[], format: unknown): boolean {
  const bronzeRounds = rounds.flatMap((round, index) => {
    if (!isJsonObject(round) || !Array.isArray(round.matches)
      || !round.matches.some(match => isJsonObject(match) && match.purpose === 'bronze')) return []
    return [{ round, index }]
  })
  if (!bronzeRounds.length) return true
  if (format !== 'singleElimination' || bronzeRounds.length !== 1) return false
  const bronzeRound = bronzeRounds[0]
  if (!bronzeRound || bronzeRound.index !== rounds.length - 1
    || !isJsonObject(bronzeRound.round) || bronzeRound.round.kind !== 'main'
    || !Array.isArray(bronzeRound.round.matches) || bronzeRound.round.matches.length !== 2) return false
  const matches = bronzeRound.round.matches
  const bronze = matches.filter(match => isJsonObject(match) && match.purpose === 'bronze')
  const championship = matches.filter(match => isJsonObject(match) && !Object.hasOwn(match, 'purpose'))
  if (bronze.length !== 1 || championship.length !== 1) return false
  const bronzeMatch = bronze[0]
  const championshipMatch = championship[0]
  if (!isJsonObject(bronzeMatch) || !isJsonObject(championshipMatch)
    || !Array.isArray(bronzeMatch.participants) || bronzeMatch.participants.length !== 2
    || !Array.isArray(championshipMatch.participants) || championshipMatch.participants.length !== 2) return false
  const bronzeParticipants = bronzeMatch.participants
  const championshipParticipants = championshipMatch.participants
  return !bronzeParticipants.some(team => championshipParticipants.includes(team))
}

function validStageTeams(value: JsonValue[]): boolean {
  const ids = value.map(team => isJsonObject(team) && typeof team.teamId === 'string' ? team.teamId : null)
  return ids.every(id => id !== null && nonEmpty(id)) && new Set(ids).size === ids.length
}

function isValidMatchFormat(value: unknown): value is MatchFormat {
  return isJsonObject(value)
    && hasExactKeys(value, ['type', 'value'])
    && (value.type === 'fixedGames' || value.type === 'firstToWins')
    && typeof value.value === 'number'
    && Number.isInteger(value.value) && value.value > 0 && value.value <= 0xffff
}

function isCompatibleStageMatchFormat(
  format: unknown,
  matchFormat: MatchFormat,
): boolean {
  const elimination = format === 'singleElimination' || format === 'doubleElimination'
  return elimination ? matchFormat.type === 'firstToWins' : matchFormat.type === 'fixedGames'
}

function isValidClockConfig(value: unknown): value is JsonObject & {
  mainTimeMs: number | null
  actionTimeMs: number | null
  outOfTurnChoiceTimeMs: number | null
} {
  if (!isJsonObject(value)
    || !('mainTimeMs' in value) || !('actionTimeMs' in value)
    || !('outOfTurnChoiceTimeMs' in value)) return false
  return [value.mainTimeMs, value.actionTimeMs, value.outOfTurnChoiceTimeMs]
    .every(time => time === null || (typeof time === 'number'
      && Number.isSafeInteger(time) && time >= 0))
}

function uniqueIds(ids: string[]): string[] {
  return [...new Set(ids.map(id => id.trim()))]
}

function isKnownExecuteOperation(operation: ExecuteOperation): operation is ExecuteOperation {
  return isJsonObject(operation) && OPERATION_NAMES.includes(operation.type as CompetitionOperationName)
}

function isMatchScopedOperation(operation: ExecuteOperation): boolean {
  return operation.type === 'matchCreate'
    || operation.type === 'matchView'
    || MATCH_MUTATION_NAMES.has(operation.type)
}

function matchIdFromOperation(operation: MatchExecuteOperation): string {
  return operation.type === 'matchCreate' ? operation.config.matchId : operation.matchId
}

function isMatchMutation(operation: MatchExecuteOperation): boolean {
  return operation.type.startsWith('match')
}

function isClockOperation(operation: { type: string }): boolean {
  return operation.type.startsWith('clock')
}

function isPauseResumeOperation(
  operation: MatchExecuteOperation,
): operation is Extract<MatchExecuteOperation, { type: 'clockPause' | 'clockResume' }> {
  return operation.type === 'clockPause' || operation.type === 'clockResume'
}

function isClockEligibilityProbePlan(
  result: MatchExecuteInternalResult,
): result is ClockEligibilityProbePlan {
  return 'kind' in result && result.kind === 'clockEligibilityProbe'
}

function clockEligibilitySnapshot(
  request: CompetitionRoomMatchExecuteRequest,
  current: CompetitionMetaRow,
  acl: AclRow,
  row: MatchRow,
  latestGameProvision: GameProvisionRow | undefined,
  linkedGame: GameProvisionRow | undefined,
): string {
  return stableStringify({
    request,
    current,
    acl,
    match: row,
    latestGameProvision: latestGameProvision ?? null,
    linkedGame: linkedGame ?? null,
  })
}

function operationReason(operation: MatchExecuteOperation): string | null {
  return 'reason' in operation && typeof operation.reason === 'string'
    ? operation.reason
    : null
}

function validateClockReferee(
  callerUserId: string,
  checkpoint: JsonObject,
): { code: string; message: string } | null {
  const config = checkpoint.config
  if (!isJsonObject(config) || !Array.isArray(config.teams) || !Array.isArray(config.referees)) {
    return { code: 'engineInvalidMatchCheckpoint', message: 'match config cannot authorize clock referee' }
  }
  if (!config.teams.every(team => typeof team === 'string')) {
    return { code: 'engineInvalidMatchCheckpoint', message: 'match config teams are invalid' }
  }
  const teams = config.teams as string[]
  const referee = config.referees.find(entry => isJsonObject(entry) && entry.refereeId === callerUserId)
  if (!isJsonObject(referee) || typeof referee.refereeId !== 'string'
    || !('conflictedTeam' in referee)
    || (referee.conflictedTeam !== null && typeof referee.conflictedTeam !== 'string')) {
    return { code: 'forbidden', message: 'referee is not assigned to this match or conflicts with a team' }
  }
  if (referee.conflictedTeam !== null && teams.includes(referee.conflictedTeam)) {
    return { code: 'forbidden', message: 'referee is not assigned to this match or conflicts with a team' }
  }
  return null
}

function engineOperation(operation: MatchExecuteOperation): Exclude<CompetitionOperation, {
  type: 'registrationCreate'
    | 'registrationExecute'
    | 'registrationView'
    | 'registrationExportLockedRoster'
    | 'tournamentCreate'
    | 'tournamentStartMatch'
    | 'tournamentSubmitResult'
    | 'tournamentCorrectResult'
    | 'tournamentDropTeam'
}> {
  switch (operation.type) {
    case 'matchCreate':
      return operation
    case 'matchAssignFirstPlayer':
      return operation
    case 'matchSubmitGame':
      return operation
    case 'matchCorrectRuling':
      return operation
    case 'clockCreate': {
      const { matchId: _matchId, type: _type, ...rest } = operation
      return { type: 'clockCreate', ...rest }
    }
    case 'clockBeginPreparation':
      return { type: 'clockBeginPreparation' }
    case 'clockFinishPreparation': {
      const { matchId: _matchId, type: _type, ...rest } = operation
      return { type: 'clockFinishPreparation', ...rest }
    }
    case 'clockBeginAction': {
      const { matchId: _matchId, type: _type, ...rest } = operation
      return { type: 'clockBeginAction', ...rest }
    }
    case 'clockBeginOutOfTurnChoice': {
      const { matchId: _matchId, type: _type, ...rest } = operation
      return { type: 'clockBeginOutOfTurnChoice', ...rest }
    }
    case 'clockObserve':
      return { type: 'clockObserve' }
    case 'clockEndWindow':
      return { type: 'clockEndWindow' }
    case 'clockPause': {
      const { matchId: _matchId, type: _type, ...rest } = operation
      return { type: 'clockPause', ...rest }
    }
    case 'clockResume': {
      const { matchId: _matchId, type: _type, ...rest } = operation
      return { type: 'clockResume', ...rest }
    }
    case 'clockNewGame': {
      const { matchId: _matchId, type: _type, ...rest } = operation
      return { type: 'clockNewGame', ...rest }
    }
    case 'clockRuleGame': {
      const { matchId: _matchId, type: _type, ...rest } = operation
      return { type: 'clockRuleGame', ...rest }
    }
  }
}

function validateScopedEngineState(
  state: CompetitionServerState,
  matchId: string,
  inputCheckpoint: JsonObject | null,
  inputClock: JsonObject | null,
  operation: MatchExecuteOperation,
): { code: string; message: string } | null {
  if (state.registration !== null || state.tournament !== null) {
    return { code: 'engineCrossScopeMutation', message: 'scoped match execution returned global state' }
  }
  const matches = state.matches ?? {}
  const keys = Object.keys(matches)
  if (keys.length !== 1 || keys[0] !== matchId) {
    return { code: 'engineCrossMatchMutation', message: 'scoped match execution returned another match' }
  }
  const checkpoint = matches[matchId]
  if (!isJsonObject(checkpoint)
    || !isJsonObject(checkpoint.config)
    || checkpoint.config.matchId !== matchId
    || !Array.isArray(checkpoint.history)
    || !checkpoint.history.every(isJsonValue)) {
    return { code: 'engineInvalidMatchCheckpoint', message: 'scoped engine returned an invalid match checkpoint' }
  }
  if (isClockOperation(operation) && !sameJson(checkpoint, inputCheckpoint)) {
    return { code: 'engineCrossScopeMutation', message: 'clock operation changed match checkpoint' }
  }
  if (isMatchMutation(operation) && operation.type !== 'matchCreate'
    && inputCheckpoint !== null && !isJsonObject(checkpoint)) {
    return { code: 'engineInvalidMatchCheckpoint', message: 'match operation returned no match checkpoint' }
  }
  if (!sameJson(state.clock, inputClock) && isMatchMutation(operation)) {
    return { code: 'engineCrossScopeMutation', message: 'match operation changed clock checkpoint' }
  }
  if (isClockOperation(operation)) {
    if (operation.type === 'clockCreate') {
      if (!isJsonObject(state.clock)) {
        return { code: 'engineInvalidClockCheckpoint', message: 'clockCreate must return a clock checkpoint' }
      }
    } else if (!isJsonObject(inputClock) || !isJsonObject(state.clock)) {
      return { code: 'engineInvalidClockCheckpoint', message: 'clock mutation requires valid input and output checkpoints' }
    }
  } else if (state.clock !== null && !isJsonObject(state.clock)) {
    return { code: 'engineInvalidClockCheckpoint', message: 'scoped engine returned an invalid clock checkpoint' }
  }
  return null
}

function sameMatches(
  left: JsonObject | undefined,
  right: JsonObject | undefined,
): boolean {
  return stableStringify(left ?? {}) === stableStringify(right ?? {})
}

function sameJson(left: unknown, right: unknown): boolean {
  return stableStringify(left) === stableStringify(right)
}

function isOfficialOperation(operation: ExecuteOperation): boolean {
  return operation.type.startsWith('clock')
    || operation.type.startsWith('tournament')
    || operation.type === 'registrationExportLockedRoster'
}

function isPlayerRegistrationOperation(operation: ExecuteOperation): boolean {
  return operation.type === 'registrationView' || operation.type === 'registrationExecute'
}

function isRosterMutation(operation: ExecuteOperation): boolean {
  return operation.type === 'registrationExecute'
    && (operation.action.type === 'registerTeam' || operation.action.type === 'replaceRoster')
}

function participantsFromOperation(operation: ExecuteOperation, callerUserId: string): string[] {
  if (operation.type !== 'registrationExecute') return []
  if (operation.action.type === 'registerTeam') {
    return uniqueIds([callerUserId, operation.action.captain, ...operation.action.members])
  }
  if (operation.action.type === 'replaceRoster') {
    return uniqueIds([callerUserId, ...operation.action.members])
  }
  return []
}

async function commandFingerprint(request: CompetitionRoomExecuteRequest): Promise<string> {
  return semanticFingerprint({
    competitionId: request.competitionId,
    callerUserId: request.callerUserId,
    commandId: request.commandId,
    expectedRevision: request.expectedRevision,
    operation: request.operation,
  })
}

async function matchCommandFingerprint(
  request: CompetitionRoomMatchExecuteRequest,
): Promise<string> {
  return semanticFingerprint({
    competitionId: request.competitionId,
    matchId: request.matchId,
    callerUserId: request.callerUserId,
    commandId: request.commandId,
    expectedMatchRevision: request.expectedMatchRevision,
    operation: request.operation,
  })
}

async function startScheduledMatchFingerprint(
  request: CompetitionRoomStartScheduledMatchRequest,
): Promise<string> {
  return semanticFingerprint({
    competitionId: request.competitionId,
    stageId: request.stageId,
    matchId: request.matchId,
    callerUserId: request.callerUserId,
    commandId: request.commandId,
    expectedRevision: request.expectedRevision,
    referees: request.referees,
  })
}

async function createTournamentFingerprint(
  request: CompetitionRoomCreateTournamentRequest,
): Promise<string> {
  return semanticFingerprint({
    competitionId: request.competitionId,
    callerUserId: request.callerUserId,
    commandId: request.commandId,
    expectedRevision: request.expectedRevision,
    stage: request.stage,
  })
}

async function terminalIngestFingerprint(
  request: CompetitionRoomTerminalIngestRequest,
): Promise<string> {
  return semanticFingerprint({
    competitionId: request.competitionId,
    matchId: request.matchId,
    callerUserId: request.callerUserId,
    commandId: request.commandId,
    expectedMatchRevision: request.expectedMatchRevision,
    submissionId: request.submissionId,
    gameRecordId: request.gameRecordId,
    rulingRecordId: request.rulingRecordId,
    startKind: request.startKind,
    firstPlayer: request.firstPlayer,
    terminalEvidence: request.terminalEvidence,
  })
}

async function initialGameStartedFingerprint(
  request: CompetitionRoomInitialGameStartedRequest,
): Promise<string> {
  return semanticFingerprint({
    operationType: 'ingestInitialGameStarted',
    competitionId: request.competitionId,
    matchId: request.matchId,
    callerUserId: request.callerUserId,
    commandId: request.commandId,
    expectedMatchRevision: request.expectedMatchRevision,
    evidence: request.evidence,
  })
}

type AutomaticContinuationKind = 'ordinaryDraw' | 'eliminationTiebreak'

type AutomaticContinuation =
  | { kind: 'ordinaryDraw' }
  | {
    kind: 'eliminationTiebreak'
    continuation: Extract<GameStartKind, { type: 'eliminationDrawContinuation' }>['continuation']
  }

function classifyAutomaticContinuation(
  request: CompetitionRoomTerminalIngestRequest,
  checkpoint: JsonObject,
  projection: CompetitionRoomPublicProjection,
): AutomaticContinuation | false | null {
  const lastEvent = Array.isArray(checkpoint.history) ? checkpoint.history.at(-1) : null
  const checkpointNeedsTiebreak = isJsonObject(lastEvent) && lastEvent.type === 'submitGame'
    && isJsonObject(lastEvent.submission)
    && lastEvent.submission.gameRecordId === request.gameRecordId
    && lastEvent.submission.rulingRecordId === request.rulingRecordId
    && isJsonObject(lastEvent.submission.ruling)
    && lastEvent.submission.ruling.type === 'needsTiebreak'
  if (projection.operation !== 'matchSubmitGame'
    || !isJsonObject(projection.public)
    || !hasExactKeys(projection.public, ['match'])
    || !isJsonObject(projection.public.match)) return checkpointNeedsTiebreak ? null : false
  const match = projection.public.match
  const rulingRecords = Array.isArray(match.rulings)
    ? match.rulings.filter(raw => isJsonObject(raw)
      && raw.rulingRecordId === request.rulingRecordId && raw.gameRecordId === request.gameRecordId)
    : []
  // 已適用加賽的判定不能因 nextGame 損壞而降級成「不需自動續局」。
  const needsTiebreak = checkpointNeedsTiebreak || rulingRecords.some(raw => (
    isJsonObject(raw) && isJsonObject(raw.ruling) && raw.ruling.type === 'needsTiebreak'
  ))
  const games = match.games
  if (!hasExactKeys(match, [
    'config', 'status', 'score', 'games', 'rulings', 'corrections', 'outcome',
    'nextFirstPlayer', 'nextGame',
  ]) || match.status !== 'open' || match.outcome !== null
    || !isJsonObject(match.config) || !isTeamPair(match.config.teams)
    || !isJsonObject(match.config.format)
    || !hasExactKeys(match.config.format, ['type', 'value'])
    || !safeNonNegativeInteger(match.config.format.value)
    || !Array.isArray(games) || games.length < 1
    || !Array.isArray(match.rulings) || !isJsonObject(match.nextGame)
    || !hasExactKeys(match.nextGame, ['gameNumber', 'kind', 'firstPlayer'])
    || !Number.isSafeInteger(match.nextGame.gameNumber)
    || match.nextGame.gameNumber !== games.length + 1
    || !isGameStartKind(match.nextGame.kind)
    || !isJsonObject(match.nextGame.firstPlayer)
    || !hasExactKeys(match.nextGame.firstPlayer, ['type', 'team'])
    || match.nextGame.firstPlayer.type !== 'assigned'
    || !nonEmpty(match.nextGame.firstPlayer.team)) return needsTiebreak ? null : false
  const previous = games.at(-1)
  if (!isJsonObject(previous)
    || !hasExactKeys(previous, [
      'gameNumber', 'gameRecordId', 'startKind', 'firstPlayer', 'terminalEvidence', 'rulingRecordId',
    ])
    || previous.gameRecordId !== request.gameRecordId
    || previous.rulingRecordId !== request.rulingRecordId
    || !isGameStartKind(previous.startKind)
    || !sameJson(previous.startKind, request.startKind)
    || previous.firstPlayer !== request.firstPlayer
    || !match.config.teams.includes(request.firstPlayer)
    || match.nextGame.firstPlayer.team !== match.config.teams.find(team => team !== request.firstPlayer)) {
    return null
  }
  const activeRuling = rulingRecords[0]
  if (rulingRecords.length !== 1 || !isJsonObject(activeRuling)
    || !hasExactKeys(activeRuling, ['rulingRecordId', 'gameRecordId', 'ruling'])
    || !isJsonObject(activeRuling.ruling)
    || !isJsonObject(checkpoint.config) || !sameJson(checkpoint.config, match.config)) return null
  const drawEvidence = isJsonObject(request.terminalEvidence)
    && request.terminalEvidence.type === 'canonicalGameEnded'
    && isJsonObject(request.terminalEvidence.conclusion)
    && isJsonObject(request.terminalEvidence.conclusion.outcome)
    && request.terminalEvidence.conclusion.outcome.type === 'draw'
  if (!drawEvidence || !sameJson(previous.terminalEvidence, request.terminalEvidence)) return null

  const assignedNextTeam = match.config.teams.find(team => team !== request.firstPlayer)
  if (match.nextGame.firstPlayer.team !== assignedNextTeam
    || match.nextFirstPlayer !== assignedNextTeam) return null

  const ruling = activeRuling.ruling
  if (ruling.type === 'gameAccepted' && hasExactKeys(ruling, ['type', 'outcome'])
    && ruling.outcome === 'draw') {
    if (match.config.format.type !== 'fixedGames'
      || match.config.format.value <= games.length
      || request.startKind.type !== 'ordinaryNewGame'
      || previous.startKind.type !== 'ordinaryNewGame'
      || !hasExactKeys(match.nextGame.kind, ['type'])
      || match.nextGame.kind.type !== 'ordinaryNewGame') return false
    return { kind: 'ordinaryDraw' }
  }

  if (ruling.type !== 'needsTiebreak') return needsTiebreak ? null : false
  const nextKind = match.nextGame.kind as unknown as JsonObject
  if (!hasExactKeys(ruling, ['type', 'continuation'])
    || match.config.format.type !== 'firstToWins'
    || !isValidEliminationContinuation(ruling.continuation, match.config.teams)
    || !hasExactKeys(nextKind, ['type', 'continuation'])
    || nextKind.type !== 'eliminationDrawContinuation'
    || !sameJson(nextKind.continuation, ruling.continuation)) return null
  return {
    kind: 'eliminationTiebreak',
    continuation: ruling.continuation as unknown as Extract<GameStartKind, { type: 'eliminationDrawContinuation' }>['continuation'],
  }
}

async function automaticContinuationProvisionFingerprint(value: {
  terminalCommandId: string
  competitionId: string
  matchId: string
  gameNumber: number
  kind: AutomaticContinuationKind
}): Promise<string> {
  return semanticFingerprint({
    kind: `automatic-${value.kind}-provision`,
    terminalCommandId: value.terminalCommandId,
    competitionId: value.competitionId,
    matchId: value.matchId,
    gameNumber: value.gameNumber,
  })
}

async function refereeFingerprint(
  request: CompetitionRoomRefereeRequest,
  operationType: 'grantReferee' | 'revokeReferee',
): Promise<string> {
  return semanticFingerprint({
    operationType,
    competitionId: request.competitionId,
    callerUserId: request.callerUserId,
    targetUserId: request.targetUserId,
    commandId: request.commandId,
    expectedRevision: request.expectedRevision,
  })
}

async function semanticFingerprint(value: unknown): Promise<string> {
  const encoded = new TextEncoder().encode(stableStringify(value))
  const digest = await crypto.subtle.digest('SHA-256', encoded)
  return [...new Uint8Array(digest)].map(byte => byte.toString(16).padStart(2, '0')).join('')
}

async function clockControlIdentity(value: {
  competitionId: string
  matchId: string
  gameId: string
  gameNumber: number
  sourceCommandId: string
  matchRevision: number
}): Promise<string> {
  return `competition-clock-control:${await semanticFingerprint([
    'competition-clock-control', value.competitionId, value.matchId, value.gameId,
    value.gameNumber, value.sourceCommandId, value.matchRevision,
  ])}`
}

function stableStringify(value: unknown): string {
  if (value === null || typeof value !== 'object') return JSON.stringify(value)
  if (Array.isArray(value)) return `[${value.map(stableStringify).join(',')}]`
  const object = value as Record<string, unknown>
  const entries = Object.keys(object).sort().map(key => `${JSON.stringify(key)}:${stableStringify(object[key])}`)
  return `{${entries.join(',')}}`
}

function validateProvisionScheduledGameRequest(
  request: CompetitionRoomProvisionScheduledGameRequest,
): { code: string; message: string } | null {
  if (!isJsonObject(request)
    || !hasExactKeys(request as unknown as JsonObject, [
      'competitionId', 'matchId', 'callerUserId', 'commandId',
      'expectedMatchRevision', 'trustedTimeMs', 'firstPlayer', 'turnOrder',
    ])) {
    return { code: 'invalidRequest', message: 'game provision request contains unsupported fields' }
  }
  if (!nonEmpty(request.competitionId) || !nonEmpty(request.matchId)
    || !nonEmpty(request.callerUserId) || !nonEmpty(request.commandId)
    || !nonEmpty(request.firstPlayer)) {
    return { code: 'invalidRequest', message: 'game provision identity fields are required' }
  }
  if (!Number.isInteger(request.expectedMatchRevision) || request.expectedMatchRevision < 1
    || !Number.isSafeInteger(request.trustedTimeMs) || request.trustedTimeMs < 0) {
    return { code: 'invalidRequest', message: 'game provision revision or trusted time is invalid' }
  }
  if (!Array.isArray(request.turnOrder)
    || ![2, 4].includes(request.turnOrder.length)
    || request.turnOrder.some(player => !nonEmpty(player))
    || new Set(request.turnOrder).size !== request.turnOrder.length
    || request.turnOrder[0] !== request.firstPlayer) {
    return { code: 'invalidLineup', message: 'trusted turn order is invalid' }
  }
  return null
}

function validateProvisionInitialScheduledGameRequest(
  request: CompetitionRoomProvisionInitialScheduledGameRequest,
): { code: string; message: string } | null {
  if (!isJsonObject(request)
    || !hasExactKeys(request as unknown as JsonObject, [
      'competitionId', 'matchId', 'callerUserId', 'commandId',
      'expectedMatchRevision', 'trustedTimeMs',
    ])
    || !nonEmpty(request.competitionId) || !nonEmpty(request.matchId)
    || !nonEmpty(request.callerUserId) || !nonEmpty(request.commandId)) {
    return { code: 'invalidRequest', message: 'initial game provision request is invalid' }
  }
  if (!Number.isSafeInteger(request.expectedMatchRevision) || request.expectedMatchRevision < 1
    || !Number.isSafeInteger(request.trustedTimeMs) || request.trustedTimeMs < 0) {
    return { code: 'invalidRequest', message: 'initial game revision or trusted time is invalid' }
  }
  return null
}

function validateProvisionAssignedScheduledGameRequest(
  request: CompetitionRoomProvisionAssignedScheduledGameRequest,
): { code: string; message: string } | null {
  if (!isJsonObject(request)
    || !hasExactKeys(request as unknown as JsonObject, [
      'competitionId', 'matchId', 'callerUserId', 'commandId',
      'expectedMatchRevision', 'trustedTimeMs',
    ])
    || !nonEmpty(request.competitionId) || !nonEmpty(request.matchId)
    || !nonEmpty(request.callerUserId) || !nonEmpty(request.commandId)) {
    return { code: 'invalidRequest', message: 'assigned game provision request is invalid' }
  }
  if (!Number.isSafeInteger(request.expectedMatchRevision) || request.expectedMatchRevision < 1
    || !Number.isSafeInteger(request.trustedTimeMs) || request.trustedTimeMs < 0) {
    return { code: 'invalidRequest', message: 'assigned game revision or trusted time is invalid' }
  }
  return null
}

function validateAssignNextFirstPlayerByCaptainRequest(
  request: CompetitionRoomAssignNextFirstPlayerByCaptainRequest,
): { code: string; message: string } | null {
  if (!isJsonObject(request)
    || !hasExactKeys(request as unknown as JsonObject, [
      'competitionId', 'matchId', 'callerUserId', 'commandId',
      'expectedMatchRevision', 'trustedTimeMs', 'selectedFirstPlayerTeam',
    ])
    || !nonEmpty(request.competitionId) || !nonEmpty(request.matchId)
    || !nonEmpty(request.callerUserId) || !nonEmpty(request.commandId)
    || !nonEmpty(request.selectedFirstPlayerTeam)) {
    return { code: 'invalidRequest', message: 'captain first-player request is invalid' }
  }
  if (!Number.isSafeInteger(request.expectedMatchRevision) || request.expectedMatchRevision < 1
    || !Number.isSafeInteger(request.trustedTimeMs) || request.trustedTimeMs < 0) {
    return { code: 'invalidRequest', message: 'captain first-player revision or trusted time is invalid' }
  }
  return null
}

function validateProvisionCaptainAssignedScheduledGameRequest(
  request: CompetitionRoomProvisionCaptainAssignedScheduledGameRequest,
): { code: string; message: string } | null {
  if (!isJsonObject(request)
    || !hasExactKeys(request as unknown as JsonObject, [
      'competitionId', 'matchId', 'callerUserId', 'commandId', 'assignmentCommandId',
      'expectedMatchRevision', 'trustedTimeMs',
    ])
    || !nonEmpty(request.competitionId) || !nonEmpty(request.matchId)
    || !nonEmpty(request.callerUserId) || !nonEmpty(request.commandId)
    || !nonEmpty(request.assignmentCommandId)) {
    return { code: 'invalidRequest', message: 'captain game provision request is invalid' }
  }
  if (!Number.isSafeInteger(request.expectedMatchRevision) || request.expectedMatchRevision < 2
    || !Number.isSafeInteger(request.trustedTimeMs) || request.trustedTimeMs < 0) {
    return { code: 'invalidRequest', message: 'captain game revision or trusted time is invalid' }
  }
  return null
}

function isValidCaptainAssignmentAudit(
  audit: AuditRow,
  request: CompetitionRoomProvisionCaptainAssignedScheduledGameRequest,
  revision: number,
  assignmentTrustedTimeMs: number,
): boolean {
  return Number.isSafeInteger(audit.id) && audit.id > 0
    && audit.actor_user_id === request.callerUserId
    && audit.role === 'player'
    && audit.operation_type === 'assignNextFirstPlayerByCaptain'
    && audit.match_id === request.matchId
    && audit.command_id === request.assignmentCommandId
    && audit.revision === revision
    && audit.outcome_code === 'success'
    && Number.isSafeInteger(audit.trusted_time_ms)
    && audit.trusted_time_ms >= 0
    && audit.trusted_time_ms === assignmentTrustedTimeMs
    && audit.reason === CAPTAIN_FIRST_PLAYER_REASON
}

function sameCaptainAssignmentAudit(left: AuditRow, right: AuditRow): boolean {
  return left.id === right.id
    && left.match_id === right.match_id
    && left.actor_user_id === right.actor_user_id
    && left.role === right.role
    && left.operation_type === right.operation_type
    && left.command_id === right.command_id
    && left.revision === right.revision
    && left.outcome_code === right.outcome_code
    && left.trusted_time_ms === right.trusted_time_ms
    && left.reason === right.reason
}

async function provisionScheduledGameFingerprint(
  request: CompetitionRoomProvisionScheduledGameRequest,
): Promise<string> {
  return semanticFingerprint({
    competitionId: request.competitionId,
    matchId: request.matchId,
    callerUserId: request.callerUserId,
    commandId: request.commandId,
    expectedMatchRevision: request.expectedMatchRevision,
    firstPlayer: request.firstPlayer,
    turnOrder: request.turnOrder,
  })
}

async function provisionInitialScheduledGameFingerprint(
  request: CompetitionRoomProvisionInitialScheduledGameRequest,
): Promise<string> {
  return semanticFingerprint({
    competitionId: request.competitionId,
    matchId: request.matchId,
    callerUserId: request.callerUserId,
    commandId: request.commandId,
    expectedMatchRevision: request.expectedMatchRevision,
  })
}

async function provisionAssignedScheduledGameFingerprint(
  request: CompetitionRoomProvisionAssignedScheduledGameRequest,
): Promise<string> {
  return semanticFingerprint({
    competitionId: request.competitionId,
    matchId: request.matchId,
    callerUserId: request.callerUserId,
    commandId: request.commandId,
    expectedMatchRevision: request.expectedMatchRevision,
  })
}

async function assignNextFirstPlayerByCaptainFingerprint(
  request: CompetitionRoomAssignNextFirstPlayerByCaptainRequest,
): Promise<string> {
  return semanticFingerprint({
    competitionId: request.competitionId,
    matchId: request.matchId,
    callerUserId: request.callerUserId,
    commandId: request.commandId,
    expectedMatchRevision: request.expectedMatchRevision,
    selectedFirstPlayerTeam: request.selectedFirstPlayerTeam,
  })
}

async function provisionCaptainAssignedScheduledGameFingerprint(
  request: CompetitionRoomProvisionCaptainAssignedScheduledGameRequest,
): Promise<string> {
  return semanticFingerprint({
    competitionId: request.competitionId,
    matchId: request.matchId,
    callerUserId: request.callerUserId,
    commandId: request.commandId,
    assignmentCommandId: request.assignmentCommandId,
    expectedMatchRevision: request.expectedMatchRevision,
  })
}

function secureRandomBit(): number {
  const bytes = new Uint8Array(1)
  crypto.getRandomValues(bytes)
  return bytes[0]! & 1
}

/** 以伺服器 CSPRNG 抽出不偏斜的首輪種子順序，呼叫端須在建立賽程前持久保存結果。 */
function drawInitialSeedTeams(
  teamIds: string[],
  randomUint32: () => number = secureRandomUint32,
): { teams: StageConfig['teams']; randomSeed: number } {
  if (!isUniqueNonEmptyStringArray(teamIds) || teamIds.length < 2 || teamIds.length > 0xffff_ffff) {
    throw new Error('首輪抽籤名單不合法')
  }
  const shuffled = [...teamIds].sort((left, right) => left < right ? -1 : left > right ? 1 : 0)
  for (let index = shuffled.length - 1; index > 0; index -= 1) {
    const otherIndex = secureRandomIndex(index + 1, randomUint32)
    ;[shuffled[index], shuffled[otherIndex]] = [shuffled[otherIndex]!, shuffled[index]!]
  }
  return {
    teams: shuffled.map((teamId, index) => ({ teamId, seed: index + 1 })),
    randomSeed: checkedRandomUint32(randomUint32),
  }
}

function secureRandomIndex(exclusiveUpperBound: number, randomUint32: () => number): number {
  const range = 0x1_0000_0000
  if (!Number.isSafeInteger(exclusiveUpperBound) || exclusiveUpperBound < 1 || exclusiveUpperBound > range) {
    throw new Error('首輪抽籤範圍不合法')
  }
  const acceptanceLimit = range - (range % exclusiveUpperBound)
  let value: number
  do { value = checkedRandomUint32(randomUint32) } while (value >= acceptanceLimit)
  return value % exclusiveUpperBound
}

function checkedRandomUint32(randomUint32: () => number): number {
  const value = randomUint32()
  if (!Number.isSafeInteger(value) || value < 0 || value > 0xffff_ffff) {
    throw new Error('伺服器隨機來源未回傳有效的 32 位元整數')
  }
  return value
}

function secureRandomUint32(): number {
  const value = new Uint32Array(1)
  crypto.getRandomValues(value)
  return value[0]!
}

type ProvisionIntent = {
  teams: [string, string]
  gameNumber: number
  startKind: GameStartKind
  assignedFirstPlayerTeam: string
  nextGame: JsonObject
}

type AutomaticDrawProvision = {
  kind: AutomaticContinuationKind
  gameNumber: number
  gameId: string
  binding: CompetitionGameBinding
  nextGame: JsonObject
  commandId: string
  fingerprint: string
}

function currentControllableGameNumber(
  row: MatchRow,
  serverState: CompetitionServerState,
): number | null {
  const checkpoint = parseJsonObject(row.match_checkpoint_json, 'match checkpoint')
  const projection = parseOptionalProjection(row.match_projection_json)
  const match = isJsonObject(projection?.public) && isJsonObject(projection.public.match)
    ? projection.public.match
    : null
  if (!projection || !match || !isJsonObject(checkpoint.config)
    || !Array.isArray(checkpoint.history)
    || !checkpoint.history.every(isJsonValue)
    || !sameJson(serverState.matches?.[row.match_id], checkpoint)
    || !sameJson(match.config, checkpoint.config)
    || match.status !== 'open'
    || match.outcome !== null
    || !Array.isArray(match.games)
    || !isJsonObject(match.nextGame)) {
    return null
  }
  const nextGame = match.nextGame
  const firstPlayer = nextGame.firstPlayer
  const submittedGames = checkpoint.history.filter(event => (
    isJsonObject(event) && event.type === 'submitGame'
  )).length
  if (!Number.isSafeInteger(nextGame.gameNumber)
    || (nextGame.gameNumber as number) < 1
    || nextGame.gameNumber !== match.games.length + 1
    || submittedGames !== match.games.length
    || !isJsonObject(firstPlayer)
    || firstPlayer.type !== 'assigned'
    || typeof firstPlayer.team !== 'string'
    || match.nextFirstPlayer !== firstPlayer.team) {
    return null
  }
  const scheduled = findScheduledMatch(serverState.tournament, row.match_id)
  if (!scheduled || row.stage_id !== scheduled.stageId
    || scheduled.status !== 'inProgress' || !scheduled.started) {
    return null
  }
  return nextGame.gameNumber as number
}

function deriveProvisionIntent(
  row: MatchRow,
  serverState: CompetitionServerState,
  firstPlayer: string | null,
  turnOrder: string[] | null,
  scheduleStatus: 'inProgress' | 'pending' = 'inProgress',
): { ok: true; value: ProvisionIntent } | { ok: false; code: string; message: string } {
  try {
    const checkpoint = parseJsonObject(row.match_checkpoint_json, 'match checkpoint')
    const projection = parseOptionalProjection(row.match_projection_json)
    if (!projection || !isJsonObject(projection.public)
      || !hasExactKeys(projection.public, ['match'])
      || !isJsonObject(projection.public.match)
      || !isJsonObject(checkpoint.config)
      || !Array.isArray(checkpoint.history)
      || !checkpoint.history.every(isJsonValue)
      || !Number.isSafeInteger(row.revision)
      || row.revision < 1 + checkpoint.history.length
      || !sameJson(serverState.matches?.[row.match_id], checkpoint)) {
      throw new Error('match projection and checkpoint do not agree')
    }
    const scheduled = findScheduledMatch(serverState.tournament, row.match_id, false, false, scheduleStatus)
    if (!scheduled || row.stage_id !== scheduled.stageId
      || scheduled.status !== scheduleStatus || scheduled.started !== (scheduleStatus === 'inProgress')) {
      throw new Error('scheduled match is not active')
    }

    const match = projection.public.match
    const requiredKeys = [
      'config', 'status', 'score', 'games', 'rulings', 'corrections', 'outcome',
      'nextFirstPlayer', 'nextGame',
    ]
    if (!hasExactKeys(match, requiredKeys)
      || !sameJson(match.config, checkpoint.config)
      || !isJsonObject(match.config)
      || match.config.matchId !== row.match_id
      || !Array.isArray(match.config.teams)
      || match.config.teams.length !== 2
      || !match.config.teams.every(nonEmpty)
      || match.config.teams[0] === match.config.teams[1]
      || !sameJson(match.config.teams, scheduled.teams)
      || !sameJson(match.config.format, scheduled.format)
      || (match.status !== 'open' && match.status !== 'awaitingFirstPlayer')
      || match.outcome !== null
      || !Array.isArray(match.games)
      || !Array.isArray(match.rulings)
      || !Array.isArray(match.corrections)
      || !isJsonObject(match.nextGame)) {
      throw new Error('match is not provisionable')
    }
    if (!isValidOpenMatchContinuation(match)) {
      throw new Error('match continuation checkpoint is invalid')
    }
    const firstTeam = match.config.teams[0]
    const secondTeam = match.config.teams[1]
    if (!nonEmpty(firstTeam) || !nonEmpty(secondTeam)) {
      throw new Error('match teams are invalid')
    }
    const nextGame = match.nextGame
    const gameNumber = nextGame.gameNumber
    const startKind = nextGame.kind
    const firstPlayerIntent = nextGame.firstPlayer
    if (!hasExactKeys(nextGame, ['gameNumber', 'kind', 'firstPlayer'])
      || typeof gameNumber !== 'number'
      || !Number.isSafeInteger(gameNumber) || gameNumber < 1
      || gameNumber !== match.games.length + 1
      || !isGameStartKind(startKind)
      || !isJsonObject(firstPlayerIntent)) {
      throw new Error('next game is invalid')
    }
    if (firstPlayerIntent.type === 'pending') {
      return { ok: false, code: 'firstPlayerPending', message: 'next game first player is still pending' }
    }
    const assignedFirstPlayerTeam = firstPlayerIntent.team
    if (match.status !== 'open'
      || !hasExactKeys(firstPlayerIntent, ['type', 'team'])
      || firstPlayerIntent.type !== 'assigned'
      || !nonEmpty(assignedFirstPlayerTeam)
      || !match.config.teams.includes(assignedFirstPlayerTeam)
      || match.nextFirstPlayer !== assignedFirstPlayerTeam) {
      throw new Error('assigned first player is invalid')
    }
    const submittedGames = checkpoint.history.filter(event => (
      isJsonObject(event) && event.type === 'submitGame'
    )).length
    const teams = [firstTeam, secondTeam] as [string, string]
    if (submittedGames !== match.games.length
      || !isProvisionableMatchHistory(checkpoint, match, teams)
      || (firstPlayer !== null && turnOrder !== null && turnOrder[0] !== firstPlayer)) {
      throw new Error('match history or lineup is invalid')
    }
    return {
      ok: true,
      value: {
        teams: [firstTeam, secondTeam],
        gameNumber,
        startKind,
        assignedFirstPlayerTeam,
        nextGame,
      },
    }
  } catch {
    return { ok: false, code: 'matchNotProvisionable', message: 'match state cannot provision a game room' }
  }
}

function isValidStoredMatchView(
  response: CompetitionEngineResponse,
  serverState: CompetitionServerState,
  row: MatchRow,
): boolean {
  try {
    const raw = response as unknown as JsonObject
    const projection = parseOptionalProjection(row.match_projection_json)
    return isJsonObject(raw)
      && hasExactKeys(raw, ['ok', 'operation', 'public', 'serverState'])
      && raw.ok === true
      && raw.operation === 'matchView'
      && isJsonObject(raw.public)
      && hasExactKeys(raw.public, ['match'])
      && projection !== null
      && isJsonObject(projection.public)
      && hasExactKeys(projection.public, ['match'])
      && sameJson(raw.public.match, projection.public.match)
      && sameJson(raw.serverState, serverState)
  } catch {
    return false
  }
}

function parseParticipantServerState(value: string): CompetitionServerState | null {
  try {
    const raw = JSON.parse(value) as unknown
    if (!isJsonObject(raw)
      || !hasExactKeys(raw, ['registration', 'clock', 'tournament', 'matches'])
      || (raw.registration !== null && !isJsonObject(raw.registration))
      || (raw.clock !== null && !isJsonObject(raw.clock))
      || (raw.tournament !== null && !isJsonObject(raw.tournament))
      || !isJsonObject(raw.matches)) return null
    return raw as unknown as CompetitionServerState
  } catch {
    return null
  }
}

function participantMatchTeams(
  row: MatchRow,
  serverState: CompetitionServerState,
): [string, string] | null {
  try {
    const checkpoint = parseJsonObject(row.match_checkpoint_json, 'match checkpoint')
    const projection = parseOptionalProjection(row.match_projection_json)
    const match = isJsonObject(projection?.public) && isJsonObject(projection.public.match)
      ? projection.public.match : null
    const config = checkpoint.config
    if (!hasExactKeys(checkpoint, ['config', 'history'])
      || !Array.isArray(checkpoint.history) || !checkpoint.history.every(isJsonValue)
      || !isJsonObject(config)
      || !hasExactKeys(config, ['matchId', 'teams', 'format', 'firstGame', 'referees'])
      || config.matchId !== row.match_id || !isTeamPair(config.teams)
      || !isValidMatchFormat(config.format)
      || !Array.isArray(config.referees)
      || !sameJson(serverState.matches?.[row.match_id], checkpoint)
      || !match || !hasExactKeys(match, [
        'config', 'status', 'score', 'games', 'rulings', 'corrections', 'outcome',
        'nextFirstPlayer', 'nextGame',
      ]) || !sameJson(match.config, config)
      || !Array.isArray(match.score) || match.score.length !== 2
      || !match.score.every(safeNonNegativeInteger)
      || !Array.isArray(match.games) || !Array.isArray(match.rulings)
      || !Array.isArray(match.corrections)) return null

    const teams = config.teams
    const scheduled = findScheduledMatch(serverState.tournament, row.match_id, true, true)
    if (!scheduled || row.stage_id !== scheduled.stageId
      || !sameJson(scheduled.teams, teams) || !sameJson(scheduled.format, config.format)
      || scheduled.started !== true
      || (match.status === 'completed') !== (scheduled.status === 'completed')) return null

    if (match.status === 'completed' || match.status === 'awaitingReferee') {
      if (match.nextGame !== null || match.nextFirstPlayer !== null
        || !isJsonObject(match.outcome) || match.games.length === 0) return null
    } else if (match.status === 'awaitingFirstPlayer') {
      if (match.outcome !== null
        || !isJsonObject(match.nextGame)
        || !isJsonObject(match.nextGame.firstPlayer)
        || match.nextGame.firstPlayer.type !== 'pending'
        || match.nextFirstPlayer !== null
        || !isValidOpenMatchContinuation(match)) return null
    } else if (match.status === 'open') {
      if (match.outcome !== null || !isValidOpenMatchContinuation(match)) return null
    } else {
      return null
    }
    return [teams[0], teams[1]]
  } catch {
    return null
  }
}

type CaptainAssignmentIntent = {
  checkpoint: JsonObject
  publicMatch: JsonObject
  teams: [string, string]
  losingTeam: string
  selectedFirstPlayerTeam: string
}

function deriveCaptainAssignmentIntent(
  row: MatchRow,
  serverState: CompetitionServerState,
  selectedTeam: string,
): { ok: true; value: CaptainAssignmentIntent } | { ok: false; code: string; message: string } {
  return deriveCaptainMatchIntent(row, serverState, { mode: 'pending', selectedTeam })
}

function deriveCaptainProvisionIntent(
  row: MatchRow,
  serverState: CompetitionServerState,
): { ok: true; value: CaptainAssignmentIntent } | { ok: false; code: string; message: string } {
  return deriveCaptainMatchIntent(row, serverState, { mode: 'assigned' })
}

function deriveCaptainMatchIntent(
  row: MatchRow,
  serverState: CompetitionServerState,
  state: { mode: 'pending'; selectedTeam: string } | { mode: 'assigned' },
): { ok: true; value: CaptainAssignmentIntent } | { ok: false; code: string; message: string } {
  const fail = (code = 'firstPlayerPolicyUnresolved'): {
    ok: false; code: string; message: string
  } => ({ ok: false, code, message: 'the persisted match state does not allow captain first-player selection' })
  try {
    const checkpoint = parseJsonObject(row.match_checkpoint_json, 'match checkpoint')
    const projection = parseOptionalProjection(row.match_projection_json)
    if (!projection || !isJsonObject(projection.public)
      || !hasExactKeys(projection.public, ['match']) || !isJsonObject(projection.public.match)
      || !isJsonObject(checkpoint.config) || !Array.isArray(checkpoint.history)
      || !checkpoint.history.every(isJsonValue)
      || !Number.isSafeInteger(row.revision) || row.revision < 1 + checkpoint.history.length
      || !sameJson(serverState.matches?.[row.match_id], checkpoint)) return fail()
    const scheduled = findScheduledMatch(serverState.tournament, row.match_id)
    if (!scheduled || row.stage_id !== scheduled.stageId || scheduled.status !== 'inProgress'
      || !scheduled.started) return fail()
    const match = projection.public.match
    if (!hasExactKeys(match, [
      'config', 'status', 'score', 'games', 'rulings', 'corrections', 'outcome',
      'nextFirstPlayer', 'nextGame',
    ]) || !sameJson(match.config, checkpoint.config) || !isJsonObject(match.config)
      || match.config.matchId !== row.match_id || !isTeamPair(match.config.teams)
      || !sameJson(match.config.teams, scheduled.teams)
      || !sameJson(match.config.format, scheduled.format)
      || match.status !== (state.mode === 'pending' ? 'awaitingFirstPlayer' : 'open')
      || match.outcome !== null || !Array.isArray(match.games)
      || match.games.length < 1 || !Array.isArray(match.rulings)
      || !Array.isArray(match.corrections) || !isJsonObject(match.nextGame)
      || !isValidOpenMatchContinuation(match)) return fail()
    const teams = match.config.teams as [string, string]
    const latestBeforeEligibility = match.games[match.games.length - 1]
    const latestRulingRecord = isJsonObject(latestBeforeEligibility)
      ? match.rulings.filter(ruling => isJsonObject(ruling)
        && ruling.gameRecordId === latestBeforeEligibility.gameRecordId
        && ruling.rulingRecordId === latestBeforeEligibility.rulingRecordId)
      : []
    const isEliminationDrawLoss = latestRulingRecord.length === 1
      && isJsonObject(latestRulingRecord[0]) && isJsonObject(latestRulingRecord[0].ruling)
      && parseCompetitionRuling(latestRulingRecord[0].ruling, teams)?.type === 'eliminationDrawLoss'
    if (!isJsonObject(match.nextGame)
      || !hasExactKeys(match.nextGame, ['gameNumber', 'kind', 'firstPlayer'])
      || match.nextGame.gameNumber !== match.games.length + 1
      || typeof match.nextGame.gameNumber !== 'number' || match.nextGame.gameNumber <= 1
      || !isJsonObject(match.nextGame.kind)
      || !hasExactKeys(match.nextGame.kind, ['type'])
      || match.nextGame.kind.type !== 'ordinaryNewGame'
      || !isJsonObject(match.nextGame.firstPlayer)) {
      return fail(isEliminationDrawLoss ? 'eliminationDrawLossUnresolved' : 'firstPlayerPolicyUnresolved')
    }
    const firstPlayerIntent = match.nextGame.firstPlayer
    let selectedTeam: string
    if (state.mode === 'pending') {
      selectedTeam = state.selectedTeam
      if (!teams.includes(selectedTeam) || match.nextFirstPlayer !== null
        || !hasExactKeys(firstPlayerIntent, ['type', 'reason'])
        || firstPlayerIntent.type !== 'pending' || !nonEmpty(firstPlayerIntent.reason)) return fail()
    } else {
      if (match.nextFirstPlayer === null || !hasExactKeys(firstPlayerIntent, ['type', 'team'])
        || firstPlayerIntent.type !== 'assigned' || !nonEmpty(firstPlayerIntent.team)
        || !teams.includes(firstPlayerIntent.team) || match.nextFirstPlayer !== firstPlayerIntent.team) return fail()
      selectedTeam = firstPlayerIntent.team
    }
    if (!isProvisionableMatchHistory(checkpoint, match, teams)) return fail()
    const latestGame = match.games[match.games.length - 1]
    if (!isJsonObject(latestGame) || !hasExactKeys(latestGame, [
      'gameNumber', 'gameRecordId', 'startKind', 'firstPlayer', 'terminalEvidence', 'rulingRecordId',
    ]) || latestGame.gameNumber !== match.games.length || !nonEmpty(latestGame.gameRecordId)
      || !nonEmpty(latestGame.rulingRecordId)) return fail()
    const active = match.rulings.filter(ruling => isJsonObject(ruling)
      && ruling.gameRecordId === latestGame.gameRecordId
      && ruling.rulingRecordId === latestGame.rulingRecordId)
    if (active.length !== 1 || !isJsonObject(active[0]) || !isJsonObject(active[0].ruling)) return fail()
    const ruling = parseCompetitionRuling(active[0].ruling, teams)
    if (!ruling) return fail()
    let losingTeam: string
    if (ruling.type === 'timeoutLoss' || ruling.type === 'eliminationDrawLoss') {
      losingTeam = ruling.loser
    } else if (ruling.type === 'gameAccepted'
      && isJsonObject(ruling.outcome) && nonEmpty(ruling.outcome['winner'])) {
      const winner = ruling.outcome['winner'] as string
      losingTeam = teams.find(team => team !== winner) ?? ''
    } else {
      return fail()
    }
    if (!teams.includes(losingTeam)) return fail()
    return {
      ok: true,
      value: { checkpoint, publicMatch: match, teams, losingTeam, selectedFirstPlayerTeam: selectedTeam },
    }
  } catch {
    return fail()
  }
}

function validateCaptainFirstPlayerResponse(
  response: CompetitionEngineResponse,
  matchId: string,
  previousState: CompetitionServerState,
  previousCheckpoint: JsonObject,
  previousMatch: JsonObject,
  selectedTeam: string,
): {
  checkpoint: JsonObject
  projection: CompetitionRoomPublicProjection
  serverState: CompetitionServerState
} | null {
  if (!response.ok || response.operation !== 'matchAssignFirstPlayer'
    || !hasExactKeys(response as unknown as JsonObject, ['ok', 'operation', 'public', 'serverState'])
    || !isJsonObject(response.serverState)
    || !hasExactKeys(response.serverState, ['registration', 'clock', 'tournament', 'matches'])) return null
  const expectedState = structuredClone(previousState) as CompetitionServerState
  const matches = { ...(expectedState.matches ?? {}) }
  const checkpoint = response.serverState.matches?.[matchId]
  if (!isJsonObject(checkpoint) || !hasExactKeys(checkpoint, ['config', 'history'])
    || !sameJson(checkpoint.config, previousCheckpoint.config)
    || !Array.isArray(checkpoint.history)
    || !sameJson(checkpoint.history, [
      ...(previousCheckpoint.history as JsonValue[]),
      { type: 'assignFirstPlayer', team: selectedTeam, reason: CAPTAIN_FIRST_PLAYER_REASON },
    ])) return null
  matches[matchId] = checkpoint
  expectedState.matches = matches
  if (!sameJson(response.serverState, expectedState)) return null
  const expectedMatch = structuredClone(previousMatch)
  expectedMatch.status = 'open'
  expectedMatch.nextFirstPlayer = selectedTeam
  if (!isJsonObject(expectedMatch.nextGame)) return null
  expectedMatch.nextGame.firstPlayer = { type: 'assigned', team: selectedTeam }
  if (!isJsonObject(response.public) || !hasExactKeys(response.public, ['match'])
    || !sameJson(response.public.match, expectedMatch)) return null
  return {
    checkpoint,
    projection: { ok: true, operation: 'matchAssignFirstPlayer', public: response.public },
    serverState: response.serverState,
  }
}

function isProvisionableMatchHistory(
  checkpoint: JsonObject,
  publicMatch: JsonObject,
  teams: [string, string],
): boolean {
  if (!Array.isArray(checkpoint.history)) return false
  const submissions: JsonObject[] = []
  const submissionIds = new Set<string>()
  const gameRecordIds = new Set<string>()
  const rulingRecordIds = new Set<string>()
  const correctionIds = new Set<string>()
  const activeRulings = new Map<string, string>()
  for (const event of checkpoint.history) {
    if (!isJsonObject(event)) return false
    if (event.type === 'assignFirstPlayer') {
      if (!hasExactKeys(event, ['type', 'team', 'reason'])
        || !nonEmpty(event.team) || !teams.includes(event.team)
        || !nonEmpty(event.reason)) return false
      continue
    }
    if (event.type === 'submitGame') {
      if (!hasExactKeys(event, ['type', 'submission'])
        || !isJsonObject(event.submission)) return false
      const submission = event.submission
      if (!hasExactKeys(submission, [
        'submissionId', 'gameRecordId', 'rulingRecordId', 'startKind',
        'firstPlayer', 'terminalEvidence', 'ruling',
      ]) || !nonEmpty(submission.submissionId) || submissionIds.has(submission.submissionId)
        || !nonEmpty(submission.gameRecordId) || gameRecordIds.has(submission.gameRecordId)
        || !nonEmpty(submission.rulingRecordId) || rulingRecordIds.has(submission.rulingRecordId)
        || !isGameStartKind(submission.startKind)
        || !nonEmpty(submission.firstPlayer) || !teams.includes(submission.firstPlayer)
        || !isGameTerminalEvidence(submission.terminalEvidence)
        || !isJsonObject(submission.ruling)
        || !parseCompetitionRuling(submission.ruling, teams)) return false
      submissionIds.add(submission.submissionId)
      gameRecordIds.add(submission.gameRecordId)
      rulingRecordIds.add(submission.rulingRecordId)
      activeRulings.set(submission.gameRecordId, submission.rulingRecordId)
      submissions.push(submission)
      continue
    }
    if (event.type === 'correctRuling') {
      if (!hasExactKeys(event, [
        'type', 'correctionId', 'refereeId', 'gameRecordId', 'replacementRulingRecordId',
        'replacementRuling', 'reason',
      ]) || !nonEmpty(event.correctionId) || correctionIds.has(event.correctionId)
        || !nonEmpty(event.refereeId) || !nonEmpty(event.gameRecordId)
        || !activeRulings.has(event.gameRecordId)
        || !nonEmpty(event.replacementRulingRecordId)
        || rulingRecordIds.has(event.replacementRulingRecordId)
        || !isJsonObject(event.replacementRuling)
        || !parseCompetitionRuling(event.replacementRuling, teams)
        || !nonEmpty(event.reason)) return false
      correctionIds.add(event.correctionId)
      rulingRecordIds.add(event.replacementRulingRecordId)
      activeRulings.set(event.gameRecordId, event.replacementRulingRecordId)
      continue
    }
    return false
  }
  const latest = submissions.at(-1)
  if (!latest) {
    return submissions.length === 0
      && Array.isArray(publicMatch.games) && publicMatch.games.length === 0
      && Array.isArray(publicMatch.rulings) && publicMatch.rulings.length === 0
      && Array.isArray(publicMatch.corrections) && publicMatch.corrections.length === 0
  }
  if (!matchCheckpointMatchesPublic(
    publicMatch,
    checkpoint,
    latest as unknown as CompetitionTerminalSubmission,
  )) return false
  const lastGame = Array.isArray(publicMatch.games) ? publicMatch.games.at(-1) : null
  const latestGameRecordId = latest.gameRecordId
  if (typeof latestGameRecordId !== 'string') return false
  const activeRulingRecordId = activeRulings.get(latestGameRecordId)
  if (!activeRulingRecordId) return false
  const activeRuling = Array.isArray(publicMatch.rulings)
    ? publicMatch.rulings.find(ruling => isJsonObject(ruling)
      && ruling.gameRecordId === latestGameRecordId
      && ruling.rulingRecordId === activeRulingRecordId)
    : null
  if (!isJsonObject(lastGame) || !isJsonObject(activeRuling)
    || !isJsonObject(activeRuling.ruling) || !isJsonObject(publicMatch.nextGame)
    || !isJsonObject(publicMatch.nextGame.kind)) return false
  if (activeRuling.ruling.type === 'needsTiebreak') {
    const nextFirstPlayer = publicMatch.nextGame.firstPlayer
    const expectedTeam = teams.find(team => team !== lastGame.firstPlayer)
    return hasExactKeys(activeRuling.ruling, ['type', 'continuation'])
      && isValidEliminationContinuation(activeRuling.ruling.continuation, teams)
      && hasExactKeys(publicMatch.nextGame.kind, ['type', 'continuation'])
      && publicMatch.nextGame.kind.type === 'eliminationDrawContinuation'
      && sameJson(publicMatch.nextGame.kind.continuation, activeRuling.ruling.continuation)
      && isJsonObject(nextFirstPlayer)
      && hasExactKeys(nextFirstPlayer, ['type', 'team'])
      && nextFirstPlayer.type === 'assigned'
      && nextFirstPlayer.team === expectedTeam
      && publicMatch.nextFirstPlayer === expectedTeam
  }
  return publicMatch.nextGame.kind.type !== 'eliminationDrawContinuation'
}

type LockedRosterBinding = {
  ruleset: CompetitionGameBinding['ruleset']
  ruleVersion: '5.16' | '5.17'
  enabledRuleModules: string[]
  initialHp: number
  players: CompetitionGameBinding['players']
  deckSnapshot: CompetitionGameBinding['deckSnapshot']
  firstPlayer: string
  turnOrder: string[]
}

interface InitialScheduledGameState {
  checkpoint: JsonObject
  clock: JsonObject
  publicMatch: JsonObject
  teams: [string, string]
}

function deriveInitialScheduledGameState(
  current: CompetitionMetaRow,
  row: MatchRow,
  request: CompetitionRoomProvisionInitialScheduledGameRequest,
  expectedScheduleStatus: 'pending' | 'inProgress' = 'inProgress',
): InitialScheduledGameState {
  if (row.revision !== 1 || row.stage_id === null || row.match_id !== request.matchId
    || row.clock_checkpoint_json === null || row.match_projection_json === null
    || row.clock_projection_json === null) throw new Error('initial match row is incomplete')
  const state = parseServerState(current.server_state_json)
  const checkpoint = parseJsonObject(row.match_checkpoint_json, 'match checkpoint')
  if (!hasExactKeys(checkpoint, ['config', 'history']) || !isJsonObject(checkpoint.config)
    || !Array.isArray(checkpoint.history) || checkpoint.history.length !== 0) {
    throw new Error('initial match checkpoint is not pristine')
  }
  const config = checkpoint.config
  const configuredTeamIds = Array.isArray(config.teams)
    ? config.teams.filter((team): team is string => nonEmpty(team))
    : []
  if (!hasExactKeys(config, ['matchId', 'teams', 'format', 'firstGame', 'referees'])
    || config.matchId !== request.matchId || !Array.isArray(config.teams)
    || config.teams.length !== 2 || !config.teams.every(nonEmpty)
    || config.teams[0] === config.teams[1] || !isValidMatchFormat(config.format)
    || !Array.isArray(config.referees)
    || config.referees.some(referee => !isJsonObject(referee)
      || !hasExactKeys(referee, ['refereeId', 'conflictedTeam'])
      || !nonEmpty(referee.refereeId)
      || (referee.conflictedTeam !== null
        && (!nonEmpty(referee.conflictedTeam) || !configuredTeamIds.includes(referee.conflictedTeam))))
    || new Set(config.referees.map(referee => isJsonObject(referee) ? referee.refereeId : null)).size
      !== config.referees.length
    || !isJsonObject(config.firstGame)
    || !hasExactKeys(config.firstGame, ['type', 'reason'])
    || config.firstGame.type !== 'pending'
    || config.firstGame.reason !== '等待可信隨機先手') {
    throw new Error('initial match configuration is not awaiting trusted first player')
  }
  const teams = config.teams as [string, string]
  if (!sameJson(state.matches?.[request.matchId], checkpoint)) {
    throw new Error('global match mirror differs from materialized checkpoint')
  }
  const scheduled = deriveScheduledMatch(state.tournament, row.stage_id, request.matchId, false, false, expectedScheduleStatus)
  if (row.stage_id !== scheduled.stageId || scheduled.status !== expectedScheduleStatus
    || scheduled.started !== (expectedScheduleStatus === 'inProgress') || !sameJson(scheduled.teams, teams)
    || !sameJson(scheduled.format, config.format)) {
    throw new Error('materialized match differs from active tournament schedule')
  }
  const projectionValue = JSON.parse(row.match_projection_json) as unknown
  if (!isJsonObject(projectionValue)
    || !hasExactKeys(projectionValue, ['ok', 'operation', 'public'])
    || projectionValue.ok !== true || projectionValue.operation !== 'matchCreate'
    || !isJsonObject(projectionValue.public)
    || !hasExactKeys(projectionValue.public, ['match'])
    || !isJsonObject(projectionValue.public.match)) {
    throw new Error('initial match public projection is invalid')
  }
  const publicMatch = projectionValue.public.match
  if (!hasExactKeys(publicMatch, [
    'config', 'status', 'score', 'games', 'rulings', 'corrections', 'outcome',
    'nextFirstPlayer', 'nextGame',
  ]) || !sameJson(publicMatch.config, config) || publicMatch.status !== 'awaitingFirstPlayer'
    || !sameJson(publicMatch.score, [0, 0]) || !Array.isArray(publicMatch.games)
    || publicMatch.games.length !== 0 || !Array.isArray(publicMatch.rulings)
    || publicMatch.rulings.length !== 0 || !Array.isArray(publicMatch.corrections)
    || publicMatch.corrections.length !== 0 || publicMatch.outcome !== null
    || publicMatch.nextFirstPlayer !== null || !isJsonObject(publicMatch.nextGame)
    || !hasExactKeys(publicMatch.nextGame, ['gameNumber', 'kind', 'firstPlayer'])
    || publicMatch.nextGame.gameNumber !== 1 || !isJsonObject(publicMatch.nextGame.kind)
    || !hasExactKeys(publicMatch.nextGame.kind, ['type'])
    || publicMatch.nextGame.kind.type !== 'ordinaryNewGame'
    || !isJsonObject(publicMatch.nextGame.firstPlayer)
    || !hasExactKeys(publicMatch.nextGame.firstPlayer, ['type', 'reason'])
    || publicMatch.nextGame.firstPlayer.type !== 'pending'
    || publicMatch.nextGame.firstPlayer.reason !== '等待可信隨機先手') {
    throw new Error('initial match projection is not awaiting the trusted first player')
  }
  const clock = parseJsonObject(row.clock_checkpoint_json, 'clock checkpoint')
  const clockProjection = JSON.parse(row.clock_projection_json) as unknown
  if (!isJsonObject(clockProjection) || !hasExactKeys(clockProjection, ['ok', 'operation', 'public'])
    || clockProjection.ok !== true || clockProjection.operation !== 'clockCreate'
    || !isJsonValue(clockProjection.public)) throw new Error('initial clock projection is invalid')
  const storedClockProjection = clockProjection as unknown as CompetitionRoomMatchProjection
  if (!isValidClockCheckpoint(clock, teams) || !sameJson(clock.config, scheduled.clockConfig)
    || !Array.isArray(clock.teams)
    || !sameJson(clock.teams.map(team => isJsonObject(team) ? team.team : null), teams)
    || validateClockPublicView(clockProjection.public, clock, teams) !== null
    || storedClockProjectionDeadline(storedClockProjection)
      !== (isJsonObject(clockProjection.public)
        ? clockProjection.public.nextTimeoutDeadlineMs
        : undefined)) {
    throw new Error('initial clock checkpoint differs from the scheduled phase settings')
  }
  return {
    checkpoint,
    clock,
    publicMatch,
    teams,
  }
}

function validateInitialFirstPlayerResponse(
  response: CompetitionEngineResponse,
  matchId: string,
  previous: InitialScheduledGameState,
  selectedTeam: string,
  reason: string,
): { checkpoint: JsonObject; projection: CompetitionRoomMatchProjection } | null {
  if (!response.ok || response.operation !== 'matchAssignFirstPlayer'
    || !hasExactKeys(response as unknown as JsonObject, ['ok', 'operation', 'public', 'serverState'])
    || !isJsonObject(response.serverState)
    || !hasExactKeys(response.serverState, ['registration', 'clock', 'tournament', 'matches'])
    || response.serverState.registration !== null || response.serverState.tournament !== null
    || !sameJson(response.serverState.clock, previous.clock)
    || !isJsonObject(response.serverState.matches)
    || !hasExactKeys(response.serverState.matches, [matchId])) return null
  const checkpoint = response.serverState.matches[matchId]
  if (!isJsonObject(checkpoint) || !hasExactKeys(checkpoint, ['config', 'history'])
    || !sameJson(checkpoint.config, previous.checkpoint.config)
    || !Array.isArray(checkpoint.history)
    || !sameJson(checkpoint.history, [
      ...(previous.checkpoint.history as JsonValue[]),
      { type: 'assignFirstPlayer', team: selectedTeam, reason },
    ])) return null
  const expectedMatch = structuredClone(previous.publicMatch)
  expectedMatch.status = 'open'
  expectedMatch.nextFirstPlayer = selectedTeam
  const nextGame = expectedMatch.nextGame as JsonObject
  nextGame.firstPlayer = { type: 'assigned', team: selectedTeam }
  if (!isJsonObject(response.public) || !hasExactKeys(response.public, ['match'])
    || !sameJson(response.public.match, expectedMatch)) return null
  const projection: CompetitionRoomMatchProjection = {
    ok: true,
    operation: 'matchAssignFirstPlayer',
    public: response.public,
  }
  return { checkpoint, projection }
}

function webRulesetFromRegistration(value: unknown): CompetitionGameBinding['ruleset'] | null {
  // Rust 的 canonical RulesetId 與 web 房間 DTO 是不同邊界，不接受 web 值反向冒充。
  return value === 'base' ? 'fewfc-base' : null
}

function deriveLockedRosterBinding(
  value: JsonValue | undefined,
  matchTeams: [string, string],
  expectedOrganizer: string,
  assignedFirstPlayerTeam: string,
  requestedFirstPlayer: string | null,
  requestedTurnOrder: string[] | null,
): { ok: true; value: LockedRosterBinding } | { ok: false; code: string; message: string } {
  const failed = (): { ok: false; code: string; message: string } => ({
    ok: false,
    code: 'invalidLockedRoster',
    message: 'locked roster cannot produce a valid game binding',
  })
  if (!isJsonObject(value) || !hasExactKeys(value, ['announcement', 'teams'])
    || !isJsonObject(value.announcement) || !Array.isArray(value.teams)) return failed()
  const announcement = value.announcement
  const enabledRuleModules = announcement.enabledRuleModules
  const teamSize = announcement.teamSize
  const initialHp = announcement.initialHp
  const ruleset = webRulesetFromRegistration(announcement.ruleset)
  if (!hasExactKeys(announcement, [
    'organizer', 'ruleVersion', 'ruleset', 'enabledRuleModules', 'teamSize', 'initialHp',
    'registrationMode', 'personalDecks', 'registrationDeadline', 'checkInDeadline', 'plan',
  ])
    || announcement.organizer !== expectedOrganizer
    || (announcement.ruleVersion !== '5.16' && announcement.ruleVersion !== '5.17')
    || ruleset === null
    || !isUniqueNonEmptyStringArray(enabledRuleModules)
    || typeof teamSize !== 'number' || !Number.isSafeInteger(teamSize) || ![1, 2].includes(teamSize)
    || typeof initialHp !== 'number' || !Number.isSafeInteger(initialHp) || initialHp <= 0
    || !parseCompetitionAnnouncementPlan(announcement.plan)) return failed()
  const usesPersonalDeck = enabledRuleModules.includes('personal-deck')
  if (announcement.personalDecks !== (usesPersonalDeck ? 'required' : 'disabled')) return failed()

  const parsedTeams = new Map<string, {
    members: string[]
    decks: Array<{ player: string; name: string; cards: string[] }>
  }>()
  for (const rawTeam of value.teams) {
    if (!isJsonObject(rawTeam)) return failed()
    const teamId = rawTeam.teamId
    const captain = rawTeam.captain
    const rosterVersion = rawTeam.rosterVersion
    const members = rawTeam.members
    const rawDecks = rawTeam.decks
    if (!hasExactKeys(rawTeam, ['teamId', 'captain', 'members', 'rosterVersion', 'decks'])
      || !nonEmpty(teamId) || !nonEmpty(captain)
      || typeof rosterVersion !== 'number'
      || !Number.isSafeInteger(rosterVersion) || rosterVersion < 1
      || !isUniqueNonEmptyStringArray(members)
      || members.length !== teamSize
      || !members.includes(captain)
      || !Array.isArray(rawDecks)
      || parsedTeams.has(teamId)) return failed()
    const decks: Array<{ player: string; name: string; cards: string[] }> = []
    for (const rawDeck of rawDecks) {
      if (!isJsonObject(rawDeck)) return failed()
      const deckPlayer = rawDeck.player
      const deckName = rawDeck.name
      const cards = rawDeck.cards
      if (!hasExactKeys(rawDeck, ['player', 'name', 'cards'])
        || !nonEmpty(deckPlayer) || !nonEmpty(deckName)
        || !isStringArray(cards)) return failed()
      decks.push({
        player: deckPlayer,
        name: deckName,
        cards: [...cards],
      })
    }
    if (usesPersonalDeck
      ? decks.length !== members.length
        || new Set(decks.map(deck => deck.player)).size !== decks.length
        || decks.some(deck => !members.includes(deck.player))
      : decks.length !== 0) return failed()
    parsedTeams.set(teamId, { members: [...members], decks })
  }
  const firstTeam = parsedTeams.get(matchTeams[0])
  const secondTeam = parsedTeams.get(matchTeams[1])
  if (!firstTeam || !secondTeam) return failed()
  if ((requestedFirstPlayer === null) !== (requestedTurnOrder === null)) return failed()
  const assignedTeamMembers = parsedTeams.get(assignedFirstPlayerTeam)
  const opposingTeamId = assignedFirstPlayerTeam === matchTeams[0] ? matchTeams[1] : matchTeams[0]
  const opposingTeamMembers = parsedTeams.get(opposingTeamId)
  if (!assignedTeamMembers || !opposingTeamMembers
    || assignedTeamMembers.members.length !== opposingTeamMembers.members.length) return failed()
  const turnOrder = requestedTurnOrder ?? assignedTeamMembers.members.flatMap((member, index) => {
    const opponent = opposingTeamMembers.members[index]
    return opponent === undefined ? [] : [member, opponent]
  })
  const firstPlayer = requestedFirstPlayer ?? turnOrder[0]
  if (!firstPlayer) return failed()
  const memberTeam = new Map<string, string>()
  for (const member of firstTeam.members) {
    if (memberTeam.has(member)) return failed()
    memberTeam.set(member, matchTeams[0])
  }
  for (const member of secondTeam.members) {
    if (memberTeam.has(member)) return failed()
    memberTeam.set(member, matchTeams[1])
  }
  if (memberTeam.size !== turnOrder.length
    || turnOrder.some(member => !memberTeam.has(member))
    || memberTeam.get(firstPlayer) === undefined
    || memberTeam.get(firstPlayer) !== assignedFirstPlayerTeam) {
    return failed()
  }
  for (let index = 1; index < turnOrder.length; index += 1) {
    const previousPlayer = turnOrder[index - 1]
    const currentPlayer = turnOrder[index]
    if (previousPlayer === undefined || currentPlayer === undefined
      || memberTeam.get(previousPlayer) === memberTeam.get(currentPlayer)) return failed()
  }
  const players: CompetitionGameBinding['players'] = []
  for (const player of turnOrder) {
    const team = memberTeam.get(player)
    if (!team) return failed()
    players.push({ userId: player, player, team })
  }
  const deckByPlayer = new Map(
    [...firstTeam.decks, ...secondTeam.decks].map(deck => [deck.player, deck]),
  )
  const deckSnapshot: CompetitionGameBinding['deckSnapshot'] = []
  if (usesPersonalDeck) {
    for (const player of turnOrder) {
      const deck = deckByPlayer.get(player)
      if (!deck) return failed()
      deckSnapshot.push({ ...deck, player })
    }
  }
  return {
    ok: true,
    value: {
      ruleset,
      ruleVersion: announcement.ruleVersion,
      enabledRuleModules: [...enabledRuleModules],
      initialHp,
      players,
      deckSnapshot,
      firstPlayer,
      turnOrder: [...turnOrder],
    },
  }
}

async function deriveScheduledGameProvision(
  privateRoster: JsonValue | undefined,
  competitionId: string,
  matchId: string,
  organizerUserId: string,
  expectedMatchRevision: number,
  intent: ProvisionIntent,
  roundContext: { roundRef: CompetitionRoundRef; roundStartMs: number },
): Promise<{ ok: true; value: { gameId: string; binding: CompetitionGameBinding } }
  | { ok: false; code: string; message: string }> {
  const roster = deriveLockedRosterBinding(
    privateRoster,
    intent.teams,
    organizerUserId,
    intent.assignedFirstPlayerTeam,
    null,
    null,
  )
  if (!roster.ok) return roster
  const gameId = `competition-game-${await semanticFingerprint({
    kind: 'competition-game', competitionId, matchId, gameNumber: intent.gameNumber,
  })}`
  const binding: CompetitionGameBinding = {
    roundRef: structuredClone(roundContext.roundRef),
    roundStartMs: roundContext.roundStartMs,
    competitionId,
    matchId,
    gameNumber: intent.gameNumber,
    organizerUserId,
    expectedMatchRevision,
    startKind: intent.startKind,
    ruleVersion: roster.value.ruleVersion,
    ruleset: roster.value.ruleset,
    enabledRuleModules: roster.value.enabledRuleModules,
    initialHp: intent.teams.map(team => ({ team, hp: roster.value.initialHp })),
    players: roster.value.players,
    deckSnapshot: roster.value.deckSnapshot,
    firstPlayer: roster.value.firstPlayer,
    turnOrder: roster.value.turnOrder,
    deckSeed: crypto.randomUUID(),
  }
  try {
    validateCompetitionGameBinding(binding)
  } catch {
    return {
      ok: false,
      code: 'invalidLockedRoster',
      message: 'locked roster cannot produce a valid game binding',
    }
  }
  return { ok: true, value: { gameId, binding } }
}

function parseStoredCompetitionGameBinding(value: string): CompetitionGameBinding {
  const parsed = JSON.parse(value) as unknown
  validateCompetitionGameBinding(parsed)
  return parsed
}

function parseServerState(value: string): CompetitionServerState {
  const parsed = JSON.parse(value) as unknown
  if (!isJsonObject(parsed)) throw new Error('stored server state is not an object')
  return {
    registration: nullableObject(parsed.registration),
    clock: nullableObject(parsed.clock),
    tournament: nullableObject(parsed.tournament),
    matches: parsed.matches === undefined ? {} : requiredObject(parsed.matches, 'matches'),
  }
}

function parseJsonObject(value: string, name: string): JsonObject {
  const parsed = JSON.parse(value) as unknown
  if (!isJsonObject(parsed)) throw new Error(`${name} is invalid`)
  return parsed
}

function requiredObject(value: unknown, name: string): JsonObject {
  if (!isJsonObject(value)) throw new Error(`${name} is invalid`)
  return value
}

function parseOptionalProjection(value: string | null): CompetitionRoomMatchProjection | null {
  if (value === null) return null
  const parsed = JSON.parse(value) as unknown
  if (!isJsonObject(parsed)
    || parsed.ok !== true
    || typeof parsed.operation !== 'string'
    || !OPERATION_NAMES.includes(parsed.operation as CompetitionOperationName)
    || !isJsonValue(parsed.public)) {
    throw new Error('stored match projection is invalid')
  }
  return {
    ok: true,
    operation: parsed.operation as CompetitionOperationName,
    public: parsed.public,
  }
}

function nullableObject(value: unknown): JsonObject | null {
  if (value === null) return null
  if (!isJsonObject(value)) throw new Error('stored server state field is invalid')
  return value
}

function parseJsonValue(value: string): CompetitionRoomPrivateProjection {
  const parsed = JSON.parse(value) as unknown
  if (!isJsonValue(parsed)) throw new Error('stored projection is invalid')
  return parsed
}

function parseStoredProjection(value: string): PublicCompetitionSuccess {
  const parsed = JSON.parse(value) as unknown
  if (!isJsonObject(parsed)
    || parsed.ok !== true
    || typeof parsed.operation !== 'string'
    || !OPERATION_NAMES.includes(parsed.operation as CompetitionOperationName)
    || !isJsonValue(parsed.public)) {
    throw new Error('stored public projection is invalid')
  }
  return {
    ok: true,
    operation: parsed.operation as CompetitionOperationName,
    public: parsed.public,
  }
}

function parseStoredExecuteResult(value: string): CompetitionRoomExecuteResult {
  const parsed = JSON.parse(value) as unknown
  if (!isJsonObject(parsed) || typeof parsed.ok !== 'boolean') {
    throw new Error('stored command receipt is invalid')
  }
  if (parsed.ok === false) {
    if (!isJsonObject(parsed.error)
      || typeof parsed.error.code !== 'string'
      || typeof parsed.error.message !== 'string') {
      throw new Error('stored command failure is invalid')
    }
    return roomFailure(parsed.error.code, parsed.error.message)
  }
  if (!isJsonObject(parsed.value)
    || typeof parsed.value.competitionId !== 'string'
    || typeof parsed.value.revision !== 'number'
    || typeof parsed.value.commandId !== 'string'
    || typeof parsed.value.idempotent !== 'boolean'
    || !isJsonObject(parsed.value.projection)
    || parsed.value.projection.ok !== true
    || typeof parsed.value.projection.operation !== 'string'
    || !OPERATION_NAMES.includes(parsed.value.projection.operation as CompetitionOperationName)
    || !isJsonValue(parsed.value.projection.public)) {
    throw new Error('stored command success is invalid')
  }
  const projection: PublicCompetitionSuccess = {
    ok: true,
    operation: parsed.value.projection.operation as CompetitionOperationName,
    public: parsed.value.projection.public,
  }
  if (parsed.value.projection.private !== undefined) {
    if (!isJsonValue(parsed.value.projection.private)) throw new Error('stored private projection is invalid')
    projection.private = parsed.value.projection.private
  }
  return roomSuccess({
    competitionId: parsed.value.competitionId,
    revision: parsed.value.revision,
    commandId: parsed.value.commandId,
    idempotent: true,
    projection,
  })
}

function parseStoredInitialGameStartedReceipt(
  value: string,
  request: CompetitionRoomInitialGameStartedRequest,
): CompetitionRoomInitialGameStartedResult {
  const parsed = JSON.parse(value) as unknown
  if (!isJsonObject(parsed) || !hasExactKeys(parsed, ['ack', 'evidence'])
    || !sameJson(parsed.evidence, request.evidence)
    || !isJsonObject(parsed.ack) || !hasExactKeys(parsed.ack, ['ok', 'value'])
    || parsed.ack.ok !== true || !isJsonObject(parsed.ack.value)
    || parsed.ack.value.idempotent !== false) {
    throw new Error('stored initial GameStarted receipt is invalid')
  }
  const stored = parseStoredMatchExecuteResult(JSON.stringify(parsed.ack))
  if (!stored.ok || stored.value.competitionId !== request.competitionId
    || stored.value.matchId !== request.matchId
    || stored.value.commandId !== request.commandId
    || stored.value.matchRevision !== request.expectedMatchRevision + 1) {
    throw new Error('stored initial GameStarted ACK contradicts its request')
  }
  const expectedOperation = request.evidence.initialWindow.type === 'preparation'
    ? 'clockBeginPreparation' : 'clockBeginAction'
  const projection = stored.value.clockProjection
  if (!projection || projection.operation !== expectedOperation
    || !isJsonObject(projection.public) || !isJsonObject(projection.public.clock)
    || projection.public.clock.lastObservedMs !== request.evidence.startedAtMs) {
    throw new Error('stored initial GameStarted clock evidence is inconsistent')
  }
  return stored
}

function parseStoredMatchExecuteResult(
  value: string,
  terminal = false,
): CompetitionRoomResult<CompetitionRoomMatchExecuteValue> {
  const parsed = JSON.parse(value) as unknown
  if (!isJsonObject(parsed) || typeof parsed.ok !== 'boolean') {
    throw new Error('stored match command receipt is invalid')
  }
  if (parsed.ok === false) {
    if (!isJsonObject(parsed.error)
      || typeof parsed.error.code !== 'string'
      || typeof parsed.error.message !== 'string') {
      throw new Error('stored match command failure is invalid')
    }
    return roomFailure(parsed.error.code, parsed.error.message)
  }
  const storedValueKeys = new Set([
    'competitionId', 'matchId', 'matchRevision', 'commandId', 'idempotent',
    'timeoutEvidence', 'nextClockDeadlineMs', 'matchProjection', 'clockProjection',
  ])
  if (!isJsonObject(parsed.value)
    || Object.keys(parsed.value).length !== storedValueKeys.size
    || Object.keys(parsed.value).some(key => !storedValueKeys.has(key))
    || typeof parsed.value.competitionId !== 'string'
    || typeof parsed.value.matchId !== 'string'
    || !safeNonNegativeInteger(parsed.value.matchRevision)
    || typeof parsed.value.commandId !== 'string'
    || typeof parsed.value.idempotent !== 'boolean'
    || (parsed.value.nextClockDeadlineMs !== null
      && !safeNonNegativeInteger(parsed.value.nextClockDeadlineMs))
    || (terminal && parsed.value.nextClockDeadlineMs !== null)
    || (parsed.value.timeoutEvidence !== null
      && (!isGameTerminalEvidence(parsed.value.timeoutEvidence)
        || parsed.value.timeoutEvidence.type !== 'timeout'))) {
    throw new Error('stored match command receipt is invalid')
  }
  const parseProjection = (raw: unknown): CompetitionRoomMatchProjection | null => {
    if (raw === null) return null
    if (!isJsonObject(raw)
      || raw.ok !== true
      || typeof raw.operation !== 'string'
      || !OPERATION_NAMES.includes(raw.operation as CompetitionOperationName)
      || !isJsonValue(raw.public)) {
      throw new Error('stored match projection is invalid')
    }
    const projection: CompetitionRoomMatchProjection = {
      ok: true,
      operation: raw.operation as CompetitionOperationName,
      public: raw.public,
    }
    if (projection.operation === 'clockCreate') storedClockProjectionDeadline(projection)
    return projection
  }
  const clockProjection = parseProjection(parsed.value.clockProjection)
  if (clockProjection && clockProjection.operation.startsWith('clock')) {
    const projectionDeadline = storedClockProjectionDeadline(clockProjection)
    if (!terminal && clockProjection.operation !== 'clockCreate'
      && projectionDeadline !== parsed.value.nextClockDeadlineMs) {
      throw new Error('stored clock projection contradicts its deadline')
    }
  }
  return roomSuccess({
    competitionId: parsed.value.competitionId,
    matchId: parsed.value.matchId,
    matchRevision: parsed.value.matchRevision,
    commandId: parsed.value.commandId,
    idempotent: true,
    timeoutEvidence: parsed.value.timeoutEvidence as Extract<GameTerminalEvidence, { type: 'timeout' }> | null,
    nextClockDeadlineMs: parsed.value.nextClockDeadlineMs as number | null,
    matchProjection: parseProjection(parsed.value.matchProjection),
    clockProjection,
  })
}

function storedClockProjectionDeadline(projection: CompetitionRoomMatchProjection): number | null {
  const publicValue = projection.public
  let clock: unknown = null
  if (projection.operation === 'clockCreate') {
    clock = publicValue
  } else if (isJsonObject(publicValue)
    && Object.keys(publicValue).length === 2
    && isJsonObject(publicValue.result)
    && 'clock' in publicValue) {
    clock = publicValue.clock
  }
  if (!isValidStoredClockPublicView(clock)) {
    throw new Error('stored clock projection is invalid')
  }
  return clock.nextTimeoutDeadlineMs as number | null
}

function isValidStoredClockPublicView(value: unknown): value is JsonObject {
  if (!isJsonObject(value)) return false
  const allowed = new Set([
    'config', 'teams', 'activeWindow', 'pausedWindow', 'expiredTeam', 'needsDecision',
    'timeoutRuling', 'byoyomiEntryOrder', 'lastObservedMs', 'nextTimeoutDeadlineMs',
  ])
  if (Object.keys(value).length !== allowed.size || Object.keys(value).some(key => !allowed.has(key))) {
    return false
  }
  if (!isExactClockConfig(value.config) || !isValidClockConfig(value.config)
    || !Array.isArray(value.teams) || value.teams.length !== 2
    || !value.teams.every(isValidStoredClockTeam)
    || !isValidStoredPublicWindow(value.activeWindow)
    || !isValidStoredPublicWindow(value.pausedWindow)
    || (value.expiredTeam !== null && !nonEmpty(value.expiredTeam))
    || (value.needsDecision !== null && ![
      'zeroTimeControl', 'simultaneousByoyomiEntry', 'unlimitedEliminationDraw', 'zeroTimeEliminationDraw',
    ].includes(value.needsDecision as string))
    || !isValidByoyomiEntryOrder(value.byoyomiEntryOrder)
    || !validNonNegativeInteger(value.lastObservedMs)
    || (value.nextTimeoutDeadlineMs !== null && !safeNonNegativeInteger(value.nextTimeoutDeadlineMs))) {
    return false
  }
  const deadline = value.nextTimeoutDeadlineMs as number | null
  const deadlineForbidden = value.activeWindow === null
    || value.pausedWindow !== null
    || value.expiredTeam !== null
    || value.needsDecision !== null
    || value.timeoutRuling !== null
  if (deadline !== null
    && (deadlineForbidden || deadline < (value.lastObservedMs as number))) {
    return false
  }
  if (value.timeoutRuling === null || !isJsonObject(value.timeoutRuling)) return value.timeoutRuling === null
  if (value.timeoutRuling.type === 'timeoutLoss') {
    return Object.keys(value.timeoutRuling).length === 2 && nonEmpty(value.timeoutRuling.loser)
  }
  return value.timeoutRuling.type === 'doubleTimeoutLoss'
    && Object.keys(value.timeoutRuling).length === 2
    && isTeamPair(value.timeoutRuling.teams)
}

function isValidStoredClockTeam(value: JsonValue): boolean {
  return isJsonObject(value)
    && Object.keys(value).length === 7
    && nonEmpty(value.team)
    && (value.mainTimeRemainingMs === null || validNonNegativeInteger(value.mainTimeRemainingMs))
    && (value.actionTurn === null || validNonNegativeInteger(value.actionTurn))
    && (value.actionTimeRemainingMs === null || validNonNegativeInteger(value.actionTimeRemainingMs))
    && typeof value.actionByoyomiStarted === 'boolean'
    && typeof value.enteredByoyomi === 'boolean'
    && (value.byoyomiEntrySequence === null || validNonNegativeInteger(value.byoyomiEntrySequence))
}

function isValidStoredPublicWindow(value: JsonValue | undefined): boolean {
  if (value === null) return true
  if (value === undefined) return false
  if (!isJsonObject(value) || typeof value.type !== 'string') return false
  if (value.type === 'preparation') {
    return Object.keys(value).length === 2
      && Array.isArray(value.teams)
      && value.teams.length > 0
      && value.teams.every(team => nonEmpty(team))
  }
  if (value.type === 'action') {
    return Object.keys(value).length === 3 && nonEmpty(value.team)
      && validNonNegativeInteger(value.turn)
  }
  return value.type === 'outOfTurnChoice'
    && Object.keys(value).length === 3
    && nonEmpty(value.team)
    && nonEmpty(value.choiceId)
}

function parseStoredStartScheduledMatchResult(
  value: string,
  request: CompetitionRoomStartScheduledMatchRequest,
): CompetitionRoomStartScheduledMatchResult {
  const parsed = JSON.parse(value) as unknown
  if (!isJsonObject(parsed) || !hasExactKeys(parsed, ['ok', 'value'])
    || parsed.ok !== true || !isJsonObject(parsed.value)
    || !hasExactKeys(parsed.value, [
      'competitionId', 'stageId', 'matchId', 'revision', 'matchRevision',
      'commandId', 'idempotent', 'tournamentProjection', 'matchProjection', 'clockProjection',
    ])
    || !nonEmpty(parsed.value.competitionId)
    || !nonEmpty(parsed.value.stageId)
    || !nonEmpty(parsed.value.matchId)
    || typeof parsed.value.revision !== 'number'
    || !Number.isSafeInteger(parsed.value.revision) || parsed.value.revision < 1
    || typeof parsed.value.matchRevision !== 'number'
    || !Number.isSafeInteger(parsed.value.matchRevision)
    || parsed.value.matchRevision !== 1
    || !nonEmpty(parsed.value.commandId)
    || parsed.value.idempotent !== false
    || parsed.value.competitionId !== request.competitionId
    || parsed.value.stageId !== request.stageId
    || parsed.value.matchId !== request.matchId
    || parsed.value.commandId !== request.commandId
    || parsed.value.revision !== request.expectedRevision + 1) {
    throw new Error('stored scheduled match start receipt is invalid')
  }
  const parseProjection = (raw: unknown, operation: CompetitionOperationName): CompetitionRoomMatchProjection => {
    if (!isJsonObject(raw) || !hasExactKeys(raw, ['ok', 'operation', 'public'])
      || raw.ok !== true || raw.operation !== operation || !isJsonValue(raw.public)
      || containsPrivateRosterFields(raw.public)) {
      throw new Error('stored scheduled match start projection is invalid')
    }
    if (operation === 'tournamentStartMatch'
      && (!isJsonObject(raw.public)
        || !hasExactKeys(raw.public, ['result', 'tournament'])
        || !isJsonObject(raw.public.result)
        || !hasExactKeys(raw.public.result, ['type'])
        || raw.public.result.type !== 'started'
        || !isJsonObject(raw.public.tournament))) {
      throw new Error('stored scheduled match start projection is invalid')
    }
    const projection: CompetitionRoomMatchProjection = {
      ok: true,
      operation,
      public: raw.public,
    }
    if (operation === 'clockCreate') storedClockProjectionDeadline(projection)
    return projection
  }
  return roomSuccess({
    competitionId: parsed.value.competitionId,
    stageId: parsed.value.stageId,
    matchId: parsed.value.matchId,
    revision: parsed.value.revision,
    matchRevision: parsed.value.matchRevision,
    commandId: parsed.value.commandId,
    idempotent: true,
    tournamentProjection: parseProjection(parsed.value.tournamentProjection, 'tournamentStartMatch'),
    matchProjection: parseProjection(parsed.value.matchProjection, 'matchCreate'),
    clockProjection: parseProjection(parsed.value.clockProjection, 'clockCreate'),
  })
}

function parseStoredCaptainFirstPlayerResult(
  value: string,
  request: CompetitionRoomAssignNextFirstPlayerByCaptainRequest,
): CompetitionRoomAssignNextFirstPlayerByCaptainValue {
  const parsed = JSON.parse(value) as unknown
  if (!isJsonObject(parsed) || !hasExactKeys(parsed, ['ok', 'value']) || parsed.ok !== true
    || !isJsonObject(parsed.value)
    || !hasExactKeys(parsed.value, [
      'competitionId', 'matchId', 'matchRevision', 'commandId', 'idempotent', 'matchProjection',
    ])
    || parsed.value.competitionId !== request.competitionId
    || parsed.value.matchId !== request.matchId
    || parsed.value.matchRevision !== request.expectedMatchRevision + 1
    || parsed.value.commandId !== request.commandId
    || parsed.value.idempotent !== false
    || !isJsonObject(parsed.value.matchProjection)
    || !hasExactKeys(parsed.value.matchProjection, ['ok', 'operation', 'public'])
    || parsed.value.matchProjection.ok !== true
    || parsed.value.matchProjection.operation !== 'matchAssignFirstPlayer'
    || !isJsonObject(parsed.value.matchProjection.public)
    || !hasExactKeys(parsed.value.matchProjection.public, ['match'])
    || !isJsonObject(parsed.value.matchProjection.public.match)) {
    throw new Error('stored captain first-player receipt is invalid')
  }
  const match = parsed.value.matchProjection.public.match
  if (!hasExactKeys(match, [
    'config', 'status', 'score', 'games', 'rulings', 'corrections', 'outcome',
    'nextFirstPlayer', 'nextGame',
  ]) || !isJsonObject(match.config)
    || !hasExactKeys(match.config, ['matchId', 'teams', 'format', 'firstGame', 'referees'])
    || match.config.matchId !== request.matchId || !isTeamPair(match.config.teams)
    || !match.config.teams.includes(request.selectedFirstPlayerTeam)
    || match.status !== 'open' || match.outcome !== null
    || match.nextFirstPlayer !== request.selectedFirstPlayerTeam
    || !Array.isArray(match.games) || !Array.isArray(match.rulings)
    || !Array.isArray(match.corrections) || !isJsonObject(match.nextGame)
    || !hasExactKeys(match.nextGame, ['gameNumber', 'kind', 'firstPlayer'])
    || !safeNonNegativeInteger(match.nextGame.gameNumber) || match.nextGame.gameNumber < 2
    || !isJsonObject(match.nextGame.kind) || !hasExactKeys(match.nextGame.kind, ['type'])
    || match.nextGame.kind.type !== 'ordinaryNewGame'
    || !isJsonObject(match.nextGame.firstPlayer)
    || !hasExactKeys(match.nextGame.firstPlayer, ['type', 'team'])
    || match.nextGame.firstPlayer.type !== 'assigned'
    || match.nextGame.firstPlayer.team !== request.selectedFirstPlayerTeam
    || containsForbiddenCaptainReceiptField(match)
    || !isStrictCaptainReceiptMatch(match, request.selectedFirstPlayerTeam)) {
    throw new Error('stored captain first-player projection is invalid')
  }
  return {
    competitionId: request.competitionId,
    matchId: request.matchId,
    matchRevision: request.expectedMatchRevision + 1,
    commandId: request.commandId,
    idempotent: true,
    matchProjection: parsed.value.matchProjection as unknown as CompetitionRoomPublicProjection,
  }
}

async function parseStoredProvisionCaptainAssignedScheduledGameResult(
  value: string,
  request: CompetitionRoomProvisionCaptainAssignedScheduledGameRequest,
): Promise<CompetitionRoomProvisionCaptainAssignedScheduledGameValue> {
  const parsed = JSON.parse(value) as unknown
  if (!isJsonObject(parsed) || !hasExactKeys(parsed, ['ok', 'value']) || parsed.ok !== true
    || !isJsonObject(parsed.value)
    || !hasExactKeys(parsed.value, [
      'competitionId', 'matchId', 'matchRevision', 'gameNumber', 'gameId',
      'commandId', 'idempotent', 'status',
    ])
    || parsed.value.competitionId !== request.competitionId
    || parsed.value.matchId !== request.matchId
    || parsed.value.matchRevision !== request.expectedMatchRevision
    || !safeNonNegativeInteger(parsed.value.gameNumber)
    || parsed.value.gameNumber < 2
    || parsed.value.gameNumber < 1
    || typeof parsed.value.gameId !== 'string'
    || parsed.value.commandId !== request.commandId
    || parsed.value.idempotent !== false
    || parsed.value.status !== 'pending') {
    throw new Error('stored captain game provision receipt is invalid')
  }
  const expectedGameId = `competition-game-${await semanticFingerprint({
    kind: 'competition-game',
    competitionId: request.competitionId,
    matchId: request.matchId,
    gameNumber: parsed.value.gameNumber,
  })}`
  if (parsed.value.gameId !== expectedGameId) {
    throw new Error('stored captain game provision identity is invalid')
  }
  return {
    competitionId: request.competitionId,
    matchId: request.matchId,
    matchRevision: request.expectedMatchRevision,
    gameNumber: parsed.value.gameNumber as number,
    gameId: parsed.value.gameId,
    commandId: request.commandId,
    idempotent: true,
    status: 'pending',
  }
}

function containsForbiddenCaptainReceiptField(value: JsonValue): boolean {
  if (Array.isArray(value)) return value.some(containsForbiddenCaptainReceiptField)
  if (!isJsonObject(value)) return false
  if (Object.keys(value).some(key => [
    'private', 'serverState', 'authorizedPrivate', 'announcement', 'captain',
    'members', 'decks', 'rosterVersion', 'lockedRoster',
  ].includes(key))) return true
  return Object.values(value).some(containsForbiddenCaptainReceiptField)
}

function isStrictCaptainReceiptMatch(match: JsonObject, selectedTeam: string): boolean {
  const config = match.config
  if (!isJsonObject(config) || !isTeamPair(config.teams)) return false
  const teams = config.teams
  if (!isJsonObject(config.format)
    || !((config.format.type === 'firstToWins' || config.format.type === 'fixedGames')
      && hasExactKeys(config.format, ['type', 'value'])
      && safeNonNegativeInteger(config.format.value) && config.format.value > 0)
    || !isJsonObject(config.firstGame)
    || !['assigned', 'pending'].includes(config.firstGame.type as string)
    || (config.firstGame.type === 'assigned'
      ? !hasExactKeys(config.firstGame, ['type', 'team']) || !teams.includes(config.firstGame.team as string)
      : !hasExactKeys(config.firstGame, ['type', 'reason']) || !nonEmpty(config.firstGame.reason))
    || !Array.isArray(config.referees)
    || config.referees.some(referee => !isJsonObject(referee)
      || !hasExactKeys(referee, ['refereeId', 'conflictedTeam'])
      || !nonEmpty(referee.refereeId)
      || (referee.conflictedTeam !== null && !teams.includes(referee.conflictedTeam as string)))
    || !Array.isArray(match.score) || match.score.length !== 2
    || !match.score.every(safeNonNegativeInteger)
    || !Array.isArray(match.games) || !Array.isArray(match.rulings)
    || !Array.isArray(match.corrections)) return false
  const rulings = new Map<string, string>()
  for (const item of match.rulings) {
    if (!isJsonObject(item) || !hasExactKeys(item, ['rulingRecordId', 'gameRecordId', 'ruling'])
      || !nonEmpty(item.rulingRecordId) || !nonEmpty(item.gameRecordId)
      || !isJsonObject(item.ruling) || rulings.has(item.rulingRecordId)
      || !parseCompetitionRuling(item.ruling, teams)) return false
    rulings.set(item.rulingRecordId, item.gameRecordId)
  }
  for (let index = 0; index < match.games.length; index += 1) {
    const game = match.games[index]
    if (!isJsonObject(game) || !hasExactKeys(game, [
      'gameNumber', 'gameRecordId', 'startKind', 'firstPlayer', 'terminalEvidence', 'rulingRecordId',
    ]) || game.gameNumber !== index + 1 || !nonEmpty(game.gameRecordId)
      || !nonEmpty(game.firstPlayer) || !teams.includes(game.firstPlayer)
      || !isGameStartKind(game.startKind) || !isGameTerminalEvidence(game.terminalEvidence)
      || !nonEmpty(game.rulingRecordId) || rulings.get(game.rulingRecordId) !== game.gameRecordId) return false
  }
  for (const correction of match.corrections) {
    if (!isJsonObject(correction) || !hasExactKeys(correction, [
      'correctionId', 'refereeId', 'gameRecordId', 'previousRulingRecordId',
      'replacementRulingRecordId', 'replacementRuling', 'reason',
    ]) || !nonEmpty(correction.correctionId) || !nonEmpty(correction.refereeId)
      || !nonEmpty(correction.gameRecordId) || !nonEmpty(correction.previousRulingRecordId)
      || !nonEmpty(correction.replacementRulingRecordId) || !nonEmpty(correction.reason)
      || !isJsonObject(correction.replacementRuling)
      || !parseCompetitionRuling(correction.replacementRuling, teams)) return false
  }
  return isJsonObject(match.nextGame)
    && match.nextGame.gameNumber === match.games.length + 1
    && match.nextFirstPlayer === selectedTeam
}

async function parseStoredProvisionInitialScheduledGameResult(
  value: string,
  request: CompetitionRoomProvisionInitialScheduledGameRequest,
): Promise<CompetitionRoomProvisionInitialScheduledGameValue> {
  const parsed = JSON.parse(value) as unknown
  const expectedGameId = `competition-game-${await semanticFingerprint({
    kind: 'competition-game',
    competitionId: request.competitionId,
    matchId: request.matchId,
    gameNumber: 1,
  })}`
  if (!isJsonObject(parsed) || !hasExactKeys(parsed, ['ok', 'value']) || parsed.ok !== true
    || !isJsonObject(parsed.value)
    || !hasExactKeys(parsed.value, [
      'competitionId', 'matchId', 'matchRevision', 'gameNumber', 'gameId',
      'commandId', 'idempotent', 'status', 'binding',
    ])
    || parsed.value.competitionId !== request.competitionId
    || parsed.value.matchId !== request.matchId
    || parsed.value.matchRevision !== 2 || parsed.value.gameNumber !== 1
    || parsed.value.gameId !== expectedGameId || parsed.value.commandId !== request.commandId
    || parsed.value.idempotent !== false || parsed.value.status !== 'pending'
    || !isJsonObject(parsed.value.binding)) {
    throw new Error('stored initial scheduled game receipt is invalid')
  }
  validateCompetitionGameBinding(parsed.value.binding)
  const binding = parsed.value.binding as unknown as CompetitionGameBinding
  if (binding.competitionId !== request.competitionId || binding.matchId !== request.matchId
    || binding.gameNumber !== 1 || binding.expectedMatchRevision !== 2
    || binding.startKind.type !== 'ordinaryNewGame'
    || binding.firstPlayer !== binding.turnOrder[0]) {
    throw new Error('stored initial scheduled game binding is invalid')
  }
  return {
    competitionId: request.competitionId,
    matchId: request.matchId,
    matchRevision: 2,
    gameNumber: 1,
    gameId: expectedGameId,
    commandId: request.commandId,
    idempotent: false,
    status: 'pending',
    binding,
  }
}

async function parseStoredProvisionAssignedScheduledGameResult(
  value: string,
  request: CompetitionRoomProvisionAssignedScheduledGameRequest,
): Promise<CompetitionRoomProvisionAssignedScheduledGameValue> {
  const parsed = JSON.parse(value) as unknown
  if (!isJsonObject(parsed) || !hasExactKeys(parsed, ['ok', 'value']) || parsed.ok !== true
    || !isJsonObject(parsed.value)
    || !hasExactKeys(parsed.value, [
      'competitionId', 'matchId', 'matchRevision', 'gameNumber', 'gameId',
      'commandId', 'idempotent', 'status', 'binding',
    ])
    || parsed.value.competitionId !== request.competitionId
    || parsed.value.matchId !== request.matchId
    || !Number.isSafeInteger(parsed.value.matchRevision)
    || (parsed.value.matchRevision as number) < 1
    || !Number.isSafeInteger(parsed.value.gameNumber)
    || (parsed.value.gameNumber as number) < 1
    || typeof parsed.value.gameId !== 'string'
    || parsed.value.commandId !== request.commandId
    || parsed.value.idempotent !== false
    || parsed.value.status !== 'pending'
    || !isJsonObject(parsed.value.binding)) {
    throw new Error('stored assigned scheduled game receipt is invalid')
  }
  validateCompetitionGameBinding(parsed.value.binding)
  const binding = parsed.value.binding as unknown as CompetitionGameBinding
  if (binding.competitionId !== request.competitionId
    || binding.matchId !== request.matchId
    || binding.gameNumber !== parsed.value.gameNumber
    || binding.expectedMatchRevision !== parsed.value.matchRevision
    || binding.firstPlayer !== binding.turnOrder[0]) {
    throw new Error('stored assigned scheduled game binding is invalid')
  }
  const expectedGameId = `competition-game-${await semanticFingerprint({
    kind: 'competition-game',
    competitionId: request.competitionId,
    matchId: request.matchId,
    gameNumber: parsed.value.gameNumber,
  })}`
  if (parsed.value.gameId !== expectedGameId) {
    throw new Error('stored assigned scheduled game id is invalid')
  }
  return {
    competitionId: request.competitionId,
    matchId: request.matchId,
    matchRevision: parsed.value.matchRevision as number,
    gameNumber: parsed.value.gameNumber as number,
    gameId: parsed.value.gameId,
    commandId: request.commandId,
    idempotent: false,
    status: parsed.value.status,
    binding,
  }
}

function parseStoredCreateTournamentResult(
  value: string,
): CompetitionRoomCreateTournamentResult {
  const parsed = JSON.parse(value) as unknown
  if (!isJsonObject(parsed) || parsed.ok !== true || !isJsonObject(parsed.value)
    || typeof parsed.value.competitionId !== 'string'
    || !Number.isSafeInteger(parsed.value.revision)
    || typeof parsed.value.commandId !== 'string'
    || parsed.value.idempotent !== false
    || !isJsonObject(parsed.value.projection)
    || parsed.value.projection.ok !== true
    || parsed.value.projection.operation !== 'tournamentCreate'
    || !isJsonValue(parsed.value.projection.public)
    || 'private' in parsed.value.projection
    || containsPrivateRosterFields(parsed.value.projection.public)) {
    throw new Error('stored tournament creation receipt is invalid')
  }
  return roomSuccess({
    competitionId: parsed.value.competitionId,
    revision: parsed.value.revision as number,
    commandId: parsed.value.commandId,
    idempotent: true,
    projection: {
      ok: true,
      operation: 'tournamentCreate',
      public: parsed.value.projection.public,
    },
  })
}

function parseStoredAclResult(
  value: string,
): CompetitionRoomResult<CompetitionRoomAclValue> {
  const parsed = JSON.parse(value) as unknown
  if (!isJsonObject(parsed) || parsed.ok !== true || !isJsonObject(parsed.value)
    || typeof parsed.value.competitionId !== 'string'
    || typeof parsed.value.revision !== 'number'
    || typeof parsed.value.targetUserId !== 'string'
    || (parsed.value.role !== 'organizer'
      && parsed.value.role !== 'referee'
      && parsed.value.role !== 'player')
    || typeof parsed.value.active !== 'boolean') {
    throw new Error('stored ACL receipt is invalid')
  }
  return roomSuccess({
    competitionId: parsed.value.competitionId,
    revision: parsed.value.revision,
    targetUserId: parsed.value.targetUserId,
    role: parsed.value.role,
    active: parsed.value.active,
    idempotent: true,
  })
}
