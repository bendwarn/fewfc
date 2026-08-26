import {
  isOnlineGameAction,
  type CommandReceipt,
  type CommandTransactionStatus,
  type GameRecord,
  type GameRoomMetadata,
  type GameRoomRequest,
  type GameRoomResponse,
  type OnlineGameAction,
  type RulesEngineResult,
  type RulesNeedsRandomnessResult,
  type RulesReadyResult,
  type TrustedRandomnessAction,
} from '../../shared/game-room'
import type { PlayerId } from '../../app/types/fewfc'
import { RulesEngineError } from '../rules-engine-error'

type PlayerCommandRequest = Extract<GameRoomRequest, { type: 'submitCommand' }>

type RulesEngineAction =
  | OnlineGameAction
  | { type: 'trustedRandomHandCandidates'; player: PlayerId; candidateAction: OnlineGameAction }
  | TrustedRandomnessAction

interface ResolutionTransaction {
  schemaVersion: 1
  gameInstanceId: string
  transactionId: string
  rootActorUserId: string
  rootActor: PlayerId
}

interface TransactionStorage {
  get<T>(key: string): Promise<T | undefined>
  put(entries: Record<string, unknown>): Promise<void>
  delete(key: string): Promise<boolean>
  transaction<T>(callback: (storage: TransactionStorage) => Promise<T>): Promise<T>
}

/**
 * Game Room 提供應用程式專屬規則與回應轉接器。交易模組擁有命令協定，絕不將
 * 儲存空間公開成公用儲存庫。
 */
interface OnlineCommandTransactionHost {
  storage: TransactionStorage
  metadata(): Promise<GameRoomMetadata | undefined>
  gameRecord(): Promise<GameRecord>
  playerFor(metadata: GameRoomMetadata, userId: string): PlayerId | undefined
  actionForPlayer(action: OnlineGameAction, player: PlayerId): OnlineGameAction
  callRules(
    action: RulesEngineAction,
    viewer: string | undefined,
    record: GameRecord,
  ): Promise<RulesEngineResult>
  response(
    metadata: GameRoomMetadata,
    actorUserId: string,
    playableActions?: RulesReadyResult['playableActions'],
    receipt?: CommandReceipt,
  ): Promise<GameRoomResponse>
  shuffle<T>(values: T[]): T[]
}

export interface ExecutedPlayerCommand {
  response: GameRoomResponse
  receipt?: CommandReceipt
  metadata: GameRoomMetadata
  previousPlayer?: PlayerId | null
  nextPlayer?: PlayerId | null
  committed: boolean
}

export class OnlineCommandTransactionError extends Error {
  constructor(
    readonly code: 'gameInstanceMismatch' | 'idempotencyConflict' | 'staleWrite' | 'commandValidation',
    readonly statusCode: 400 | 403 | 409,
    message: string,
  ) {
    super(message)
    this.name = 'OnlineCommandTransactionError'
  }
}

/**
 * 透過單一介面執行完整的玩家命令交易。它會在任何受信任隨機性工作前提交每個
 * 標準檢查點，並只在得知持久結果後回傳目前的房間回應。
 */
export async function executePlayerCommand(
  host: OnlineCommandTransactionHost,
  request: PlayerCommandRequest,
): Promise<ExecutedPlayerCommand> {
  const metadata = await host.metadata()
  if (!metadata) throw new Error('game room has not been created')
  if (metadata.status !== 'Active') {
    throw new OnlineCommandTransactionError('commandValidation', 409, 'room has not started')
  }

  const actor = host.playerFor(metadata, request.actorUserId)
  if (!actor) {
    throw new OnlineCommandTransactionError('commandValidation', 403, 'only room players may submit commands')
  }
  if (!isOnlineGameAction(request.action)) {
    throw new OnlineCommandTransactionError('commandValidation', 400, 'unsupported player action')
  }

  const record = await host.gameRecord()
  if (request.gameInstanceId !== record.gameInstanceId) {
    throw new OnlineCommandTransactionError('gameInstanceMismatch', 409, 'game instance does not match')
  }

  const action = host.actionForPlayer(request.action, actor)
  if (action.type === 'playableActions') {
    const rules = await requireReady(host.callRules(action, actor, record))
    return {
      response: await host.response(metadata, request.actorUserId, rules.playableActions),
      metadata,
      committed: false,
    }
  }

  const transactionId = request.transactionId?.trim() || `transaction:${request.commandId}`
  const payloadIdentity = commandIdentity({
    gameInstanceId: record.gameInstanceId,
    transactionId,
    actorUserId: request.actorUserId,
    actor,
    action,
  })
  const receiptKey = commandReceiptKey(record.gameInstanceId, request.commandId)
  const existingReceipt = await host.storage.get<CommandReceipt>(receiptKey)
  if (existingReceipt) {
    if (existingReceipt.payloadIdentity !== payloadIdentity) {
      throw new OnlineCommandTransactionError('idempotencyConflict', 409, 'command id was reused for another command')
    }
    await resumeTrustedRandomnessIfNeeded(host, metadata, record, existingReceipt)
    return {
      response: await host.response(metadata, request.actorUserId, undefined, existingReceipt),
      receipt: existingReceipt,
      metadata,
      committed: false,
    }
  }

  let current: RulesEngineResult
  let previousPlayer: PlayerId | null | undefined
  try {
    const currentRules = await host.callRules({ type: 'refresh' }, 'observer', record)
    previousPlayer = currentRules.type === 'ready' ? currentRules.state.currentPlayer : undefined
    const activeTransaction = await host.storage.get<ResolutionTransaction>(transactionKey(record.gameInstanceId))
    validateContinuation(currentRules, activeTransaction, transactionId, action, actor)
    const preparedAction = await withTrustedRandomness(host, action, actor, record)
    current = await host.callRules(preparedAction, actor, record)
  } catch (error) {
    const validationError = (
      error instanceof RulesEngineError && error.statusCode === 400
    ) || (
      error instanceof OnlineCommandTransactionError && error.code === 'commandValidation'
    )
    if (validationError) {
      const receipt = await commitRejectedReceipt(host, {
        record,
        commandId: request.commandId,
        transactionId,
        actorUserId: request.actorUserId,
        actor,
        payloadIdentity,
        errorCode: error.code,
      })
      return {
        response: await host.response(metadata, request.actorUserId, undefined, receipt),
        receipt,
        metadata,
        committed: false,
      }
    }
    throw error
  }

  const accepted = await commitPlayerCheckpoint(host, {
    metadata,
    record,
    result: current,
    commandId: request.commandId,
    transactionId,
    actorUserId: request.actorUserId,
    actor,
    action,
    payloadIdentity,
  })
  const finalCheckpoint = await drainTrustedRandomness(host, accepted.metadata, accepted.record, current)
  const finalMetadata = finalCheckpoint.metadata

  return {
    response: await host.response(finalMetadata, request.actorUserId, undefined, accepted.receipt),
    receipt: accepted.receipt,
    metadata: finalMetadata,
    previousPlayer,
    nextPlayer: finalCheckpoint.nextPlayer,
    committed: true,
  }
}

async function withTrustedRandomness(
  host: OnlineCommandTransactionHost,
  action: OnlineGameAction,
  actor: PlayerId,
  record: GameRecord,
): Promise<OnlineGameAction> {
  if (!('player' in action)) return action
  const candidates = await requireReady(host.callRules({
    type: 'trustedRandomHandCandidates',
    player: actor,
    candidateAction: action,
  }, actor, record))
  if (!candidates.trustedRandomCandidates) return action
  return {
    ...action,
    trustedRandomCards: host.shuffle(candidates.trustedRandomCandidates)
      .slice(0, candidates.trustedRandomCandidateCount ?? 0),
  } as OnlineGameAction
}

function validateContinuation(
  current: RulesEngineResult,
  active: ResolutionTransaction | undefined,
  transactionId: string,
  action: OnlineGameAction,
  actor: PlayerId,
) {
  if (current.type === 'needsRandomness') {
    throw new OnlineCommandTransactionError('commandValidation', 409, 'trusted randomness is still pending')
  }
  const pendingChoice = current.state.pendingChoice
  if (!pendingChoice) {
    if (active) {
      throw new Error('Engine invariant error: a transaction exists without a canonical waiting point')
    }
    return
  }
  if (!active || active.transactionId !== transactionId) {
    throw new OnlineCommandTransactionError('commandValidation', 409, 'command does not continue the pending transaction')
  }
  if (action.type !== 'answerChoice' || pendingChoice.player !== actor) {
    throw new OnlineCommandTransactionError('commandValidation', 409, 'only the canonical Pending Choice player may continue')
  }
}

async function commitPlayerCheckpoint(
  host: OnlineCommandTransactionHost,
  input: {
    metadata: GameRoomMetadata
    record: GameRecord
    result: RulesEngineResult
    commandId: string
    transactionId: string
    actorUserId: string
    actor: PlayerId
    action: OnlineGameAction
    payloadIdentity: string
  },
) {
  const nextSequence = input.record.sequence + 1
  const now = new Date().toISOString()
  const transactionStatus = statusFor(input.result)
  const nextRecord: GameRecord = {
    ...input.record,
    sequence: nextSequence,
    rulesRecord: input.result.record,
    finishedAt: input.result.type === 'ready' && input.result.state.status === 'Finished'
      ? now
      : input.record.finishedAt,
  }
  const nextMetadata: GameRoomMetadata = {
    ...input.metadata,
    status: input.result.type === 'ready' && input.result.state.status === 'Finished' ? 'Finished' : 'Active',
    updatedAt: now,
  }
  const receipt: CommandReceipt = {
    schemaVersion: 1,
    commandId: input.commandId,
    transactionId: input.transactionId,
    gameInstanceId: input.record.gameInstanceId,
    actorUserId: input.actorUserId,
    actor: input.actor,
    payloadIdentity: input.payloadIdentity,
    outcome: 'accepted',
    committedSequence: nextSequence,
    transactionStatus,
    createdAt: now,
  }
  const transaction: ResolutionTransaction = await host.storage.get<ResolutionTransaction>(
    transactionKey(input.record.gameInstanceId),
  ) ?? {
    schemaVersion: 1,
    gameInstanceId: input.record.gameInstanceId,
    transactionId: input.transactionId,
    rootActorUserId: input.actorUserId,
    rootActor: input.actor,
  }

  await commitCanonical(host, {
    expectedSequence: input.record.sequence,
    record: nextRecord,
    metadata: nextMetadata,
    event: {
      sequence: nextSequence,
      type: 'RulesCommandApplied',
      commandId: input.commandId,
      transactionId: input.transactionId,
      actor: input.actor,
      payload: input.action,
      createdAt: now,
    },
    receipt,
    transaction: transactionStatus === 'complete' ? undefined : transaction,
  })
  return { record: nextRecord, metadata: nextMetadata, receipt }
}

async function drainTrustedRandomness(
  host: OnlineCommandTransactionHost,
  metadata: GameRoomMetadata,
  record: GameRecord,
  initial: RulesEngineResult,
): Promise<{ record: GameRecord; metadata: GameRoomMetadata; nextPlayer?: PlayerId | null }> {
  if (initial.type === 'ready') {
    return { record, metadata, nextPlayer: initial.state.currentPlayer }
  }

  let pending = initial
  let currentRecord = record
  let currentMetadata = metadata

  while (true) {
    const request = pending.request
    const answer: TrustedRandomnessAction = {
      type: 'resolveRandomness',
      requestId: request.requestId,
      shuffledOrder: host.shuffle(request.currentOrder),
    }
    const result = await host.callRules(answer, 'observer', currentRecord)
    const nextSequence = currentRecord.sequence + 1
    const now = new Date().toISOString()
    const checkpoint: GameRecord = {
      ...currentRecord,
      sequence: nextSequence,
      rulesRecord: result.record,
      finishedAt: result.type === 'ready' && result.state.status === 'Finished'
        ? now
        : currentRecord.finishedAt,
    }
    const transactionStatus = statusFor(result)
    const transaction = await requireTransaction(host, currentRecord.gameInstanceId)
    const checkpointMetadata: GameRoomMetadata = {
      ...currentMetadata,
      status: result.type === 'ready' && result.state.status === 'Finished' ? 'Finished' : 'Active',
      updatedAt: now,
    }
    await commitCanonical(host, {
      expectedSequence: currentRecord.sequence,
      record: checkpoint,
      metadata: checkpointMetadata,
      event: {
        sequence: nextSequence,
        type: 'TrustedRandomnessResolved',
        transactionId: transaction.transactionId,
        payload: { requestId: request.requestId, status: transactionStatus },
        createdAt: now,
      },
      trustedReceipt: {
        schemaVersion: 1,
        requestId: request.requestId,
        transactionId: transaction.transactionId,
        gameInstanceId: currentRecord.gameInstanceId,
        committedSequence: nextSequence,
        answer,
        createdAt: now,
      },
      transaction: transactionStatus === 'complete'
        ? undefined
        : transaction,
    })
    currentRecord = checkpoint
    currentMetadata = checkpointMetadata
    if (result.type === 'ready') {
      return { record: currentRecord, metadata: currentMetadata, nextPlayer: result.state.currentPlayer }
    }
    pending = result
  }
}

async function resumeTrustedRandomnessIfNeeded(
  host: OnlineCommandTransactionHost,
  metadata: GameRoomMetadata,
  record: GameRecord,
  receipt: CommandReceipt,
) {
  if (receipt.outcome !== 'accepted') return
  const current = await host.callRules({ type: 'refresh' }, 'observer', record)
  if (current.type !== 'needsRandomness') return
  const transaction = await host.storage.get<ResolutionTransaction>(transactionKey(record.gameInstanceId))
  if (!transaction || transaction.transactionId !== receipt.transactionId) return
  await drainTrustedRandomness(host, metadata, record, current)
}

async function commitRejectedReceipt(
  host: OnlineCommandTransactionHost,
  input: {
    record: GameRecord
    commandId: string
    transactionId: string
    actorUserId: string
    actor: PlayerId
    payloadIdentity: string
    errorCode: string
  },
): Promise<CommandReceipt> {
  const receipt: CommandReceipt = {
    schemaVersion: 1,
    commandId: input.commandId,
    transactionId: input.transactionId,
    gameInstanceId: input.record.gameInstanceId,
    actorUserId: input.actorUserId,
    actor: input.actor,
    payloadIdentity: input.payloadIdentity,
    outcome: 'rejected',
    observedSequence: input.record.sequence,
    errorCode: input.errorCode,
    createdAt: new Date().toISOString(),
  }
  await host.storage.transaction(async (storage) => {
    const nextSequence = await storage.get<number>('nextSequence') ?? 1
    if (nextSequence - 1 !== input.record.sequence) throw staleWrite()
    await storage.put({ [commandReceiptKey(input.record.gameInstanceId, input.commandId)]: receipt })
  })
  return receipt
}

async function commitCanonical(
  host: OnlineCommandTransactionHost,
  input: {
    expectedSequence: number
    record: GameRecord
    metadata: GameRoomMetadata
    event: {
      sequence: number
      type: string
      commandId?: string
      transactionId?: string
      actor?: PlayerId
      payload: unknown
      createdAt: string
    }
    receipt?: CommandReceipt
    trustedReceipt?: unknown
    transaction?: ResolutionTransaction
  },
) {
  await host.storage.transaction(async (storage) => {
    const nextSequence = await storage.get<number>('nextSequence') ?? 1
    if (nextSequence - 1 !== input.expectedSequence) throw staleWrite()
    const values: Record<string, unknown> = {
      [eventKey(input.event.sequence)]: input.event,
      nextSequence: input.event.sequence + 1,
      gameRecord: input.record,
      metadata: input.metadata,
    }
    if (input.receipt) values[commandReceiptKey(input.record.gameInstanceId, input.receipt.commandId)] = input.receipt
    if (input.trustedReceipt) {
      const requestId = (input.trustedReceipt as { requestId: string }).requestId
      values[trustedReceiptKey(input.record.gameInstanceId, requestId)] = input.trustedReceipt
    }
    if (input.transaction) values[transactionKey(input.record.gameInstanceId)] = input.transaction
    await storage.put(values)
    if (!input.transaction) await storage.delete(transactionKey(input.record.gameInstanceId))
  })
}

function statusFor(result: RulesEngineResult): CommandTransactionStatus {
  if (result.type === 'needsRandomness') return 'awaitingRandomness'
  return result.state.pendingChoice ? 'awaitingChoice' : 'complete'
}

async function requireTransaction(host: OnlineCommandTransactionHost, gameInstanceId: string) {
  const transaction = await host.storage.get<ResolutionTransaction>(transactionKey(gameInstanceId))
  if (!transaction) throw new Error('resolution transaction is missing')
  return transaction
}

async function requireReady(result: Promise<RulesEngineResult>): Promise<RulesReadyResult> {
  const resolved = await result
  if (resolved.type === 'needsRandomness') throw new Error('Rules Engine unexpectedly requested randomness')
  return resolved
}

function staleWrite() {
  return new OnlineCommandTransactionError('staleWrite', 409, 'game record changed before command commit')
}

function commandIdentity(value: unknown): string {
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

function commandReceiptKey(gameInstanceId: string, commandId: string) {
  return `commandReceipt:${gameInstanceId}:${commandId}`
}

function trustedReceiptKey(gameInstanceId: string, requestId: string) {
  return `trustedReceipt:${gameInstanceId}:${requestId}`
}

function transactionKey(gameInstanceId: string) {
  return `transaction:${gameInstanceId}`
}

function eventKey(sequence: number) {
  return `event:${sequence.toString().padStart(12, '0')}`
}
