import type { PlayerId, PublicGameState } from '../app/types/fewfc'
import type { CompetitionRoomMatchOperation } from './competition-room'

export const roomTimeLimits = [30, 60, 120] as const
export type RoomTimeLimit = typeof roomTimeLimits[number]

export function isRoomTimeLimit(value: unknown): value is RoomTimeLimit {
  return roomTimeLimits.some(seconds => value === seconds)
}

export interface RoomOperationClock {
  players: PlayerId[]
  deadlineMs: number
}

export interface StoredRoomOperationClock extends RoomOperationClock {
  kind: 'preparation' | 'choice' | 'action'
  turnNumber: number
}

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

/** 時鐘轉換所需的最小公開狀態，不保留手牌或選項內容。 */
export interface GameTimingState {
  status: PublicGameState['status']
  phase: PublicGameState['phase']
  turnNumber: number
  currentPlayer: PlayerId | null
  initialPouchSelection: PublicGameState['initialPouchSelection']
  pendingChoice: TimingPendingChoice | null
}

type TimingPendingChoice =
  | { visibility: 'visible'; choiceId: number; player: PlayerId }
  | { visibility: 'hidden'; player: PlayerId }

/** 保存 randomness 前的 clock boundary 時，只複製非私密的轉換欄位。 */
export function gameTimingState(state: PublicGameState): GameTimingState {
  const choice = state.pendingChoice
  return {
    status: state.status,
    phase: state.phase,
    turnNumber: state.turnNumber,
    currentPlayer: state.currentPlayer,
    initialPouchSelection: state.initialPouchSelection
      ? { remainingPlayers: [...state.initialPouchSelection.remainingPlayers] }
      : null,
    pendingChoice: choice === null
      ? null
      : choice.visibility === 'visible'
        ? { visibility: 'visible', choiceId: choice.choiceId, player: choice.player }
        : { visibility: 'hidden', player: choice.player },
  }
}

interface TeamClockContext {
  matchId: string
  players: ReadonlyArray<{ player: PlayerId; team: string }>
}

/** 隊伍時計策略從相鄰狀態產生交付指令，實際扣時由隊伍時計處理。 */
export function competitionClockOperationsForTransition(
  binding: TeamClockContext,
  previous: GameTimingState,
  next: GameTimingState,
): CompetitionClockOperation[] {
  const operations: CompetitionClockOperation[] = []
  if (next.status === 'Finished') {
    // 終局也要先結束最後一段玩家視窗，讓權威時計計入終局命令前的耗時。
    operations.push({ type: 'clockEndWindow', matchId: binding.matchId })
    return operations
  }

  const oldWindows = timingWindows(previous)
  const newWindows = timingWindows(next)
  const previousRemaining = oldWindows.preparation ?? []
  const nextRemaining = newWindows.preparation ?? []
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
  const firstActionStarted = next.status === 'InProgress'
    && ((previous.initialPouchSelection !== null && next.initialPouchSelection === null)
      || (previous.status === 'Preparing' && previous.initialPouchSelection === null))
  if (firstActionStarted) {
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

  const oldOutOfTurn = independentClockChoice(binding, oldWindows.choice, previous.currentPlayer)
  const newOutOfTurn = independentClockChoice(binding, newWindows.choice, next.currentPlayer)
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

  if (newWindows.action !== null
    && (previous.currentPlayer !== next.currentPlayer || previous.turnNumber !== next.turnNumber)) {
    operations.push({ type: 'clockEndWindow', matchId: binding.matchId })
    operations.push({
      type: 'clockBeginAction',
      matchId: binding.matchId,
      team: teamForPlayer(binding, newWindows.action),
      turn: next.turnNumber,
    })
  }
  return operations
}

/** 只選取需獨立時計視窗的回合外選擇；同隊隊友仍屬回合外選擇。 */
function independentClockChoice(
  binding: TeamClockContext,
  choice: TimingPendingChoice | null,
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

function requireCurrentPlayer(binding: TeamClockContext, state: GameTimingState): PlayerId {
  if (state.currentPlayer === null) throw new Error('competition action transition has no current player')
  teamForPlayer(binding, state.currentPlayer)
  return state.currentPlayer
}

function teamForPlayer(binding: TeamClockContext, player: PlayerId): string {
  const team = binding.players.find(member => member.player === player)?.team
  if (!team) throw new Error('competition state refers to an unbound player')
  return team
}

function timingWindows(state: GameTimingState) {
  return {
    preparation: state.initialPouchSelection?.remainingPlayers ?? null,
    choice: state.pendingChoice,
    action: state.status === 'InProgress' && state.phase === 'ActiveEffects' ? state.currentPlayer : null,
  }
}

/** 操作限時策略。省略時限即可停用計時，所有期限由伺服器時間推導。 */
export function nextRoomOperationClock(
  limit: RoomTimeLimit | undefined,
  previous: StoredRoomOperationClock | undefined,
  state: PublicGameState | undefined,
  now: number,
  answeredChoice = false,
): StoredRoomOperationClock | undefined {
  if (!limit || !state || state.status === 'Finished' || state.pendingRandomness) return

  const { preparation, choice, action } = timingWindows(state)
  const kind = preparation ? 'preparation' : choice ? 'choice' : 'action'
  const players = preparation ?? (choice ? [choice.player] : action ? [action] : [])
  if (players.length === 0) return

  // 準備期間所有玩家共用起算時間；同一回合內連續的主動效果也沿用期限。
  const sameWindow = previous?.kind === kind && previous.turnNumber === state.turnNumber
    && players.every(player => previous.players.includes(player))
    && !(kind === 'choice' && answeredChoice)
  return {
    kind,
    turnNumber: state.turnNumber,
    players,
    deadlineMs: sameWindow ? previous.deadlineMs : now + limit * 1000,
  }
}
