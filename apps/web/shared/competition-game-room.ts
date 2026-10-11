import type { GameConclusion, GameStartKind } from './competition'
import type { PlayerId, PublicGameState, PublicPendingChoice } from '../app/types/fewfc'
import type { PlayerDeckList, RuleVersion, RulesGameSetup } from './game-room'
import type {
  CompetitionRoomMatchOperation,
  CompetitionRoomMatchExecuteRequest,
} from './competition-room'

export type CompetitionClockOperation = Extract<CompetitionRoomMatchOperation, {
  type: 'clockBeginPreparation'
    | 'clockFinishPreparation'
    | 'clockBeginAction'
    | 'clockBeginOutOfTurnChoice'
    | 'clockObserve'
    | 'clockEndWindow'
    | 'clockPause'
    | 'clockResume'
    | 'clockNewGame'
}>

/** 從相鄰的 canonical 公開狀態推導時鐘視窗轉換；不讀取或重算時間。 */
export function competitionClockOperationsForTransition(
  binding: CompetitionGameBinding,
  previous: PublicGameState,
  next: PublicGameState,
): CompetitionClockOperation[] {
  const operations: CompetitionClockOperation[] = []
  if (next.status === 'Finished') {
    // 終局也要先結束最後一段玩家視窗，讓權威時計計入終局命令前的耗時。
    operations.push({ type: 'clockEndWindow', matchId: binding.matchId })
    return operations
  }

  const previousRemaining = previous.initialPouchSelection?.remainingPlayers ?? []
  const nextRemaining = next.initialPouchSelection?.remainingPlayers ?? []
  const teamIds = [...new Set(binding.players.map(player => player.team))]
  for (const team of teamIds) {
    const wasWaiting = binding.players.some(player => player.team === team
      && previousRemaining.includes(player.player))
    const isWaiting = binding.players.some(player => player.team === team
      && nextRemaining.includes(player.player))
    if (wasWaiting && !isWaiting) {
      operations.push({ type: 'clockFinishPreparation', matchId: binding.matchId, team })
    }
  }
  if (previous.initialPouchSelection !== null
    && next.initialPouchSelection === null
    && next.status === 'InProgress') {
    const player = requireCurrentPlayer(binding, next)
    const team = teamForPlayer(binding, player)
    operations.push({
      type: 'clockBeginAction',
      matchId: binding.matchId,
      team,
      turn: next.turnNumber,
    })
    return operations
  }

  const oldOutOfTurn = independentClockChoice(binding, previous.pendingChoice, previous.currentPlayer)
  const newOutOfTurn = independentClockChoice(binding, next.pendingChoice, next.currentPlayer)
  if (oldOutOfTurn || newOutOfTurn) {
    if (oldOutOfTurn && (!newOutOfTurn || oldOutOfTurn.choiceId !== newOutOfTurn.choiceId)) {
      operations.push({ type: 'clockEndWindow', matchId: binding.matchId })
    }
    if (newOutOfTurn && (!oldOutOfTurn || oldOutOfTurn.choiceId !== newOutOfTurn.choiceId)) {
      if (!oldOutOfTurn) operations.push({ type: 'clockEndWindow', matchId: binding.matchId })
      operations.push({
        type: 'clockBeginOutOfTurnChoice',
        matchId: binding.matchId,
        team: newOutOfTurn.team,
        choiceId: newOutOfTurn.choiceId,
      })
    } else if (oldOutOfTurn && !newOutOfTurn && next.status === 'InProgress') {
      const player = requireCurrentPlayer(binding, next)
      operations.push({
        type: 'clockBeginAction',
        matchId: binding.matchId,
        team: teamForPlayer(binding, player),
        turn: next.turnNumber,
      })
    }
    return operations
  }

  if (next.phase === 'ActiveEffects'
    && next.currentPlayer !== null
    && (previous.currentPlayer !== next.currentPlayer || previous.turnNumber !== next.turnNumber)) {
    operations.push({ type: 'clockEndWindow', matchId: binding.matchId })
    operations.push({
      type: 'clockBeginAction',
      matchId: binding.matchId,
      team: teamForPlayer(binding, next.currentPlayer),
      turn: next.turnNumber,
    })
  }
  return operations
}

/** 只選取需獨立時計視窗的回合外選擇；同隊隊友仍屬回合外選擇。 */
function independentClockChoice(
  binding: CompetitionGameBinding,
  choice: PublicPendingChoice | null,
  currentPlayer: PlayerId | null,
): { team: string; choiceId: string } | null {
  if (!choice || choice.player === currentPlayer) return null
  if (choice.visibility !== 'visible' || currentPlayer === null
    || !Number.isSafeInteger(choice.choiceId) || choice.choiceId < 0) {
    throw new Error('competition out-of-turn choice lacks a verifiable public identity')
  }
  const team = teamForPlayer(binding, choice.player)
  if (team === teamForPlayer(binding, currentPlayer)) return null
  return {
    team,
    choiceId: String(choice.choiceId),
  }
}

function requireCurrentPlayer(binding: CompetitionGameBinding, state: PublicGameState): PlayerId {
  if (state.currentPlayer === null) throw new Error('competition action transition has no current player')
  teamForPlayer(binding, state.currentPlayer)
  return state.currentPlayer
}

function teamForPlayer(binding: CompetitionGameBinding, player: PlayerId): string {
  const team = binding.players.find(member => member.player === player)?.team
  if (!team) throw new Error('competition state refers to an unbound player')
  return team
}

/** 由 GameRoom 內部建立的 server-only 時鐘交付請求。 */
export type CompetitionClockDeliveryRequest = Omit<CompetitionRoomMatchExecuteRequest, 'operation'> & {
  operation: CompetitionClockOperation
}

export interface CompetitionGamePlayerBinding {
  userId: string
  player: PlayerId
  team: string
}

export interface CompetitionGameDeckSnapshot extends PlayerDeckList {
  player: PlayerId
}

/** 只由可信賽事編排層提供；此型別不得加入一般玩家 request union。 */
export interface CompetitionGameBinding {
  roundRef: CompetitionRoundRef
  roundStartMs: number
  competitionId: string
  matchId: string
  gameNumber: number
  organizerUserId: string
  expectedMatchRevision: number
  startKind: GameStartKind
  ruleVersion: RuleVersion
  ruleset: 'fewfc-base'
  enabledRuleModules: string[]
  initialHp: Array<{ team: string; hp: number }>
  players: CompetitionGamePlayerBinding[]
  deckSnapshot: CompetitionGameDeckSnapshot[]
  firstPlayer: PlayerId
  turnOrder: PlayerId[]
  deckSeed: string
}

export interface CompetitionGameCreateRequest {
  gameId: string
  binding: CompetitionGameBinding
}

export interface CompetitionGameStartRequest {
  gameId: string
  binding: CompetitionGameBinding
}

/** 僅供伺服器核對目前固定對局；結果只回 boolean，不回傳私密房間資料。 */
export interface CompetitionGameClockControlEligibilityRequest {
  competitionId: string
  matchId: string
  gameId: string
  gameNumber: number
}

export function isCompetitionGameClockControlEligibilityRequest(
  value: unknown,
): value is CompetitionGameClockControlEligibilityRequest {
  try {
    if (!isRecord(value)
      || (Object.getPrototypeOf(value) !== Object.prototype && Object.getPrototypeOf(value) !== null)) return false
    const keys = ['competitionId', 'matchId', 'gameId', 'gameNumber']
    const actual = Reflect.ownKeys(value)
    if (actual.length !== keys.length || !actual.every(key => typeof key === 'string' && keys.includes(key))
      || !keys.every(key => {
        const descriptor = Object.getOwnPropertyDescriptor(value, key)
        return descriptor?.enumerable === true && Object.hasOwn(descriptor, 'value')
      })) return false
    return [value.competitionId, value.matchId, value.gameId].every(id => (
      isNonEmptyId(id) && id.trim() === id
    )) && typeof value.gameNumber === 'number' && Number.isSafeInteger(value.gameNumber) && value.gameNumber > 0
  } catch {
    return false
  }
}

/** 固定賽事成員的明確準備狀態；不可攜帶帳號牌組或 caller 控制欄位。 */
export interface CompetitionGameReadyRequest {
  actorUserId: string
  ready: boolean
}

export type CompetitionGameReadyResult =
  | { ok: true; ready: boolean }
  | {
      ok: false
      code:
        | 'invalidRequest'
        | 'notCompetitionRoom'
        | 'invalidCompetitionRoom'
        | 'fixedMemberRequired'
        | 'playerNotConnected'
        | 'lockedDeckMissing'
        | 'gameAlreadyStarted'
        | 'gameFinished'
        | 'roomNotWaiting'
    }

export function isCompetitionGameReadyRequest(
  value: unknown,
): value is CompetitionGameReadyRequest {
  return isRecord(value)
    && hasExactKeys(value, ['actorUserId', 'ready'])
    && isNonEmptyId(value.actorUserId)
    && value.actorUserId.trim() === value.actorUserId
    && typeof value.ready === 'boolean'
}

/** 裁判計時控制的持久交付；僅供 CompetitionRoom 與 GameRoom 之間使用。 */
export interface CompetitionClockControlRequest {
  controlId: string
  competitionId: string
  matchId: string
  gameId: string
  gameNumber: number
  sourceCommandId: string
  matchRevision: number
  operation: 'clockPause' | 'clockResume'
  trustedTimeMs: number
  nextClockDeadlineMs: number | null
}

export interface CompetitionClockControlAck extends CompetitionClockControlRequest {
  idempotent: boolean
}

export interface CompetitionGameRoomResult {
  gameId: string
  gameInstanceId?: string
  status: 'Waiting' | 'Active' | 'Finished' | 'Dissolved'
  idempotent: boolean
}

/** 驗證跨 DO 控制回覆的完整 camelCase payload，避免只憑部分欄位推進 revision。 */
export function isCompetitionClockControlAck(
  value: unknown,
  request: CompetitionClockControlRequest,
): value is { ok: true; value: CompetitionClockControlAck } {
  if (!isRecord(value) || !hasExactKeys(value, ['ok', 'value']) || value.ok !== true
    || !isRecord(value.value)
    || !hasExactKeys(value.value, [
      'controlId', 'competitionId', 'matchId', 'gameId', 'gameNumber', 'sourceCommandId',
      'matchRevision', 'operation', 'trustedTimeMs', 'nextClockDeadlineMs', 'idempotent',
    ])) return false
  const ack = value.value
  return ack.controlId === request.controlId
    && ack.competitionId === request.competitionId
    && ack.matchId === request.matchId
    && ack.gameId === request.gameId
    && ack.gameNumber === request.gameNumber
    && ack.sourceCommandId === request.sourceCommandId
    && ack.matchRevision === request.matchRevision
    && ack.operation === request.operation
    && ack.trustedTimeMs === request.trustedTimeMs
    && ack.nextClockDeadlineMs === request.nextClockDeadlineMs
    && typeof ack.idempotent === 'boolean'
}

export class CompetitionGameBindingError extends Error {
  constructor(readonly code: 'invalidCompetitionBinding' | 'competitionBindingConflict' | 'competitionGameNotReady' | 'competitionBindingDigestFailed') {
    super(code === 'competitionBindingConflict'
      ? 'competition game binding does not match the existing room'
      : code === 'competitionGameNotReady'
        ? 'all fixed competition members must be connected and ready'
        : code === 'competitionBindingDigestFailed'
          ? 'competition game binding fingerprint could not be computed'
          : 'competition game binding is invalid')
    this.name = 'CompetitionGameBindingError'
  }
}

export function validateCompetitionGameRequest(
  value: unknown,
): asserts value is CompetitionGameCreateRequest {
  try {
    if (!roundReleaseExactRecord(value, ['gameId', 'binding']) || !roundReleaseId(value.gameId)) {
      throw new CompetitionGameBindingError('invalidCompetitionBinding')
    }
    validateCompetitionGameBinding(value.binding)
  } catch {
    throw new CompetitionGameBindingError('invalidCompetitionBinding')
  }
}

export function validateCompetitionGameBinding(
  value: unknown,
): asserts value is CompetitionGameBinding {
  try {
    if (!isCompetitionGameBindingShape(value)) {
      throw new CompetitionGameBindingError('invalidCompetitionBinding')
    }
    const binding = value
    const players = binding.players
    if (!hasUniqueNonEmptyStrings(binding.enabledRuleModules)
      || !hasUniqueNonEmptyStrings(players.map(player => player.userId))
      || !hasUniqueNonEmptyStrings(players.map(player => player.player))) {
      throw new CompetitionGameBindingError('invalidCompetitionBinding')
    }

    const teams = [...new Set(players.map(player => player.team))]
    const teamCounts = new Map(teams.map(team => [team, 0]))
    for (const player of players) {
      teamCounts.set(player.team, (teamCounts.get(player.team) ?? 0) + 1)
    }
    if (teams.length !== 2 || new Set(teamCounts.values()).size !== 1) {
      throw new CompetitionGameBindingError('invalidCompetitionBinding')
    }

    const playerById = new Map(players.map(player => [player.player, player]))
    const turnOrder = binding.turnOrder
    if (turnOrder.length !== players.length
      || !hasUniqueNonEmptyStrings(turnOrder)
      || turnOrder.some(player => !playerById.has(player))
      || binding.firstPlayer !== turnOrder[0]) {
      throw new CompetitionGameBindingError('invalidCompetitionBinding')
    }
    for (let index = 1; index < turnOrder.length; index += 1) {
      if (playerById.get(turnOrder[index - 1]!)?.team === playerById.get(turnOrder[index]!)?.team) {
        throw new CompetitionGameBindingError('invalidCompetitionBinding')
      }
    }

    const decks = binding.deckSnapshot
    const usesPersonalDeck = binding.enabledRuleModules.includes('personal-deck')
    if ((usesPersonalDeck ? decks.length !== players.length : decks.length !== 0)
      || !hasUniqueNonEmptyStrings(decks.map(deck => deck.player))
      || decks.some(deck => !playerById.has(deck.player))) {
      throw new CompetitionGameBindingError('invalidCompetitionBinding')
    }

    const initialHp = binding.initialHp
    if (initialHp.length !== teams.length
      || !hasUniqueNonEmptyStrings(initialHp.map(entry => entry.team))
      || initialHp.some(entry => !teamCounts.has(entry.team)
        || !Number.isInteger(entry.hp) || entry.hp <= 0)) {
      throw new CompetitionGameBindingError('invalidCompetitionBinding')
    }

    if (binding.startKind.type === 'eliminationDrawContinuation') {
      const entries = binding.startKind.continuation.teams
      if (entries.length !== teams.length
        || !hasUniqueNonEmptyStrings(entries.map(entry => entry.team))
        || entries.some(entry => !teamCounts.has(entry.team))) {
        throw new CompetitionGameBindingError('invalidCompetitionBinding')
      }
    }
  } catch {
    // 跨可信 JSON 邊界仍固定拒絕 reflection／資料錯誤，不反射私密例外。
    throw new CompetitionGameBindingError('invalidCompetitionBinding')
  }
}

export function competitionRulesSetup(binding: CompetitionGameBinding): RulesGameSetup {
  validateCompetitionGameBinding(binding)

  return {
    ruleVersion: binding.ruleVersion,
    players: binding.players.map(player => ({
      id: player.player,
      team: player.team,
    })),
    turnOrder: [...binding.turnOrder],
    enabledRuleModules: [...binding.enabledRuleModules],
    deckLists: binding.deckSnapshot.map(deck => ({
      player: deck.player,
      name: deck.name,
      cards: [...deck.cards],
    })),
    initialHp: binding.initialHp.map(entry => ({ ...entry })),
  }
}

export function competitionGameBindingEquals(
  left: CompetitionGameBinding,
  right: CompetitionGameBinding,
): boolean {
  try {
    validateCompetitionGameBinding(left)
    validateCompetitionGameBinding(right)
    return JSON.stringify(canonicalBinding(left)) === JSON.stringify(canonicalBinding(right))
  } catch {
    throw new CompetitionGameBindingError('invalidCompetitionBinding')
  }
}

export function competitionDeckEquals(
  left: PlayerDeckList,
  right: CompetitionGameDeckSnapshot,
): boolean {
  return left.name === right.name
    && left.cards.length === right.cards.length
    && left.cards.every((card, index) => card === right.cards[index])
}

/** 建立 owned canonical snapshot；陣列保留權威順序，不持有輸入的 nested reference。 */
function canonicalBinding(binding: CompetitionGameBinding) {
  const startKind: GameStartKind = binding.startKind.type === 'ordinaryNewGame'
    ? { type: 'ordinaryNewGame' }
    : {
        type: 'eliminationDrawContinuation',
        continuation: {
          teams: binding.startKind.continuation.teams.map(entry => ({
            team: entry.team,
            mainTimeRemainingMs: entry.mainTimeRemainingMs,
          })),
        },
      }
  return {
    roundRef: {
      competitionId: binding.roundRef.competitionId,
      stageId: binding.roundRef.stageId,
      roundNumber: binding.roundRef.roundNumber,
      roundGenerationId: binding.roundRef.roundGenerationId,
    },
    roundStartMs: binding.roundStartMs,
    binding: {
      competitionId: binding.competitionId,
      matchId: binding.matchId,
      gameNumber: binding.gameNumber,
      organizerUserId: binding.organizerUserId,
      expectedMatchRevision: binding.expectedMatchRevision,
      startKind,
      ruleVersion: binding.ruleVersion,
      ruleset: binding.ruleset,
      enabledRuleModules: [...binding.enabledRuleModules],
      initialHp: binding.initialHp.map(entry => ({ team: entry.team, hp: entry.hp })),
      players: binding.players.map(player => ({
        userId: player.userId,
        player: player.player,
        team: player.team,
      })),
      deckSnapshot: binding.deckSnapshot.map(deck => ({
        player: deck.player,
        name: deck.name,
        cards: [...deck.cards],
      })),
      firstPlayer: binding.firstPlayer,
      turnOrder: [...binding.turnOrder],
      deckSeed: binding.deckSeed,
    },
  }
}

function canonicalCompetitionGameRequest(value: unknown) {
  try {
    validateCompetitionGameRequest(value)
    return { gameId: value.gameId, ...canonicalBinding(value.binding) }
  } catch {
    throw new CompetitionGameBindingError('invalidCompetitionBinding')
  }
}

/** 完整驗證後同步擷取 snapshot，再 await SHA256；fingerprint 不是 ACL／開局授權。 */
export async function competitionGameBindingRefForRequest(
  value: unknown,
): Promise<CompetitionGameBindingRef> {
  const snapshot = canonicalCompetitionGameRequest(value)
  const bytes = new TextEncoder().encode(JSON.stringify({
    domain: COMPETITION_GAME_BINDING_DIGEST_DOMAIN,
    ...snapshot,
  }))
  try {
    const digest = await crypto.subtle.digest('SHA-256', bytes)
    if (digest.byteLength !== 32) throw new CompetitionGameBindingError('competitionBindingDigestFailed')
    const bindingDigest = Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, '0')).join('')
    return {
      roundRef: snapshot.roundRef,
      roundStartMs: snapshot.roundStartMs,
      matchId: snapshot.binding.matchId,
      gameId: snapshot.gameId,
      gameNumber: snapshot.binding.gameNumber,
      bindingDigest,
    }
  } catch {
    throw new CompetitionGameBindingError('competitionBindingDigestFailed')
  }
}

/** 規則引擎回傳的終局結論是跨 DO outbox 的唯一 canonical evidence。 */
export function isCompetitionGameConclusion(value: unknown): value is GameConclusion {
  if (!isRecord(value) || !hasExactKeys(value, ['outcome', 'causes'])
    || !Array.isArray(value.causes) || value.causes.length === 0) {
    return false
  }
  if (!isRecord(value.outcome) || typeof value.outcome.type !== 'string') return false
  if (value.outcome.type === 'draw') {
    if (!hasExactKeys(value.outcome, ['type'])) return false
  } else if (value.outcome.type === 'winner') {
    if (!hasExactKeys(value.outcome, ['type', 'team']) || !isNonEmptyId(value.outcome.team)) {
      return false
    }
  } else {
    return false
  }

  return value.causes.every(cause => {
    if (!isRecord(cause) || typeof cause.type !== 'string') return false
    if (cause.type === 'teamHpDepleted') {
      return hasExactKeys(cause, ['type', 'teams'])
        && Array.isArray(cause.teams)
        && cause.teams.length > 0
        && hasUniqueNonEmptyStrings(cause.teams)
    }
    if (cause.type === 'directVictory') {
      return hasExactKeys(cause, ['type', 'rule', 'team'])
        && isNonEmptyId(cause.rule)
        && isNonEmptyId(cause.team)
    }
    return false
  })
}

function isCompetitionGameBindingShape(value: unknown): value is CompetitionGameBinding {
  return roundReleaseExactRecord(value, [
    'roundRef', 'roundStartMs', 'competitionId', 'matchId', 'gameNumber', 'organizerUserId',
    'expectedMatchRevision', 'startKind', 'ruleVersion', 'ruleset', 'enabledRuleModules',
    'initialHp', 'players', 'deckSnapshot', 'firstPlayer', 'turnOrder', 'deckSeed',
  ])
    && isCompetitionRoundRef(value.roundRef)
    && value.roundRef.competitionId === value.competitionId
    && competitionRoundReadyCutoffMs(value.roundStartMs) !== null
    && roundReleaseId(value.competitionId)
    && roundReleaseId(value.matchId)
    && roundReleaseUnsignedInteger(value.gameNumber, ROUND_RELEASE_MAX_GAME_NUMBER)
    && value.gameNumber > 0
    && roundReleaseId(value.organizerUserId)
    && roundReleaseUnsignedInteger(value.expectedMatchRevision, Number.MAX_SAFE_INTEGER)
    && value.expectedMatchRevision > 0
    && isGameStartKind(value.startKind)
    && isRuleVersion(value.ruleVersion)
    && value.ruleset === 'fewfc-base'
    && competitionBindingDenseArray(value.enabledRuleModules)
    && value.enabledRuleModules.every(roundReleaseId)
    && competitionBindingDenseArray(value.initialHp)
    && value.initialHp.every(isInitialHp)
    && competitionBindingDenseArray(value.players)
    && (value.players.length === 2 || value.players.length === 4)
    && value.players.every(isPlayerBinding)
    && competitionBindingDenseArray(value.deckSnapshot)
    && value.deckSnapshot.every(isDeckSnapshot)
    && roundReleaseId(value.firstPlayer)
    && competitionBindingDenseArray(value.turnOrder)
    && value.turnOrder.every(roundReleaseId)
    && isNonEmptyId(value.deckSeed)
}

function isGameStartKind(value: unknown): value is GameStartKind {
  return (roundReleaseExactRecord(value, ['type']) && value.type === 'ordinaryNewGame')
    || (roundReleaseExactRecord(value, ['type', 'continuation'])
      && value.type === 'eliminationDrawContinuation'
      && roundReleaseExactRecord(value.continuation, ['teams'])
      && competitionBindingDenseArray(value.continuation.teams)
      && value.continuation.teams.every(entry => roundReleaseExactRecord(entry, ['team', 'mainTimeRemainingMs'])
        && roundReleaseId(entry.team)
        && (entry.mainTimeRemainingMs === null
          || roundReleaseUnsignedInteger(entry.mainTimeRemainingMs, Number.MAX_SAFE_INTEGER))))
}

function isRuleVersion(value: unknown): value is RuleVersion {
  return value === '5.16' || value === '5.17'
}

function isPlayerBinding(value: unknown): value is CompetitionGamePlayerBinding {
  return roundReleaseExactRecord(value, ['userId', 'player', 'team'])
    && roundReleaseId(value.userId)
    && roundReleaseId(value.player)
    && roundReleaseId(value.team)
}

function isDeckSnapshot(value: unknown): value is CompetitionGameDeckSnapshot {
  return roundReleaseExactRecord(value, ['player', 'name', 'cards'])
    && roundReleaseId(value.player)
    && isNonEmptyId(value.name)
    && competitionBindingDenseArray(value.cards)
    && value.cards.every(card => typeof card === 'string')
}

function isInitialHp(value: unknown): value is { team: string; hp: number } {
  return roundReleaseExactRecord(value, ['team', 'hp'])
    && roundReleaseId(value.team)
    && typeof value.hp === 'number'
}

/** 密集 plain array 只接受 own enumerable 資料元素；凍結 length 不影響合法性。 */
function competitionBindingDenseArray(value: unknown): value is unknown[] {
  if (!Array.isArray(value) || Object.getPrototypeOf(value) !== Array.prototype) return false
  const length = Object.getOwnPropertyDescriptor(value, 'length')
  if (!length || length.enumerable !== false || !Object.hasOwn(length, 'value')
    || typeof length.value !== 'number' || !Number.isSafeInteger(length.value) || length.value < 0) return false
  const keys = Reflect.ownKeys(value)
  if (keys.length !== length.value + 1) return false
  for (let index = 0; index < length.value; index += 1) {
    const element = Object.getOwnPropertyDescriptor(value, String(index))
    if (!element || element.enumerable !== true || !Object.hasOwn(element, 'value')) return false
  }
  return true
}

function isNonEmptyId(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0
}

function hasUniqueNonEmptyStrings(values: unknown[]): values is string[] {
  return values.every(isNonEmptyId) && new Set(values).size === values.length
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function hasExactKeys(value: Record<string, unknown>, keys: string[]): boolean {
  const actual = Object.keys(value).sort()
  return JSON.stringify(actual) === JSON.stringify([...keys].sort())
}

/** 輪次發布與完整 binding 共用 required 身分及 T，不加 optional 或 fallback。 */
export const COMPETITION_ROUND_ROOM_LEAD_TIME_MS = 300_000
/** 完整 canonical envelope 的 v1 domain；不得用部分 binding 計算 fingerprint。 */
export const COMPETITION_GAME_BINDING_DIGEST_DOMAIN = 'fewfc.competitionGameBinding.roundRelease.v1'

export interface CompetitionRoundRef {
  competitionId: string
  stageId: string
  roundNumber: number
  roundGenerationId: string
}

export interface CompetitionGameBindingRef {
  roundRef: CompetitionRoundRef
  roundStartMs: number
  matchId: string
  gameId: string
  gameNumber: number
  /** 完整已驗 binding 的 v1 SHA256 lowercase hex；此 ref guard 仍只驗格式。 */
  bindingDigest: string
}

export interface CompetitionRoomReadyAck {
  gameId: string
  bindingDigest: string
  status: 'Waiting'
}

/** release 的 commandId 就是父持久 releaseDecisionId；revoke 用原固定 outbox ID。 */
export interface CompetitionGameAdmissionRequest {
  bindingRef: CompetitionGameBindingRef
  commandId: string
}

export type CompetitionGameReleaseRequest = CompetitionGameAdmissionRequest
export type CompetitionGameRevokeRequest = CompetitionGameAdmissionRequest
export type CompetitionGameReleaseResult = boolean
export type CompetitionGameRevokeResult = 'revoked' | 'started' | 'invalid'

export type CompetitionGameInitialStartWindow =
  | { type: 'preparation' }
  | { type: 'action'; team: string; turn: number }

/** 只供首局；真 instance／事件序號／視窗及開局授權仍須可信 consumer 核對。 */
export interface CompetitionGameInitialStartEvidence {
  bindingRef: CompetitionGameBindingRef
  releaseDecisionId: string
  gameInstanceId: string
  startSequence: number
  startedAtMs: number
  initialWindow: CompetitionGameInitialStartWindow
}

const ROUND_RELEASE_MAX_EPOCH_MS = 8_640_000_000_000_000
const ROUND_RELEASE_MAX_ROUND_NUMBER = 0xffff_ffff
const ROUND_RELEASE_MAX_GAME_NUMBER = 0xffff

/** 只做安全五分鐘算術；不採 client now、不證明全房 ACK 或允許開局。 */
export function competitionRoundReadyCutoffMs(value: unknown): number | null {
  if (!roundReleaseEpochMs(value) || value < COMPETITION_ROUND_ROOM_LEAD_TIME_MS) return null
  return value - COMPETITION_ROUND_ROOM_LEAD_TIME_MS
}

export function isCompetitionRoundRef(value: unknown): value is CompetitionRoundRef {
  try {
    return roundReleaseExactRecord(value, ['competitionId', 'stageId', 'roundNumber', 'roundGenerationId'])
      && roundReleaseId(value.competitionId)
      && roundReleaseId(value.stageId)
      && roundReleaseId(value.roundGenerationId)
      && roundReleaseUnsignedInteger(value.roundNumber, ROUND_RELEASE_MAX_ROUND_NUMBER)
      && value.roundNumber > 0
  } catch {
    return false
  }
}

export function isCompetitionGameBindingRef(value: unknown): value is CompetitionGameBindingRef {
  try {
    return roundReleaseExactRecord(value, [
      'roundRef', 'roundStartMs', 'matchId', 'gameId', 'gameNumber', 'bindingDigest',
    ])
      && isCompetitionRoundRef(value.roundRef)
      && competitionRoundReadyCutoffMs(value.roundStartMs) !== null
      && roundReleaseId(value.matchId)
      && roundReleaseId(value.gameId)
      && roundReleaseUnsignedInteger(value.gameNumber, ROUND_RELEASE_MAX_GAME_NUMBER)
      && value.gameNumber > 0
      && typeof value.bindingDigest === 'string'
      && value.bindingDigest.length === 64
      && /^[0-9a-f]{64}$/.test(value.bindingDigest)
  } catch {
    return false
  }
}

/** expected 必取可信持久 manifest；相等不代表實體房間已持久就緒。 */
export function isCompetitionRoomReadyAck(
  value: unknown,
  expected: CompetitionGameBindingRef,
): value is CompetitionRoomReadyAck {
  try {
    return isCompetitionGameBindingRef(expected)
      && roundReleaseExactRecord(value, ['gameId', 'bindingDigest', 'status'])
      && value.status === 'Waiting'
      && value.gameId === expected.gameId
      && value.bindingDigest === expected.bindingDigest
  } catch {
    return false
  }
}

/** 只驗結構；capability、current manifest 與持久命令授權仍由可信 consumer 驗證。 */
export function isCompetitionGameAdmissionRequestShape(
  value: unknown,
): value is CompetitionGameAdmissionRequest {
  try {
    return roundReleaseAdmissionShape(value)
  } catch {
    return false
  }
}

/** 同一 closed parser 供 release／revoke；expected commandId 不能來自 received payload。 */
export function isCompetitionGameAdmissionRequest(
  value: unknown,
  expected: CompetitionGameAdmissionRequest,
): value is CompetitionGameAdmissionRequest {
  try {
    return roundReleaseAdmissionShape(value)
      && roundReleaseAdmissionShape(expected)
      && value.commandId === expected.commandId
      && roundReleaseBindingRefsEqual(value.bindingRef, expected.bindingRef)
  } catch {
    return false
  }
}

export function isCompetitionGameReleaseResult(value: unknown): value is CompetitionGameReleaseResult {
  return typeof value === 'boolean'
}

/** 辨識三種結果；只有 consumer 判斷 value === 'revoked' 才能建立撤銷 proof。 */
export function isCompetitionGameRevokeResult(value: unknown): value is CompetitionGameRevokeResult {
  return value === 'revoked' || value === 'started' || value === 'invalid'
}

export function isCompetitionGameInitialStartWindow(value: unknown): value is CompetitionGameInitialStartWindow {
  try {
    return (roundReleaseExactRecord(value, ['type']) && value.type === 'preparation')
      || (roundReleaseExactRecord(value, ['type', 'team', 'turn'])
        && value.type === 'action'
        && roundReleaseId(value.team)
        && roundReleaseUnsignedInteger(value.turn, Number.MAX_SAFE_INTEGER))
  } catch {
    return false
  }
}

/** 此 guard 只證結構／父 release 身分／可信 sample 範圍，不證 canonical 事件或任何 start 授權。 */
export function isCompetitionGameInitialStartEvidence(
  value: unknown,
  expected: CompetitionGameReleaseRequest,
): value is CompetitionGameInitialStartEvidence {
  try {
    return roundReleaseAdmissionShape(expected)
      && roundReleaseExactRecord(value, [
        'bindingRef', 'releaseDecisionId', 'gameInstanceId', 'startSequence', 'startedAtMs', 'initialWindow',
      ])
      && isCompetitionGameBindingRef(value.bindingRef)
      && value.bindingRef.gameNumber === 1
      && roundReleaseBindingRefsEqual(value.bindingRef, expected.bindingRef)
      && value.releaseDecisionId === expected.commandId
      && roundReleaseId(value.gameInstanceId)
      && roundReleaseUnsignedInteger(value.startSequence, Number.MAX_SAFE_INTEGER)
      && roundReleaseEpochMs(value.startedAtMs)
      && isCompetitionGameInitialStartWindow(value.initialWindow)
  } catch {
    return false
  }
}

function roundReleaseAdmissionShape(value: unknown): value is CompetitionGameAdmissionRequest {
  return roundReleaseExactRecord(value, ['bindingRef', 'commandId'])
    && isCompetitionGameBindingRef(value.bindingRef)
    && roundReleaseId(value.commandId)
}

function roundReleaseBindingRefsEqual(left: CompetitionGameBindingRef, right: CompetitionGameBindingRef): boolean {
  return left.roundRef.competitionId === right.roundRef.competitionId
    && left.roundRef.stageId === right.roundRef.stageId
    && left.roundRef.roundNumber === right.roundRef.roundNumber
    && left.roundRef.roundGenerationId === right.roundRef.roundGenerationId
    && left.roundStartMs === right.roundStartMs
    && left.matchId === right.matchId
    && left.gameId === right.gameId
    && left.gameNumber === right.gameNumber
    && left.bindingDigest === right.bindingDigest
}

function roundReleaseId(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0 && value.trim() === value
}

function roundReleaseUnsignedInteger(value: unknown, maximum: number): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 && value <= maximum
}

function roundReleaseEpochMs(value: unknown): value is number {
  return roundReleaseUnsignedInteger(value, ROUND_RELEASE_MAX_EPOCH_MS)
}

/** 完整 binding／round-release 共用 strict 資料 shape，不放寬 ready／clock DTO。 */
function roundReleaseExactRecord(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false
  const prototype = Object.getPrototypeOf(value)
  if (prototype !== Object.prototype && prototype !== null) return false
  const actual = Reflect.ownKeys(value)
  return actual.length === keys.length
    && actual.every(key => typeof key === 'string' && keys.includes(key))
    && keys.every(key => {
      const descriptor = Object.getOwnPropertyDescriptor(value, key)
      return descriptor?.enumerable === true && Object.hasOwn(descriptor, 'value')
    })
}
