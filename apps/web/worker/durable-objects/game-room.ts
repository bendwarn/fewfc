import { DurableObject } from 'cloudflare:workers'
import { parseSocketRequest } from '../../shared/realtime'
import { attachRealtimeIdentity, durableSocket, durableWebSocketHandler, realtimeEvent, requireRealtimeIdentity, socketFailure, socketResponse } from '../realtime-handler'
import { customDeckForUser } from '../../server/utils/player-deck'
import { updatePublicRoom } from '../../server/utils/public-room-index'
import { competitionGameReadyError } from '../../server/utils/competition-game-room'
import { buildCompetitionMatchClockReadRequest, readCompetitionMatchClock, serverCompetitionMatchClockTime } from '../../server/utils/competition-match-clock'
import { callPersonalDeckResolution } from '../rules-engine'
import { maintenanceEnabled } from '../maintenance'
import {
  emptyPublicState,
  invitationCredentialMatches,
  normalizeGameRoomMetadata,
  projectGameRoomCompetitionContext,
  requireReadyRulesResult,
  systemBattleRecord,
  type GameRoomAccess,
  type GameRoomCapacity,
  type CommandReceipt,
  type CompletedReplayDraft,
  type GameRoomInvitation,
  type GameRoomMember,
  type GameRoomObserver,
  type GameRoomMetadata,
  type GameRoomRequest,
  type GameRoomResponse,
  type GameRecord,
  type OnlineGameAction,
  type PlayerDeckList,
  type PlayerNotification,
  type RulesGameSetup,
  type RulesEngineResult,
  type RulesReadyResult,
  type StoredGameEvent,
} from '../../shared/game-room'
import {
  competitionDeckEquals,
  competitionClockOperationsForTransition,
  competitionGameBindingEquals,
  competitionGameBindingRefForRequest,
  competitionGameNoShowCommandId,
  competitionRoundReadyCutoffMs,
  competitionRulesSetup,
  validateCompetitionGameBinding,
  validateCompetitionGameRequest,
  isCompetitionGameConclusion,
  CompetitionGameBindingError,
  type CompetitionClockDeliveryRequest,
  type CompetitionClockTransitionState,
  type CompetitionClockControlRequest,
  type CompetitionClockControlAck,
  type CompetitionClockOperation,
  type CompetitionGameBinding,
  type CompetitionGameBindingRef,
  type CompetitionGameNoShowDecision,
  type CompetitionGameNoShowRequest,
  type CompetitionGameAdmissionRequest,
  type CompetitionGameInitialStartEvidence,
  type CompetitionGameRevokeResult,
  type CompetitionRoomReadyAck,
  type CompetitionGameClockControlEligibilityRequest,
  type CompetitionReplayDraftRequest,
  type CompetitionGameCreateRequest,
  type CompetitionGameReadyRequest,
  type CompetitionGameReadyResult,
  type CompetitionGameRoomResult,
  type CompetitionGameStartRequest,
  isCompetitionGameNoShowRequest,
  isCompetitionGameNoShowDecision,
  isCompetitionGameReadyRequest,
  isCompetitionGameClockControlEligibilityRequest,
  isCompetitionReplayDraftRequest,
  isCompetitionGameAdmissionRequestShape,
  isCompetitionGameAdmissionRequest,
  isCompetitionGameInitialStartEvidence,
  isCompetitionGameBindingRef,
} from '../../shared/competition-game-room'
import {
  isCompetitionRoomInitialGameStartedRequest,
  isCompetitionRoomRoundPublicationReadValue,
  type CompetitionRoomInitialGameStartedRequest,
  type CompetitionRoomInitialGameStartedResult,
  type CompetitionRoomRoundPublicationReadRequest,
  type CompetitionRoomRoundPublicationReadResult,
  type CompetitionRoomTerminalIngestRequest,
  type CompetitionRoomTerminalIngestResult,
  competitionReplayId,
} from '../../shared/competition-room'
import {
  isExactCompetitionJson,
  isJsonObject,
  isJsonValue,
  type CompetitionOperationName,
} from '../../shared/competition'
import type { BattleRecord, PlayableAction, PlayerId, PublicGameState } from '../../app/types/fewfc'
import { replacePlayerLabels } from '../../shared/utils/battle-record-display'
import {
  callRuleModuleResolution,
  callRulesEngine as callRulesEngineResult,
} from '../rules-engine'
import { RulesEngineError } from '../rules-engine-error'
import {
  type CanonicalCompetitionDeliveryCommit,
  type OnlineCommandReadStorage,
  executePlayerCommand,
  OnlineCommandTransactionError,
} from './online-command-transaction'

async function callRulesEngine(request: unknown): Promise<RulesReadyResult> {
  return requireReadyRulesResult(await callRulesEngineResult(request))
}

function canonicalEventTypes(record: unknown[]): string[] {
  return record.flatMap((decision) => {
    if (!decision || typeof decision !== 'object' || !('events' in decision)) return []
    const events = decision.events
    if (!Array.isArray(events)) return []
    return events.flatMap((event) => (
      event && typeof event === 'object' ? Object.keys(event) : []
    ))
  })
}

interface GameRoomEnv {
  DB?: D1Database
  APP_ENV?: string
  MAINTENANCE_MODE?: string
  PLAYER_NOTIFICATIONS: DurableObjectNamespace
  COMPETITION_ROOM?: CompetitionRoomNamespace
}

interface CompetitionRoomNamespace {
  idFromName(name: string): DurableObjectId
  get(id: DurableObjectId): {
    ingestInitialGameStarted(
      request: CompetitionRoomInitialGameStartedRequest,
    ): Promise<CompetitionRoomInitialGameStartedResult>
    executeMatch?(
      request: import('../../shared/competition-room').CompetitionRoomMatchExecuteRequest,
    ): Promise<CompetitionRoomTerminalIngestResult>
    ingestGameTerminal(
      request: CompetitionRoomTerminalIngestRequest,
    ): Promise<CompetitionRoomTerminalIngestResult>
    readRoundPublication(
      request: CompetitionRoomRoundPublicationReadRequest,
    ): Promise<CompetitionRoomRoundPublicationReadResult>
  }
}

/** 沿原生 storage 契約；transaction callback 是不具巢狀交易能力的 DurableObjectTransaction。 */
type GameRoomAtomicStorage = Pick<DurableObjectStorage,
  'get' | 'list' | 'put' | 'delete' | 'setAlarm' | 'deleteAlarm' | 'transaction'
>

/** storage 與 transaction 都能協調 alarm；此 helper 不要求讀寫或開啟交易能力。 */
type GameRoomAlarmStorage = Pick<DurableObjectTransaction, 'get' | 'setAlarm' | 'deleteAlarm'>

interface CompetitionDeliveryQueue {
  schemaVersion: 1
  revisionCursor: number
  entries: CompetitionDeliveryEntry[]
  receipts: CompetitionDeliveryReceipt[]
  pendingTimeout: CompetitionPendingTimeout | null
  nextClockDeadlineMs?: number | null
  terminalTimeout?: CompetitionPendingTimeout['timeoutEvidence'] | null
  refereePaused?: boolean
  clockControlReceipts?: CompetitionClockControlReceipt[]
}

interface CompetitionClockControlReceipt {
  request: CompetitionClockControlRequest
  ack: CompetitionClockControlAck
}

interface CompetitionDeliveryReceipt {
  schemaVersion: 1
  kind: 'competitionClockEnqueue'
  requestCommandId: string
  commandId: string
  fingerprint: string
}

type CompetitionDeliveryEntry = CompetitionInitialGameStartedEntry | CompetitionClockDeliveryEntry | CompetitionTerminalDeliveryEntry

/** 六欄請求完整持久化；重試不重取 sample、不改 expected revision 或首局證據。 */
interface CompetitionInitialGameStartedEntry extends CompetitionRoomInitialGameStartedRequest {
  schemaVersion: 1
  kind: 'competitionInitialGameStarted'
  attempts: number
  nextAttemptAt: number
}

interface CompetitionClockDeliveryEntry {
  schemaVersion: 1
  kind: 'competitionClockMutation'
  competitionId: string
  matchId: string
  callerUserId: string
  commandId: string
  trustedTimeMs: number
  operation: CompetitionClockOperation
  attempts: number
  nextAttemptAt: number
}

interface CompetitionTerminalDeliveryEntry {
  schemaVersion: 1
  kind: 'competitionGameTerminal'
  competitionId: string
  matchId: string
  gameNumber: number
  gameInstanceId: string
  commandId: string
  submissionId: string
  gameRecordId: string
  rulingRecordId: string
  callerUserId: string
  startKind: CompetitionGameBinding['startKind']
  firstPlayer: string
  trustedTimeMs: number
  terminalEvidence: {
    type: 'canonicalGameEnded'
    conclusion: NonNullable<import('../../app/types/fewfc').PublicGameState['gameConclusion']>
  } | Extract<import('../../shared/competition').GameTerminalEvidence, { type: 'timeout' }>
  attempts: number
  nextAttemptAt: number
}

interface CompetitionPendingTimeout {
  schemaVersion: 1
  kind: 'competitionPendingTimeout'
  competitionId: string
  matchId: string
  commandId: string
  timeoutEvidence: Extract<import('../../shared/competition').GameTerminalEvidence, { type: 'timeout' }>
}

interface SocketAttachment {
  userId: string
  i?: string
}

interface PreparedCompetitionGameStart {
  request: CompetitionGameStartRequest
  binding: CompetitionGameBinding
  setup: ReturnType<typeof competitionRulesSetup>
  roomName: string
  admission: CompetitionGameAdmission
}

interface CompetitionGameAdmission {
  schemaVersion: 1
  bindingRef: CompetitionGameBindingRef
  bindingFingerprint: string | null
  status: 'unreleased' | 'released' | 'revoked' | 'noShow' | 'started'
  version: number
  releaseCommandId: string | null
  startRetryAtMs: number | null
  start: { gameInstanceId: string; startSequence: number; startedAtMs: number } | null
  receipts: Array<{
    kind: 'release' | 'revoke'
    commandId: string
    fingerprint: string
    result: true | CompetitionGameRevokeResult
  }>
}

interface CompetitionRoomReadyNoticeRecipient {
  userId: string
  notificationId: string
  createdAt: string
  sent: boolean
}

interface CompetitionRoomReadyNoticeOutbox {
  schemaVersion: 1
  bindingRef: CompetitionGameBindingRef
  recipients: CompetitionRoomReadyNoticeRecipient[]
}

interface CompetitionGameNoShowSnapshot {
  schemaVersion: 1
  bindingRef: CompetitionGameBindingRef
  commandId: string
  deadlineMs: number
  decision: CompetitionGameNoShowDecision
}

const competitionAdmissionKey = 'competitionGameAdmission'
const competitionInitialStartEvidenceKey = 'competitionInitialStartEvidence'
const competitionNoShowDecisionKey = 'competitionNoShowDecision'
const competitionRoomReadyNoticeOutboxKey = 'competitionRoomReadyNoticeOutbox'

interface NotificationEnvelope {
  targetUserId: string
  notification: PlayerNotification
}

type RulesEngineAction =
  | OnlineGameAction
  | { type: 'startDevelopmentScenario'; player: PlayerId; scenario: string }
  | { type: 'developmentScenarioAction'; player: PlayerId; scenario: string }
  | { type: 'prepareDevelopmentScenario'; player: PlayerId; scenario: string }
  | {
      type: 'trustedRandomHandCandidates'
      player: PlayerId
      candidateAction: OnlineGameAction
    }
  | { type: 'resolveRandomness'; requestId: string; shuffledOrder: number[] }

export class GameRoom extends DurableObject<GameRoomEnv> {
  private replySocketId: string | undefined
  private directoryUpdates: Promise<void> = Promise.resolve()
  private readonly socketHandler = durableWebSocketHandler({
    open: peer => { attachRealtimeIdentity(durableSocket(this.ctx, peer), peer.request) },
    message: async (peer, message) => {
      const wire = message.text()
      if (wire === 'ping') { peer.send('pong'); return }
      let requestId = 'invalid'
      try {
        const raw = wire.length <= 65_536 ? JSON.parse(wire) : undefined
        if (typeof raw?.requestId === 'string' && raw.requestId.length <= 200) requestId = raw.requestId
        const request = parseSocketRequest(raw, 'room')
        if (!request) throw { statusCode: 400, code: 'invalidRequest', message: '無效的連線請求。' }
        requestId = request.requestId
        if (!this.env.DB) throw new Error('Missing database binding')
        const actor = await requireRealtimeIdentity(durableSocket(this.ctx, peer), this.env.DB)
        if (request.operation !== 'state' && request.operation !== 'clock' && maintenanceEnabled(this.env.MAINTENANCE_MODE)) {
          throw { statusCode: 503, code: 'maintenanceMode', message: '系統維護中，請稍後再試。' }
        }
        if (request.operation === 'clock') {
          const binding = await this.competitionBinding()
          if (!binding) throw competitionGameReadyError('notCompetitionRoom')
          const data = await readCompetitionMatchClock(realtimeEvent(this.env), buildCompetitionMatchClockReadRequest(
            binding.competitionId, binding.matchId, actor.userId, serverCompetitionMatchClockTime(),
          ))
          peer.send(JSON.stringify({ type: 'result', requestId, ok: true, data }))
          return
        }
        if (request.operation === 'competitionReady') {
          const result = await this.setCompetitionReady({ actorUserId: actor.userId, ready: request.input.ready })
          if (!result.ok) throw competitionGameReadyError(result.code)
          peer.send(JSON.stringify({ type: 'result', requestId, ok: true, data: { ready: result.ready } }))
          return
        }
        let body: GameRoomRequest
        switch (request.operation) {
          case 'state': body = { type: 'getState', actorUserId: actor.userId }; break
          case 'command': body = { ...request.input, type: 'submitCommand', actorUserId: actor.userId }; break
          case 'ready': case 'start': {
            const candidate = await customDeckForUser(realtimeEvent(this.env), actor.userId)
            const resolved = await callPersonalDeckResolution(actor.userId, candidate)
            const deckList = { name: resolved.effective.name, cards: resolved.effective.cards }
            body = request.operation === 'ready'
              ? { type: 'toggleReady', actorUserId: actor.userId, deckList, ready: request.input.ready }
              : { type: 'startGame', actorUserId: actor.userId, deckList }
            break
          }
          case 'rules': body = { type: 'updateRuleModules', actorUserId: actor.userId, ...request.input }; break
          case 'remove': body = { type: 'removePlayer', actorUserId: actor.userId, targetUserId: request.input.userId }; break
          case 'leave': body = { type: 'leaveGame', actorUserId: actor.userId }; break
          case 'dissolve': body = { type: 'dissolveGame', actorUserId: actor.userId }; break
          case 'reset': body = { type: 'resetGame', actorUserId: actor.userId }; break
        }
        // 共用現有權限、賽事入口與序列化執行；不經過對外 HTTP 或第二次提交。
        const response = await this.fetch(new Request('https://game-room.internal/', {
          method: 'POST', headers: { 'content-type': 'application/json', 'x-fewfc-reply-socket': peer.id },
          body: JSON.stringify(body),
        }))
        const result = await socketResponse(requestId, response)
        peer.send(JSON.stringify(result))
        if (result.ok && result.data && typeof result.data === 'object' && 'metadata' in result.data) {
          // 回覆不等待大廳索引，但同房索引依序寫入，避免快速棄牌讓舊狀態最後覆寫。
          this.directoryUpdates = this.directoryUpdates
            .then(() => updatePublicRoom(realtimeEvent(this.env), result.data as GameRoomResponse))
            .catch(error => console.error('room directory update failed', error))
          this.ctx.waitUntil(this.directoryUpdates)
        }
        if (result.ok && (request.operation === 'leave' || request.operation === 'dissolve')) peer.close(1000, 'room closed')
      } catch (error) {
        peer.send(JSON.stringify(socketFailure(requestId, error)))
        if ((error as { statusCode?: number })?.statusCode === 401) peer.close(4001, 'session expired')
      }
    },
  })
  private operationTail: Promise<void> = Promise.resolve()
  private competitionDeliveryInFlight: Promise<void> | null = null
  private competitionReadyNoticeInFlight = false

  /** 純伺服器資格探測只回 primitive，不提供房間或牌組資料。 */
  async getCompetitionClockControlEligibility(
    request: CompetitionGameClockControlEligibilityRequest,
  ): Promise<boolean> {
    try {
      if (!isCompetitionGameClockControlEligibilityRequest(request)) return false
      // 單次 get(keys) 是原子／隔離快照；不等待會回呼 CompetitionRoom 的 serialized tail。
      const stored = await this.ctx.storage.get<unknown>([
        'metadata', 'competitionBinding', 'gameRecord', this.competitionDeliveryQueueKey(),
        competitionAdmissionKey,
      ])
      const metadata = stored.get('metadata')
      const binding = stored.get('competitionBinding')
      const record = stored.get('gameRecord')
      const queue = stored.get(this.competitionDeliveryQueueKey())
      const admission = stored.get(competitionAdmissionKey)
      if (![metadata, binding, record, queue].every(value => isCompetitionSnapshotData(value))) return false
      validateCompetitionGameBinding(binding)
      if (!hasCompetitionReadyMetadataShape(metadata)
        || !competitionAdmissionMatchesBinding(admission, metadata, binding)
        || admission.status !== 'started'
        || !this.competitionMembersMatchBinding(metadata, binding)
        || metadata.gameId !== request.gameId || binding.competitionId !== request.competitionId
        || binding.matchId !== request.matchId || binding.gameNumber !== request.gameNumber
        || metadata.status !== 'Active'
        || !hasCompetitionSnapshotKeys(record, [
          'schemaVersion', 'gameInstanceId', 'sequence', 'firstPlayer', 'deckSeed', 'setup', 'rulesRecord',
        ], ['finishedAt'])
        || !this.isCompleteCompetitionGameRecord(record, metadata, binding)
        || admission.start === null || record.gameInstanceId !== admission.start.gameInstanceId
        || record.sequence < admission.start.startSequence
        || !isSafeNonNegativeInteger(record.sequence) || record.sequence < 1
        || !record.rulesRecord.every(decision => isRecord(decision)
          && hasCompetitionSnapshotKeys(decision, ['source', 'events']) && Array.isArray(decision.events))
        || record.finishedAt !== undefined || canonicalEventTypes(record.rulesRecord).includes('GameEnded')
        || !isCompetitionDeliveryQueue(queue)
        || !competitionQueueMatchesBinding(queue, binding, record.gameInstanceId)
        || queue.revisionCursor < binding.expectedMatchRevision
        || queue.clockControlReceipts?.some(receipt => (
          receipt.request.competitionId !== binding.competitionId || receipt.request.matchId !== binding.matchId
            || receipt.request.gameId !== metadata.gameId || receipt.request.gameNumber !== binding.gameNumber
            || receipt.request.matchRevision > queue.revisionCursor
        ))
        || queue.pendingTimeout !== null || queue.terminalTimeout
        || queue.entries.some(entry => entry.kind === 'competitionGameTerminal'
          || (entry.kind === 'competitionClockMutation' && entry.operation.matchId !== entry.matchId)
          || (entry.kind === 'competitionInitialGameStarted'
            && !initialEvidenceMatchesAdmission(entry.evidence, admission)))) return false
      return true
    } catch {
      return false
    }
  }

  /** 賽事已授權公開後，從已完成的競賽局建立可重試的唯讀重播草稿。 */
  async getCompetitionReplayDraft(
    request: CompetitionReplayDraftRequest,
  ): Promise<CompletedReplayDraft | null> {
    try {
      if (!isCompetitionReplayDraftRequest(request)) return null
      // 先用 admission 找到事件鍵，再用同一批 get 讀取最終 admission 與該事件，
      // 避免把不同版本的 room state 拼成可公開的 replay draft。
      const admissionHint = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
      const hintedStart = isRecord(admissionHint) && isRecord(admissionHint.start)
        ? admissionHint.start
        : null
      const hintedSequence = hintedStart && isSafeNonNegativeInteger(hintedStart.startSequence)
        && hintedStart.startSequence > 0
        ? hintedStart.startSequence
        : null
      const snapshotKeys = [
        'metadata', 'competitionBinding', 'gameRecord', competitionAdmissionKey,
        ...(hintedSequence === null ? [] : [this.eventKey(hintedSequence)]),
      ]
      const current = await this.ctx.storage.get<unknown>(snapshotKeys)
      const metadata = current.get('metadata')
      const binding = current.get('competitionBinding')
      const record = current.get('gameRecord')
      const admission = current.get(competitionAdmissionKey)
      if (!isCompetitionSnapshotData(metadata) || !hasCompetitionReadyMetadataShape(metadata)
        || !competitionBindingIsValid(binding) || !isCompetitionSnapshotData(record)
        || metadata.status !== 'Finished' || metadata.gameId !== request.gameId
        || binding.competitionId !== request.competitionId || !this.competitionMembersMatchBinding(metadata, binding)
        || !competitionAdmissionMatchesBinding(admission, metadata, binding) || admission.status !== 'started'
        || !this.hasDurableCompetitionStart(current, metadata, binding, admission)
        || !this.isCompleteCompetitionGameRecord(record, metadata, binding)
        || !isSafeNonNegativeInteger(record.sequence) || record.sequence < 1
        || typeof record.finishedAt !== 'string' || !record.finishedAt.trim()
        || !record.rulesRecord.every(decision => isRecord(decision)
          && hasCompetitionSnapshotKeys(decision, ['source', 'events']) && Array.isArray(decision.events))) {
        return null
      }
      const finished = await this.callReadyRules({ type: 'refresh' }, 'observer', record)
      const conclusion = finished.state.gameConclusion
      if (finished.state.status !== 'Finished' || !isCompetitionGameConclusion(conclusion)
        || !conclusionUsesSetupTeams(conclusion, record)
        || !hasCanonicalTerminalRecord(record.rulesRecord, conclusion)) return null

      return {
        schemaVersion: 1,
        replayId: competitionReplayId(request.competitionId, metadata.gameId),
        sourceGameId: metadata.gameId,
        finishedAt: record.finishedAt,
        setup: record.setup,
        record: record.rulesRecord,
        players: metadata.members.map(member => ({ player: member.player, displayName: member.displayName })),
        originalUserIds: metadata.members.map(member => member.userId),
        roomName: metadata.name,
        result: { status: finished.state.status, hp: finished.state.hp },
        firstPlayer: record.firstPlayer,
      }
    } catch {
      return null
    }
  }

  /** 可信賽事編排層專用 RPC；一般玩家只能走 fetch request。 */
  async createCompetitionGame(
    request: CompetitionGameCreateRequest,
  ): Promise<CompetitionRoomReadyAck> {
    // 必須先取得完整私密 owned snapshot，hash／engine await 後不再讀 caller 可變資料。
    let owned: CompetitionGameCreateRequest
    try {
      validateCompetitionGameRequest(request)
      owned = structuredClone(request)
      validateCompetitionGameRequest(owned)
    } catch {
      throw new CompetitionGameBindingError('invalidCompetitionBinding')
    }
    const bindingRef = await competitionGameBindingRefForRequest(owned)
    await this.validateCompetitionRules(owned.binding)
    return await this.serialized(async () => {
      const binding = owned.binding
      const capacity = binding.players.length as GameRoomCapacity
      const now = new Date().toISOString()
      const metadata: GameRoomMetadata = {
        schemaVersion: 5,
        gameId: owned.gameId,
        name: owned.gameId,
        access: 'private',
        capacity,
        ruleset: 'fewfc-base',
        ruleVersion: binding.ruleVersion,
        enabledRuleModules: [...binding.enabledRuleModules],
        players: binding.players.map(player => player.player),
        members: binding.players.map((player, index) => ({
          userId: player.userId,
          displayName: player.userId,
          player: player.player,
          ready: false,
          connected: false,
          owner: index === 0,
        })),
        observers: [],
        status: 'Waiting',
        createdAt: now,
        updatedAt: now,
      }
      const initialEvent: StoredGameEvent = {
        sequence: 1,
        type: 'GameCreated',
        payload: {
          capacity,
          access: metadata.access,
          ruleset: metadata.ruleset,
          ruleVersion: metadata.ruleVersion,
          enabledRuleModules: [...metadata.enabledRuleModules],
        },
        createdAt: now,
      }

      await this.ctx.storage.transaction(async storage => {
        // 這裡只做本地 I/O；晚 create 不得穿過先持久化的 pristine tombstone。
        const current = await storage.list<unknown>()
        if (current.size !== 0) {
          const existing = current.get('metadata')
          const storedBinding = current.get('competitionBinding')
          const admission = current.get(competitionAdmissionKey)
          if (!isCompetitionSnapshotData(existing)
            || !hasCompetitionReadyMetadataShape(existing)
            || !competitionBindingIsValid(storedBinding)
            || !competitionGameBindingEquals(storedBinding, binding)
            || !competitionAdmissionMatchesBinding(admission, existing, storedBinding)
            || !sameCompetitionBindingRef(admission.bindingRef, bindingRef)
            || admission.status !== 'unreleased' || existing.status !== 'Waiting'
            || existing.gameInstanceId !== undefined || current.has('gameRecord')
            || !this.competitionMembersMatchBinding(existing, storedBinding)
            || !isPristineCompetitionWaitingQueue(current.get(this.competitionDeliveryQueueKey()), binding)) {
            throw new CompetitionGameBindingError('competitionBindingConflict')
          }
          return
        }
        await storage.put({
          metadata,
          competitionBinding: binding,
          competitionDeliveryQueue: emptyCompetitionDeliveryQueue(binding.expectedMatchRevision),
          [competitionAdmissionKey]: newCompetitionAdmission(bindingRef, binding),
          nextSequence: 2,
          [this.eventKey(initialEvent.sequence)]: initialEvent,
        })
      })
      return { gameId: owned.gameId, bindingDigest: bindingRef.bindingDigest, status: 'Waiting' }
    })
  }

  /** 父房的固定 release 決議只提交本地 permit；不等待自動開局或任何父房交付。 */
  async releaseCompetitionGame(request: CompetitionGameAdmissionRequest): Promise<boolean> {
    const released = await this.changeCompetitionAdmission(request, 'release') === true
    if (released) {
      this.queueCompetitionRoomReadyNotice()
      try {
        const metadata = await this.metadata()
        const binding = await this.competitionBinding()
        if (metadata && binding && await this.competitionCanAutoStart(metadata, binding)) {
          this.queueCompetitionAutoStart(metadata.gameId, binding)
        }
      } catch {
        // 授權與立即鬧鐘已持久化；低延遲啟動失敗仍由 durable alarm 重試。
      }
    }
    return released
  }

  /** 放行許可持久化後才提醒；通知失敗不會改變房間授權或賽程 ACK。 */
  private queueCompetitionRoomReadyNotice(): void {
    if (this.competitionReadyNoticeInFlight) return
    this.competitionReadyNoticeInFlight = true
    this.ctx.waitUntil((async () => {
      try {
        if (await this.ctx.storage.get<boolean>('competitionRoomReadyNoticeSent') === true) return
        const metadata = await this.metadata()
        const binding = await this.competitionBinding()
        const admission = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
        if (!metadata || !binding || !competitionBindingIsValid(binding)
          || !hasCompetitionReadyMetadataShape(metadata)
          || !competitionAdmissionMatchesBinding(admission, metadata, binding)
          || (admission.status !== 'released' && admission.status !== 'started')) return
        const noticeAtMs = binding.gameNumber === 1
          ? competitionRoundReadyCutoffMs(binding.roundStartMs)
          : Date.now()
        if (noticeAtMs === null) return
        if (Date.now() < noticeAtMs) {
          await this.serialized(async () => {
            await this.scheduleCompetitionAdmissionAlarm(
              this.ctx.storage, metadata, binding, admission,
              await this.ctx.storage.get<unknown>(this.competitionDeliveryQueueKey()),
            )
          })
          return
        }
        if (!await this.competitionRoundPlayerEntryAllowed(metadata.gameId, binding, admission)) return
        const outbox = await this.serialized(async () => {
          const currentMetadata = await this.metadata()
          const currentBinding = await this.competitionBinding()
          const currentAdmission = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
          if (!currentMetadata || !currentBinding || !isCompetitionGameAdmission(currentAdmission)
            || !competitionAdmissionMatchesBinding(currentAdmission, currentMetadata, currentBinding)
            || (currentAdmission.status !== 'released' && currentAdmission.status !== 'started')
            || !await this.competitionRoundPlayerEntryAllowed(
              currentMetadata.gameId, currentBinding, currentAdmission,
            )) return null

          const existing = await this.ctx.storage.get<unknown>(competitionRoomReadyNoticeOutboxKey)
          if (existing !== undefined) {
            if (!isCompetitionRoomReadyNoticeOutbox(existing)
              || stableDeliveryFingerprint(existing.bindingRef) !== stableDeliveryFingerprint(currentAdmission.bindingRef)) {
              throw new Error('competition room ready notice outbox is invalid')
            }
            return structuredClone(existing)
          }

          const recipients = [...currentMetadata.members, ...currentMetadata.observers]
            .map(member => member.userId)
            .filter((userId, index, all) => all.indexOf(userId) === index)
            .map(userId => ({
              userId,
              notificationId: crypto.randomUUID(),
              createdAt: new Date().toISOString(),
              sent: false,
            }))
          const created: CompetitionRoomReadyNoticeOutbox = {
            schemaVersion: 1,
            bindingRef: structuredClone(currentAdmission.bindingRef),
            recipients,
          }
          await this.ctx.storage.put(competitionRoomReadyNoticeOutboxKey, created)
          return created
        })
        if (!outbox) return

        for (const recipient of outbox.recipients) {
          if (recipient.sent) continue
          const maySend = await this.serialized(async () => {
            const currentMetadata = await this.metadata()
            const currentBinding = await this.competitionBinding()
            const currentAdmission = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
            const storedOutbox = await this.ctx.storage.get<unknown>(competitionRoomReadyNoticeOutboxKey)
            return Boolean(currentMetadata && currentBinding && isCompetitionGameAdmission(currentAdmission)
              && competitionAdmissionMatchesBinding(currentAdmission, currentMetadata, currentBinding)
              && (currentAdmission.status === 'released' || currentAdmission.status === 'started')
              && isCompetitionRoomReadyNoticeOutbox(storedOutbox)
              && stableDeliveryFingerprint(storedOutbox.bindingRef)
                === stableDeliveryFingerprint(currentAdmission.bindingRef)
              && storedOutbox.recipients.some(item => item.userId === recipient.userId
                && item.notificationId === recipient.notificationId && !item.sent)
              && await this.competitionRoundPlayerEntryAllowed(
                currentMetadata.gameId, currentBinding, currentAdmission,
              ))
          })
          if (!maySend) return

          await this.sendNotification(recipient.userId, {
            kind: 'roomChanged',
            message: '賽事固定房間已開放，可從賽事頁進入。',
          }, {
            id: recipient.notificationId,
            gameId: metadata.gameId,
            createdAt: recipient.createdAt,
          })
          await this.serialized(async () => {
            const storedOutbox = await this.ctx.storage.get<unknown>(competitionRoomReadyNoticeOutboxKey)
            if (!isCompetitionRoomReadyNoticeOutbox(storedOutbox)
              || stableDeliveryFingerprint(storedOutbox.bindingRef)
                !== stableDeliveryFingerprint(outbox.bindingRef)) {
              throw new Error('competition room ready notice outbox changed during delivery')
            }
            const current = storedOutbox.recipients.find(item => item.userId === recipient.userId
              && item.notificationId === recipient.notificationId)
            if (!current) throw new Error('competition room ready notice recipient is missing')
            if (!current.sent) {
              current.sent = true
              await this.ctx.storage.put(competitionRoomReadyNoticeOutboxKey, storedOutbox)
            }
          })
        }

        await this.serialized(async () => {
          const currentMetadata = await this.metadata()
          const currentBinding = await this.competitionBinding()
          const currentAdmission = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
          const storedOutbox = await this.ctx.storage.get<unknown>(competitionRoomReadyNoticeOutboxKey)
          if (!currentMetadata || !currentBinding || !isCompetitionGameAdmission(currentAdmission)
            || !competitionAdmissionMatchesBinding(currentAdmission, currentMetadata, currentBinding)
            || (currentAdmission.status !== 'released' && currentAdmission.status !== 'started')
            || !isCompetitionRoomReadyNoticeOutbox(storedOutbox)
            || stableDeliveryFingerprint(storedOutbox.bindingRef)
              !== stableDeliveryFingerprint(currentAdmission.bindingRef)
            || !storedOutbox.recipients.every(recipient => recipient.sent)
            || !await this.competitionRoundPlayerEntryAllowed(
              currentMetadata.gameId, currentBinding, currentAdmission,
            )
            || await this.ctx.storage.get<boolean>('competitionRoomReadyNoticeSent') === true) return
          await this.ctx.storage.put('competitionRoomReadyNoticeSent', true)
          await this.scheduleCompetitionAdmissionAlarm(
            this.ctx.storage, currentMetadata, currentBinding, currentAdmission,
            await this.ctx.storage.get<unknown>(this.competitionDeliveryQueueKey()),
          )
        })
      } catch {
        // 通知失敗以短 alarm 重試；玩家仍可從賽事頁進入。
        try {
          await this.serialized(async () => {
            const metadata = await this.metadata()
            const binding = await this.competitionBinding()
            const admission = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
            if (metadata && binding && isCompetitionGameAdmission(admission)) {
              await this.scheduleCompetitionAdmissionAlarm(
                this.ctx.storage, metadata, binding, admission,
                await this.ctx.storage.get<unknown>(this.competitionDeliveryQueueKey()),
              )
            }
          })
        } catch {
          // 下一次玩家請求或房間工作會重新排程共用 alarm。
        }
      } finally {
        this.competitionReadyNoticeInFlight = false
      }
    })())
  }

  /** 固定首局入口等父房整輪公告；子房本地 release permit 不足以提早開門。 */
  private async competitionRoundPlayerEntryAllowed(
    gameId: string,
    binding: CompetitionGameBinding,
    admission: CompetitionGameAdmission,
  ): Promise<boolean> {
    if (admission.status !== 'released' && admission.status !== 'started') return false
    if (binding.gameNumber !== 1) return true
    const noticeAtMs = competitionRoundReadyCutoffMs(binding.roundStartMs)
    if (noticeAtMs === null || Date.now() < noticeAtMs) return false
    const namespace = this.env.COMPETITION_ROOM
    if (!namespace) return false
    const request: CompetitionRoomRoundPublicationReadRequest = {
      competitionId: binding.competitionId,
      stageId: binding.roundRef.stageId,
      roundNumber: binding.roundRef.roundNumber,
      callerUserId: binding.organizerUserId,
    }
    let result: unknown
    try {
      result = structuredClone(await namespace.get(namespace.idFromName(binding.competitionId))
        .readRoundPublication(request))
    } catch {
      return false
    }
    if (!isExactCompetitionJson(result) || !isJsonObject(result)
      || !hasExactKeys(result, ['ok', 'value']) || result.ok !== true
      || !isCompetitionRoomRoundPublicationReadValue(result.value, request)) return false
    const publication = result.value.publication
    const room = publication?.rooms.find(entry => entry.gameId === gameId
      && entry.matchId === binding.matchId)
    return publication !== null && publication.status === 'released' && !publication.requiresReferee
      && publication.releaseDecisionId === admission.releaseCommandId
      && publication.roundStartMs === binding.roundStartMs
      && stableDeliveryFingerprint(publication.roundRef) === stableDeliveryFingerprint(binding.roundRef)
      && room !== undefined && room.ackConfirmedAtMs !== null && room.ackConfirmedAtMs <= noticeAtMs
  }

  /** 撤銷與 start 共用同一 mutator／交易提交點；primitive ACK 不攜帶私密資料。 */
  async revokeCompetitionGame(request: CompetitionGameAdmissionRequest): Promise<CompetitionGameRevokeResult> {
    const result = await this.changeCompetitionAdmission(request, 'revoke')
    return result === true ? 'invalid' : result
  }

  private async changeCompetitionAdmission(
    request: CompetitionGameAdmissionRequest,
    kind: 'release' | 'revoke',
  ): Promise<true | CompetitionGameRevokeResult> {
    let owned: CompetitionGameAdmissionRequest
    try {
      // 首次命令可信來源是 parent namespace capability；shape 本身不證歷史授權。
      if (!isCompetitionGameAdmissionRequestShape(request)) return 'invalid'
      owned = structuredClone(request)
    } catch {
      return 'invalid'
    }
    return await this.serialized(async () => {
      const storage: GameRoomAtomicStorage = this.ctx.storage
      return await storage.transaction<true | CompetitionGameRevokeResult>(async transaction => {
        const current = await transaction.list<unknown>()
        let admission = current.get(competitionAdmissionKey)
        if (kind === 'revoke' && current.size === 0) {
          const tombstone: CompetitionGameAdmission = {
            ...newCompetitionAdmission(owned.bindingRef, null), status: 'revoked',
            receipts: [{ kind, commandId: owned.commandId, fingerprint: stableDeliveryFingerprint(owned), result: 'revoked' }],
          }
          await transaction.put({ [competitionAdmissionKey]: tombstone })
          await this.scheduleCompetitionAdmissionAlarm(transaction, undefined, undefined, tombstone, undefined)
          return 'revoked'
        }
        if (!isCompetitionGameAdmission(admission)
          || !sameCompetitionBindingRef(owned.bindingRef, admission.bindingRef)) return 'invalid'
        const fingerprint = stableDeliveryFingerprint(owned)
        const receipt = admission.receipts.find(item => item.kind === kind && item.commandId === owned.commandId)
        if (receipt && (!isCompetitionGameAdmissionRequest(owned, {
          bindingRef: admission.bindingRef, commandId: receipt.commandId,
        }) || receipt.fingerprint !== fingerprint)) return 'invalid'
        const metadata = current.get('metadata')
        const binding = current.get('competitionBinding')
        const record = current.get('gameRecord')
        const queue = current.get(this.competitionDeliveryQueueKey())
        const tombstoneOnly = current.size === 0
          || (current.size === 1 && current.has(competitionAdmissionKey)
            && admission.status === 'revoked' && admission.bindingFingerprint === null)
        if (!tombstoneOnly) {
          if (!competitionBindingIsValid(binding)
            || !isCompetitionSnapshotData(metadata) || !hasCompetitionReadyMetadataShape(metadata)
            || !this.competitionMembersMatchBinding(metadata, binding)
            || !competitionAdmissionMatchesBinding(admission, metadata, binding)) return 'invalid'
          if (admission.status === 'started') {
            if (!this.hasDurableCompetitionStart(current, metadata, binding, admission)) return 'invalid'
          } else if (metadata.status !== 'Waiting' || metadata.gameInstanceId !== undefined
            || record !== undefined || !isPristineCompetitionWaitingQueue(queue, binding)) return 'invalid'
        }
        if (receipt) return receipt.result
        if (admission.version >= Number.MAX_SAFE_INTEGER) return 'invalid'
        if (kind === 'release' && (tombstoneOnly || admission.status === 'revoked'
          || (admission.releaseCommandId !== null && admission.releaseCommandId !== owned.commandId))) return 'invalid'
        const result = kind === 'release' ? true
          : admission.status === 'started' ? 'started' : 'revoked'
        // current fullRef／完整 binding 驗證後，首次 ID 同交易捕捉；retry 只認持久 receipt。
        const next: CompetitionGameAdmission = {
          ...admission,
          status: kind === 'release'
            ? admission.status === 'unreleased' ? 'released' : admission.status
            : admission.status === 'started' ? 'started' : 'revoked',
          version: admission.version + 1,
          releaseCommandId: kind === 'release' ? owned.commandId : admission.releaseCommandId,
          startRetryAtMs: null,
          receipts: [...admission.receipts, { kind, commandId: owned.commandId, fingerprint, result }],
        }
        await transaction.put({ [competitionAdmissionKey]: next })
        // 真 started 的歷史 ACK 不改 delivery／deadline alarm。
        if (next.status !== 'started') {
          await this.scheduleCompetitionAdmissionAlarm(transaction, metadata, binding, next, queue)
        }
        return result
      })
    })
  }

  /** 可信賽事編排層專用 RPC；不接受一般玩家的 startGame request。 */
  async startCompetitionGame(
    request: CompetitionGameStartRequest,
  ): Promise<CompetitionGameRoomResult> {
    const preparation = await this.serialized(() => this.prepareCompetitionGameStart(request))
    if ('result' in preparation) return preparation.result

    await this.validateCompetitionRules(preparation.binding)
    const rules = await callRulesEngine({
      action: { type: 'start' },
      viewer: 'observer',
      setup: preparation.setup,
      deckSeed: preparation.binding.deckSeed,
    })
    return await this.serialized(() => this.commitCompetitionGameStart(preparation, rules))
  }

  /** 截止時與 GameStarted 共用 serial lane；只記房間 admission，不造遊戲事件。 */
  async adjudicateCompetitionNoShow(
    request: CompetitionGameNoShowRequest,
  ): Promise<CompetitionGameNoShowDecision> {
    return this.serialized(() => this.adjudicateCompetitionNoShowInternal(request))
  }

  /** 固定賽事成員準備 RPC；只讀 binding 的鎖定牌組，不接受或查詢帳號牌組。 */
  async setCompetitionReady(
    request: CompetitionGameReadyRequest,
  ): Promise<CompetitionGameReadyResult> {
    return await this.serialized<CompetitionGameReadyResult>(async () => {
      if (!isCompetitionGameReadyRequest(request)) {
        return { ok: false, code: 'invalidRequest' }
      }

      const storedBinding = await this.ctx.storage.get<unknown>('competitionBinding')
      if (storedBinding === undefined) {
        return { ok: false, code: 'notCompetitionRoom' }
      }

      let binding: CompetitionGameBinding
      try {
        validateCompetitionGameBinding(storedBinding)
        binding = storedBinding
      } catch {
        if (bindingMissingActorDeck(storedBinding, request.actorUserId)) {
          return { ok: false, code: 'lockedDeckMissing' }
        }
        return { ok: false, code: 'invalidCompetitionRoom' }
      }

      const storedMetadata = await this.ctx.storage.get<unknown>('metadata')
      let admission = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
      if (!isCompetitionSnapshotData(storedMetadata)
        || !hasCompetitionReadyMetadataShape(storedMetadata)
        || !competitionAdmissionMatchesBinding(admission, storedMetadata, binding)
        || admission.status === 'revoked'
        || !this.competitionMembersMatchBinding(storedMetadata, binding)) {
        return { ok: false, code: 'invalidCompetitionRoom' }
      }

      let metadata: GameRoomMetadata
      try {
        // 競賽 raw metadata 已完整驗證；只取 owned 資料，不能經普通房 normalize 產生 own undefined。
        metadata = structuredClone(storedMetadata)
      } catch {
        return { ok: false, code: 'invalidCompetitionRoom' }
      }
      if (!metadata.members.every(member => typeof member.ready === 'boolean'
          && typeof member.connected === 'boolean')) {
        return { ok: false, code: 'invalidCompetitionRoom' }
      }

      const fixedPlayer = binding.players.find(player => player.userId === request.actorUserId)
      if (!fixedPlayer) {
        return { ok: false, code: 'fixedMemberRequired' }
      }
      const actor = metadata.members.find(member => member.userId === request.actorUserId)
      if (!actor) {
        return { ok: false, code: 'invalidCompetitionRoom' }
      }
      if (metadata.status === 'Active') {
        return { ok: false, code: 'gameAlreadyStarted' }
      }
      if (metadata.status === 'Finished') {
        return { ok: false, code: 'gameFinished' }
      }
      if (metadata.status !== 'Waiting') {
        return { ok: false, code: 'roomNotWaiting' }
      }
      if (binding.gameNumber === 1 && Date.now() >= binding.roundStartMs) {
        // T 已凍結為 ready 時，斷線者仍須重連並準備；noShow 判定不可逆。
        const decision = await this.adjudicateCompetitionNoShowInternal({
          bindingRef: admission.bindingRef,
          commandId: competitionGameNoShowCommandId(admission.bindingRef),
          deadlineMs: binding.roundStartMs,
        })
        if (decision.type !== 'ready') {
          return { ok: false, code: 'checkInDeadlinePassed' }
        }
        admission = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
        if (!competitionAdmissionMatchesBinding(admission, metadata, binding)
          || admission.status !== 'released') {
          return { ok: false, code: 'checkInDeadlinePassed' }
        }
      }
      if (!actor.connected) {
        return { ok: false, code: 'playerNotConnected' }
      }
      if (binding.enabledRuleModules.includes('personal-deck')
        && !binding.deckSnapshot.some(deck => deck.player === fixedPlayer.player)) {
        return { ok: false, code: 'lockedDeckMissing' }
      }
      if (actor.ready === request.ready) {
        return { ok: true, ready: actor.ready }
      }

      const updatedMetadata: GameRoomMetadata = {
        ...metadata,
        members: metadata.members.map(member => member.userId === request.actorUserId
          ? { ...member, ready: request.ready }
          : member),
        updatedAt: new Date().toISOString(),
      }
      await this.persistCompetitionWaitingMetadata(updatedMetadata, binding, {
        type: request.ready ? 'PlayerReady' : 'PlayerUnready',
        actor: fixedPlayer.player,
        payload: { player: fixedPlayer.player },
      })
      if (request.ready && await this.competitionCanAutoStart(updatedMetadata, binding)) {
        this.queueCompetitionAutoStart(updatedMetadata.gameId, binding)
      }
      this.ctx.waitUntil(this.afterRoomMutation(updatedMetadata))

      return { ok: true, ready: request.ready }
    })
  }

  private async adjudicateCompetitionNoShowInternal(
    request: CompetitionGameNoShowRequest,
  ): Promise<CompetitionGameNoShowDecision> {
    if (!isCompetitionGameNoShowRequest(request)) throw new Error('invalid competition no-show request')
    const current = await this.ctx.storage.list<unknown>()
    const binding = current.get('competitionBinding')
    const metadata = current.get('metadata')
    const admission = current.get(competitionAdmissionKey)
    if (!competitionBindingIsValid(binding) || !isCompetitionSnapshotData(metadata)
      || !hasCompetitionReadyMetadataShape(metadata)
      || !competitionAdmissionMatchesBinding(admission, metadata, binding)
      || !sameCompetitionBindingRef(admission.bindingRef, request.bindingRef)
      || request.bindingRef.gameId !== metadata.gameId
      || request.bindingRef.matchId !== binding.matchId || binding.gameNumber !== 1
      || request.bindingRef.roundStartMs !== binding.roundStartMs
      || request.commandId !== competitionGameNoShowCommandId(request.bindingRef)) {
      throw new Error('competition no-show binding is invalid')
    }

    const stored = current.get(competitionNoShowDecisionKey)
    if (stored !== undefined) {
      if (!isCompetitionGameNoShowSnapshot(stored)
        || !sameCompetitionBindingRef(stored.bindingRef, request.bindingRef)
        || stored.commandId !== request.commandId || stored.deadlineMs !== request.deadlineMs) {
        throw new Error('stored competition no-show decision is invalid')
      }
      return structuredClone(stored.decision)
    }

    const now = Date.now()
    if (!isSafeNonNegativeInteger(now) || now < request.deadlineMs) {
      throw new Error('competition no-show deadline has not arrived')
    }
    if (admission.status === 'started') {
      if (!this.hasDurableCompetitionStart(current, metadata, binding, admission) || !admission.start) {
        throw new Error('competition start evidence is invalid')
      }
      return { type: 'started', startedAtMs: admission.start.startedAtMs }
    }
    if (admission.status !== 'released' || metadata.status !== 'Waiting') {
      return { type: 'unavailable' }
    }
    if (!this.competitionMembersMatchBinding(metadata, binding)
      || metadata.members.length !== binding.players.length
      || !metadata.members.every(member => typeof member.connected === 'boolean'
        && typeof member.ready === 'boolean')) {
      throw new Error('competition no-show member snapshot is invalid')
    }

    const teams = [...new Set(binding.players.map(player => player.team))]
    const absentTeams = teams.filter(team => binding.players.some(player => player.team === team
      && !metadata.members.some(member => member.userId === player.userId
        && member.connected && member.ready)))
    const decision: CompetitionGameNoShowDecision = absentTeams.length > 0
      ? { type: 'noShow', absentTeams, sampledAtMs: now }
      : { type: 'ready', sampledAtMs: now }
    const snapshot: CompetitionGameNoShowSnapshot = {
      schemaVersion: 1,
      bindingRef: structuredClone(request.bindingRef),
      commandId: request.commandId,
      deadlineMs: request.deadlineMs,
      decision,
    }
    if (!isCompetitionGameNoShowSnapshot(snapshot)) throw new Error('competition no-show snapshot is invalid')
    const nextAdmission: CompetitionGameAdmission = decision.type === 'noShow'
      ? { ...admission, status: 'noShow', version: admission.version + 1, startRetryAtMs: null }
      : { ...admission, startRetryAtMs: now }
    if (decision.type === 'noShow' && admission.version >= Number.MAX_SAFE_INTEGER) {
      throw new Error('competition admission version overflow')
    }

    await this.ctx.storage.transaction(async storage => {
      const latest = await storage.list<unknown>()
      if (!competitionBindingIsValid(latest.get('competitionBinding'))
        || stableDeliveryFingerprint(latest.get('competitionBinding')) !== stableDeliveryFingerprint(binding)
        || stableDeliveryFingerprint(latest.get('metadata')) !== stableDeliveryFingerprint(metadata)
        || stableDeliveryFingerprint(latest.get(competitionAdmissionKey)) !== stableDeliveryFingerprint(admission)
        || latest.has(competitionNoShowDecisionKey)) {
        throw new Error('competition no-show state changed before commit')
      }
      await storage.put({
        [competitionNoShowDecisionKey]: snapshot,
        [competitionAdmissionKey]: nextAdmission,
      })
      await this.scheduleCompetitionAdmissionAlarm(
        storage, metadata, binding, nextAdmission, latest.get(this.competitionDeliveryQueueKey()),
      )
    })
    return structuredClone(decision)
  }

  private async prepareCompetitionGameStart(
    request: CompetitionGameStartRequest,
  ): Promise<PreparedCompetitionGameStart | { result: CompetitionGameRoomResult }> {
    validateCompetitionGameRequest(request)

    const metadata = await this.requireMetadata()
    const storedBinding = await this.competitionBinding()
    let admission = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
    if (!storedBinding
      || metadata.gameId !== request.gameId
      || !competitionAdmissionMatchesBinding(admission, metadata, storedBinding)
      || !competitionGameBindingEquals(storedBinding, request.binding)) {
      throw new CompetitionGameBindingError('competitionBindingConflict')
    }
    if (storedBinding.gameNumber === 1 && Date.now() >= storedBinding.roundStartMs) {
      await this.adjudicateCompetitionNoShowInternal({
        bindingRef: admission.bindingRef,
        commandId: competitionGameNoShowCommandId(admission.bindingRef),
        deadlineMs: storedBinding.roundStartMs,
      })
      admission = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
    }
    if (metadata.status === 'Active') {
      const current = await this.ctx.storage.list<unknown>()
      if (!this.hasDurableCompetitionStart(current, metadata, storedBinding, admission)) {
        throw new Error('competition game record is incomplete')
      }
      return { result: this.competitionResult(metadata, true) }
    }
    if (metadata.status !== 'Waiting') {
      throw new CompetitionGameBindingError('competitionBindingConflict')
    }
    if (!this.competitionStartAdmitted(metadata, storedBinding, admission, Date.now())) {
      throw new CompetitionGameBindingError('competitionGameNotReady')
    }

    return {
      request: structuredClone(request),
      binding: structuredClone(storedBinding),
      setup: competitionRulesSetup(storedBinding),
      roomName: metadata.name,
      admission: structuredClone(admission),
    }
  }

  private async commitCompetitionGameStart(
    preparation: PreparedCompetitionGameStart,
    rules: RulesEngineResult,
  ): Promise<CompetitionGameRoomResult> {
    validateCompetitionGameRequest(preparation.request)
    const metadata = await this.requireMetadata()
    const storedBinding = await this.competitionBinding()
    let admission = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
    if (!storedBinding
      || !competitionAdmissionMatchesBinding(admission, metadata, storedBinding)
      || metadata.gameId !== preparation.request.gameId
      || !competitionGameBindingEquals(storedBinding, preparation.binding)
      || !competitionGameBindingEquals(storedBinding, preparation.request.binding)) {
      throw new CompetitionGameBindingError('competitionBindingConflict')
    }
    if (storedBinding.gameNumber === 1 && Date.now() >= storedBinding.roundStartMs) {
      await this.adjudicateCompetitionNoShowInternal({
        bindingRef: admission.bindingRef,
        commandId: competitionGameNoShowCommandId(admission.bindingRef),
        deadlineMs: storedBinding.roundStartMs,
      })
      admission = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
    }
    if (metadata.status === 'Active') {
      const current = await this.ctx.storage.list<unknown>()
      if (!this.hasDurableCompetitionStart(current, metadata, storedBinding, admission)) {
        throw new Error('competition game record is incomplete')
      }
      return this.competitionResult(metadata, true)
    }
    if (metadata.status !== 'Waiting') {
      throw new CompetitionGameBindingError('competitionBindingConflict')
    }
    if (admission.version !== preparation.admission.version
      || !sameCompetitionBindingRef(admission.bindingRef, preparation.admission.bindingRef)
      || !this.competitionStartAdmitted(metadata, storedBinding, admission, Date.now())) {
      throw new CompetitionGameBindingError('competitionGameNotReady')
    }

    const startClockOperation = this.competitionStartClockOperation(rules.state, storedBinding)

    const setup = preparation.setup
    const gameInstanceId = crypto.randomUUID()
    const firstPlayer = storedBinding.firstPlayer
    const trustedTimeMs = Date.now()
    if (!this.competitionStartAdmitted(metadata, storedBinding, admission, trustedTimeMs)) {
      throw new CompetitionGameBindingError('competitionGameNotReady')
    }
    const sequence = await this.nextSequence()
    if (!isSafeNonNegativeInteger(sequence) || sequence >= Number.MAX_SAFE_INTEGER) {
      throw new CompetitionGameBindingError('competitionGameNotReady')
    }
    const snapshot: GameRecord = {
      schemaVersion: 7,
      gameInstanceId,
      sequence,
      firstPlayer,
      deckSeed: storedBinding.deckSeed,
      setup,
      rulesRecord: rules.record,
    }

    let startEntries: CompetitionDeliveryEntry[]
    let initialWindow: CompetitionGameInitialStartEvidence['initialWindow'] | undefined
    let initialStartIdentityPrefix: string | undefined
    if (storedBinding.gameNumber === 1) {
      if (admission.releaseCommandId === null) throw new CompetitionGameBindingError('competitionGameNotReady')
      initialWindow = startClockOperation.type === 'clockBeginPreparation'
        ? { type: 'preparation' as const }
        : startClockOperation.type === 'clockBeginAction'
          ? { type: 'action' as const, team: startClockOperation.team, turn: startClockOperation.turn }
          : undefined
      if (!initialWindow) throw new CompetitionGameBindingError('competitionGameNotReady')
      initialStartIdentityPrefix = await competitionInitialStartIdentityPrefix(
        storedBinding,
        {
          bindingRef: admission.bindingRef,
          releaseDecisionId: admission.releaseCommandId,
          gameInstanceId,
          startSequence: sequence,
          initialWindow,
        },
      )
      startEntries = []
    } else {
      const startOperations: CompetitionClockOperation[] = [{
        type: 'clockNewGame',
        matchId: storedBinding.matchId,
        time: storedBinding.startKind.type === 'ordinaryNewGame' ? 'reset' : 'carryForward',
      }, startClockOperation]
      startEntries = await Promise.all(startOperations.map((operation, index) => (
        competitionClockEntry(storedBinding, `start:${gameInstanceId}:${sequence}:${index}`, operation, trustedTimeMs)
      )))
    }
    let committedMetadata: GameRoomMetadata | undefined

    await this.ctx.storage.transaction(async storage => {
      const current = await storage.list<unknown>()
      const currentBinding = current.get('competitionBinding')
      const currentMetadata = current.get('metadata')
      const currentAdmission = current.get(competitionAdmissionKey)
      if (!competitionBindingIsValid(currentBinding)
        || !competitionGameBindingEquals(currentBinding, preparation.binding)
        || !isCompetitionSnapshotData(currentMetadata)
        || !hasCompetitionReadyMetadataShape(currentMetadata)
        || !competitionAdmissionMatchesBinding(currentAdmission, currentMetadata, currentBinding)
        || currentAdmission.version >= Number.MAX_SAFE_INTEGER
        || currentAdmission.version !== preparation.admission.version
        || !sameCompetitionBindingRef(currentAdmission.bindingRef, preparation.admission.bindingRef)
        || !this.competitionStartAdmitted(currentMetadata, currentBinding, currentAdmission, Date.now())
        || current.has('gameRecord') || currentMetadata.gameInstanceId !== undefined
        || current.get('nextSequence') !== sequence
        || stableDeliveryFingerprint(currentMetadata) !== stableDeliveryFingerprint(metadata)
        || !isPristineCompetitionWaitingQueue(current.get(this.competitionDeliveryQueueKey()), currentBinding)) {
        throw new CompetitionGameBindingError('competitionGameNotReady')
      }
      if (!storage.setAlarm) throw new Error('competition delivery storage does not support alarms')

      // 首局的 S 必須晚於序號／identity 準備及交易內完整 late snapshot／CAS。
      const startedAtMs = storedBinding.gameNumber === 1 ? Date.now() : trustedTimeMs
      const now = new Date(startedAtMs).toISOString()
      const event: StoredGameEvent = {
        sequence,
        type: 'GameStarted',
        payload: { gameInstanceId, firstPlayer },
        createdAt: now,
      }
      const updatedMetadata: GameRoomMetadata = {
        ...metadata,
        gameInstanceId,
        status: 'Active',
        updatedAt: now,
      }
      let initialStartEvidence: CompetitionGameInitialStartEvidence | undefined
      if (storedBinding.gameNumber === 1) {
        if (!initialWindow || !initialStartIdentityPrefix) {
          throw new CompetitionGameBindingError('competitionGameNotReady')
        }
        const evidence = {
          bindingRef: structuredClone(admission.bindingRef),
          releaseDecisionId: admission.releaseCommandId,
          gameInstanceId,
          startSequence: sequence,
          startedAtMs,
          initialWindow,
        }
        if (!isCompetitionGameInitialStartEvidence(evidence, {
          bindingRef: admission.bindingRef, commandId: admission.releaseCommandId,
        })) throw new CompetitionGameBindingError('competitionGameNotReady')
        initialStartEvidence = evidence
        startEntries = [competitionInitialStartEntry(
          storedBinding, evidence, initialStartIdentityPrefix,
        )]
      }
      const deliveryQueue: CompetitionDeliveryQueue = {
        ...emptyCompetitionDeliveryQueue(storedBinding.expectedMatchRevision),
        entries: startEntries,
      }
      await storage.put({
        metadata: updatedMetadata,
        gameRecord: snapshot,
        nextSequence: sequence + 1,
        [this.eventKey(sequence)]: event,
        [this.competitionDeliveryQueueKey()]: deliveryQueue,
        ...(initialStartEvidence ? { [competitionInitialStartEvidenceKey]: initialStartEvidence } : {}),
        [competitionAdmissionKey]: {
          ...currentAdmission,
          status: 'started',
          version: currentAdmission.version + 1,
          startRetryAtMs: null,
          start: { gameInstanceId, startSequence: sequence, startedAtMs },
        } satisfies CompetitionGameAdmission,
      })
      await storage.setAlarm(startedAtMs)
      committedMetadata = updatedMetadata
    })

    if (!committedMetadata) throw new Error('competition game start transaction did not commit metadata')
    this.ctx.waitUntil(Promise.all([
      this.broadcast(committedMetadata),
      this.notifyMembers(committedMetadata, {
        kind: 'gameStarted',
        message: `「${preparation.roomName}」已開始。`,
      }),
    ]).then(() => undefined))

    return this.competitionResult(committedMetadata, false)
  }

  private competitionStartClockOperation(
    state: PublicGameState,
    binding: CompetitionGameBinding,
  ): CompetitionClockOperation {
    const invalidState = () => new CompetitionGameBindingError('invalidCompetitionBinding')
    if (state.ruleVersion !== binding.ruleVersion
      || JSON.stringify(state.enabledRuleModules) !== JSON.stringify(binding.enabledRuleModules)
      || JSON.stringify(state.turnOrder) !== JSON.stringify(binding.turnOrder)
      || state.players.length !== binding.players.length
      || !state.players.every((player, index) => (
        player.id === binding.players[index]?.player
        && player.team === binding.players[index]?.team
      ))
      || state.winnerTeam !== null
      || state.gameConclusion !== null
      || state.terminalResolution !== null
      || state.pendingChoice !== null
      || state.pendingRandomness !== null) {
      throw invalidState()
    }

    if (state.status === 'Preparing') {
      const selection = state.initialPouchSelection
      if (state.phase !== 'TurnStart'
        || (state.currentPlayer !== null && state.currentPlayer !== binding.firstPlayer)
        || state.turnNumber !== 1
        || !binding.enabledRuleModules.includes('pouch')
        || !isRecord(selection)
        || !hasExactKeys(selection, ['remainingPlayers'])
        || !Array.isArray(selection.remainingPlayers)
        || JSON.stringify(selection.remainingPlayers) !== JSON.stringify(binding.turnOrder)) {
        throw invalidState()
      }
      return { type: 'clockBeginPreparation', matchId: binding.matchId }
    }

    if (state.status !== 'InProgress'
      || state.phase !== 'ActiveEffects'
      || state.initialPouchSelection !== null
      || state.currentPlayer !== binding.firstPlayer
      || state.turnNumber !== 1) {
      throw invalidState()
    }
    const currentPlayer = binding.players.find(player => player.player === state.currentPlayer)
    if (!currentPlayer) throw invalidState()
    return {
      type: 'clockBeginAction',
      matchId: binding.matchId,
      team: currentPlayer.team,
      turn: state.turnNumber,
    }
  }

  /**
   * 競賽時鐘的唯一 server-only 入口。此切片只建立有序交付佇列，不替遊戲流程
   * 決定何時應該建立哪一個 clock window；E4d3 會在既有序列化邊界呼叫它。
   */
  async enqueueCompetitionClockMutation(
    request: Omit<CompetitionClockDeliveryRequest, 'expectedMatchRevision'>,
  ): Promise<{ commandId: string; idempotent: boolean; queued: true }> {
    return await this.serialized(async () => {
      const binding = await this.competitionBinding()
      const current = await this.ctx.storage.list<unknown>()
      const metadata = current.get('metadata')
      const admission = current.get(competitionAdmissionKey)
      if (!binding
        || !hasCompetitionReadyMetadataShape(metadata)
        || metadata.status !== 'Active'
        || !competitionAdmissionMatchesBinding(admission, metadata, binding)
        || !this.hasDurableCompetitionStart(current, metadata, binding, admission)
        || request.competitionId !== binding.competitionId
        || request.matchId !== binding.matchId
        || request.callerUserId !== binding.organizerUserId
        || !isNonEmpty(request.commandId)
        || !Number.isSafeInteger(request.trustedTimeMs)
        || request.trustedTimeMs < 0
        || !isCompetitionClockOperation(request.operation)
        || request.operation.matchId !== binding.matchId) {
        throw new Error('invalid competition clock delivery request')
      }

      const queue = await this.competitionDeliveryQueue(binding)
      const fingerprint = stableDeliveryFingerprint({
        competitionId: request.competitionId,
        matchId: request.matchId,
        callerUserId: request.callerUserId,
        requestCommandId: request.commandId,
        trustedTimeMs: request.trustedTimeMs,
        operation: request.operation,
      })
      const commandId = await competitionClockIdentity(
        binding.competitionId,
        binding.matchId,
        request.commandId,
        request.operation,
      )
      const receipt = queue.receipts.find(candidate => candidate.requestCommandId === request.commandId)
      if (receipt) {
        if (receipt.fingerprint !== fingerprint) throw new Error('competition clock command identity conflict')
        return { commandId: receipt.commandId, idempotent: true, queued: true }
      }
      const existing = queue.entries.find(entry => entry.kind === 'competitionClockMutation'
        && entry.commandId === commandId)
      if (existing) {
        if (JSON.stringify(existing.operation) !== JSON.stringify(request.operation)
          || existing.trustedTimeMs !== request.trustedTimeMs
          || existing.callerUserId !== request.callerUserId) {
          throw new Error('competition clock command identity conflict')
        }
        return { commandId, idempotent: true, queued: true }
      }

      const now = Date.now()
      const entry: CompetitionClockDeliveryEntry = {
        schemaVersion: 1,
        kind: 'competitionClockMutation',
        competitionId: binding.competitionId,
        matchId: binding.matchId,
        callerUserId: binding.organizerUserId,
        commandId,
        trustedTimeMs: request.trustedTimeMs,
        operation: structuredClone(request.operation),
        attempts: 0,
        nextAttemptAt: now,
      }
      const nextQueue: CompetitionDeliveryQueue = {
        ...queue,
        entries: [...queue.entries, entry],
        receipts: [...queue.receipts, {
          schemaVersion: 1,
          kind: 'competitionClockEnqueue',
          requestCommandId: request.commandId,
          commandId,
          fingerprint,
        }],
      }
      await this.persistCompetitionDeliveryQueue(nextQueue)
      this.ctx.waitUntil(this.deliverCompetitionQueue().catch(error => {
        console.error('competition delivery failed', error)
      }))
      return { commandId, idempotent: false, queued: true }
    })
  }

  /** 可信 CompetitionRoom 專用；控制和本地 FIFO／revision 游標在同一交易提交。 */
  async synchronizeCompetitionClock(
    request: CompetitionClockControlRequest,
  ): Promise<{ ok: true; value: CompetitionClockControlAck }> {
    return await this.serialized(async () => {
      if (!isCompetitionClockControlRequest(request)) {
        throw new Error('invalid competition clock control request')
      }
      const binding = await this.competitionBinding()
      const metadata = await this.metadata()
      const record = await this.ctx.storage.get<GameRecord>('gameRecord')
      if (!binding || !metadata || metadata.gameId !== request.gameId
        || request.competitionId !== binding.competitionId
        || request.matchId !== binding.matchId
        || request.gameNumber !== binding.gameNumber) {
        throw new Error('competition clock control binding mismatch')
      }
      const queue = await this.competitionDeliveryQueue(binding, record?.gameInstanceId)
      const fingerprint = stableDeliveryFingerprint(request)
      const previous = (queue.clockControlReceipts ?? []).find(item => (
        item.request.controlId === request.controlId
      ))
      if (previous) {
        if (stableDeliveryFingerprint(previous.request) !== fingerprint) {
          throw new Error('competition clock control identity conflict')
        }
        return { ok: true, value: { ...previous.ack, idempotent: true } }
      }
      if (!record || metadata.status !== 'Active') {
        throw new Error('competition clock control has no active game record')
      }
      if ((request.operation === 'clockPause' && request.nextClockDeadlineMs !== null)
        || (request.operation === 'clockResume'
          && request.nextClockDeadlineMs !== null
          && request.nextClockDeadlineMs < request.trustedTimeMs)) {
        throw new Error('competition clock control deadline is invalid')
      }

      if (queue.pendingTimeout || queue.terminalTimeout) {
        await this.persistRefereePauseFailClosed()
        throw new Error('competition clock control cannot follow a timeout terminal')
      }
      if (queue.revisionCursor + 1 !== request.matchRevision) {
        await this.persistRefereePauseFailClosed()
        throw new Error('competition clock control revision gap')
      }
      let entries = queue.entries
      if (entries.length > 0) {
        const staleObservation = request.operation === 'clockPause'
          && await this.isProvablyStalePauseObservation(
            entries,
            queue,
            binding,
            record.gameInstanceId,
            request,
          )
        if (staleObservation) {
          entries = []
        } else {
          await this.persistRefereePauseFailClosed()
          throw new Error('competition clock control blocked by pending delivery')
        }
      }

      const ack: CompetitionClockControlAck = { ...request, idempotent: false }
      const nextQueue: CompetitionDeliveryQueue = {
        ...queue,
        revisionCursor: request.matchRevision,
        entries,
        nextClockDeadlineMs: request.nextClockDeadlineMs,
        refereePaused: request.operation === 'clockPause',
        clockControlReceipts: [...(queue.clockControlReceipts ?? []), { request, ack }],
      }
      const storage: GameRoomAtomicStorage = this.ctx.storage
      await storage.transaction(async transaction => {
        const current = await transaction.get<unknown>(this.competitionDeliveryQueueKey())
        if (!isCompetitionDeliveryQueue(current)) {
          throw new Error('competition delivery queue changed shape')
        }
        const normalized = normalizeCompetitionDeliveryQueue(current)
        if (normalized.revisionCursor !== queue.revisionCursor
          || stableDeliveryFingerprint(normalized) !== stableDeliveryFingerprint(queue)) {
          throw new Error('competition delivery queue changed during clock control')
        }
        if (!transaction.setAlarm) throw new Error('competition delivery storage does not support alarms')
        await transaction.put({ [this.competitionDeliveryQueueKey()]: nextQueue })
        const alarmAt = nextCompetitionAlarmAt(nextQueue, Date.now())
        if (alarmAt === null) await transaction.deleteAlarm?.()
        else await transaction.setAlarm(alarmAt)
      })
      return { ok: true, value: ack }
    })
  }

  private async persistRefereePauseFailClosed(): Promise<void> {
    const storage: GameRoomAtomicStorage = this.ctx.storage
    await storage.transaction(async transaction => {
      const current = await transaction.get<unknown>(this.competitionDeliveryQueueKey())
      if (!isCompetitionDeliveryQueue(current)) throw new Error('competition delivery queue is invalid')
      const normalized = normalizeCompetitionDeliveryQueue(current)
      const frozen = { ...normalized, refereePaused: true }
      if (!transaction.setAlarm) throw new Error('competition delivery storage does not support alarms')
      await transaction.put({ [this.competitionDeliveryQueueKey()]: frozen })
      const alarmAt = nextCompetitionAlarmAt(frozen, Date.now())
      if (alarmAt === null) await transaction.deleteAlarm?.()
      else await transaction.setAlarm(alarmAt)
    })
  }

  private async isProvablyStalePauseObservation(
    entries: CompetitionDeliveryEntry[],
    queue: CompetitionDeliveryQueue,
    binding: CompetitionGameBinding,
    gameInstanceId: string,
    request: CompetitionClockControlRequest,
  ): Promise<boolean> {
    const entry = entries[0]
    const deadline = queue.nextClockDeadlineMs ?? null
    if (request.operation !== 'clockPause' || entries.length !== 1
      || !entry || entry.kind !== 'competitionClockMutation'
      || entry.operation.type !== 'clockObserve'
      || deadline === null || entry.trustedTimeMs !== deadline
      || entry.operation.matchId !== binding.matchId) return false
    const sourceId = await competitionDeadlineIdentity(gameInstanceId, queue.revisionCursor, deadline)
    const expected = await competitionClockEntry(
      binding,
      sourceId,
      { type: 'clockObserve', matchId: binding.matchId },
      deadline,
    )
    return entry.competitionId === expected.competitionId
      && entry.matchId === expected.matchId
      && entry.callerUserId === expected.callerUserId
      && entry.commandId === expected.commandId
      && entry.trustedTimeMs === expected.trustedTimeMs
      && stableDeliveryFingerprint(entry.operation) === stableDeliveryFingerprint(expected.operation)
  }

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url)

    if (request.method === 'POST' && url.pathname.endsWith('/manage/purge-legacy')) {
      const body = await request.json() as { epoch?: unknown }
      return await this.serialized(async () => {
        if (typeof body.epoch !== 'string' || !body.epoch.trim()) {
          return this.json({ error: 'a purge epoch is required' }, 400)
        }
        const result = await this.purgeLegacyGameData(body.epoch.trim())
        return this.json(result.body, result.statusCode)
      })
    }

    if (request.method === 'POST' && url.pathname.endsWith('/manage/probe-legacy')) {
      return await this.serialized(async () => {
        const result = await this.legacyPurgeProbe()
        return this.json(result.body, result.statusCode)
      })
    }

    if (request.method === 'POST' && url.pathname.endsWith('/manage/verify-legacy')) {
      return await this.serialized(async () => {
        const result = await this.legacyPurgeVerification()
        return this.json(result, result.status === 'Absent' ? 404 : 200)
      })
    }

    let competitionBinding: CompetitionGameBinding | undefined
    try {
      competitionBinding = await this.competitionBinding()
    } catch (error) {
      // 損壞的 present binding 交給各路徑原有錯誤處理，不讓入口閘門先改變回應契約。
      // Storage I/O 失敗則必須在任何 socket 或請求副作用前拒絕。
      if (!(error instanceof CompetitionGameBindingError)) {
        return this.json({
          error: '賽事房間入口狀態暫時無法驗證。',
          code: 'competitionRoundNotAnnounced',
        }, 409)
      }
    }
    if (competitionBinding?.gameNumber === 1) {
      let metadata: GameRoomMetadata | undefined
      let admission: unknown
      let preserveExistingFailure = false
      try {
        metadata = await this.metadata()
        admission = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
      } catch (error) {
        // 結構損壞保留既有 handler 回應；暫時儲存讀取失敗一律 fail closed。
        if (!(error instanceof CompetitionGameBindingError)) {
          return this.json({
            error: '賽事房間入口狀態暫時無法驗證。',
            code: 'competitionRoundNotAnnounced',
          }, 409)
        }
        preserveExistingFailure = true
      }
      if (!preserveExistingFailure && (!metadata
        || !competitionAdmissionMatchesBinding(admission, metadata, competitionBinding)
        || !await this.competitionRoundPlayerEntryAllowed(metadata.gameId, competitionBinding, admission))) {
        return this.json({
          error: '賽事房間尚未到公告開放時間或整輪入口尚未就緒。',
          code: 'competitionRoundNotAnnounced',
        }, 409)
      }
    }

    if (request.method === 'GET' && url.pathname.endsWith('/socket')) {
      return await this.serialized(() => this.connect(request))
    }

    if (request.method === 'GET') {
      const metadata = await this.metadata()
      if (!metadata) return this.json({ error: 'room not found' }, 404)
      if (await this.isCompetitionRoom()) {
        return this.json({
          error: '競賽進行中僅限固定成員查詢房間狀態。',
          code: 'competitionMemberRequired',
        }, 403)
      }
      return this.json(await this.response(metadata))
    }

    if (request.method !== 'POST') {
      return this.json({ error: 'method not allowed' }, 405)
    }

    const body = (await request.json()) as GameRoomRequest

    return await this.serialized(async () => {
      this.replySocketId = request.headers.get('x-fewfc-reply-socket') ?? undefined
      try {
      switch (body.type) {
        case 'createGame':
          if (await this.isCompetitionRoom()) {
            return this.competitionMutationRejected()
          }
          return this.json(await this.createGame(body))
        case 'joinGame':
          return await this.joinGame(body.actorUserId, body.actorName, body.credential)
        case 'toggleReady':
          return await this.toggleReady(body.actorUserId, body.deckList, body.ready)
        case 'updateRuleModules':
          return await this.updateRuleModules(body.actorUserId, body.enabledRuleModules, body.ruleVersion)
        case 'leaveGame':
          return await this.leaveGame(body.actorUserId)
        case 'removePlayer':
          return await this.removePlayer(body.actorUserId, body.targetUserId)
        case 'dissolveGame':
          return await this.dissolveGame(body.actorUserId)
        case 'startGame':
          return await this.startGame(body.actorUserId, body.deckList)
        case 'resetGame':
          return await this.resetGame(body.actorUserId)
        case 'getCompletedReplayDraft':
          return await this.getCompletedReplayDraft(body.actorUserId)
        case 'seedDevelopmentScenario':
          return await this.seedDevelopmentScenario(body.actorUserId, body.scenario)
        case 'inspectDevelopmentRecord':
          return await this.inspectDevelopmentRecord(body.actorUserId, body.commandId)
        case 'getState':
          return await this.getState(body.actorUserId)
        case 'submitCommand':
          return await this.submitCommand(body)
        default:
          return this.json({ error: 'unknown game-room request' }, 400)
      }
      } catch (error) {
        if (error instanceof RulesEngineError) {
          if (error.statusCode === 500) console.error('Rules Engine request failed', error.detail)
          return this.json({ error: error.message, code: error.code }, error.statusCode)
        }

        if (error instanceof OnlineCommandTransactionError) {
          return this.json({ error: error.message, code: error.code }, error.statusCode)
        }

        if (error instanceof Error && error.message === 'game room has not been created') {
          return this.json({ error: '找不到遊戲房間。', code: 'roomNotFound' }, 404)
        }

        console.error('Game Room request failed', error)
        return this.json({
          error: '房間服務暫時無法處理要求，請稍後再試。',
          code: 'gameRoomFailure',
        }, 500)
      } finally {
        this.replySocketId = undefined
      }
    })
  }

  async webSocketMessage(socket: WebSocket, message: string | ArrayBuffer) {
    await this.socketHandler.handleDurableMessage(this, socket, message)
  }

  async webSocketClose(socket: WebSocket, code = 1000, reason = '', wasClean = true) {
    await this.socketHandler.handleDurableClose(this, socket, code, reason, wasClean)
    await this.serialized(() => this.disconnect(socket))
  }

  async webSocketError(socket: WebSocket) {
    await this.serialized(() => this.disconnect(socket))
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
    } finally {
      release()
    }
  }

  private async createGame(
    request: Extract<GameRoomRequest, { type: 'createGame' }>,
  ): Promise<GameRoomResponse> {
    const existing = await this.metadata()

    if (existing) {
      return await this.response(existing, request.actorUserId)
    }

    const capacity = request.capacity === 4 ? 4 : 2
    const players = Array.from({ length: capacity }, (_, index) => `player-${index + 1}`)
    const now = new Date().toISOString()
    const metadata: GameRoomMetadata = {
      schemaVersion: 5,
      gameId: request.gameId,
      name: request.name?.trim() || request.gameId,
      access: request.access ?? 'private',
      capacity,
      ruleset: 'fewfc-base',
      ruleVersion: request.ruleVersion ?? '5.17',
      enabledRuleModules: (await callRuleModuleResolution(request.enabledRuleModules, request.ruleVersion ?? '5.17')).modules,
      players,
      members: [{
        userId: request.actorUserId,
        displayName: request.actorName,
        player: players[0] ?? 'player-1',
        ready: false,
        connected: false,
        owner: true,
      }],
      observers: [],
      status: 'Waiting',
      createdAt: now,
      updatedAt: now,
    }
    const initialEvent: StoredGameEvent = {
      sequence: 1,
      type: 'GameCreated',
      payload: {
        capacity,
        access: metadata.access,
        ruleset: metadata.ruleset,
        ruleVersion: metadata.ruleVersion,
        enabledRuleModules: [...metadata.enabledRuleModules],
      },
      createdAt: now,
    }

    await this.ctx.storage.put('metadata', metadata)
    await this.ctx.storage.put('invitation', request.invitation)
    await this.ctx.storage.put('nextSequence', 2)
    await this.ctx.storage.put(this.eventKey(initialEvent.sequence), initialEvent)

    return await this.response(metadata, request.actorUserId)
  }

  private async joinGame(
    actorUserId: string,
    actorName: string,
    credential?: Extract<GameRoomRequest, { type: 'joinGame' }>['credential'],
  ): Promise<Response> {
    const metadata = await this.requireMetadata()
    const competition = await this.competitionBinding()

    if (competition) {
      if (this.memberFor(metadata, actorUserId)) {
        return this.json(await this.response(metadata, actorUserId))
      }
      return this.json({ error: '競賽房間僅允許已固定的成員加入。' }, 403)
    }

    if (this.participantFor(metadata, actorUserId)) {
      return this.json(await this.response(metadata, actorUserId))
    }

    if (metadata.access === 'private' && !await this.validCredential(credential)) {
      return this.json({ error: 'room not found' }, 404)
    }

    if (metadata.status === 'Dissolved') {
      return this.json({ error: 'room not found' }, 404)
    }

    if (metadata.status === 'Finished') {
      return this.json({ error: 'room has finished' }, 409)
    }

    const occupiedPlayers = new Set(metadata.members.map((member) => member.player))
    const openPlayer = metadata.players.find((player) => !occupiedPlayers.has(player))

    const now = new Date().toISOString()
    if (!openPlayer || metadata.status !== 'Waiting') {
      const observer: GameRoomObserver = {
        userId: actorUserId,
        displayName: actorName,
        connected: false,
      }
      const nextMetadata: GameRoomMetadata = {
        ...metadata,
        observers: [...metadata.observers, observer],
        updatedAt: now,
      }
      await this.ctx.storage.put('metadata', nextMetadata)
      const updatedMetadata = nextMetadata
      this.ctx.waitUntil(this.afterRoomMutation(updatedMetadata, {
        kind: 'roomChanged',
        message: `${actorName} 正在觀戰「${metadata.name}」。`,
      }))
      return this.json(await this.response(updatedMetadata, actorUserId))
    }

    const member: GameRoomMember = {
      userId: actorUserId,
      displayName: actorName,
      player: openPlayer,
      ready: false,
      connected: false,
      owner: false,
    }
    const updatedMetadata = await this.storeRoomEvent({
      ...metadata,
      members: [...metadata.members, member],
      updatedAt: now,
    }, 'PlayerJoined', openPlayer, { player: openPlayer })

    this.ctx.waitUntil(this.afterRoomMutation(updatedMetadata, {
      kind: 'roomChanged',
      message: `${actorName} 已加入「${metadata.name}」。`,
    }))

    return this.json(await this.response(updatedMetadata, actorUserId))
  }

  private async getState(actorUserId: string): Promise<Response> {
    const metadata = await this.metadata()
    if (!metadata) return this.json({ error: 'room not found' }, 404)

    const competition = await this.competitionBinding()
    const participant = competition
      ? this.memberFor(metadata, actorUserId)
      : this.participantFor(metadata, actorUserId)
    if (!participant) {
      return this.json(
        { error: metadata.access === 'private' ? 'room not found' : 'player is not in this room' },
        metadata.access === 'private' ? 404 : 403,
      )
    }

    return this.json(await this.response(metadata, actorUserId))
  }

  private async validCredential(
    credential?: Extract<GameRoomRequest, { type: 'joinGame' }>['credential'],
  ): Promise<boolean> {
    if (!credential?.value) {
      return false
    }

    const invitation = await this.ctx.storage.get<GameRoomInvitation>('invitation')
    return invitationCredentialMatches(invitation, credential)
  }

  private async toggleReady(
    actorUserId: string,
    deckList: PlayerDeckList,
    desiredReady?: boolean,
  ): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)
    const competition = await this.competitionBinding()

    if (!actor) {
      return this.json({ error: 'only room players may ready' }, 403)
    }

    if (competition) {
      if (competition.enabledRuleModules.includes('personal-deck')) {
        const lockedDeck = competition.deckSnapshot.find(deck => deck.player === actor.player)
        if (!lockedDeck || !competitionDeckEquals(deckList, lockedDeck)) {
          return this.json({ error: 'competition deck does not match the locked deck' }, 409)
        }
      }
      if (!actor.connected) {
        return this.json({ error: 'player must be connected to ready' }, 409)
      }
      if (metadata.status !== 'Waiting') {
        return this.json({ error: 'room has already started' }, 409)
      }

      if (competition.gameNumber === 1 && Date.now() >= competition.roundStartMs) {
        const admission = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
        if (!competitionAdmissionMatchesBinding(admission, metadata, competition)) {
          return this.json({ error: '競賽房間狀態無法驗證。', code: 'invalidCompetitionRoom' }, 409)
        }
        const decision = await this.adjudicateCompetitionNoShowInternal({
          bindingRef: admission.bindingRef,
          commandId: competitionGameNoShowCommandId(admission.bindingRef),
          deadlineMs: competition.roundStartMs,
        })
        const currentAdmission = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
        if (decision.type !== 'ready'
          || !competitionAdmissionMatchesBinding(currentAdmission, metadata, competition)
          || currentAdmission.status !== 'released') {
          return this.json({
            error: '比賽開始時間已到，無法變更房間報到狀態。',
            code: 'checkInDeadlinePassed',
          }, 409)
        }
      }

      const ready = desiredReady ?? !actor.ready
      if (ready === actor.ready) return this.json(await this.response(metadata, actorUserId))
      const updatedMetadata = {
        ...metadata,
        members: metadata.members.map(member => (
          member.userId === actorUserId ? { ...member, ready } : member
        )),
        updatedAt: new Date().toISOString(),
      }
      await this.persistCompetitionWaitingMetadata(updatedMetadata, competition, {
        type: ready ? 'PlayerReady' : 'PlayerUnready',
        actor: actor.player,
        payload: { player: actor.player },
      })
      if (await this.competitionCanAutoStart(updatedMetadata, competition)) {
        this.queueCompetitionAutoStart(updatedMetadata.gameId, competition)
      }
      this.ctx.waitUntil(this.afterRoomMutation(updatedMetadata))
      return this.json(await this.response(updatedMetadata, actorUserId))
    }

    if (actor.owner) {
      return this.json({ error: 'room owner does not ready' }, 409)
    }

    if (!actor.connected) {
      return this.json({ error: 'player must be connected to ready' }, 409)
    }

    if (metadata.status !== 'Waiting') {
      return this.json({ error: 'room has already started' }, 409)
    }

    const ready = desiredReady ?? !actor.ready
    if (ready === actor.ready) return this.json(await this.response(metadata, actorUserId))
    if (ready) {
      await this.ctx.storage.put(this.lockedDeckKey(actorUserId), deckList)
    } else {
      await this.ctx.storage.delete(this.lockedDeckKey(actorUserId))
    }
    const updatedMetadata = await this.storeRoomEvent({
      ...metadata,
      members: metadata.members.map((member) => (
        member.userId === actorUserId ? { ...member, ready } : member
      )),
      updatedAt: new Date().toISOString(),
    }, ready ? 'PlayerReady' : 'PlayerUnready', actor.player, {
      player: actor.player,
    })

    this.ctx.waitUntil(this.afterRoomMutation(updatedMetadata))

    return this.json(await this.response(updatedMetadata, actorUserId))
  }

  private async updateRuleModules(
    actorUserId: string,
    enabledRuleModules?: string[],
    requestedVersion?: '5.16' | '5.17',
  ): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)

    if (await this.isCompetitionRoom()) {
      return this.competitionMutationRejected()
    }

    if (!actor?.owner) {
      return this.json({ error: 'only room owner may update rules' }, 403)
    }
    if (metadata.status !== 'Waiting') {
      return this.json({ error: 'room has already started' }, 409)
    }

    const ruleVersion = requestedVersion ?? metadata.ruleVersion
    const candidate = ruleVersion !== metadata.ruleVersion ? undefined : enabledRuleModules ?? metadata.enabledRuleModules
    let modules: string[]
    try {
      modules = (await callRuleModuleResolution(candidate, ruleVersion)).modules
    } catch {
      return this.json({ error: '規則版本或模組設定無效。' }, 400)
    }
    for (const member of metadata.members) {
      await this.ctx.storage.delete(this.lockedDeckKey(member.userId))
    }
    const updatedMetadata = await this.storeRoomEvent({
      ...metadata,
      ruleVersion,
      enabledRuleModules: modules,
      members: metadata.members.map(member => ({ ...member, ready: false })),
      updatedAt: new Date().toISOString(),
    }, 'RuleModulesChanged', actor.player, { ruleVersion, enabledRuleModules: modules })

    this.ctx.waitUntil(this.afterRoomMutation(updatedMetadata))
    return this.json(await this.response(updatedMetadata, actorUserId))
  }

  private async leaveGame(actorUserId: string): Promise<Response> {
    const metadata = await this.requireMetadata()
    if (await this.isCompetitionRoom()) {
      return this.competitionMutationRejected()
    }
    const actor = this.memberFor(metadata, actorUserId)
    const observer = this.observerFor(metadata, actorUserId)

    if (!actor && !observer) {
      return this.json({ error: 'member is not in this room' }, 404)
    }

    if (observer) {
      const nextMetadata: GameRoomMetadata = {
        ...metadata,
        observers: metadata.observers.filter(candidate => candidate.userId !== actorUserId),
        updatedAt: new Date().toISOString(),
      }
      await this.ctx.storage.put('metadata', nextMetadata)
      const updatedMetadata = nextMetadata
      this.closeUserSockets(actorUserId, 4000, 'left observation')
      this.ctx.waitUntil(this.afterRoomMutation(updatedMetadata, {
        kind: 'roomChanged',
        message: `${observer.displayName} 已停止觀戰「${metadata.name}」。`,
      }))
      return this.json(await this.response(updatedMetadata, actorUserId))
    }

    if (actor.owner) {
      return this.json({ error: 'room owner must dissolve the room' }, 409)
    }

    if (metadata.status !== 'Waiting') {
      return this.json({ error: 'players cannot leave an active match' }, 409)
    }

    const updatedMetadata = await this.storeRoomEvent({
      ...metadata,
      members: metadata.members.filter((member) => member.userId !== actorUserId),
      updatedAt: new Date().toISOString(),
    }, 'PlayerLeft', actor.player, { player: actor.player })
    await this.ctx.storage.delete(this.lockedDeckKey(actorUserId))
    const promotedMetadata = await this.promoteObservers(updatedMetadata)
    this.closeUserSockets(actorUserId, 4000, 'left room')

    this.ctx.waitUntil(this.afterRoomMutation(promotedMetadata, {
      kind: 'roomChanged',
      message: `${actor.displayName} 已離開「${metadata.name}」。`,
    }))

    return this.json(await this.response(promotedMetadata, actorUserId))
  }

  private async removePlayer(actorUserId: string, targetUserId: string): Promise<Response> {
    const metadata = await this.requireMetadata()
    if (await this.isCompetitionRoom()) {
      return this.competitionMutationRejected()
    }
    const actor = this.memberFor(metadata, actorUserId)
    const target = this.memberFor(metadata, targetUserId)
    const observer = this.observerFor(metadata, targetUserId)

    if (!actor?.owner) {
      return this.json({ error: 'only room owner may remove players' }, 403)
    }

    if ((!target && !observer) || target?.owner) {
      return this.json({ error: 'target member cannot be removed' }, 409)
    }

    if (metadata.status !== 'Waiting') {
      return this.json({ error: 'players cannot be removed from an active match' }, 409)
    }

    const nextMetadata: GameRoomMetadata = {
      ...metadata,
      members: metadata.members.filter((member) => member.userId !== targetUserId),
      observers: metadata.observers.filter((candidate) => candidate.userId !== targetUserId),
      updatedAt: new Date().toISOString(),
    }
    const updatedMetadata = target
      ? await this.storeRoomEvent(nextMetadata, 'PlayerRemoved', target.player, { player: target.player })
      : (await this.ctx.storage.put('metadata', nextMetadata), nextMetadata)
    await this.ctx.storage.delete(this.lockedDeckKey(targetUserId))
    const promotedMetadata = target ? await this.promoteObservers(updatedMetadata) : updatedMetadata
    this.closeUserSockets(targetUserId, 4003, 'removed from room')

    this.ctx.waitUntil(Promise.all([
      this.afterRoomMutation(promotedMetadata, {
        kind: 'roomChanged',
        message: `${(target ?? observer)!.displayName} 已被移出「${metadata.name}」。`,
      }),
      this.sendNotification(targetUserId, {
        kind: 'removed',
        message: `你已被移出「${metadata.name}」。`,
      }),
    ]))

    return this.json(await this.response(promotedMetadata, actorUserId))
  }

  private async dissolveGame(actorUserId: string): Promise<Response> {
    const metadata = await this.requireMetadata()
    if (await this.isCompetitionRoom()) {
      return this.competitionMutationRejected()
    }
    const actor = this.memberFor(metadata, actorUserId)

    if (!actor?.owner) {
      return this.json({ error: 'only room owner may dissolve the room' }, 403)
    }

    if (metadata.status !== 'Waiting') {
      return this.json({ error: 'an active match cannot be dissolved' }, 409)
    }

    const updatedMetadata: GameRoomMetadata = {
      ...metadata,
      status: 'Dissolved',
      updatedAt: new Date().toISOString(),
    }

    await this.ctx.storage.put('metadata', updatedMetadata)
    this.ctx.waitUntil(this.notifyMembers(metadata, {
      kind: 'dissolved',
      message: `「${metadata.name}」已解散。`,
    }))
    this.broadcastDissolved(metadata.gameId)

    return this.json(await this.response(updatedMetadata, actorUserId))
  }

  private async startGame(
    actorUserId: string,
    deckList: PlayerDeckList,
  ): Promise<Response> {
    const metadata = await this.requireMetadata()
    if (await this.isCompetitionRoom()) {
      return this.competitionMutationRejected()
    }
    const owner = metadata.members.find((member) => member.owner)

    if (owner?.userId !== actorUserId) {
      return this.json({ error: 'only room owner may start' }, 403)
    }

    if (metadata.status !== 'Waiting') {
      return this.json({ error: 'room has already started' }, 409)
    }

    if (metadata.members.length !== metadata.capacity) {
      return this.json({ error: 'room is waiting for players' }, 409)
    }

    if (metadata.members.some((member) => !member.connected)) {
      return this.json({ error: 'all players must be connected' }, 409)
    }

    if (metadata.members.some((member) => !member.owner && !member.ready)) {
      return this.json({ error: 'not all joined players are ready' }, 409)
    }

    // 已完成的草稿只能儲存到下一場對局開始前。
    await this.ctx.storage.delete('lastCompletedReplayDraft')

    await this.ctx.storage.put(this.lockedDeckKey(actorUserId), deckList)
    const lockedDecks = await Promise.all(metadata.members.map(async member => ({
      player: member.player,
      ...await this.requireLockedDeck(member.userId),
    })))
    const setup = this.randomSetup(metadata, lockedDecks)
    const firstPlayer = setup.turnOrder[0] ?? metadata.players[0] ?? 'player-1'
    const deckSeed = crypto.randomUUID()
    const gameInstanceId = crypto.randomUUID()
    const rules = await callRulesEngine({
      action: { type: 'start' },
      viewer: 'observer',
      setup,
      deckSeed,
    })
    const now = new Date().toISOString()
    const sequence = await this.nextSequence()
    const event: StoredGameEvent = {
      sequence,
      type: 'GameStarted',
      actor: owner.player,
      payload: {
        gameInstanceId,
        setup,
        firstPlayer,
        deckSeed,
      },
      createdAt: now,
    }
    const snapshot: GameRecord = {
      schemaVersion: 7,
      gameInstanceId,
      sequence,
      firstPlayer,
      deckSeed,
      setup,
      rulesRecord: rules.record,
    }
    const updatedMetadata: GameRoomMetadata = {
      ...metadata,
      gameInstanceId,
      status: 'Active',
      updatedAt: now,
    }

    await this.ctx.storage.put('metadata', updatedMetadata)
    await this.ctx.storage.put('nextSequence', sequence + 1)
    await this.ctx.storage.put('gameRecord', snapshot)
    await this.ctx.storage.put(this.eventKey(sequence), event)

    this.ctx.waitUntil(Promise.all([
      this.broadcast(updatedMetadata),
      this.notifyMembers(updatedMetadata, {
        kind: 'gameStarted',
        message: `「${metadata.name}」已開始。`,
      }),
    ]).then(() => undefined))

    return this.json(await this.response(updatedMetadata, actorUserId))
  }

  private async resetGame(actorUserId: string): Promise<Response> {
    const metadata = await this.requireMetadata()
    if (await this.isCompetitionRoom()) {
      return this.competitionMutationRejected()
    }

    if (!this.memberFor(metadata, actorUserId)) {
      return this.json({ error: 'only room players may return to the room' }, 403)
    }

    if (metadata.status !== 'Finished') {
      return this.json({ error: 'match has not finished' }, 409)
    }

    const snapshot = await this.requireGameRecord()
    const finished = await this.callReadyRules({ type: 'refresh' }, 'observer', snapshot)
    const draft: CompletedReplayDraft = {
      schemaVersion: 1,
      replayId: crypto.randomUUID(),
      sourceGameId: metadata.gameId,
      finishedAt: snapshot.finishedAt ?? new Date().toISOString(),
      setup: snapshot.setup,
      record: snapshot.rulesRecord,
      players: metadata.members.map(member => ({
        player: member.player,
        displayName: member.displayName,
      })),
      originalUserIds: metadata.members.map(member => member.userId),
      roomName: metadata.name,
      result: {
        status: finished.state.status,
        hp: finished.state.hp,
      },
      firstPlayer: snapshot.firstPlayer,
    }
    await this.ctx.storage.put('lastCompletedReplayDraft', draft)

    const updatedMetadata = await this.storeRoomEvent({
      ...metadata,
      gameInstanceId: undefined,
      members: metadata.members.map((member) => ({
        ...member,
        ready: false,
      })),
      status: 'Waiting',
      updatedAt: new Date().toISOString(),
    }, 'PlayersReturnedToRoom', undefined, {})
    for (const member of metadata.members) {
      await this.ctx.storage.delete(this.lockedDeckKey(member.userId))
    }
    await this.ctx.storage.delete('gameRecord')
    await this.deleteGameInstanceTransactions(snapshot.gameInstanceId)

    this.ctx.waitUntil(this.afterRoomMutation(updatedMetadata))

    return this.json(await this.response(updatedMetadata, actorUserId))
  }

  private async getCompletedReplayDraft(actorUserId: string): Promise<Response> {
    if (await this.isCompetitionRoom()) {
      return this.competitionMutationRejected()
    }
    const draft = await this.ctx.storage.get<CompletedReplayDraft>('lastCompletedReplayDraft')
    if (!draft) {
      return this.json({ error: 'replay is no longer available', code: 'replayNoLongerAvailable' }, 409)
    }
    if (!draft.originalUserIds.includes(actorUserId)) {
      return this.json({ error: 'only original players may save this replay' }, 403)
    }
    return this.json(draft)
  }

  private async seedDevelopmentScenario(
    actorUserId: string,
    scenario: import('../../shared/development-scenarios').DevelopmentScenario,
  ): Promise<Response> {
    if (await this.isCompetitionRoom()) {
      return this.competitionMutationRejected()
    }
    switch (scenario.name) {
      case 'star-endgame':
        return await this.seedEndgameFixture(actorUserId)
      case 'hero-schools-transition':
        return await this.seedHeroSchoolsFixture(actorUserId)
      case 'spirit-skill':
        return await this.seedSpiritFixture(actorUserId, scenario.options?.spirit)
      case 'echo-pure-fire':
        return await this.seedEchoFixture(actorUserId, 'pureFire', scenario.options?.mode)
      case 'echo-ringing-metal':
        return await this.seedEchoFixture(actorUserId, 'ringingMetal')
      case 'echo-split-earth':
        return await this.seedEchoFixture(actorUserId, 'splitEarth')
      case 'tribulation-earth-rending':
        return await this.seedTribulationFixture(actorUserId)
      case 'tribulation-rusted-forest':
        return await this.seedRustedForestFixture(actorUserId)
      case 'pouch-chain-sheep':
        return await this.seedPouchChainSheepFixture(actorUserId)
    }
  }

  private async inspectDevelopmentRecord(
    actorUserId: string,
    commandId: string,
  ): Promise<Response> {
    if (await this.isCompetitionRoom()) {
      return this.competitionMutationRejected()
    }
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)
    if (!actor?.owner) {
      return this.json({ error: 'only room owner may inspect a test record' }, 403)
    }

    const commandCommitCount = (await this.events()).filter((event) => (
      event.type === 'RulesCommandApplied' && event.commandId === commandId
    )).length
    const snapshot = await this.requireGameRecord()

    return this.json({
      commandCommitCount,
      canonicalEventTypes: canonicalEventTypes(snapshot.rulesRecord),
    })
  }

  private async seedEndgameFixture(actorUserId: string): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)

    if (!actor?.owner) {
      return this.json({ error: 'only room owner may seed a test fixture' }, 403)
    }
    if (metadata.status !== 'Active') {
      return this.json({ error: 'test fixture requires an active match' }, 409)
    }

    const snapshot = await this.requireGameRecord()
    const teams = [...new Set(snapshot.setup.players.map(player => player.team))]
    const setup: RulesGameSetup = {
      ...snapshot.setup,
      initialHp: teams.map(team => ({ team, hp: 1 })),
    }
    const rules = await callRulesEngine({
      action: { type: 'start' },
      viewer: 'observer',
      setup,
      deckSeed: snapshot.deckSeed,
    })

    await this.ctx.storage.put('gameRecord', {
      ...snapshot,
      setup,
      rulesRecord: rules.record,
    } satisfies GameRecord)
    this.ctx.waitUntil(this.broadcast(metadata))

    return this.json(await this.response(metadata, actorUserId))
  }

  private async seedHeroSchoolsFixture(actorUserId: string): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)

    if (!actor?.owner) {
      return this.json({ error: 'only room owner may seed a test fixture' }, 403)
    }
    if (metadata.status !== 'Active') {
      return this.json({ error: 'test fixture requires an active match' }, 409)
    }

    const snapshot = await this.requireGameRecord()
    if (!snapshot.setup.enabledRuleModules.includes('hero-schools')) {
      return this.json({ error: 'test fixture requires Hero Schools' }, 409)
    }
    const setup: RulesGameSetup = {
      ...snapshot.setup,
      turnOrder: [
        actor.player,
        ...snapshot.setup.turnOrder.filter(player => player !== actor.player),
      ],
    }
    const deckSeed = 'development:hero-schools-transition'
    let rules = await callRulesEngine({
      action: {
        type: 'startDevelopmentScenario',
        player: actor.player,
        scenario: 'hero-schools-transition',
      },
      viewer: actor.player,
      setup,
      deckSeed,
    })
    const transition = await callRulesEngine({
      action: {
        type: 'developmentScenarioAction',
        player: actor.player,
        scenario: 'hero-schools-transition',
      },
      viewer: actor.player,
      setup,
      deckSeed,
      record: rules.record,
    })
    const profession = transition.playableActions.find(
      action => action.type === 'changeProfession',
    )
    if (!profession || profession.type !== 'changeProfession') {
      return this.json({ error: 'test fixture could not find a Mesmer transition Card' }, 500)
    }
    rules = await callRulesEngine({
      action: {
        type: 'changeProfession',
        player: actor.player,
        professionId: profession.id,
        cards: profession.cards,
      },
      viewer: actor.player,
      setup,
      deckSeed,
      record: rules.record,
    })
    const discardChoice = rules.state.pendingChoice?.visibility === 'visible'
      && rules.state.pendingChoice.choice.type === 'card'
      ? { choiceId: rules.state.pendingChoice.choiceId, card: rules.state.pendingChoice.choice.cards[0] }
      : undefined
    if (discardChoice) {
      rules = await callRulesEngine({
        action: {
          type: 'answerChoice',
          player: actor.player,
          choiceId: discardChoice.choiceId,
          answer: { type: 'cards', cards: [discardChoice.card.id] },
        },
        viewer: actor.player,
        setup,
        deckSeed,
        record: rules.record,
      })
    }
    const opponent = setup.turnOrder.find(player => player !== actor.player)
    if (!opponent || rules.state.currentPlayer !== opponent) {
      return this.json({ error: 'test fixture did not advance to the opponent' }, 500)
    }
    rules = await callRulesEngine({
      action: { type: 'refresh' },
      viewer: opponent,
      setup,
      deckSeed,
      record: rules.record,
    })
    const opponentHand = rules.state.hands.find(entry => entry.player === opponent)
    const opponentCard = opponentHand?.cards.kind === 'known'
      ? opponentHand.cards.cards[0]
      : undefined
    if (!opponentCard) {
      return this.json({ error: 'test fixture could not inspect the opponent hand' }, 500)
    }
    const options = await callRulesEngine({
      action: {
        type: 'playableActions',
        player: opponent,
        cards: [opponentCard.id],
      },
      viewer: opponent,
      setup,
      deckSeed,
      record: rules.record,
    })
    const formation = options.playableActions.find(action => action.type === 'performFormation')
    if (!formation || formation.type !== 'performFormation') {
      return this.json({ error: 'test fixture could not find an opponent Formation' }, 500)
    }
    rules = await callRulesEngine({
      action: {
        type: 'performFormation',
        player: opponent,
        formationId: formation.id,
        cards: formation.cards,
        starSubstitutionCard: formation.starSubstitution?.card,
        matchOptionRole: formation.matchOption?.role,
        matchOptionCard: formation.matchOption?.card,
        matchOptionSlots: formation.matchOption?.slots,
      },
      viewer: opponent,
      setup,
      deckSeed,
      record: rules.record,
    })
    const opponentDiscardChoice = rules.state.pendingChoice?.visibility === 'visible'
      && rules.state.pendingChoice.choice.type === 'card'
      ? { choiceId: rules.state.pendingChoice.choiceId, card: rules.state.pendingChoice.choice.cards[0] }
      : undefined
    if (opponentDiscardChoice) {
      rules = await callRulesEngine({
        action: {
          type: 'answerChoice',
          player: opponent,
          choiceId: opponentDiscardChoice.choiceId,
          answer: { type: 'cards', cards: [opponentDiscardChoice.card.id] },
        },
        viewer: opponent,
        setup,
        deckSeed,
        record: rules.record,
      })
    }

    await this.ctx.storage.put('gameRecord', {
      ...snapshot,
      firstPlayer: actor.player,
      deckSeed,
      setup,
      rulesRecord: rules.record,
    } satisfies GameRecord)
    this.ctx.waitUntil(this.broadcast(metadata))

    return this.json(await this.response(metadata, actorUserId))
  }

  private async seedSpiritFixture(
    actorUserId: string,
    spirit: 'Metal' | 'Fire' = 'Metal',
  ): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)

    if (!actor?.owner) {
      return this.json({ error: 'only room owner may seed a test fixture' }, 403)
    }
    if (metadata.status !== 'Active') {
      return this.json({ error: 'test fixture requires an active match' }, 409)
    }

    const snapshot = await this.requireGameRecord()
    if (!snapshot.setup.enabledRuleModules.includes('spirit')) {
      return this.json({ error: 'test fixture requires Spirit' }, 409)
    }
    const setup: RulesGameSetup = {
      ...snapshot.setup,
      turnOrder: [
        actor.player,
        ...snapshot.setup.turnOrder.filter(player => player !== actor.player),
      ],
    }

    const scenarioName = spirit === 'Fire' ? 'spirit-fire' : 'spirit-metal'
    const deckSeed = `development:${scenarioName}`
    let rules = await callRulesEngine({
      action: {
        type: 'startDevelopmentScenario',
        player: actor.player,
        scenario: scenarioName,
      },
      viewer: actor.player,
      setup,
      deckSeed,
    })
    const scenario = await callRulesEngine({
      action: {
        type: 'developmentScenarioAction',
        player: actor.player,
        scenario: scenarioName,
      },
      viewer: actor.player,
      setup,
      deckSeed,
      record: rules.record,
    })
    const summoningAction = scenario.playableActions.find(
      (action): action is Extract<PlayableAction, { type: 'performFormation' }> => (
        action.type === 'performFormation'
      ),
    )
    if (!summoningAction) {
      return this.json({ error: `test fixture could not find two ${spirit} Cards` }, 500)
    }

    rules = await callRulesEngine({
      action: {
        type: 'performFormation',
        player: actor.player,
        formationId: summoningAction.id,
        cards: summoningAction.cards,
      },
      viewer: actor.player,
      setup,
      deckSeed,
      record: rules.record,
    })

    rules = await callRulesEngine({
      action: {
        type: 'prepareDevelopmentScenario',
        player: actor.player,
        scenario: scenarioName,
      },
      viewer: actor.player,
      setup,
      deckSeed,
      record: rules.record,
    })

    await this.ctx.storage.put('gameRecord', {
      ...snapshot,
      setup,
      deckSeed,
      rulesRecord: rules.record,
    } satisfies GameRecord)
    this.ctx.waitUntil(this.broadcast(metadata))

    return this.json(await this.response(metadata, actorUserId))
  }

  private async seedEchoFixture(
    actorUserId: string,
    melody: 'pureFire' | 'ringingMetal' | 'splitEarth',
    mode?: 'actionDetail',
  ): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)

    if (!actor?.owner) {
      return this.json({ error: 'only room owner may seed a test fixture' }, 403)
    }
    if (metadata.status !== 'Active') {
      return this.json({ error: 'test fixture requires an active match' }, 409)
    }

    const snapshot = await this.requireGameRecord()
    if (!snapshot.setup.enabledRuleModules.includes('echo')) {
      return this.json({ error: 'test fixture requires Echo' }, 409)
    }
    const setup: RulesGameSetup = {
      ...snapshot.setup,
      turnOrder: [
        actor.player,
        ...snapshot.setup.turnOrder.filter(player => player !== actor.player),
      ],
    }

    const scenario = {
      pureFire: 'echo-pure-fire',
      ringingMetal: 'echo-ringing-metal',
      splitEarth: 'echo-split-earth',
    }[melody]
    const formationId = {
      pureFire: 'echo:pure-fire',
      ringingMetal: 'echo:ringing-metal',
      splitEarth: 'echo:split-earth',
    }[melody]
    const formationName = {
      pureFire: 'Pure Fire',
      ringingMetal: 'Ringing Metal',
      splitEarth: 'Split Earth',
    }[melody]
    const deckSeed = `development:${scenario}`
    let rules = await callRulesEngine({
      action: {
        type: 'startDevelopmentScenario',
        player: actor.player,
        scenario,
      },
      viewer: actor.player,
      setup,
      deckSeed,
    })
    const actions = await callRulesEngine({
      action: {
        type: 'developmentScenarioAction',
        player: actor.player,
        scenario,
      },
      viewer: actor.player,
      setup,
      deckSeed,
      record: rules.record,
    })
    const formation = actions.playableActions.find(
      (action): action is Extract<PlayableAction, { type: 'performFormation' }> => (
        action.type === 'performFormation' && action.id === formationId
      ),
    )
    if (!formation) {
      return this.json({ error: `test fixture could not find ${formationName} Cards` }, 500)
    }

    if (mode === 'actionDetail') {
      await this.ctx.storage.put('gameRecord', {
        ...snapshot,
        setup,
        deckSeed,
        rulesRecord: rules.record,
      } satisfies GameRecord)
      this.ctx.waitUntil(this.broadcast(metadata))

      return this.json({
        ...await this.response(metadata, actorUserId),
        fixtureCards: formation.cards,
      })
    }

    await this.ctx.storage.put('gameRecord', {
      ...snapshot,
      setup,
      deckSeed,
      rulesRecord: rules.record,
    } satisfies GameRecord)

    // 待選擇狀態是標準交易檢查點。透過與玩家相同的命令路徑建立它，讓測試
    // 固定資料也擁有授權後續選擇回應的持久交易。
    const commandId = `development:${scenario}:${crypto.randomUUID()}`
    const response = await this.submitCommand({
      type: 'submitCommand',
      commandId,
      gameInstanceId: snapshot.gameInstanceId,
      transactionId: `transaction:${commandId}`,
      actorUserId,
      action: {
        type: 'performFormation',
        player: actor.player,
        formationId: formation.id,
        cards: formation.cards,
      },
    })
    const body = await response.clone().json<GameRoomResponse>()
    if (!response.ok || body.receipt?.outcome !== 'accepted' || !body.state.pendingChoice) {
      return this.json({ error: `test fixture did not reach ${formationName} choice` }, 500)
    }

    return response
  }

  private async seedTribulationFixture(actorUserId: string): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)

    if (!actor?.owner) {
      return this.json({ error: 'only room owner may seed a test fixture' }, 403)
    }
    if (metadata.status !== 'Active') {
      return this.json({ error: 'test fixture requires an active match' }, 409)
    }

    const snapshot = await this.requireGameRecord()
    if (!snapshot.setup.enabledRuleModules.includes('tribulation')) {
      return this.json({ error: 'test fixture requires Tribulation' }, 409)
    }
    const setup: RulesGameSetup = {
      ...snapshot.setup,
      turnOrder: [
        actor.player,
        ...snapshot.setup.turnOrder.filter(player => player !== actor.player),
      ],
    }
    const deckSeed = 'development:tribulation-earth-rending'
    let rules = await callRulesEngine({
      action: {
        type: 'startDevelopmentScenario',
        player: actor.player,
        scenario: 'tribulation-earth-rending',
      },
      viewer: actor.player,
      setup,
      deckSeed,
    })
    const actions = await callRulesEngine({
      action: {
        type: 'developmentScenarioAction',
        player: actor.player,
        scenario: 'tribulation-earth-rending',
      },
      viewer: actor.player,
      setup,
      deckSeed,
      record: rules.record,
    })
    const earthRending = actions.playableActions.find(
      (action): action is Extract<PlayableAction, { type: 'performFormation' }> => (
        action.type === 'performFormation'
      ),
    )
    if (!earthRending) {
      return this.json({ error: 'test fixture could not find Earth Rending Cards' }, 500)
    }

    await this.ctx.storage.put('gameRecord', {
      ...snapshot,
      setup,
      deckSeed,
      rulesRecord: rules.record,
    } satisfies GameRecord)

    const commandId = `development:tribulation-earth-rending:${crypto.randomUUID()}`
    const response = await this.submitCommand({
      type: 'submitCommand',
      commandId,
      gameInstanceId: snapshot.gameInstanceId,
      transactionId: `transaction:${commandId}`,
      actorUserId,
      action: {
        type: 'performFormation',
        player: actor.player,
        formationId: earthRending.id,
        cards: earthRending.cards,
      },
    })
    const body = await response.clone().json<GameRoomResponse>()
    if (
      !response.ok
      || body.receipt?.outcome !== 'accepted'
      || body.state.pendingChoice?.choice.type !== 'environment'
      || body.state.pendingChoice.choice.environments.length !== 5
    ) {
      return this.json({ error: 'test fixture did not reach Environment choice' }, 500)
    }

    return response
  }

  private async seedRustedForestFixture(actorUserId: string): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)

    if (!actor?.owner) {
      return this.json({ error: 'only room owner may seed a test fixture' }, 403)
    }
    if (metadata.status !== 'Active') {
      return this.json({ error: 'test fixture requires an active match' }, 409)
    }

    const snapshot = await this.requireGameRecord()
    if (!snapshot.setup.enabledRuleModules.includes('tribulation')) {
      return this.json({ error: 'test fixture requires Tribulation' }, 409)
    }
    const setup: RulesGameSetup = {
      ...snapshot.setup,
      turnOrder: [
        actor.player,
        ...snapshot.setup.turnOrder.filter(player => player !== actor.player),
      ],
    }
    const deckSeed = 'development:tribulation-rusted-forest'
    const rules = await callRulesEngine({
      action: {
        type: 'startDevelopmentScenario',
        player: actor.player,
        scenario: 'tribulation-rusted-forest',
      },
      viewer: actor.player,
      setup,
      deckSeed,
    })
    const actions = await callRulesEngine({
      action: {
        type: 'developmentScenarioAction',
        player: actor.player,
        scenario: 'tribulation-rusted-forest',
      },
      viewer: actor.player,
      setup,
      deckSeed,
      record: rules.record,
    })
    const rustedForest = actions.playableActions.find(
      (action): action is Extract<PlayableAction, { type: 'performFormation' }> => (
        action.type === 'performFormation'
        && action.id === 'tribulation:rusted-forest'
      ),
    )
    if (!rustedForest) {
      return this.json({ error: 'test fixture could not find Rusted Forest Cards' }, 500)
    }

    await this.ctx.storage.put('gameRecord', {
      ...snapshot,
      firstPlayer: actor.player,
      setup,
      deckSeed,
      rulesRecord: rules.record,
    } satisfies GameRecord)
    this.ctx.waitUntil(this.broadcast(metadata))

    return this.json({
      ...await this.response(metadata, actorUserId),
      fixtureAction: {
        type: 'performFormation',
        player: actor.player,
        formationId: rustedForest.id,
        cards: rustedForest.cards,
      },
    })
  }

  private async seedPouchChainSheepFixture(actorUserId: string): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)
    if (!actor?.owner) {
      return this.json({ error: 'only room owner may seed a test fixture' }, 403)
    }
    if (metadata.status !== 'Active') {
      return this.json({ error: 'test fixture requires an active match' }, 409)
    }

    const snapshot = await this.requireGameRecord()
    if (!snapshot.setup.enabledRuleModules.includes('pouch')) {
      return this.json({ error: 'test fixture requires Pouch' }, 409)
    }
    const setup: RulesGameSetup = {
      ...snapshot.setup,
      turnOrder: [
        actor.player,
        ...snapshot.setup.turnOrder.filter(player => player !== actor.player),
      ],
    }
    const deckSeed = 'development:pouch-chain-sheep'
    const rules = await callRulesEngine({
      action: {
        type: 'startDevelopmentScenario',
        player: actor.player,
        scenario: 'pouch-chain-sheep',
      },
      viewer: actor.player,
      setup,
      deckSeed,
    })
    const scenario = await callRulesEngine({
      action: {
        type: 'developmentScenarioAction',
        player: actor.player,
        scenario: 'pouch-chain-sheep',
      },
      viewer: actor.player,
      setup,
      deckSeed,
      record: rules.record,
    })
    const chain = scenario.playableActions.find(
      (action): action is Extract<PlayableAction, { type: 'performFormation' }> => (
        action.type === 'performFormation' && action.id === 'pouch:chain'
      ),
    )
    if (!chain) {
      return this.json({ error: 'test fixture could not find Chain Cards' }, 500)
    }

    await this.ctx.storage.put('gameRecord', {
      ...snapshot,
      firstPlayer: actor.player,
      setup,
      deckSeed,
      rulesRecord: rules.record,
    } satisfies GameRecord)
    this.ctx.waitUntil(this.broadcast(metadata))

    return this.json({
      ...await this.response(metadata, actorUserId),
      fixtureAction: {
        type: 'performFormation',
        player: actor.player,
        formationId: chain.id,
        cards: chain.cards,
      },
    })
  }

  private async submitCommand(
    request: Extract<GameRoomRequest, { type: 'submitCommand' }>,
  ): Promise<Response> {
    if (await this.isCompetitionRoom() && request.action.type === 'start') {
      return this.competitionMutationRejected()
    }
    const executed = await executePlayerCommand({
      storage: this.ctx.storage,
      metadata: () => this.metadata(),
      gameRecord: () => this.requireGameRecord(),
      playerFor: (metadata, userId) => this.playerFor(metadata, userId),
      actionForPlayer: (action, player) => this.actionForPlayer(action, player),
      callRules: (action, viewer, record) => this.callRules(action, viewer, record),
      response: (metadata, actorUserId, playableActions, receipt) => (
        this.response(metadata, actorUserId, playableActions, receipt)
      ),
      shuffle: values => this.shuffle(values),
      competitionDelivery: (record, previous, result, commandId, nextSequence) => (
        this.competitionDeliveryFor(record, previous, result, commandId, nextSequence)
      ),
      competitionCommandFrozen: async (readStorage?: OnlineCommandReadStorage, gameInstanceId?: string) => {
        let binding: CompetitionGameBinding | undefined
        if (readStorage) {
          const storedBinding = await readStorage.get<unknown>('competitionBinding')
          if (storedBinding !== undefined) {
            validateCompetitionGameBinding(storedBinding)
            binding = storedBinding
          }
        } else {
          binding = await this.competitionBinding()
        }
        if (!binding) return false
        const record = gameInstanceId
          ? undefined
          : readStorage
            ? await readStorage.get<GameRecord>('gameRecord')
            : await this.requireGameRecord()
        const activeGameInstanceId = gameInstanceId ?? record?.gameInstanceId
        if (!activeGameInstanceId) return false
        const queue = await this.competitionDeliveryQueue(binding, activeGameInstanceId, readStorage)
        const now = Date.now()
        const deadline = queue.nextClockDeadlineMs ?? null
        const deadlineExpired = !queue.refereePaused && deadline !== null && deadline <= now
        const frozen = Boolean(queue.pendingTimeout || queue.terminalTimeout || queue.refereePaused || deadlineExpired)

        if (deadlineExpired && !readStorage) {
          const observation = await this.enqueueDueClockObservation(
            queue,
            binding,
            activeGameInstanceId,
            now,
          )
          if (observation) await this.persistCompetitionDeliveryQueue(observation)
        }
        if (deadlineExpired || queue.pendingTimeout) {
          this.ctx.waitUntil(this.deliverCompetitionQueue().catch(error => {
            console.error('competition timeout delivery failed', error)
          }))
        }
        return frozen
      },
    }, request)

    if (executed.committed) {
      this.ctx.waitUntil(Promise.all([
        this.broadcast(executed.metadata),
        executed.nextPlayer && executed.nextPlayer !== executed.previousPlayer
          ? this.notifyPlayer(executed.metadata, executed.nextPlayer, {
              kind: 'yourTurn',
              message: `「${executed.metadata.name}」輪到你行動。`,
            })
          : Promise.resolve(),
      ]).catch(error => {
        console.error('post-commit game-room delivery failed', error)
      }))
    }

    // 低延遲送出不屬於玩家命令的公開回應；失敗時由持久 outbox 與 alarm 接手。
    this.ctx.waitUntil(this.deliverCompetitionQueue().catch(error => {
      console.error('competition delivery failed', error)
    }))

    return this.json(executed.response)
  }

  /** Durable Objects 每個房間共用唯一 alarm；alarm 與玩家命令共用 serialized 邊界。 */
  async alarm() {
    // 子房可在公告點前備妥；公告通知只在 T-五分鐘後送出。
    this.queueCompetitionRoomReadyNotice()
    const alarmWork = await this.serialized(async () => {
      const binding = await this.competitionBinding()
      // Waiting alarm 保留原本損壞／未準備時的清除收尾，不透過唯讀查詢修補 metadata。
      const metadata = binding
        ? await this.ctx.storage.get<GameRoomMetadata>('metadata')
        : await this.metadata()
      if (binding && metadata?.status === 'Waiting') {
        let admission = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
        const queue = await this.ctx.storage.get<unknown>(this.competitionDeliveryQueueKey())
        if (binding.gameNumber === 1 && isCompetitionGameAdmission(admission) && admission.status === 'released'
          && Date.now() >= binding.roundStartMs) {
          await this.adjudicateCompetitionNoShowInternal({
            bindingRef: admission.bindingRef,
            commandId: competitionGameNoShowCommandId(admission.bindingRef),
            deadlineMs: binding.roundStartMs,
          })
          admission = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
        }
        if (!isCompetitionSnapshotData(metadata)
          || !hasCompetitionReadyMetadataShape(metadata)
          || !competitionAdmissionMatchesBinding(admission, metadata, binding)
          || !this.competitionStartAdmitted(metadata, binding, admission, Date.now())) {
          if (isCompetitionGameAdmission(admission)) {
            await this.scheduleCompetitionAdmissionAlarm(
              this.ctx.storage, metadata, binding, admission, queue,
            )
          } else await this.ctx.storage.deleteAlarm?.()
          return { kind: 'waiting-not-ready' as const }
        }
        return { kind: 'competition-start' as const, request: { gameId: metadata.gameId, binding } }
      }
      return { kind: 'competition-delivery' as const }
    })

    if (alarmWork.kind === 'competition-start') {
      try {
        await this.startCompetitionGame(alarmWork.request)
      } catch (error) {
        await this.serialized(async () => {
          await this.scheduleCompetitionStartRetry(
            alarmWork.request.gameId,
            alarmWork.request.binding,
          )
        })
        console.error('competition auto-start failed; retry eligibility reevaluated', error)
      }
      return
    }
    if (alarmWork.kind === 'competition-delivery') {
      await this.deliverCompetitionQueue()
    }
  }

  private async connect(request: Request): Promise<Response> {
    if (request.headers.get('Upgrade')?.toLowerCase() !== 'websocket') {
      return this.json({ error: 'websocket upgrade required' }, 426)
    }

    const actorUserId = request.headers.get('x-fewfc-user-id')
    const encodedActorName = request.headers.get('x-fewfc-user-name')

    if (!actorUserId || !encodedActorName) {
      return this.json({ error: 'authenticated player required' }, 401)
    }

    const actorName = decodeURIComponent(encodedActorName)

    const metadata = await this.requireMetadata()
    const competition = await this.competitionBinding()
    if (competition) {
      let admission = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
      if (!competitionAdmissionMatchesBinding(admission, metadata, competition)
        || admission.status === 'revoked') return this.competitionMutationRejected()
      if (competition.gameNumber === 1 && metadata.status === 'Waiting' && admission.status === 'released'
        && Date.now() >= competition.roundStartMs) {
        await this.adjudicateCompetitionNoShowInternal({
          bindingRef: admission.bindingRef,
          commandId: competitionGameNoShowCommandId(admission.bindingRef),
          deadlineMs: competition.roundStartMs,
        })
        admission = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
        if (!competitionAdmissionMatchesBinding(admission, metadata, competition)) {
          return this.competitionMutationRejected()
        }
      }
    }
    const actor = competition
      ? this.memberFor(metadata, actorUserId)
      : this.participantFor(metadata, actorUserId)

    if (!actor) {
      return this.json({ error: 'only room members may connect' }, 403)
    }

    const previousSockets = new Set(this.ctx.getWebSockets())
    const upgraded = await this.socketHandler.handleDurableUpgrade(this, request)
    const server = this.ctx.getWebSockets().find(socket => {
      const attachment = socket.deserializeAttachment<SocketAttachment>()
      return attachment?.userId === actorUserId && socket.readyState === WebSocket.OPEN
        && !previousSockets.has(socket)
    })

    const updatedMetadata: GameRoomMetadata = {
      ...metadata,
      members: metadata.members.map((member) => (
        member.userId === actorUserId
          ? competition
            ? { ...member, connected: true }
            : { ...member, displayName: actorName, connected: true }
          : member
      )),
      observers: metadata.observers.map((observer) => (
        observer.userId === actorUserId
          ? { ...observer, displayName: actorName, connected: true }
          : observer
      )),
      updatedAt: new Date().toISOString(),
    }
    const promotedMetadata = await this.promoteObservers(updatedMetadata)

    if (competition && promotedMetadata.status === 'Waiting') {
      await this.persistCompetitionWaitingMetadata(promotedMetadata, competition)
      if (await this.competitionCanAutoStart(promotedMetadata, competition)) {
        this.queueCompetitionAutoStart(promotedMetadata.gameId, competition)
      }
    } else {
      await this.ctx.storage.put('metadata', promotedMetadata)
    }
    server?.send(JSON.stringify({
      type: 'roomState',
      data: await this.response(promotedMetadata, actorUserId),
    }))
    this.ctx.waitUntil(this.broadcast(promotedMetadata))

    return upgraded
  }

  private async disconnect(socket: WebSocket) {
    const attachment = socket.deserializeAttachment() as SocketAttachment | null

    if (!attachment?.userId) {
      return
    }

    const hasAnotherConnection = this.ctx.getWebSockets().some((candidate) => {
      if (candidate === socket || candidate.readyState !== WebSocket.OPEN) {
        return false
      }

      const candidateAttachment = candidate.deserializeAttachment() as SocketAttachment | null
      return candidateAttachment?.userId === attachment.userId
    })

    if (hasAnotherConnection) {
      return
    }

    const metadata = await this.metadata()

    if (!metadata) {
      return
    }

    // 已離房或被移除的舊分頁不得再寫回任何新 membership。
    const competition = await this.competitionBinding()
    const member = this.memberFor(metadata, attachment.userId)
    const participant = competition
      ? member
      : member ?? this.observerFor(metadata, attachment.userId)
    if (!participant) {
      return
    }

    const clearsReadyOnDisconnect = metadata.status === 'Waiting'
      && (competition !== undefined || ('owner' in participant && !participant.owner))
    const unreadyEvent = competition && metadata.status === 'Waiting' && member?.ready
      ? { type: 'PlayerUnready', actor: member.player, payload: { player: member.player } }
      : undefined
    const updatedMetadata: GameRoomMetadata = {
      ...metadata,
      members: metadata.members.map((member) => (
        member.userId === attachment.userId
          ? {
              ...member,
              connected: false,
              ready: clearsReadyOnDisconnect ? false : member.ready,
            }
          : member
      )),
      observers: metadata.observers.map((observer) => (
        observer.userId === attachment.userId ? { ...observer, connected: false } : observer
      )),
      updatedAt: new Date().toISOString(),
    }

    if (competition && updatedMetadata.status === 'Waiting') {
      const disconnectedAtMs = Date.now()
      if (competition.gameNumber === 1 && disconnectedAtMs > competition.roundStartMs) {
        const admission = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
        if (competitionAdmissionMatchesBinding(admission, metadata, competition)
          && admission.status === 'released') {
          await this.adjudicateCompetitionNoShowInternal({
            bindingRef: admission.bindingRef,
            commandId: competitionGameNoShowCommandId(admission.bindingRef),
            deadlineMs: competition.roundStartMs,
          })
        }
      }
      await this.persistCompetitionWaitingMetadata(updatedMetadata, competition, unreadyEvent, {
        allowNoShow: true,
      })
      if (competition.gameNumber === 1 && disconnectedAtMs === competition.roundStartMs) {
        const admission = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
        if (competitionAdmissionMatchesBinding(admission, updatedMetadata, competition)
          && admission.status === 'released') {
          await this.adjudicateCompetitionNoShowInternal({
            bindingRef: admission.bindingRef,
            commandId: competitionGameNoShowCommandId(admission.bindingRef),
            deadlineMs: competition.roundStartMs,
          })
        }
      }
      if (await this.competitionCanAutoStart(updatedMetadata, competition)) {
        this.queueCompetitionAutoStart(updatedMetadata.gameId, competition)
      }
    } else {
      await this.ctx.storage.put('metadata', updatedMetadata)
    }
    await this.broadcast(updatedMetadata)
  }

  private async afterRoomMutation(
    metadata: GameRoomMetadata,
    notification?: Pick<PlayerNotification, 'kind' | 'message'>,
  ) {
    await this.broadcast(metadata)

    if (notification) {
      await this.notifyMembers(metadata, notification)
    }
  }

  private async broadcast(metadata?: GameRoomMetadata) {
    const excludeSocketId = this.replySocketId
    // 不使用呼叫端暫存的 metadata，避免舊廣播外洩離房後的私人房間狀態。
    const currentMetadata = await this.requireMetadata()
    const competition = await this.competitionBinding()

    await Promise.all(this.ctx.getWebSockets().map(async (socket) => {
      if (socket.readyState !== WebSocket.OPEN) {
        return
      }

      const attachment = socket.deserializeAttachment() as SocketAttachment | null

      if (excludeSocketId && attachment?.i === excludeSocketId) return

      if (!attachment?.userId) {
        return
      }

      const participant = competition
        ? this.memberFor(currentMetadata, attachment.userId)
        : this.participantFor(currentMetadata, attachment.userId)
      if (!participant) {
        socket.close(4000, 'room membership ended')
        return
      }

      try {
        socket.send(JSON.stringify({
          type: 'roomState',
          data: await this.response(currentMetadata, attachment.userId),
        }))
      } catch {
        socket.close(1011, 'state update failed')
      }
    }))
  }

  private broadcastDissolved(gameId: string) {
    for (const socket of this.ctx.getWebSockets()) {
      if (this.replySocketId && socket.deserializeAttachment<SocketAttachment>()?.i === this.replySocketId) continue
      if (socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify({ type: 'roomDissolved', gameId }))
        socket.close(1000, 'room dissolved')
      }
    }
  }

  private closeUserSockets(userId: string, code: number, reason: string) {
    for (const socket of this.ctx.getWebSockets()) {
      const attachment = socket.deserializeAttachment() as SocketAttachment | null

      if (this.replySocketId && attachment?.i === this.replySocketId) continue

      if (attachment?.userId === userId && socket.readyState === WebSocket.OPEN) {
        socket.close(code, reason)
      }
    }
  }

  private async notifyMembers(
    metadata: GameRoomMetadata,
    notification: Pick<PlayerNotification, 'kind' | 'message'>,
  ) {
    await Promise.all([...metadata.members, ...metadata.observers].map((member) => (
      this.sendNotification(member.userId, notification)
    )))
  }

  private async notifyPlayer(
    metadata: GameRoomMetadata,
    player: PlayerId,
    notification: Pick<PlayerNotification, 'kind' | 'message'>,
  ) {
    const member = metadata.members.find((candidate) => candidate.player === player)

    if (member) {
      await this.sendNotification(member.userId, notification)
    }
  }

  private async sendNotification(
    userId: string,
    notification: Pick<PlayerNotification, 'kind' | 'message'>,
    identity?: Pick<PlayerNotification, 'id' | 'gameId' | 'createdAt'>,
  ) {
    const id = this.env.PLAYER_NOTIFICATIONS.idFromName('global')
    const target = this.env.PLAYER_NOTIFICATIONS.get(id)

    const response = await target.fetch('https://player-notifications.internal/notify', {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
      },
      body: JSON.stringify({
        targetUserId: userId,
        notification: {
          id: identity?.id ?? crypto.randomUUID(),
          gameId: identity?.gameId ?? (await this.requireMetadata()).gameId,
          kind: notification.kind,
          message: notification.message,
          createdAt: identity?.createdAt ?? new Date().toISOString(),
        },
      } satisfies NotificationEnvelope),
    })
    if (!response.ok) throw new Error('player notification delivery failed')
  }

  private randomSetup(
    metadata: GameRoomMetadata,
    deckLists: Array<PlayerDeckList & { player: PlayerId }>,
  ): RulesGameSetup {
    const turnOrder = this.shuffle(metadata.members.map((member) => member.player))

    if (metadata.capacity === 2) {
      return {
        players: turnOrder.map((player, index) => ({
          id: player,
          team: `team-${index + 1}`,
        })),
        turnOrder,
        ruleVersion: metadata.ruleVersion,
        enabledRuleModules: metadata.enabledRuleModules,
        deckLists,
      }
    }

    const teamByPlayer = new Map<PlayerId, string>()
    turnOrder.forEach((player, index) => {
      teamByPlayer.set(player, index % 2 === 0 ? 'team-a' : 'team-b')
    })

    return {
      players: metadata.members.map((member) => ({
        id: member.player,
        team: teamByPlayer.get(member.player) ?? 'team-a',
      })),
      turnOrder,
      ruleVersion: metadata.ruleVersion,
      enabledRuleModules: [...metadata.enabledRuleModules],
      deckLists,
    }
  }

  private shuffle<T>(values: T[]): T[] {
    const shuffled = [...values]

    for (let index = shuffled.length - 1; index > 0; index -= 1) {
      const random = new Uint32Array(1)
      crypto.getRandomValues(random)
      const swapIndex = (random[0] ?? 0) % (index + 1)
      const current = shuffled[index]
      shuffled[index] = shuffled[swapIndex] as T
      shuffled[swapIndex] = current as T
    }

    return shuffled
  }

  private async response(
    metadata?: GameRoomMetadata,
    actorUserId?: string,
    playableActions?: PlayableAction[],
    receipt?: CommandReceipt,
  ): Promise<GameRoomResponse> {
    const currentMetadata = metadata ?? await this.requireMetadata()
    const competition = await this.competitionBinding()
    if (competition && (!actorUserId || !this.memberFor(currentMetadata, actorUserId))) {
      throw new Error('競賽進行中僅限固定成員查詢房間狀態。')
    }
    const lockedDeckName = actorUserId
      && !competition
      ? (await this.ctx.storage.get<PlayerDeckList>(this.lockedDeckKey(actorUserId)))?.name
      : undefined
    const viewer = actorUserId
      ? (this.playerFor(currentMetadata, actorUserId) ?? 'observer')
      : 'observer'

    if (currentMetadata.status === 'Waiting' || currentMetadata.status === 'Dissolved') {
      const completed = actorUserId
        ? await this.ctx.storage.get<CompletedReplayDraft>('lastCompletedReplayDraft')
        : undefined
      return {
        gameId: currentMetadata.gameId,
        metadata: currentMetadata,
        ...(competition ? { competition: projectGameRoomCompetitionContext(competition) } : {}),
        invitation: competition ? undefined : await this.invitationFor(currentMetadata, actorUserId),
        lockedDeckName,
        savableReplay: completed && completed.originalUserIds.includes(actorUserId ?? '')
          ? {
              replayId: completed.replayId,
              sourceGameId: completed.sourceGameId,
              finishedAt: completed.finishedAt,
            }
          : undefined,
        state: { ...emptyPublicState(currentMetadata.players), ruleVersion: currentMetadata.ruleVersion, enabledRuleModules: currentMetadata.enabledRuleModules },
        battleRecord: systemBattleRecord(await this.events(), event => ({
          title: this.eventTitle(event),
          summary: this.displaySummary(currentMetadata, this.eventSummary(event)),
        })),
        playableActions: playableActions ?? [],
        interaction: {
          canChooseInitialPouch: false,
        },
      }
    }

    const snapshot = await this.requireGameRecord()
    const publicRules = await this.callReadyRules({ type: 'refresh' }, viewer, snapshot)
    // refresh 是純讀取；Finished metadata 必須由 canonical checkpoint transaction
    // 與 gameRecord／receipt／event／outbox 一起提交，避免讀取路徑留下 crash gap。
    const responseMetadata = currentMetadata
    const activeTransactionId = responseMetadata.gameInstanceId
      ? (await this.ctx.storage.get<{ transactionId: string }>(
          `transaction:${responseMetadata.gameInstanceId}`,
        ))?.transactionId
      : undefined

    return {
      gameId: currentMetadata.gameId,
      metadata: responseMetadata,
      ...(competition ? { competition: projectGameRoomCompetitionContext(competition) } : {}),
      invitation: competition ? undefined : await this.invitationFor(currentMetadata, actorUserId),
      lockedDeckName,
      state: publicRules.state,
      battleRecord: this.displayBattleRecord(currentMetadata, publicRules.battleRecord),
      playableActions: playableActions ?? publicRules.playableActions,
      interaction: publicRules.interaction,
      activeTransactionId,
      activeGameVersion: {
        gameInstanceId: snapshot.gameInstanceId,
        recordSequence: snapshot.sequence,
      },
      receipt,
    }
  }

  private async invitationFor(
    metadata: GameRoomMetadata,
    actorUserId?: string,
  ): Promise<GameRoomInvitation | undefined> {
    const actor = actorUserId ? this.memberFor(metadata, actorUserId) : undefined
    if (!actor?.owner || metadata.status !== 'Waiting') {
      return undefined
    }

    return await this.ctx.storage.get<GameRoomInvitation>('invitation')
  }

  private playerFor(metadata: GameRoomMetadata, userId: string): PlayerId | undefined {
    return this.memberFor(metadata, userId)?.player
  }

  private memberFor(metadata: GameRoomMetadata, userId: string) {
    return metadata.members.find((member) => member.userId === userId)
  }

  private observerFor(metadata: GameRoomMetadata, userId: string) {
    return metadata.observers.find((observer) => observer.userId === userId)
  }

  private participantFor(metadata: GameRoomMetadata, userId: string) {
    return this.memberFor(metadata, userId) ?? this.observerFor(metadata, userId)
  }

  /** 僅在等待房補席，並按觀戰者原始加入順序略過離線者。 */
  private async promoteObservers(metadata: GameRoomMetadata): Promise<GameRoomMetadata> {
    if (metadata.status !== 'Waiting') return metadata
    if (await this.isCompetitionRoom()) return metadata

    let current = metadata
    while (true) {
      const occupied = new Set(current.members.map(member => member.player))
      const seat = current.players.find(player => !occupied.has(player))
      const observer = current.observers.find(candidate => candidate.connected)
      if (!seat || !observer) return current

      current = {
        ...current,
        members: [...current.members, {
          userId: observer.userId,
          displayName: observer.displayName,
          player: seat,
          ready: false,
          connected: true,
          owner: false,
        }],
        observers: current.observers.filter(candidate => candidate.userId !== observer.userId),
        updatedAt: new Date().toISOString(),
      }
      await this.ctx.storage.put('metadata', current)

      // 透過既有通知通道告知補位；前端不讓一般 roomChanged 蓋掉這則通知。
      this.ctx.waitUntil(this.sendNotification(observer.userId, {
        kind: 'seatPromoted',
        message: '已補為玩家，請準備',
      }))
    }
  }

  private lockedDeckKey(userId: string) {
    return `lockedDeck:${userId}`
  }

  private competitionDeliveryQueueKey() {
    return 'competitionDeliveryQueue'
  }

  private async competitionDeliveryFor(
    record: GameRecord,
    previous: CompetitionClockTransitionState,
    result: RulesReadyResult,
    sourceCommandId: string,
    nextSequence: number,
  ): Promise<CanonicalCompetitionDeliveryCommit | undefined> {
    const binding = await this.competitionBinding()
    if (!binding) return undefined
    const queue = await this.competitionDeliveryQueue(binding, record.gameInstanceId)
    const now = Date.now()
    const operations = competitionClockOperationsForTransition(binding, previous, result.state)
    const clockEntries: CompetitionClockDeliveryEntry[] = []
    for (const [index, operation] of operations.entries()) {
      const commandId = await competitionClockEventIdentity(
        record.gameInstanceId,
        nextSequence,
        sourceCommandId,
        index,
        operation,
      )
      clockEntries.push(await competitionClockEntry(
        binding,
        commandId,
        operation,
        now,
      ))
    }
    const entries = [...queue.entries, ...clockEntries]

    if (result.state.status === 'Finished') {
      const conclusion = result.state.gameConclusion
      const validConclusion = isCompetitionGameConclusion(conclusion)
      const setupTeamsMatch = validConclusion && conclusionUsesSetupTeams(conclusion, record)
      const hasTerminalEvent = hasCanonicalTerminalRecord(result.record, conclusion)
      if (!validConclusion || !setupTeamsMatch || !hasTerminalEvent) {
        // 競賽房的 Finished 若沒有 canonical conclusion，不得提交任何 checkpoint。
        throw new Error('competition terminal checkpoint has no valid game conclusion')
      }
      const firstPlayer = binding.players.find(player => player.player === binding.firstPlayer)
      if (!firstPlayer) throw new Error('competition terminal checkpoint has no bound first player')
      const identity = await competitionTerminalIdentity(
        binding.competitionId,
        binding.matchId,
        binding.gameNumber,
        record.gameInstanceId,
      )
      entries.push({
        schemaVersion: 1,
        kind: 'competitionGameTerminal',
        competitionId: binding.competitionId,
        matchId: binding.matchId,
        gameNumber: binding.gameNumber,
        gameInstanceId: record.gameInstanceId,
        commandId: `competition-terminal:${identity}`,
        submissionId: `competition-submission:${identity}`,
        gameRecordId: `competition-game-record:${identity}`,
        rulingRecordId: `competition-ruling:${identity}`,
        callerUserId: binding.organizerUserId,
        startKind: binding.startKind,
        firstPlayer: firstPlayer.team,
        trustedTimeMs: now,
        terminalEvidence: { type: 'canonicalGameEnded', conclusion },
        attempts: 0,
        nextAttemptAt: now,
      })
    }
    if (entries.length === queue.entries.length) return undefined

    const nextQueue = { ...queue, entries }
    return {
      storageKey: this.competitionDeliveryQueueKey(),
      value: nextQueue,
      alarmAt: nextCompetitionAlarmAt(nextQueue, now) ?? now,
    }
  }

  private async deliverCompetitionQueue(): Promise<void> {
    // FIFO 交付另行合併，避免同一 head 被並行 drain；本地 mutator 不等遠端 ACK。
    if (this.competitionDeliveryInFlight) return await this.competitionDeliveryInFlight
    const delivery = this.drainCompetitionQueue()
    this.competitionDeliveryInFlight = delivery
    try {
      await delivery
    } finally {
      this.competitionDeliveryInFlight = null
    }
  }

  private async drainCompetitionQueue(): Promise<void> {
    const storage: GameRoomAtomicStorage = this.ctx.storage
    const work = await this.serialized(async () => {
      const binding = await this.competitionBinding()
      if (!binding) return
      const record = await storage.get<GameRecord>('gameRecord')
      if (!record) return
      const rawQueue = await storage.get<unknown>(this.competitionDeliveryQueueKey())
      if (rawQueue === undefined) return
      if (!isCompetitionDeliveryQueue(rawQueue)
        || !competitionQueueMatchesBinding(rawQueue, binding, record.gameInstanceId)) {
        await this.rescheduleCompetitionDelivery(rawQueue, 'invalid persisted competition delivery queue')
        return
      }
      let queue = normalizeCompetitionDeliveryQueue(rawQueue)
      if (queue.terminalTimeout) {
        if (storage.deleteAlarm) await storage.deleteAlarm()
        return
      }
      const now = Date.now()
      const observed = await this.enqueueDueClockObservation(queue, binding, record.gameInstanceId, now)
      if (observed) {
        await this.persistCompetitionDeliveryQueue(observed)
        queue = observed
      }
      if (queue.entries.length === 0) {
        const alarmAt = nextCompetitionAlarmAt(queue, now)
        if (alarmAt === null) {
          if (storage.deleteAlarm) await storage.deleteAlarm()
        } else if (storage.setAlarm) {
          await storage.setAlarm(alarmAt)
        }
        return
      }
      const entry = queue.entries[0]
      if (!entry) return
      if ((entry.kind === 'competitionInitialGameStarted'
        && !this.competitionInitialEntryMatchesStart(await storage.list<unknown>(), binding, entry))
        || (binding.gameNumber === 1 && entry.kind !== 'competitionInitialGameStarted'
          && queue.revisionCursor <= binding.expectedMatchRevision)) {
        await this.rescheduleCompetitionDelivery(queue, 'invalid persisted initial start evidence')
        return
      }
      if (entry.nextAttemptAt > now) {
        const alarmAt = nextCompetitionAlarmAt(queue, now)
        if (alarmAt !== null && storage.setAlarm) await storage.setAlarm(alarmAt)
        return
      }

      const namespace = this.env.COMPETITION_ROOM
      if (!namespace) {
        await this.rescheduleCompetitionDelivery(rawQueue, 'competition room binding unavailable')
        return
      }
      const common = { binding, record, rawQueue: queue, queue, entry, namespace }
      if (entry.kind === 'competitionInitialGameStarted') {
        return { ...common, kind: entry.kind, request: competitionInitialStartRequest(entry) }
      }
      if (entry.kind === 'competitionClockMutation') {
        return { ...common, kind: entry.kind, request: {
          competitionId: entry.competitionId,
          matchId: entry.matchId,
          callerUserId: entry.callerUserId,
          commandId: entry.commandId,
          expectedMatchRevision: queue.revisionCursor,
          trustedTimeMs: entry.trustedTimeMs,
          operation: entry.operation,
        } }
      }
      return { ...common, kind: entry.kind, request: {
        competitionId: entry.competitionId,
        matchId: entry.matchId,
        callerUserId: entry.callerUserId,
        commandId: entry.commandId,
        expectedMatchRevision: queue.revisionCursor,
        trustedTimeMs: entry.trustedTimeMs,
        submissionId: entry.submissionId,
        gameRecordId: entry.gameRecordId,
        rulingRecordId: entry.rulingRecordId,
        startKind: entry.startKind,
        firstPlayer: entry.firstPlayer,
        terminalEvidence: entry.terminalEvidence,
      } }
    })
    if (!work) return
    const { binding, record, rawQueue, queue, entry, namespace } = work
    // foreign RPC 不持有 room mutator；撤銷／release ACK 不會等待父房 delivery。

    let ack: ReturnType<typeof parseCompetitionDeliveryAck>
    try {
      const target = namespace.get(namespace.idFromName(entry.competitionId))
      let response: unknown
      if (work.kind === 'competitionInitialGameStarted') {
        const ingest = target.ingestInitialGameStarted
        if (typeof ingest !== 'function') throw new Error('initial start RPC unavailable')
        response = await Reflect.apply(ingest, target, [work.request])
      } else if (work.kind === 'competitionClockMutation') {
        if (!target.executeMatch) throw new Error('competition match execution RPC unavailable')
        response = await target.executeMatch(work.request)
      } else {
        response = await target.ingestGameTerminal(work.request)
      }
      // 只釋放本次回傳 root；namespace 與共用 stub 不屬於此 ACK 的生命週期。
      ack = parseCompetitionDeliveryRpcAck(response, entry, queue.revisionCursor)
    } catch {
      await this.serialized(() => this.rescheduleCompetitionDelivery(rawQueue, 'competition room RPC failed'))
      return
    }
    if (!ack) {
      await this.serialized(() => this.rescheduleCompetitionDelivery(rawQueue, 'competition room returned an invalid acknowledgement'))
      return
    }

    const timeoutTerminal = ack.timeoutEvidence
      ? await competitionTimeoutDeliveryEntry(binding, record.gameInstanceId, ack.timeoutEvidence)
      : undefined
    const committed = await this.serialized(() => storage.transaction(async transaction => {
      const current = await transaction.get<unknown>(this.competitionDeliveryQueueKey())
      if (!isCompetitionDeliveryQueue(current)) throw new Error('competition delivery queue changed shape')
      const normalized = normalizeCompetitionDeliveryQueue(current)
      if (normalized.revisionCursor !== queue.revisionCursor
        || stableDeliveryFingerprint(normalized.entries[0]) !== stableDeliveryFingerprint(entry)
        || !competitionQueueMatchesBinding(normalized, binding, record.gameInstanceId)
        || (entry.kind === 'competitionInitialGameStarted'
          && !this.competitionInitialEntryMatchesStart(await transaction.list<unknown>(), binding, entry))) return false
      const timeoutTerminalAck = entry.kind === 'competitionGameTerminal'
        && entry.terminalEvidence.type === 'timeout'
      const entries = timeoutTerminal
        ? [timeoutTerminal]
        : timeoutTerminalAck
          ? []
          : normalized.entries.slice(1)
      const next: CompetitionDeliveryQueue = {
        schemaVersion: 1,
        revisionCursor: normalized.revisionCursor + 1,
        entries,
        receipts: normalized.receipts,
        nextClockDeadlineMs: ack.timeoutEvidence || timeoutTerminalAck
          ? null
          : entry.kind !== 'competitionGameTerminal'
            ? ack.nextClockDeadlineMs
            : null,
        terminalTimeout: timeoutTerminalAck
          ? entry.terminalEvidence.type === 'timeout' ? entry.terminalEvidence : null
          : normalized.terminalTimeout ?? null,
        refereePaused: normalized.refereePaused,
        clockControlReceipts: normalized.clockControlReceipts,
        pendingTimeout: ack.timeoutEvidence
          ? {
              schemaVersion: 1,
              kind: 'competitionPendingTimeout',
              competitionId: entry.competitionId,
              matchId: entry.matchId,
              commandId: entry.commandId,
              timeoutEvidence: ack.timeoutEvidence,
            }
          : timeoutTerminalAck ? null : normalized.pendingTimeout,
      }
      await transaction.put({ [this.competitionDeliveryQueueKey()]: next })
      if (!transaction.setAlarm) throw new Error('competition delivery storage does not support alarms')
      const alarmAt = next.terminalTimeout ? null : nextCompetitionAlarmAt(next, Date.now())
      if (alarmAt === null) await transaction.deleteAlarm?.()
      else await transaction.setAlarm(alarmAt)
      return true
    }))
    // 同一個有序 drain 可繼續處理下一個已持久化項目；每次遞迴前都重新讀取
    // queue/cursor，確保只有上一筆嚴格 ACK 後才會送出下一筆。
    if (committed) await this.drainCompetitionQueue()
  }

  private async enqueueDueClockObservation(
    queue: CompetitionDeliveryQueue,
    binding: CompetitionGameBinding,
    gameInstanceId: string,
    now: number,
  ): Promise<CompetitionDeliveryQueue | null> {
    if (queue.refereePaused) return null
    const deadline = queue.nextClockDeadlineMs ?? null
    if (deadline === null || deadline > now
      || queue.entries.some(entry => entry.kind === 'competitionGameTerminal')
      || queue.entries.some(entry => entry.kind === 'competitionClockMutation'
        && entry.operation.type === 'clockObserve'
        && entry.trustedTimeMs === deadline)) return null

    const commandId = await competitionDeadlineIdentity(
      gameInstanceId,
      queue.revisionCursor,
      deadline,
    )
    const operation: CompetitionClockOperation = {
      type: 'clockObserve',
      matchId: binding.matchId,
    }
    return {
      ...queue,
      entries: [...queue.entries, await competitionClockEntry(
        binding,
        commandId,
        operation,
        deadline,
      )],
    }
  }

  private async rescheduleCompetitionDelivery(
    queue: unknown,
    reason: string,
  ): Promise<void> {
    const storage: GameRoomAtomicStorage = this.ctx.storage
    if (!isCompetitionDeliveryQueue(queue)) {
      if (storage.setAlarm) await storage.setAlarm(Date.now() + 60_000)
      return
    }
    const normalized = normalizeCompetitionDeliveryQueue(queue)
    const head = normalized.entries[0]
    if (!head) {
      const alarmAt = nextCompetitionAlarmAt(normalized, Date.now())
      if (alarmAt !== null && storage.setAlarm) await storage.setAlarm(alarmAt)
      return
    }
    const attempts = head.attempts + 1
    const nextAttemptAt = Date.now() + Math.min(60_000, 1_000 * 2 ** Math.min(attempts, 6))
    try {
      await storage.transaction(async transaction => {
        const current = await transaction.get<unknown>(this.competitionDeliveryQueueKey())
        if (!isCompetitionDeliveryQueue(current)
          || current.revisionCursor !== normalized.revisionCursor
          || stableDeliveryFingerprint(current.entries[0]) !== stableDeliveryFingerprint(head)) return
        const normalizedCurrent = normalizeCompetitionDeliveryQueue(current)
        const nextQueue = {
          ...normalizedCurrent,
          entries: [{ ...normalizedCurrent.entries[0]!, attempts, nextAttemptAt }, ...normalizedCurrent.entries.slice(1)],
        }
        await transaction.put({
          [this.competitionDeliveryQueueKey()]: nextQueue,
        })
        if (!transaction.setAlarm) {
          throw new Error('competition delivery storage does not support alarms')
        }
        const alarmAt = nextCompetitionAlarmAt(nextQueue, Date.now())
        if (alarmAt === null) await transaction.deleteAlarm?.()
        else await transaction.setAlarm(alarmAt)
      })
    } catch (error) {
      console.error(`competition delivery retained: ${reason}`, error)
    }
  }

  private async persistCompetitionDeliveryQueue(queue: CompetitionDeliveryQueue) {
    const storage: GameRoomAtomicStorage = this.ctx.storage
    await storage.transaction(async transaction => {
      if (!transaction.setAlarm) throw new Error('competition delivery storage does not support alarms')
      await transaction.put({ [this.competitionDeliveryQueueKey()]: queue })
      const alarmAt = nextCompetitionAlarmAt(queue, Date.now())
      if (alarmAt === null) await transaction.deleteAlarm?.()
      else await transaction.setAlarm(alarmAt)
    })
  }

  private async competitionDeliveryQueue(
    binding: CompetitionGameBinding,
    gameInstanceId?: string,
    readStorage?: OnlineCommandReadStorage,
  ): Promise<CompetitionDeliveryQueue> {
    const raw = readStorage
      ? await readStorage.get<unknown>(this.competitionDeliveryQueueKey())
      : await this.ctx.storage.get<unknown>(this.competitionDeliveryQueueKey())
    if (raw === undefined) return emptyCompetitionDeliveryQueue(binding.expectedMatchRevision)
    if (!isCompetitionDeliveryQueue(raw)
      || !competitionQueueMatchesBinding(raw, binding, gameInstanceId)
      || raw.revisionCursor < binding.expectedMatchRevision) {
      throw new Error('competition delivery queue is invalid')
    }
    return normalizeCompetitionDeliveryQueue(raw)
  }

  private async competitionBinding(): Promise<CompetitionGameBinding | undefined> {
    const stored = await this.ctx.storage.get<unknown>('competitionBinding')
    if (stored === undefined) return undefined
    validateCompetitionGameBinding(stored)
    return stored
  }

  private async isCompetitionRoom(): Promise<boolean> {
    return (await this.ctx.storage.get(competitionAdmissionKey)) !== undefined
      || (await this.competitionBinding()) !== undefined
  }

  private async validateCompetitionRules(binding: CompetitionGameBinding): Promise<void> {
    try {
      const resolved = await callRuleModuleResolution(binding.enabledRuleModules, binding.ruleVersion)
      if (JSON.stringify(resolved.modules) !== JSON.stringify(binding.enabledRuleModules)) {
        throw new CompetitionGameBindingError('invalidCompetitionBinding')
      }
    } catch (error) {
      if (error instanceof CompetitionGameBindingError) throw error
      throw new CompetitionGameBindingError('invalidCompetitionBinding')
    }
  }

  private competitionResult(
    metadata: GameRoomMetadata,
    idempotent: boolean,
  ): CompetitionGameRoomResult {
    return {
      gameId: metadata.gameId,
      ...(metadata.gameInstanceId ? { gameInstanceId: metadata.gameInstanceId } : {}),
      status: metadata.status,
      idempotent,
    }
  }

  private hasDurableCompetitionStart(
    current: Map<string, unknown>,
    metadata: GameRoomMetadata,
    binding: CompetitionGameBinding,
    admission: CompetitionGameAdmission,
  ): boolean {
    const start = admission.start
    if (admission.status !== 'started' || !start
      || (metadata.status !== 'Active' && metadata.status !== 'Finished')) return false
    const record = current.get('gameRecord')
    const event = current.get(this.eventKey(start.startSequence))
    return isCompetitionSnapshotData(record) && isCompetitionSnapshotData(event)
      && hasCompetitionSnapshotKeys(record, [
        'schemaVersion', 'gameInstanceId', 'sequence', 'firstPlayer', 'deckSeed', 'setup', 'rulesRecord',
      ], ['finishedAt'])
      && hasCompetitionSnapshotKeys(event, ['sequence', 'type', 'payload', 'createdAt'])
      && this.isCompleteCompetitionGameRecord(record, metadata, binding)
      && record.gameInstanceId === start.gameInstanceId && record.sequence >= start.startSequence
      && isRecord(event) && event.type === 'GameStarted' && event.sequence === start.startSequence
      && event.createdAt === new Date(start.startedAtMs).toISOString()
      && isRecord(event.payload) && hasExactKeys(event.payload, ['gameInstanceId', 'firstPlayer'])
      && event.payload.gameInstanceId === start.gameInstanceId && event.payload.firstPlayer === binding.firstPlayer
  }

  /** 結構 guard 不是授權：交叉比對本房完整 binding、release 與真 durable GameStarted。 */
  private competitionInitialEntryMatchesStart(
    current: Map<string, unknown>,
    binding: CompetitionGameBinding,
    entry: CompetitionInitialGameStartedEntry,
  ): boolean {
    const metadata = current.get('metadata')
    const admission = current.get(competitionAdmissionKey)
    const evidence = current.get(competitionInitialStartEvidenceKey)
    const storedBinding = current.get('competitionBinding')
    if (binding.gameNumber !== 1 || !competitionBindingIsValid(storedBinding)
      || !competitionGameBindingEquals(storedBinding, binding)
      || !isCompetitionSnapshotData(metadata) || !hasCompetitionReadyMetadataShape(metadata)
      || !competitionAdmissionMatchesBinding(admission, metadata, binding)
      || !this.competitionMembersMatchBinding(metadata, binding)
      || admission.releaseCommandId === null || admission.start === null
      || !this.hasDurableCompetitionStart(current, metadata, binding, admission)
      || !initialEvidenceMatchesAdmission(evidence, admission)) return false
    return entry.expectedMatchRevision === binding.expectedMatchRevision
      && stableDeliveryFingerprint(entry.evidence) === stableDeliveryFingerprint(evidence)
  }

  private competitionStartAdmitted(
    metadata: GameRoomMetadata,
    binding: CompetitionGameBinding,
    admission: CompetitionGameAdmission,
    now: number,
  ): boolean {
    return competitionAdmissionMatchesBinding(admission, metadata, binding)
      && admission.status === 'released' && metadata.status === 'Waiting'
      && Number.isSafeInteger(now) && now >= 0 && now <= 8_640_000_000_000_000
      && this.competitionMembersMatchBinding(metadata, binding)
      && metadata.members.every(member => member.connected && member.ready)
  }

  private async scheduleCompetitionAdmissionAlarm(
    storage: GameRoomAlarmStorage,
    metadata: unknown,
    binding: unknown,
    admission: CompetitionGameAdmission,
    queue: unknown,
  ): Promise<void> {
    if (!storage.setAlarm) throw new Error('competition admission storage does not support alarms')
    const now = Date.now()
    const candidates: number[] = []
    if (isCompetitionDeliveryQueue(queue)) {
      const deliveryAt = nextCompetitionAlarmAt(queue, now)
      if (deliveryAt !== null) candidates.push(deliveryAt)
    }
    if (competitionBindingIsValid(binding) && hasCompetitionReadyMetadataShape(metadata)
      && competitionAdmissionMatchesBinding(admission, metadata, binding)
      && (admission.status === 'released' || admission.status === 'started')) {
      const noticeAtMs = binding.gameNumber === 1
        ? competitionRoundReadyCutoffMs(binding.roundStartMs)
        : now
      if (metadata.status !== 'Dissolved' && noticeAtMs !== null
        && await storage.get<boolean>('competitionRoomReadyNoticeSent') !== true) {
        candidates.push(noticeAtMs > now ? noticeAtMs : now + 5_000)
      }
      if (admission.status === 'released' && metadata.status === 'Waiting') {
        if (this.competitionStartAdmitted(metadata, binding, admission, now)) {
          candidates.push(admission.startRetryAtMs ?? now)
        }
        if (binding.gameNumber === 1 && now < binding.roundStartMs) {
          // 即使開局重試排在 T 後，也要先在 T 固定一次房間報到快照。
          candidates.push(binding.roundStartMs)
        }
      }
    }
    if (candidates.length === 0) await storage.deleteAlarm?.()
    else await storage.setAlarm(Math.min(...candidates))
  }

  private competitionMembersMatchBinding(
    metadata: GameRoomMetadata,
    binding: CompetitionGameBinding,
  ): boolean {
    return metadata.capacity === binding.players.length
      && metadata.ruleVersion === binding.ruleVersion
      && metadata.ruleset === binding.ruleset
      && JSON.stringify(metadata.enabledRuleModules) === JSON.stringify(binding.enabledRuleModules)
      && metadata.players.length === binding.players.length
      && metadata.players.every((player, index) => player === binding.players[index]?.player)
      && metadata.members.length === binding.players.length
      && metadata.members.every((member, index) => {
        const fixed = binding.players[index]
        return fixed?.userId === member.userId
          && fixed.player === member.player
          && member.owner === (index === 0)
      })
      && metadata.observers.length === 0
  }

  private isCompleteCompetitionGameRecord(
    snapshot: unknown,
    metadata: GameRoomMetadata,
    binding: CompetitionGameBinding,
  ): snapshot is GameRecord {
    if (!isRecord(snapshot)
      || snapshot.schemaVersion !== 7
      || !metadata.gameInstanceId
      || snapshot.gameInstanceId !== metadata.gameInstanceId
      || snapshot.firstPlayer !== binding.firstPlayer
      || snapshot.deckSeed !== binding.deckSeed
      || typeof snapshot.sequence !== 'number' || !Number.isInteger(snapshot.sequence)
      || !Array.isArray(snapshot.rulesRecord)) {
      return false
    }

    return JSON.stringify(snapshot.setup) === JSON.stringify(competitionRulesSetup(binding))
  }

  private competitionMutationRejected(): Response {
    return this.json({
      error: '競賽房間由賽事伺服器控制。',
      code: 'competitionServerOnly',
    }, 403)
  }

  private async requireLockedDeck(userId: string): Promise<PlayerDeckList> {
    const deck = await this.ctx.storage.get<PlayerDeckList>(this.lockedDeckKey(userId))
    if (!deck) {
      throw new Error('player deck was not locked')
    }
    return deck
  }

  private actionForPlayer(action: OnlineGameAction, player: PlayerId): OnlineGameAction {
    switch (action.type) {
      case 'performFormation':
        return {
          type: action.type,
          player,
          formationId: action.formationId,
          cards: action.cards,
          starSubstitutionCard: action.starSubstitutionCard,
          matchOptionRole: action.matchOptionRole,
          matchOptionCard: action.matchOptionCard,
          matchOptionSlots: action.matchOptionSlots,
        }
      case 'useSpiritSkill':
        return {
          type: action.type,
          player,
          skill: action.skill,
          selectedCard: action.selectedCard,
          declaredLevel: action.declaredLevel,
        }
      case 'activateProfessionAbility':
      case 'changeProfession':
      case 'chooseInitialPouch':
      case 'triggerSecretStrategy':
      case 'answerChoice':
      case 'retrievePreviousTurnDiscard':
      case 'playableActions':
        return { ...action, player }
      default:
        return action
    }
  }

  private async callRules(
    action: RulesEngineAction,
    viewer: string | undefined,
    snapshot: GameRecord,
  ): Promise<RulesEngineResult> {
    return await callRulesEngineResult({
      action,
      viewer,
      setup: snapshot.setup,
      deckSeed: snapshot.deckSeed,
      record: snapshot.rulesRecord,
    })
  }

  private async callReadyRules(
    action: RulesEngineAction,
    viewer: string | undefined,
    snapshot: GameRecord,
  ): Promise<RulesReadyResult> {
    return requireReadyRulesResult(await this.callRules(action, viewer, snapshot))
  }

  private async storeRoomEvent(
    metadata: GameRoomMetadata,
    type: string,
    actor: PlayerId | undefined,
    payload: unknown,
  ): Promise<GameRoomMetadata> {
    const sequence = await this.nextSequence()
    const event: StoredGameEvent = {
      sequence,
      type,
      actor,
      payload,
      createdAt: metadata.updatedAt,
    }

    await this.ctx.storage.put({
      metadata,
      nextSequence: sequence + 1,
      [this.eventKey(sequence)]: event,
    })

    return metadata
  }

  private async persistCompetitionWaitingMetadata(
    metadata: GameRoomMetadata,
    binding: CompetitionGameBinding,
    eventDetails?: { type: string; actor: PlayerId; payload: unknown },
    options?: { allowNoShow?: boolean },
  ): Promise<void> {
    const storage: GameRoomAtomicStorage = this.ctx.storage
    const sequence = eventDetails ? await this.nextSequence() : undefined
    const event: StoredGameEvent | undefined = eventDetails && sequence !== undefined
      ? {
          sequence,
          type: eventDetails.type,
          actor: eventDetails.actor,
          payload: eventDetails.payload,
          createdAt: metadata.updatedAt,
        }
      : undefined

    await storage.transaction(async transaction => {
      const current = await transaction.get<unknown>([
        'metadata', 'competitionBinding', 'gameRecord', competitionAdmissionKey, this.competitionDeliveryQueueKey(),
      ])
      const currentMetadata = current.get('metadata')
      const currentBinding = current.get('competitionBinding')
      const admission = current.get(competitionAdmissionKey)
      if (!competitionBindingIsValid(currentBinding) || !competitionGameBindingEquals(currentBinding, binding)
        || !isCompetitionSnapshotData(currentMetadata) || !hasCompetitionReadyMetadataShape(currentMetadata)
        || !competitionAdmissionMatchesBinding(admission, currentMetadata, currentBinding)
        || (admission.status !== 'unreleased' && admission.status !== 'released'
          && !(options?.allowNoShow && admission.status === 'noShow'))
        || currentMetadata.status !== 'Waiting' || current.has('gameRecord')
        || !this.competitionMembersMatchBinding(metadata, currentBinding)) {
        throw new CompetitionGameBindingError('competitionBindingConflict')
      }
      // noShow 裁定固定後，斷線只能改等待房狀態，不能再改 admission 或重開局。
      const nextAdmission: CompetitionGameAdmission = admission.status === 'noShow'
        ? admission
        : { ...admission, version: admission.version + 1, startRetryAtMs: null }
      if (admission.status !== 'noShow' && admission.version >= Number.MAX_SAFE_INTEGER) {
        throw new CompetitionGameBindingError('competitionBindingConflict')
      }
      await transaction.put({
        metadata,
        [competitionAdmissionKey]: nextAdmission,
        ...(sequence !== undefined ? { nextSequence: sequence + 1 } : {}),
        ...(event && sequence !== undefined ? { [this.eventKey(sequence)]: event } : {}),
      })
      await this.scheduleCompetitionAdmissionAlarm(
        transaction, metadata, currentBinding, nextAdmission, current.get(this.competitionDeliveryQueueKey()),
      )
    })
  }

  private async competitionCanAutoStart(
    metadata: GameRoomMetadata,
    binding: CompetitionGameBinding,
  ): Promise<boolean> {
    const admission = await this.ctx.storage.get<unknown>(competitionAdmissionKey)
    return competitionAdmissionMatchesBinding(admission, metadata, binding)
      && this.competitionStartAdmitted(metadata, binding, admission, Date.now())
  }

  private queueCompetitionAutoStart(gameId: string, binding: CompetitionGameBinding): void {
    this.ctx.waitUntil(this.startCompetitionGame({ gameId, binding }).catch(async error => {
      try {
        await this.serialized(() => this.scheduleCompetitionStartRetry(gameId, binding))
      } catch (retryError) {
        console.error('competition auto-start retry scheduling failed', retryError)
      }
      console.error('competition low-latency auto-start failed; retry eligibility reevaluated', error)
    }))
  }

  private async scheduleCompetitionStartRetry(
    gameId: string,
    expectedBinding: CompetitionGameBinding,
  ): Promise<void> {
    const storage: GameRoomAtomicStorage = this.ctx.storage
    const bindingBeforeRetry = await this.competitionBinding()
    const admissionBeforeRetry = await storage.get<unknown>(competitionAdmissionKey)
    if (bindingBeforeRetry && competitionGameBindingEquals(bindingBeforeRetry, expectedBinding)
      && isCompetitionGameAdmission(admissionBeforeRetry)
      && bindingBeforeRetry.gameNumber === 1 && admissionBeforeRetry.status === 'released'
      && Date.now() >= bindingBeforeRetry.roundStartMs) {
      await this.adjudicateCompetitionNoShowInternal({
        bindingRef: admissionBeforeRetry.bindingRef,
        commandId: competitionGameNoShowCommandId(admissionBeforeRetry.bindingRef),
        deadlineMs: bindingBeforeRetry.roundStartMs,
      })
    }
    await storage.transaction(async transaction => {
      const current = await transaction.get<unknown>([
        'metadata', 'competitionBinding', competitionAdmissionKey, this.competitionDeliveryQueueKey(),
      ])
      const metadata = current.get('metadata')
      const binding = current.get('competitionBinding')
      const admission = current.get(competitionAdmissionKey)
      if (!competitionBindingIsValid(binding) || !hasCompetitionReadyMetadataShape(metadata)
        || !competitionAdmissionMatchesBinding(admission, metadata, binding)) return
      if (admission.status !== 'released') {
        await this.scheduleCompetitionAdmissionAlarm(
          transaction, metadata, binding, admission, current.get(this.competitionDeliveryQueueKey()),
        )
        return
      }
      const next: CompetitionGameAdmission = {
        ...admission,
        startRetryAtMs: metadata.gameId === gameId && competitionGameBindingEquals(binding, expectedBinding)
          && this.competitionStartAdmitted(metadata, binding, admission, Date.now()) ? Date.now() + 1_000 : null,
      }
      await transaction.put({ [competitionAdmissionKey]: next })
      await this.scheduleCompetitionAdmissionAlarm(
        transaction, metadata, binding, next, current.get(this.competitionDeliveryQueueKey()),
      )
    })
  }

  private async requireMetadata(): Promise<GameRoomMetadata> {
    const metadata = await this.metadata()

    if (!metadata) {
      throw new Error('game room has not been created')
    }

    return metadata
  }

  private async requireGameRecord(): Promise<GameRecord> {
    const snapshot = await this.ctx.storage.get<Partial<GameRecord>>('gameRecord')

    if (!snapshot) {
      throw new Error('game room Game Record is missing')
    }

    if (snapshot.schemaVersion !== 7) {
      throw new Error('legacy game record must be purged before it can be used')
    }

    return snapshot as GameRecord
  }

  /**
   * 在同一個 DO 序列化操作中判定房間是否可由管理端重播。缺少權威 metadata
   * 或 Active/Finished 的權威讀取失敗才是清理候選；等待房與觀戰授權結果不參與
   * 此判定，避免把正常房間或未加入者誤判為故障。
   */
  private async legacyPurgeProbe(): Promise<{
    body: {
      status: 'preserved' | 'absent' | 'broken' | 'unavailable'
      gameId?: string
    }
    statusCode: 200 | 404 | 500 | 503
  }> {
    const metadata = await this.metadata()
    if (!metadata) {
      return { body: { status: 'absent' }, statusCode: 404 }
    }

    if (metadata.status !== 'Active' && metadata.status !== 'Finished') {
      // 等待房與已解散房本來就沒有 Game Record；其他非對局狀態也不能因 probe 被刪除。
      return { body: { status: 'preserved', gameId: metadata.gameId }, statusCode: 200 }
    }

    try {
      // 以管理端的 observer 視角驗證同一份 Game Record 與規則讀取路徑；不模擬
      // 匿名或觀戰者請求，避免把一般 401/403 誤當成清理錯誤。這裡刻意不呼叫
      // 完整 response()，因為它可能把 Active 對局的完成狀態寫回 metadata；probe
      // 只能觀察健康度，不能改動仍可回應的房間。
      const snapshot = await this.requireGameRecord()
      await this.callReadyRules({ type: 'refresh' }, 'observer', snapshot)
      return { body: { status: 'preserved', gameId: metadata.gameId }, statusCode: 200 }
    } catch (error) {
      // 只有明確可判定為舊版／毀損 Game Record 的結果才分類為 broken。
      // 未知例外（例如平台或傳輸故障）必須保留房間並讓管理端 fail closed。
      const isBroken = error instanceof RulesEngineError
        || (error instanceof Error && (
          error.message === 'game room Game Record is missing'
          || error.message === 'legacy game record must be purged before it can be used'
        ))
      if (!isBroken) {
        console.error('Legacy GameRoom probe unavailable', error)
        return { body: { status: 'unavailable', gameId: metadata.gameId }, statusCode: 503 }
      }
      console.error('Legacy GameRoom authoritative read failed', error)
      return { body: { status: 'broken', gameId: metadata.gameId }, statusCode: 500 }
    }
  }

  private async purgeLegacyGameData(epoch: string): Promise<{
    body: {
      status: 'preserved' | 'absent' | 'broken' | 'unavailable'
      gameId?: string
      epoch: string
    }
    statusCode: 200 | 404 | 500 | 503
  }> {
    if (await this.isCompetitionRoom()) {
      return { body: { status: 'preserved', epoch }, statusCode: 200 }
    }
    const probe = await this.legacyPurgeProbe()
    if (probe.statusCode === 404 || probe.statusCode === 500) {
      // 只有權威資料不存在或管理讀取明確失敗時才清除 DO。
      await this.ctx.storage.deleteAll()
      return {
        body: { ...probe.body, epoch },
        statusCode: probe.statusCode,
      }
    }
    // 可成功讀取的房間一律保留，不重設任何遊戲或等待房間資料。
    return { body: { ...probe.body, epoch }, statusCode: 200 }
  }

  private async legacyPurgeVerification(): Promise<{
    status: GameRoomMetadata['status'] | 'Absent'
    gameInstanceId?: string
    hasGameRecord: boolean
    eventTypes: string[]
  }> {
    const metadata = await this.metadata()
    if (!metadata) {
      return { status: 'Absent', hasGameRecord: false, eventTypes: [] }
    }
    const events = await this.events()
    return {
      status: metadata.status,
      gameInstanceId: metadata.gameInstanceId,
      hasGameRecord: Boolean(await this.ctx.storage.get<GameRecord>('gameRecord')),
      eventTypes: events.map(event => event.type),
    }
  }

  private async metadata(): Promise<GameRoomMetadata | undefined> {
    const stored = await this.ctx.storage.get<GameRoomMetadata>('metadata')
    const binding = await this.competitionBinding()

    if (binding) {
      // 競賽 metadata 已由可信入口固定；唯讀查詢不得先修補或寫回待驗資料。
      if (!isCompetitionSnapshotData(stored)
        || !hasCompetitionReadyMetadataShape(stored)
        || !this.competitionMembersMatchBinding(stored, binding)) {
        throw new CompetitionGameBindingError('invalidCompetitionBinding')
      }
      return stored
    }

    if (!stored) {
      return undefined
    }

    const metadata = normalizeGameRoomMetadata(stored)

    if (JSON.stringify(metadata) !== JSON.stringify(stored)) {
      await this.ctx.storage.put('metadata', metadata)
    }

    return metadata
  }

  private async nextSequence(): Promise<number> {
    return await this.ctx.storage.get<number>('nextSequence') ?? 1
  }

  private async events(): Promise<StoredGameEvent[]> {
    const entries = await this.ctx.storage.list<StoredGameEvent>({
      prefix: 'event:',
    })

    return [...entries.values()].sort((left, right) => left.sequence - right.sequence)
  }

  private async deleteGameInstanceTransactions(gameInstanceId: string) {
    const prefixes = [
      `commandReceipt:${gameInstanceId}:`,
      `trustedReceipt:${gameInstanceId}:`,
      `transaction:${gameInstanceId}`,
    ]
    for (const prefix of prefixes) {
      const entries = await this.ctx.storage.list({ prefix })
      await Promise.all([...entries.keys()].map(key => this.ctx.storage.delete(key)))
    }
  }

  private eventKey(sequence: number): string {
    return `event:${sequence.toString().padStart(12, '0')}`
  }

  private eventSummary(event: StoredGameEvent): string {
    const labels: Record<string, string> = {
      GameCreated: '遊戲房間已建立。',
      PlayerJoined: `${event.actor ?? '玩家'} 已加入房間。`,
      PlayerLeft: `${event.actor ?? '玩家'} 已離開房間。`,
      PlayerRemoved: `${event.actor ?? '玩家'} 已被移出房間。`,
      ObserverJoined: '觀戰者已加入房間。',
      ObserverLeft: '觀戰者已離開房間。',
      ObserverRemoved: '觀戰者已被移出房間。',
      ObserverPromoted: `${event.actor ?? '觀戰者'} 已補為玩家。`,
      PlayerReady: `${event.actor ?? '玩家'} 已準備。`,
      PlayerUnready: `${event.actor ?? '玩家'} 已取消準備。`,
      RuleModulesChanged: '房主已更新選用規則，所有玩家需重新準備。',
      PlayersReturnedToRoom: '玩家已返回等待房間。',
      LegacyGamePurged: '系統版本更新，上一局已清除，請重新準備。',
    }

    if (event.type in labels) {
      return labels[event.type] as string
    }

    if (event.type === 'RulesCommandApplied') {
      const actor = event.actor ?? '玩家'
      const action = this.payloadType(event.payload)
      const summaries: Record<string, string> = {
        passAction: `${actor} 已跳過行動。`,
        performFormation: `${actor} 已完成陣法行動。`,
        activateProfessionAbility: `${actor} 已發動職業能力。`,
        performFormationWithChoices: `${actor} 已完成陣法與效果選擇。`,
        answerChoice: `${actor} 已完成選擇。`,
        retrievePreviousTurnDiscard: `${actor} 已發動棄牌回收。`,
      }
      return summaries[action] ?? '戰局狀態已更新。'
    }

    if (event.type === 'GameStarted') {
      const firstPlayer = this.payloadValue(event.payload, 'firstPlayer')
      return firstPlayer ? `遊戲已開始，${firstPlayer} 先手。` : '遊戲已開始。'
    }

    return '房間狀態已更新。'
  }

  private eventTitle(event: StoredGameEvent): string {
    const titles: Record<string, string> = {
      GameCreated: '建立房間',
      PlayerJoined: '玩家加入',
      PlayerLeft: '玩家離開',
      PlayerRemoved: '移除玩家',
      ObserverJoined: '開始觀戰',
      ObserverLeft: '離開觀戰',
      ObserverRemoved: '移除觀戰者',
      ObserverPromoted: '候補為玩家',
      PlayerReady: '玩家準備',
      PlayerUnready: '取消準備',
      RuleModulesChanged: '更新規則',
      PlayersReturnedToRoom: '返回房間',
      GameStarted: '對局開始',
      RulesCommandApplied: '戰局更新',
    }

    return titles[event.type] ?? '房間更新'
  }

  private payloadType(payload: unknown): string {
    if (payload && typeof payload === 'object' && 'type' in payload) {
      return String(payload.type)
    }

    return 'unknown'
  }

  private displaySummary(metadata: GameRoomMetadata, summary: string): string {
    return replacePlayerLabels(summary, metadata.members)
  }

  private displayBattleRecord(metadata: GameRoomMetadata, record: BattleRecord): BattleRecord {
    const entry = (value: BattleRecord['preparation']['entries'][number]) => ({
      ...value,
      title: this.displaySummary(metadata, value.title),
      summary: value.summary ? this.displaySummary(metadata, value.summary) : undefined,
    })
    return {
      preparation: { entries: record.preparation.entries.map(entry) },
      turns: record.turns.map(group => ({
        ...group,
        title: this.displaySummary(metadata, group.title),
        entries: group.entries.map(entry),
      })),
    }
  }

  private payloadValue(payload: unknown, key: string): string | undefined {
    if (payload && typeof payload === 'object') {
      const record = payload as Record<string, unknown>

      if (key in record) {
        return String(record[key])
      }
    }

    return undefined
  }

  private json(body: unknown, status = 200): Response {
    return Response.json(body, { status })
  }
}

export async function competitionTerminalIdentity(
  competitionId: string,
  matchId: string,
  gameNumber: number,
  gameInstanceId: string,
): Promise<string> {
  const encoded = new TextEncoder().encode(JSON.stringify([
    competitionId,
    matchId,
    gameNumber,
    gameInstanceId,
  ]))
  const digest = await crypto.subtle.digest('SHA-256', encoded)
  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

function emptyCompetitionDeliveryQueue(revisionCursor: number): CompetitionDeliveryQueue {
  return {
    schemaVersion: 1,
    revisionCursor,
    entries: [],
    receipts: [],
    pendingTimeout: null,
    nextClockDeadlineMs: null,
    terminalTimeout: null,
    refereePaused: false,
    clockControlReceipts: [],
  }
}

function normalizeCompetitionDeliveryQueue(queue: CompetitionDeliveryQueue): CompetitionDeliveryQueue {
  return {
    ...queue,
    nextClockDeadlineMs: queue.nextClockDeadlineMs ?? null,
    terminalTimeout: queue.terminalTimeout ?? null,
    refereePaused: queue.refereePaused ?? false,
    clockControlReceipts: queue.clockControlReceipts ?? [],
  }
}

function nextCompetitionAlarmAt(queue: CompetitionDeliveryQueue, now: number): number | null {
  if (queue.terminalTimeout) return null
  const candidates: number[] = []
  const head = queue.entries[0]
  if (head) candidates.push(head.nextAttemptAt)
  const deadline = queue.nextClockDeadlineMs ?? null
  const terminalQueued = queue.entries.some(entry => entry.kind === 'competitionGameTerminal')
  const observationQueued = deadline !== null && queue.entries.some(entry => (
    entry.kind === 'competitionClockMutation'
    && entry.operation.type === 'clockObserve'
    && entry.trustedTimeMs === deadline
  ))
  if (!queue.refereePaused && deadline !== null && !terminalQueued && !observationQueued) {
    candidates.push(deadline)
  }
  if (queue.pendingTimeout && candidates.length === 0) candidates.push(now + 60_000)
  return candidates.length === 0 ? null : Math.min(...candidates)
}

async function competitionClockEntry(
  binding: CompetitionGameBinding,
  commandId: string,
  operation: CompetitionClockOperation,
  trustedTimeMs: number,
): Promise<CompetitionClockDeliveryEntry> {
  return {
    schemaVersion: 1,
    kind: 'competitionClockMutation',
    competitionId: binding.competitionId,
    matchId: binding.matchId,
    callerUserId: binding.organizerUserId,
    commandId: await competitionClockIdentity(
      binding.competitionId,
      binding.matchId,
      commandId,
      operation,
    ),
    trustedTimeMs,
    operation: structuredClone(operation),
    attempts: 0,
    nextAttemptAt: trustedTimeMs,
  }
}

async function competitionInitialStartIdentityPrefix(
  binding: CompetitionGameBinding,
  evidence: Pick<CompetitionGameInitialStartEvidence,
    'bindingRef' | 'releaseDecisionId' | 'gameInstanceId' | 'startSequence' | 'initialWindow'>,
): Promise<string> {
  return digestIdentity([
    'competition-initial-game-started', binding.competitionId, binding.matchId,
    binding.expectedMatchRevision, evidence,
  ])
}

function competitionInitialStartEntry(
  binding: CompetitionGameBinding,
  evidence: CompetitionGameInitialStartEvidence,
  identityPrefix: string,
): CompetitionInitialGameStartedEntry {
  return {
    schemaVersion: 1,
    kind: 'competitionInitialGameStarted',
    competitionId: binding.competitionId,
    matchId: binding.matchId,
    callerUserId: binding.organizerUserId,
    commandId: `competition-initial-start:${identityPrefix}:${evidence.startedAtMs}`,
    expectedMatchRevision: binding.expectedMatchRevision,
    evidence: structuredClone(evidence),
    attempts: 0,
    nextAttemptAt: evidence.startedAtMs,
  }
}

function competitionInitialStartRequest(entry: CompetitionInitialGameStartedEntry): CompetitionRoomInitialGameStartedRequest {
  return {
    competitionId: entry.competitionId, matchId: entry.matchId, callerUserId: entry.callerUserId,
    commandId: entry.commandId, expectedMatchRevision: entry.expectedMatchRevision,
    evidence: structuredClone(entry.evidence),
  }
}

function initialEvidenceMatchesAdmission(
  evidence: unknown,
  admission: CompetitionGameAdmission,
): evidence is CompetitionGameInitialStartEvidence {
  return admission.status === 'started' && admission.releaseCommandId !== null && admission.start !== null
    && isCompetitionGameInitialStartEvidence(evidence, {
      bindingRef: admission.bindingRef, commandId: admission.releaseCommandId,
    })
    && evidence.gameInstanceId === admission.start.gameInstanceId
    && evidence.startSequence === admission.start.startSequence
    && evidence.startedAtMs === admission.start.startedAtMs
}

async function competitionClockEventIdentity(
  gameInstanceId: string,
  sequence: number,
  sourceCommandId: string,
  ordinal: number,
  operation: CompetitionClockOperation,
): Promise<string> {
  return digestIdentity([
    'competition-clock-event', gameInstanceId, sequence, sourceCommandId, ordinal, operation,
  ])
}

async function competitionDeadlineIdentity(
  gameInstanceId: string,
  revisionCursor: number,
  deadlineMs: number,
): Promise<string> {
  return digestIdentity(['competition-clock-deadline', gameInstanceId, revisionCursor, deadlineMs])
}

async function digestIdentity(value: unknown): Promise<string> {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(JSON.stringify(value)),
  )
  return [...new Uint8Array(digest)].map(byte => byte.toString(16).padStart(2, '0')).join('')
}

async function competitionTimeoutDeliveryEntry(
  binding: CompetitionGameBinding,
  gameInstanceId: string,
  evidence: CompetitionPendingTimeout['timeoutEvidence'],
): Promise<CompetitionTerminalDeliveryEntry> {
  const firstPlayer = binding.players.find(player => player.player === binding.firstPlayer)
  if (!firstPlayer) throw new Error('competition game has no bound first player')
  const identity = await digestIdentity([
    'competition-timeout-terminal', binding.competitionId, binding.matchId,
    binding.gameNumber, gameInstanceId, evidence.timeoutId,
  ])
  return {
    schemaVersion: 1,
    kind: 'competitionGameTerminal',
    competitionId: binding.competitionId,
    matchId: binding.matchId,
    gameNumber: binding.gameNumber,
    gameInstanceId,
    commandId: `competition-timeout:${identity}`,
    submissionId: `competition-timeout-submission:${identity}`,
    gameRecordId: `competition-timeout-game-record:${identity}`,
    rulingRecordId: `competition-timeout-ruling:${identity}`,
    callerUserId: binding.organizerUserId,
    startKind: binding.startKind,
    firstPlayer: firstPlayer.team,
    trustedTimeMs: evidence.observedAtMonotonicMs,
    terminalEvidence: evidence,
    attempts: 0,
    nextAttemptAt: evidence.observedAtMonotonicMs,
  }
}

async function competitionClockIdentity(
  competitionId: string,
  matchId: string,
  requestCommandId: string,
  operation: CompetitionClockOperation,
): Promise<string> {
  const encoded = new TextEncoder().encode(JSON.stringify([
    'competition-clock', competitionId, matchId, requestCommandId, operation,
  ]))
  const digest = await crypto.subtle.digest('SHA-256', encoded)
  return `competition-clock:${[...new Uint8Array(digest)].map(byte => byte.toString(16).padStart(2, '0')).join('')}`
}

function stableDeliveryFingerprint(value: unknown): string {
  return JSON.stringify(sortForIdentity(value))
}

function sortForIdentity(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(sortForIdentity)
  if (!value || typeof value !== 'object') return value
  return Object.fromEntries(Object.entries(value as Record<string, unknown>)
    .filter(([, entry]) => entry !== undefined)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, entry]) => [key, sortForIdentity(entry)]))
}

const competitionOperationNames: ReadonlySet<CompetitionOperationName> = new Set<CompetitionOperationName>([
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
])

function hasCanonicalTerminalRecord(record: unknown, expectedConclusion: unknown): boolean {
  if (!Array.isArray(record) || record.length === 0) return false
  let events: unknown[] | null = null
  for (let index = record.length - 1; index >= 0; index -= 1) {
    const candidate = record[index]
    if (!isRecord(candidate) || !Array.isArray(candidate.events)) return false
    if (candidate.events.length > 0) {
      events = candidate.events
      break
    }
  }
  if (!events) return false
  const event = events.at(-1)
  if (!isRecord(event) || !hasExactKeys(event, ['GameEnded']) || !isRecord(event.GameEnded)
    || !hasExactKeys(event.GameEnded, ['conclusion'])) return false
  const recordConclusion = gameConclusionFromRulesRecord(event.GameEnded.conclusion)
  if (!recordConclusion) return false
  return JSON.stringify(sortForIdentity(recordConclusion))
    === JSON.stringify(sortForIdentity(expectedConclusion))
}

function gameConclusionFromRulesRecord(
  value: unknown,
): NonNullable<import('../../app/types/fewfc').PublicGameState['gameConclusion']> | null {
  if (isCompetitionGameConclusion(value)) return structuredClone(value)
  if (!isRecord(value) || !hasExactKeys(value, ['outcome', 'causes'])
    || !Array.isArray(value.causes) || value.causes.length === 0) return null

  let outcome: NonNullable<import('../../app/types/fewfc').PublicGameState['gameConclusion']>['outcome']
  if (value.outcome === 'Draw') {
    outcome = { type: 'draw' }
  } else if (isRecord(value.outcome) && hasExactKeys(value.outcome, ['Winner'])
    && isNonEmpty(value.outcome.Winner)) {
    outcome = { type: 'winner', team: value.outcome.Winner }
  } else {
    return null
  }

  const causes: NonNullable<import('../../app/types/fewfc').PublicGameState['gameConclusion']>['causes'] = []
  for (const cause of value.causes) {
    if (!isRecord(cause) || Object.keys(cause).length !== 1) return null
    if (hasExactKeys(cause, ['TeamHpDepleted']) && isRecord(cause.TeamHpDepleted)
      && hasExactKeys(cause.TeamHpDepleted, ['teams'])
      && Array.isArray(cause.TeamHpDepleted.teams)
      && cause.TeamHpDepleted.teams.length > 0
      && cause.TeamHpDepleted.teams.every(isNonEmpty)) {
      causes.push({ type: 'teamHpDepleted', teams: cause.TeamHpDepleted.teams })
      continue
    }
    if (hasExactKeys(cause, ['DirectVictory']) && isRecord(cause.DirectVictory)
      && hasExactKeys(cause.DirectVictory, ['rule', 'team'])
      && isNonEmpty(cause.DirectVictory.rule) && isNonEmpty(cause.DirectVictory.team)) {
      causes.push({
        type: 'directVictory',
        rule: cause.DirectVictory.rule,
        team: cause.DirectVictory.team,
      })
      continue
    }
    return null
  }

  const conclusion = { outcome, causes }
  return isCompetitionGameConclusion(conclusion) ? conclusion : null
}

function isCompetitionDeliveryQueue(value: unknown): value is CompetitionDeliveryQueue {
  if (!isRecord(value) || !isCompetitionSnapshotData(value)
    || !hasRequiredAndAllowedKeys(value,
      ['schemaVersion', 'revisionCursor', 'entries', 'receipts', 'pendingTimeout'],
      ['nextClockDeadlineMs', 'terminalTimeout', 'refereePaused', 'clockControlReceipts'])
    || value.schemaVersion !== 1
    || !Number.isSafeInteger(value.revisionCursor) || value.revisionCursor < 1
    || !Array.isArray(value.entries) || !Array.isArray(value.receipts)) return false
  if ((value.refereePaused !== undefined && typeof value.refereePaused !== 'boolean')
    || (value.clockControlReceipts !== undefined
      && (!Array.isArray(value.clockControlReceipts)
        || !value.clockControlReceipts.every(isCompetitionClockControlReceipt)))) return false
  const entries = value.entries
  if (!entries.every(isCompetitionDeliveryEntry)
    || !value.receipts.every(isCompetitionDeliveryReceipt)) return false
  const initials = entries.filter((entry): entry is CompetitionInitialGameStartedEntry => (
    entry.kind === 'competitionInitialGameStarted'
  ))
  if (initials.length > 1 || initials.some(entry => (
    entries[0] !== entry || entry.expectedMatchRevision !== value.revisionCursor
  ))) return false
  return (value.pendingTimeout === null || isCompetitionPendingTimeout(value.pendingTimeout))
    && (value.nextClockDeadlineMs === undefined
      || value.nextClockDeadlineMs === null
      || isSafeNonNegativeInteger(value.nextClockDeadlineMs))
    && (value.terminalTimeout === undefined
      || value.terminalTimeout === null
      || isTimeoutEvidence(value.terminalTimeout))
}

function hasRequiredAndAllowedKeys(
  value: Record<string, unknown>,
  required: string[],
  optional: string[],
): boolean {
  const keys = Object.keys(value)
  return required.every(key => keys.includes(key))
    && keys.every(key => required.includes(key) || optional.includes(key))
}

function isCompetitionClockControlRequest(value: unknown): value is CompetitionClockControlRequest {
  return isRecord(value)
    && hasExactKeys(value, [
      'controlId', 'competitionId', 'matchId', 'gameId', 'gameNumber', 'sourceCommandId',
      'matchRevision', 'operation', 'trustedTimeMs', 'nextClockDeadlineMs',
    ])
    && isNonEmpty(value.controlId)
    && isNonEmpty(value.competitionId)
    && isNonEmpty(value.matchId)
    && isNonEmpty(value.gameId)
    && Number.isSafeInteger(value.gameNumber) && Number(value.gameNumber) > 0
    && isNonEmpty(value.sourceCommandId)
    && Number.isSafeInteger(value.matchRevision) && Number(value.matchRevision) > 0
    && (value.operation === 'clockPause' || value.operation === 'clockResume')
    && isSafeNonNegativeInteger(value.trustedTimeMs)
    && (value.nextClockDeadlineMs === null || isSafeNonNegativeInteger(value.nextClockDeadlineMs))
}

function isCompetitionClockControlReceipt(value: unknown): value is CompetitionClockControlReceipt {
  if (!isRecord(value) || !hasExactKeys(value, ['request', 'ack'])
    || !isCompetitionClockControlRequest(value.request)
    || !isRecord(value.ack)
    || !hasExactKeys(value.ack, [
      'controlId', 'competitionId', 'matchId', 'gameId', 'gameNumber', 'sourceCommandId',
      'matchRevision', 'operation', 'trustedTimeMs', 'nextClockDeadlineMs', 'idempotent',
    ]) || typeof value.ack.idempotent !== 'boolean') return false
  const { idempotent: _idempotent, ...ackPayload } = value.ack
  return stableDeliveryFingerprint(ackPayload) === stableDeliveryFingerprint(value.request)
}

function competitionQueueMatchesBinding(
  queue: CompetitionDeliveryQueue,
  binding: CompetitionGameBinding,
  gameInstanceId?: string,
): boolean {
  return queue.entries.every(entry => (
    entry.competitionId === binding.competitionId
      && entry.matchId === binding.matchId
      && entry.callerUserId === binding.organizerUserId
      && (entry.kind !== 'competitionInitialGameStarted'
        || (binding.gameNumber === 1 && entry.expectedMatchRevision === binding.expectedMatchRevision
          && entry.evidence.bindingRef.gameNumber === binding.gameNumber
          && entry.evidence.bindingRef.matchId === binding.matchId
          && entry.evidence.bindingRef.roundStartMs === binding.roundStartMs
          && stableDeliveryFingerprint(entry.evidence.bindingRef.roundRef) === stableDeliveryFingerprint(binding.roundRef)
          && (gameInstanceId === undefined || entry.evidence.gameInstanceId === gameInstanceId)))
      && (entry.kind !== 'competitionGameTerminal'
        || gameInstanceId === undefined
        || (entry.gameInstanceId === gameInstanceId
          && entry.gameNumber === binding.gameNumber
          && entry.firstPlayer === binding.players.find(player => player.player === binding.firstPlayer)?.team
          && JSON.stringify(entry.startKind) === JSON.stringify(binding.startKind)))
  )) && (queue.pendingTimeout === null
    || (queue.pendingTimeout.competitionId === binding.competitionId
      && queue.pendingTimeout.matchId === binding.matchId))
}

function isCompetitionDeliveryReceipt(value: unknown): value is CompetitionDeliveryReceipt {
  return isRecord(value)
    && hasExactKeys(value, ['schemaVersion', 'kind', 'requestCommandId', 'commandId', 'fingerprint'])
    && value.schemaVersion === 1
    && value.kind === 'competitionClockEnqueue'
    && isNonEmpty(value.requestCommandId)
    && isNonEmpty(value.commandId)
    && isNonEmpty(value.fingerprint)
}

function isCompetitionDeliveryEntry(value: unknown): value is CompetitionDeliveryEntry {
  if (!isRecord(value) || !isCompetitionSnapshotData(value) || value.schemaVersion !== 1
    || !isNonEmpty(value.competitionId) || !isNonEmpty(value.matchId)
    || !isNonEmpty(value.callerUserId) || !isNonEmpty(value.commandId)
    || !Number.isInteger(value.attempts) || value.attempts < 0
    || !Number.isFinite(value.nextAttemptAt)) return false
  if (value.kind === 'competitionInitialGameStarted') {
    if (!isCompetitionSnapshotData(value) || !hasExactKeys(value, [
      'schemaVersion', 'kind', 'competitionId', 'matchId', 'callerUserId', 'commandId',
      'expectedMatchRevision', 'evidence', 'attempts', 'nextAttemptAt',
    ])) return false
    return isCompetitionRoomInitialGameStartedRequest({
      competitionId: value.competitionId, matchId: value.matchId, callerUserId: value.callerUserId,
      commandId: value.commandId, expectedMatchRevision: value.expectedMatchRevision, evidence: value.evidence,
    })
  }
  if (!isSafeNonNegativeInteger(value.trustedTimeMs)) return false
  if (value.kind === 'competitionClockMutation') {
    return hasExactKeys(value, [
      'schemaVersion', 'kind', 'competitionId', 'matchId', 'callerUserId', 'commandId',
      'trustedTimeMs', 'operation', 'attempts', 'nextAttemptAt',
    ]) && isCompetitionClockOperation(value.operation)
  }
  if (value.kind !== 'competitionGameTerminal' || !hasExactKeys(value, [
    'schemaVersion', 'kind', 'competitionId', 'matchId', 'gameNumber', 'gameInstanceId',
    'commandId', 'submissionId', 'gameRecordId', 'rulingRecordId', 'callerUserId',
    'startKind', 'firstPlayer', 'trustedTimeMs', 'terminalEvidence', 'attempts', 'nextAttemptAt',
  ]) || !Number.isInteger(value.gameNumber) || value.gameNumber < 1
    || !isNonEmpty(value.gameInstanceId) || !isNonEmpty(value.submissionId)
    || !isNonEmpty(value.gameRecordId) || !isNonEmpty(value.rulingRecordId)
    || !isNonEmpty(value.firstPlayer) || !isCompetitionStartKind(value.startKind)
    || !isCompetitionTerminalEvidence(value.terminalEvidence)) return false
  return true
}

function isCompetitionTerminalEvidence(
  value: unknown,
): value is CompetitionTerminalDeliveryEntry['terminalEvidence'] {
  if (!isRecord(value)) return false
  if (value.type === 'canonicalGameEnded') {
    return hasExactKeys(value, ['type', 'conclusion'])
      && isCompetitionGameConclusion(value.conclusion)
  }
  return value.type === 'timeout' && isTimeoutEvidence(value)
}

function isCompetitionPendingTimeout(value: unknown): value is CompetitionPendingTimeout {
  return isRecord(value)
    && hasExactKeys(value, ['schemaVersion', 'kind', 'competitionId', 'matchId', 'commandId', 'timeoutEvidence'])
    && value.schemaVersion === 1
    && value.kind === 'competitionPendingTimeout'
    && isNonEmpty(value.competitionId) && isNonEmpty(value.matchId) && isNonEmpty(value.commandId)
    && isTimeoutEvidence(value.timeoutEvidence)
}

/** RPC root 的唯一平台符號不屬於 JSON；nested JSON 與 semantic root 仍完全嚴格。 */
function parseCompetitionDeliveryRpcAck(
  value: unknown,
  entry: CompetitionDeliveryEntry,
  revisionCursor: number,
): ReturnType<typeof parseCompetitionDeliveryAck> {
  let dispose: (() => void) | undefined
  try {
    if (value === null || typeof value !== 'object' || Array.isArray(value)) return null
    const resource = Object.getOwnPropertyDescriptor(value, Symbol.dispose)
    if (resource && 'value' in resource && typeof resource.value === 'function') dispose = resource.value
    if (resource && !dispose) return null
    const prototype = Object.getPrototypeOf(value)
    if (prototype !== Object.prototype && prototype !== null) return null
    const semantic: Record<string, unknown> = {}
    for (const key of Reflect.ownKeys(value)) {
      if (key === Symbol.dispose) continue
      if (typeof key !== 'string' || (key !== 'ok' && key !== 'value')) return null
      const descriptor = Object.getOwnPropertyDescriptor(value, key)
      if (!descriptor || !('value' in descriptor) || !descriptor.enumerable) return null
      semantic[key] = descriptor.value
    }
    if (!isCompetitionSnapshotData(semantic)) return null
    // cleanup 可改原 RPC 物件；只有事前驗證並擁有的 snapshot 能影響 cursor／期限。
    return parseCompetitionDeliveryAck(structuredClone(semantic), entry, revisionCursor)
  } finally {
    if (dispose) Reflect.apply(dispose, value, [])
  }
}

function parseCompetitionDeliveryAck(
  value: unknown,
  entry: CompetitionDeliveryEntry,
  revisionCursor: number,
): {
  timeoutEvidence: CompetitionPendingTimeout['timeoutEvidence'] | null
  nextClockDeadlineMs: number | null
} | null {
  if (!isRecord(value) || !hasExactKeys(value, ['ok', 'value']) || value.ok !== true
    || !isRecord(value.value)
    || !hasExactKeys(value.value, [
      'competitionId', 'matchId', 'matchRevision', 'commandId', 'idempotent',
      'timeoutEvidence', 'nextClockDeadlineMs', 'matchProjection', 'clockProjection',
    ])
    || value.value.competitionId !== entry.competitionId
    || value.value.matchId !== entry.matchId
    || value.value.commandId !== entry.commandId
    || value.value.matchRevision !== revisionCursor + 1
    || typeof value.value.idempotent !== 'boolean'
    || (value.value.nextClockDeadlineMs !== null
      && !isSafeNonNegativeInteger(value.value.nextClockDeadlineMs))
    || !isCompetitionProjection(value.value.matchProjection)
    || !isCompetitionProjection(value.value.clockProjection)) return null
  if (entry.kind === 'competitionGameTerminal') {
    return value.value.timeoutEvidence === null && value.value.nextClockDeadlineMs === null
      ? { timeoutEvidence: null, nextClockDeadlineMs: null }
      : null
  }
  const deadline = value.value.nextClockDeadlineMs
  const sample = entry.kind === 'competitionInitialGameStarted' ? entry.evidence.startedAtMs : entry.trustedTimeMs
  if (deadline !== null && deadline < sample) return null
  const timeout = value.value.timeoutEvidence
  if (entry.kind === 'competitionInitialGameStarted' && timeout !== null) return null
  if (timeout !== null && !isTimeoutEvidence(timeout)) return null
  if (timeout !== null && deadline !== null) return null
  return {
    timeoutEvidence: timeout,
    nextClockDeadlineMs: deadline,
  }
}

function isCompetitionClockOperation(value: unknown): value is CompetitionClockOperation {
  if (!isRecord(value) || typeof value.type !== 'string' || !isNonEmpty(value.matchId)) return false
  switch (value.type) {
    case 'clockBeginPreparation':
    case 'clockObserve':
    case 'clockEndWindow':
      return hasExactKeys(value, ['type', 'matchId'])
    case 'clockFinishPreparation':
      return hasExactKeys(value, ['type', 'matchId', 'team']) && isNonEmpty(value.team)
    case 'clockBeginAction':
      return hasExactKeys(value, ['type', 'matchId', 'team', 'turn'])
        && isNonEmpty(value.team) && Number.isInteger(value.turn) && value.turn >= 0
    case 'clockBeginOutOfTurnChoice':
      return hasExactKeys(value, ['type', 'matchId', 'team', 'choiceId'])
        && isNonEmpty(value.team) && isNonEmpty(value.choiceId)
    case 'clockPause':
    case 'clockResume':
      return hasExactKeys(value, ['type', 'matchId', 'reason']) && isNonEmpty(value.reason)
    case 'clockNewGame':
      return hasExactKeys(value, ['type', 'matchId', 'time'])
        && (value.time === 'reset' || value.time === 'carryForward')
    default:
      return false
  }
}

function isTimeoutEvidence(value: unknown): value is CompetitionPendingTimeout['timeoutEvidence'] {
  return isRecord(value)
    && hasExactKeys(value, ['type', 'timeoutId', 'expiries', 'observedAtMonotonicMs', 'clockCheckpointDigest'])
    && value.type === 'timeout'
    && isNonEmpty(value.timeoutId)
    && Array.isArray(value.expiries) && value.expiries.length >= 1 && value.expiries.length <= 2
    && new Set(value.expiries.map(expiry => isRecord(expiry) ? expiry.team : undefined)).size === value.expiries.length
    && (value.expiries.length === 1
      || (isRecord(value.expiries[0]) && isRecord(value.expiries[1])
        && value.expiries[0].deadlineMonotonicMs === value.expiries[1].deadlineMonotonicMs))
    && value.expiries.every(expiry => isRecord(expiry)
      && hasExactKeys(expiry, ['team', 'deadlineMonotonicMs'])
      && isNonEmpty(expiry.team)
      && Number.isSafeInteger(expiry.deadlineMonotonicMs)
      && expiry.deadlineMonotonicMs >= 0
      && Number.isSafeInteger(value.observedAtMonotonicMs)
      && value.observedAtMonotonicMs >= 0
      && expiry.deadlineMonotonicMs <= value.observedAtMonotonicMs)
    && /^[0-9a-f]{64}$/.test(value.clockCheckpointDigest as string)
}

function isCompetitionProjection(value: unknown): boolean {
  if (value === null) return true
  return isRecord(value)
    && hasExactKeys(value, ['ok', 'operation', 'public'])
    && value.ok === true
    && typeof value.operation === 'string'
    && competitionOperationNames.has(value.operation)
    && isJsonValue(value.public)
}

function isCompetitionStartKind(value: unknown): value is CompetitionGameBinding['startKind'] {
  if (!isRecord(value) || typeof value.type !== 'string') return false
  if (value.type === 'ordinaryNewGame') return hasExactKeys(value, ['type'])
  if (value.type !== 'eliminationDrawContinuation'
    || !hasExactKeys(value, ['type', 'continuation'])
    || !isRecord(value.continuation)
    || !Array.isArray(value.continuation.teams)
    || value.continuation.teams.length !== 2) {
    return false
  }
  return value.continuation.teams.every(team => isRecord(team)
    && hasExactKeys(team, ['team', 'mainTimeRemainingMs'])
    && isNonEmpty(team.team)
    && (team.mainTimeRemainingMs === null
      || (Number.isInteger(team.mainTimeRemainingMs) && team.mainTimeRemainingMs >= 0)))
}

function conclusionUsesSetupTeams(
  conclusion: NonNullable<import('../../app/types/fewfc').PublicGameState['gameConclusion']>,
  record: GameRecord,
): boolean {
  const teams = new Set(record.setup.players.map(player => player.team))
  if (conclusion.outcome.type === 'winner' && !teams.has(conclusion.outcome.team)) return false
  return conclusion.causes.every(cause => (
    cause.type === 'teamHpDepleted'
      ? cause.teams.every(team => teams.has(team))
      : teams.has(cause.team)
  ))
}

function bindingMissingActorDeck(value: unknown, actorUserId: string): boolean {
  if (!isRecord(value)
    || !Array.isArray(value.enabledRuleModules)
    || !value.enabledRuleModules.includes('personal-deck')
    || !Array.isArray(value.players)
    || !Array.isArray(value.deckSnapshot)) {
    return false
  }

  const fixedPlayer = value.players.find(player => isRecord(player)
    && player.userId === actorUserId)
  return isRecord(fixedPlayer)
    && typeof fixedPlayer.player === 'string'
    && !value.deckSnapshot.some(deck => isRecord(deck) && deck.player === fixedPlayer.player)
}

function hasCompetitionReadyMetadataShape(value: unknown): value is GameRoomMetadata {
  if (!isRecord(value)) return false
  const requiredKeys = [
    'schemaVersion', 'gameId', 'name', 'access', 'capacity', 'ruleVersion', 'ruleset',
    'enabledRuleModules', 'players', 'members', 'observers', 'status', 'createdAt', 'updatedAt',
  ]
  const allowedKeys = [...requiredKeys, 'gameInstanceId']
  if (requiredKeys.some(key => !(key in value))
    || Object.keys(value).some(key => !allowedKeys.includes(key))
    || (value.gameInstanceId !== undefined && !isNonEmpty(value.gameInstanceId))
    || value.schemaVersion !== 5
    || !isNonEmpty(value.gameId)
    || !isNonEmpty(value.name)
    || value.access !== 'private'
    || (value.capacity !== 2 && value.capacity !== 4)
    || (value.ruleVersion !== '5.16' && value.ruleVersion !== '5.17')
    || value.ruleset !== 'fewfc-base'
    || !Array.isArray(value.enabledRuleModules)
    || !value.enabledRuleModules.every(isNonEmpty)
    || new Set(value.enabledRuleModules).size !== value.enabledRuleModules.length
    || !Array.isArray(value.players)
    || !value.players.every(isNonEmpty)
    || !Array.isArray(value.observers)
    || typeof value.status !== 'string'
    || !['Waiting', 'Active', 'Finished', 'Dissolved'].includes(value.status)
    || !isNonEmpty(value.createdAt)
    || !isNonEmpty(value.updatedAt)
    || !Array.isArray(value.members)) {
    return false
  }
  return value.members.every(member => isRecord(member)
    && hasExactKeys(member, ['userId', 'displayName', 'player', 'ready', 'connected', 'owner'])
    && isNonEmpty(member.userId)
    && isNonEmpty(member.displayName)
    && isNonEmpty(member.player)
    && typeof member.ready === 'boolean'
    && typeof member.connected === 'boolean'
    && typeof member.owner === 'boolean')
}

/** probe 僅檢查 raw snapshot 的資料形狀；不 normalize、不重播，也不改舊 helper 契約。 */
function isCompetitionSnapshotData(value: unknown, seen = new Set<object>()): boolean {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return true
  if (typeof value === 'number') return Number.isFinite(value)
  if (typeof value !== 'object' || seen.has(value)) return false
  try {
    seen.add(value)
    const keys = Reflect.ownKeys(value)
    if (Array.isArray(value)) {
      return Object.getPrototypeOf(value) === Array.prototype && keys.length === value.length + 1
        && keys.includes('length') && Array.from({ length: value.length }, (_, index) => String(index)).every(key => {
          const descriptor = Object.getOwnPropertyDescriptor(value, key)
          return descriptor?.enumerable === true && Object.hasOwn(descriptor, 'value')
            && isCompetitionSnapshotData(descriptor.value, seen)
        })
    }
    return (Object.getPrototypeOf(value) === Object.prototype || Object.getPrototypeOf(value) === null)
      && keys.every(key => {
        if (typeof key !== 'string') return false
        const descriptor = Object.getOwnPropertyDescriptor(value, key)
        return descriptor?.enumerable === true && Object.hasOwn(descriptor, 'value')
          && isCompetitionSnapshotData(descriptor.value, seen)
      })
  } catch {
    return false
  } finally {
    seen.delete(value)
  }
}

function hasCompetitionSnapshotKeys(value: unknown, required: string[], optional: string[] = []): boolean {
  if (!isRecord(value)) return false
  const keys = Reflect.ownKeys(value)
  return required.every(key => Object.hasOwn(value, key))
    && keys.every(key => typeof key === 'string' && (required.includes(key) || optional.includes(key)))
}

function competitionBindingIsValid(value: unknown): value is CompetitionGameBinding {
  try {
    validateCompetitionGameBinding(value)
    return true
  } catch {
    return false
  }
}

function sameCompetitionBindingRef(left: CompetitionGameBindingRef, right: CompetitionGameBindingRef): boolean {
  return isCompetitionGameAdmissionRequest(
    { bindingRef: left, commandId: 'identity' }, { bindingRef: right, commandId: 'identity' },
  )
}

function isCompetitionRoomReadyNoticeOutbox(
  value: unknown,
): value is CompetitionRoomReadyNoticeOutbox {
  if (!isCompetitionSnapshotData(value) || !isRecord(value)
    || !hasCompetitionSnapshotKeys(value, ['schemaVersion', 'bindingRef', 'recipients'])
    || value.schemaVersion !== 1 || !isCompetitionGameBindingRef(value.bindingRef)
    || !Array.isArray(value.recipients)) return false
  const recipients = value.recipients
  return recipients.every(recipient => isCompetitionSnapshotData(recipient) && isRecord(recipient)
    && hasCompetitionSnapshotKeys(recipient, ['userId', 'notificationId', 'createdAt', 'sent'])
    && isNonEmpty(recipient.userId) && isNonEmpty(recipient.notificationId)
    && typeof recipient.createdAt === 'string' && Number.isFinite(Date.parse(recipient.createdAt))
    && typeof recipient.sent === 'boolean')
    && new Set(recipients.map(recipient => (recipient as CompetitionRoomReadyNoticeRecipient).userId)).size
      === recipients.length
}

function newCompetitionAdmission(
  bindingRef: CompetitionGameBindingRef,
  binding: CompetitionGameBinding | null,
): CompetitionGameAdmission {
  return {
    schemaVersion: 1, bindingRef: structuredClone(bindingRef),
    bindingFingerprint: binding === null ? null : stableDeliveryFingerprint(binding),
    status: 'unreleased', version: 1, releaseCommandId: null, startRetryAtMs: null,
    start: null, receipts: [],
  }
}

function isCompetitionGameAdmission(value: unknown): value is CompetitionGameAdmission {
  if (!isCompetitionSnapshotData(value) || !isRecord(value)
    || !hasCompetitionSnapshotKeys(value, [
      'schemaVersion', 'bindingRef', 'bindingFingerprint', 'status', 'version',
      'releaseCommandId', 'startRetryAtMs', 'start', 'receipts',
    ]) || value.schemaVersion !== 1 || !isCompetitionGameBindingRef(value.bindingRef)
    || !isSafeNonNegativeInteger(value.version) || value.version < 1
    || (value.bindingFingerprint !== null && !isNonEmpty(value.bindingFingerprint))
    || (value.releaseCommandId !== null && !isNormalizedCompetitionId(value.releaseCommandId))
    || (value.startRetryAtMs !== null && (!isSafeNonNegativeInteger(value.startRetryAtMs)
      || value.startRetryAtMs > 8_640_000_000_000_000))
    || !Array.isArray(value.receipts)
    || !value.receipts.every(receipt => isRecord(receipt)
      && hasCompetitionSnapshotKeys(receipt, ['kind', 'commandId', 'fingerprint', 'result'])
      && (receipt.kind === 'release' || receipt.kind === 'revoke')
      && isNormalizedCompetitionId(receipt.commandId)
      && receipt.fingerprint === stableDeliveryFingerprint({ bindingRef: value.bindingRef, commandId: receipt.commandId })
      && (receipt.kind === 'release' ? receipt.result === true
        : receipt.result === 'revoked' || receipt.result === 'started'))
    || new Set(value.receipts.map(receipt => `${receipt.kind}:${receipt.commandId}`)).size !== value.receipts.length) return false
  if (value.releaseCommandId !== null && !value.receipts.some(receipt => (
    receipt.kind === 'release' && receipt.commandId === value.releaseCommandId
  ))) return false
  if (value.status === 'started') {
    return value.releaseCommandId !== null && value.bindingFingerprint !== null
      && value.startRetryAtMs === null && isRecord(value.start)
      && hasCompetitionSnapshotKeys(value.start, ['gameInstanceId', 'startSequence', 'startedAtMs'])
      && isNormalizedCompetitionId(value.start.gameInstanceId)
      && isSafeNonNegativeInteger(value.start.startSequence)
      && isSafeNonNegativeInteger(value.start.startedAtMs)
      && value.start.startedAtMs <= 8_640_000_000_000_000
      && !value.receipts.some(receipt => receipt.result === 'revoked')
  }
  if (value.start !== null || value.receipts.some(receipt => receipt.result === 'started')) return false
  if (value.status === 'revoked') return value.startRetryAtMs === null
    && value.receipts.some(receipt => receipt.result === 'revoked')
  if (value.status === 'noShow') return value.startRetryAtMs === null
    && value.releaseCommandId !== null && value.bindingFingerprint !== null
    && value.receipts.some(receipt => receipt.kind === 'release' && receipt.commandId === value.releaseCommandId)
    && !value.receipts.some(receipt => receipt.result === 'revoked')
  if (value.bindingFingerprint === null || value.receipts.some(receipt => receipt.kind === 'revoke')) return false
  return value.status === 'unreleased'
    ? value.releaseCommandId === null && value.startRetryAtMs === null && value.receipts.length === 0
    : value.status === 'released' && value.releaseCommandId !== null
}

function isCompetitionGameNoShowSnapshot(value: unknown): value is CompetitionGameNoShowSnapshot {
  return isCompetitionSnapshotData(value) && isRecord(value)
    && hasCompetitionSnapshotKeys(value, ['schemaVersion', 'bindingRef', 'commandId', 'deadlineMs', 'decision'])
    && value.schemaVersion === 1 && isCompetitionGameBindingRef(value.bindingRef)
    && isNormalizedCompetitionId(value.commandId)
    && value.commandId === competitionGameNoShowCommandId(value.bindingRef)
    && isSafeNonNegativeInteger(value.deadlineMs)
    && value.deadlineMs === value.bindingRef.roundStartMs
    && isCompetitionGameNoShowDecision(value.decision)
    && (value.decision.type === 'ready' || value.decision.type === 'noShow')
    && value.decision.sampledAtMs >= value.deadlineMs
}

function competitionAdmissionMatchesBinding(
  value: unknown,
  metadata: GameRoomMetadata,
  binding: CompetitionGameBinding,
): value is CompetitionGameAdmission {
  return isCompetitionGameAdmission(value)
    && value.bindingFingerprint === stableDeliveryFingerprint(binding)
    && value.bindingRef.gameId === metadata.gameId && value.bindingRef.matchId === binding.matchId
    && value.bindingRef.gameNumber === binding.gameNumber && value.bindingRef.roundStartMs === binding.roundStartMs
    && stableDeliveryFingerprint(value.bindingRef.roundRef) === stableDeliveryFingerprint(binding.roundRef)
    && (value.status !== 'started' || value.start?.gameInstanceId === metadata.gameInstanceId)
}

function isPristineCompetitionWaitingQueue(value: unknown, binding: CompetitionGameBinding): boolean {
  return isCompetitionSnapshotData(value) && isCompetitionDeliveryQueue(value)
    && competitionQueueMatchesBinding(value, binding)
    && value.revisionCursor === binding.expectedMatchRevision
    && value.entries.length === 0 && value.receipts.length === 0 && value.pendingTimeout === null
    && !value.terminalTimeout && !value.refereePaused
    && (value.clockControlReceipts?.length ?? 0) === 0 && (value.nextClockDeadlineMs ?? null) === null
}

function isNormalizedCompetitionId(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0 && value.trim() === value
}

function isNonEmpty(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0
}

function isSafeNonNegativeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function hasExactKeys(value: Record<string, unknown>, keys: string[]): boolean {
  const actual = Object.keys(value).sort()
  return JSON.stringify(actual) === JSON.stringify([...keys].sort())
}
