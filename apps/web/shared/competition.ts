/**
 * Rust competition_web_api 的 camelCase wire contract。
 *
 * 這些型別只描述 server-side bridge；HTTP／登入授權層仍需在外部完成。
 * `serverState` 是可信伺服器 checkpoint，永遠不應進入玩家或觀眾投影。
 */

export type JsonPrimitive = string | number | boolean | null
export type JsonValue = JsonPrimitive | JsonObject | JsonValue[]
export type JsonObject = { [key: string]: JsonValue }

export type CompetitionServerActor =
  | { type: 'organizer'; playerId: string }
  | { type: 'player'; playerId: string }
  | { type: 'referee'; playerId: string }

export type RuleVersion = '5.16' | '5.17'
export type PersonalDeckPolicy = 'disabled' | 'required'
export type RegistrationMode =
  | 'public'
  | { inviteOnly: { invitedCaptains: string[] } }

export interface CompetitionAnnouncement {
  organizer: string
  ruleVersion: RuleVersion
  ruleset: string
  enabledRuleModules: string[]
  teamSize: number
  initialHp: number
  registrationMode: RegistrationMode
  personalDecks: PersonalDeckPolicy
  registrationDeadline: number
  checkInDeadline: number
}

/** 可信伺服器的規則設定純查詢，不接受公告、生命或任何持久化寫入要求。 */
export interface CompetitionRulesSettings {
  ruleVersion: RuleVersion
  ruleset: string
  enabledRuleModules: string[]
  teamSize: number
}

export interface ResolvedCompetitionSettings {
  initialHp: number
}

export type RegistrationAction =
  | { type: 'open' }
  | { type: 'registerTeam'; teamId: string; captain: string; members: string[] }
  | {
      type: 'replaceRoster'
      teamId: string
      expectedRosterVersion: number
      members: string[]
    }
  | { type: 'confirmMembership'; teamId: string; rosterVersion: number }
  | {
      type: 'submitDeck'
      teamId: string
      rosterVersion: number
      name: string
      cards: string[]
    }
  | { type: 'checkInTeam'; teamId: string; rosterVersion: number }
  | { type: 'withdrawTeam'; teamId: string }
  | { type: 'lock' }

export interface RegistrationCheckpointCommand {
  commandId: string
  expectedRevision: number
  serverActor: CompetitionServerActor
  action: RegistrationAction
  trustedTimeMs: number
}

export interface RegistrationCheckpoint {
  announcement: CompetitionAnnouncement
  commands: RegistrationCheckpointCommand[]
}

export interface CompetitionServerState {
  registration: JsonObject | null
  clock: JsonObject | null
  tournament: JsonObject | null
  /** E1 交手檢查點以交手 ID 為鍵；舊檢查點可省略此欄。 */
  matches?: JsonObject
}

export type ClockTimeLimit = number | null
export interface ClockConfig {
  mainTimeMs: ClockTimeLimit
  actionTimeMs: ClockTimeLimit
  outOfTurnChoiceTimeMs: ClockTimeLimit
}

export interface ClockTeamSnapshot {
  team: string
  mainTimeRemainingMs: ClockTimeLimit
  actionTurn: number | null
  actionTimeRemainingMs: ClockTimeLimit
  actionByoyomiStarted: boolean
  enteredByoyomi: boolean
  byoyomiEntrySequence: number | null
}

export type ClockCheckpointWindow =
  | { type: 'preparation'; teams: string[] }
  | { type: 'action'; team: string; turn: number }
  | { type: 'outOfTurnChoice'; team: string; choiceId: string; remainingMs: ClockTimeLimit }

export interface CompetitionClockCheckpoint {
  preparationStarted: boolean
  doubleExpired: boolean
  version: number
  config: ClockConfig
  teams: ClockTeamSnapshot[]
  running: ClockCheckpointWindow | null
  paused: ClockCheckpointWindow | null
  lastObservedMs: number
  nextByoyomiEntrySequence: number
  expiredTeam: string | null
  needsDecision: string | null
}

/** 顯示取樣不是 checkpoint，也不包含虛擬觀測的裁判結果或讀秒排序。 */
export interface ClockDisplayTeam {
  team: string
  mainTimeRemainingMs: ClockTimeLimit
  /** 未開始讀秒時為設定額度；null 仍表示不限時。 */
  byoyomiTimeRemainingMs: ClockTimeLimit
  byoyomiActive: boolean
}

export interface ClockDisplay {
  sampledAtMs: number
  teams: ClockDisplayTeam[]
  running: ClockCheckpointWindow | null
  paused: ClockCheckpointWindow | null
  /** 到期限或既有不可繼續扣時狀態時停止平滑倒數，等待權威服務確認。 */
  awaitingAuthority: boolean
}

/** 僅由驗證後 checkpoint 取得的權威期限，不是顯示取樣或裁判結果。 */
export type ClockCheckpointMetadata = { nextTimeoutDeadlineMs: number | null }

export type GameConclusion = {
  outcome: { type: 'winner'; team: string } | { type: 'draw' }
  causes: GameEndCause[]
}

export type GameTerminalEvidence =
  | { type: 'canonicalGameEnded'; conclusion: GameConclusion }
  | {
      type: 'timeout'
      timeoutId: string
      expiries: { team: string; deadlineMonotonicMs: number }[]
      observedAtMonotonicMs: number
      clockCheckpointDigest: string
    }

export type GameEndCause =
  | { type: 'teamHpDepleted'; teams: string[] }
  | { type: 'directVictory'; rule: string; team: string }

export type GameResultMode = 'elimination' | 'retainDraw'
export type ByoyomiEntryOrder =
  | { type: 'none' }
  | { type: 'first'; team: string }
  | { type: 'simultaneous' }

export type MatchFormat =
  | { type: 'fixedGames'; value: number }
  | { type: 'firstToWins'; value: number }

export type FirstPlayerIntent =
  | { type: 'assigned'; team: string }
  | { type: 'pending'; reason: string }

export type GameStartKind =
  | { type: 'ordinaryNewGame' }
  | { type: 'eliminationDrawContinuation'; continuation: EliminationContinuation }

export interface EliminationContinuation {
  teams: { team: string; mainTimeRemainingMs: number | null }[]
}

export interface CompetitionMatchConfig {
  matchId: string
  teams: [string, string]
  format: MatchFormat
  firstGame: FirstPlayerIntent
  referees: { refereeId: string; conflictedTeam: string | null }[]
}

export type RulingOutcome =
  | { winner: string }
  | 'draw'

export type CompetitionRuling =
  | { type: 'timeoutLoss'; loser: string }
  | { type: 'doubleTimeoutLoss'; teams: [string, string] }
  | { type: 'gameAccepted'; outcome: RulingOutcome }
  | { type: 'eliminationDrawLoss'; loser: string; reason: 'enteredByoyomiFirst' }
  | { type: 'needsTiebreak'; continuation: EliminationContinuation }
  | { type: 'needsDecision'; reason: 'zeroTimeControl' | 'simultaneousByoyomiEntry' | 'unlimitedEliminationDraw' | 'zeroTimeEliminationDraw' }

export interface TrustedGameResult {
  submissionId: string
  gameRecordId: string
  rulingRecordId: string
  startKind: GameStartKind
  firstPlayer: string
  terminalEvidence: GameTerminalEvidence
  ruling: CompetitionRuling
}

export type CompetitionMatchCheckpointEvent =
  | { type: 'assignFirstPlayer'; team: string; reason: string }
  | { type: 'submitGame'; submission: TrustedGameResult }
  | {
      type: 'correctRuling'
      correctionId: string
      refereeId: string
      gameRecordId: string
      replacementRulingRecordId: string
      replacementRuling: CompetitionRuling
      reason: string
    }

export interface CompetitionMatchCheckpoint {
  config: CompetitionMatchConfig
  history: CompetitionMatchCheckpointEvent[]
}

export type CompetitionOperation =
  | ({ type: 'registrationResolveSettings' } & CompetitionRulesSettings)
  | { type: 'registrationCreate'; announcement: CompetitionAnnouncement }
  | {
      type: 'registrationExecute'
      commandId: string
      expectedRevision: number
      action: RegistrationAction
    }
  | { type: 'registrationView' }
  | { type: 'registrationExportLockedRoster' }
  | { type: 'clockCreate'; teams: [string, string]; config: ClockConfig }
  | { type: 'clockBeginPreparation' }
  | { type: 'clockFinishPreparation'; team: string }
  | { type: 'clockBeginAction'; team: string; turn: number }
  | { type: 'clockBeginOutOfTurnChoice'; team: string; choiceId: string }
  | { type: 'clockObserve' }
  /** 純顯示查詢；不提交 clockObserve，也不向公開 HTTP 開放 raw operation。 */
  | { type: 'clockView' }
  /** 純 getter；不進入任何 DO generic mutation allowlist 或公開 raw HTTP。 */
  | { type: 'clockCheckpointMetadata' }
  | { type: 'clockEndWindow' }
  | { type: 'clockPause'; reason: string }
  | { type: 'clockResume'; reason: string }
  | { type: 'clockNewGame'; time: 'reset' | 'carryForward' }
  | {
      type: 'clockRuleGame'
      conclusion: GameConclusion
      mode: GameResultMode
      byoyomiEntryOrder?: ByoyomiEntryOrder
    }
  | { type: 'tournamentCreate'; config: TournamentConfig }
  | { type: 'tournamentPreviewQualifiedStage'; template: QualifiedStageTemplate }
  | { type: 'tournamentCreateQualifiedStage'; template: QualifiedStageTemplate }
  /** 唯讀目前賽程；只能由可信伺服器注入 checkpoint，不是公開 raw HTTP 操作。 */
  | { type: 'tournamentView' }
  | { type: 'tournamentStartMatch'; stageId: string; matchId: string }
  | {
      type: 'tournamentSubmitResult'
      stageId: string
      matchId: string
      result: MatchResult
    }
  | {
      type: 'tournamentCorrectResult'
      stageId: string
      matchId: string
      replacement: MatchResult
      reason: string
    }
  | { type: 'tournamentDropTeam'; stageId: string; teamId: string }
  | { type: 'matchCreate'; config: CompetitionMatchConfig }
  | { type: 'matchView'; matchId: string }
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

export interface CompetitionRequest {
  serverActor: CompetitionServerActor
  trustedTimeMs: number
  operation: CompetitionOperation
  serverState: CompetitionServerState
}

export type CompetitionOperationName = CompetitionOperation['type']

export interface CompetitionEngineError {
  code: string
  layer: string
  message: string
}

export interface CompetitionEngineSuccess {
  ok: true
  operation: CompetitionOperationName
  public: JsonValue
  private?: JsonValue
  serverState: CompetitionServerState
}

export interface CompetitionEngineFailure {
  ok: false
  error: CompetitionEngineError
}

export type CompetitionEngineResponse = CompetitionEngineSuccess | CompetitionEngineFailure

export interface PublicCompetitionSuccess {
  ok: true
  operation: CompetitionOperationName
  public: JsonValue
  private?: JsonValue
}

export type PublicCompetitionResponse = PublicCompetitionSuccess | CompetitionEngineFailure

export type CompetitionProjectionAudience =
  | { type: 'public' }
  | { type: 'authorizedPrivate' }

export type MatchResult =
  | { type: 'doubleLoss' }
  | {
      type: 'win'
      winnerId: string
      loserId: string
      winnerGames: number
      loserGames: number
    }
  | { type: 'draw'; firstGames: number; secondGames: number }
  | { type: 'bye'; teamId: string }
  | { type: 'forfeit'; winnerId: string; loserId: string }

export type TournamentFormat =
  | 'singleElimination'
  | 'roundRobin'
  | 'doubleElimination'
  | 'swiss'

export interface SeededTeam {
  teamId: string
  seed: number
}

export interface MatchPoints {
  win: number
  draw: number
  loss: number
  bye: number
}

export interface DrawCredit {
  numerator: number
  denominator: number
}

export type SwissTieBreaker =
  | 'matchPoints'
  | 'headToHeadMatchPoints'
  | 'opponentWinRate'
  | 'netGameWins'
  | 'seedOrder'

export interface SwissStandingPolicy {
  matchPoints: MatchPoints
  drawCredit: DrawCredit
  opponentWinRateFloorPercent: number | null
  opponentWinRateCapPercent: number | null
  excludeByesFromOpponentWinRate: boolean
  tieBreakers: SwissTieBreaker[]
}

export type StageQualificationSource =
  | { type: 'finishedStageWinner' }
  | { type: 'uniqueStandingRank'; rank: number }

export interface StageQualificationSlot {
  sourceStageId: string
  source: StageQualificationSource
  destinationSeed: number
}

/** 只傳下一階段設定與來源名額；已解析隊伍由 Tournament 核心提供。 */
export interface QualifiedStageTemplate {
  stageId: string
  format: TournamentFormat
  randomSeed: number
  matchFormat: MatchFormat
  clockConfig: ClockConfig
  swissRounds: number | null
  swissPolicy: SwissStandingPolicy | null
  sourceSlots: StageQualificationSlot[]
}

export interface StageQualificationMapping {
  sourceStageId: string
  source: StageQualificationSource
  teamId: string
  destinationSeed: number
}

export interface ResolvedStageQualification {
  seededTeams: SeededTeam[]
  sourceMappings: StageQualificationMapping[]
}

export interface QualifiedStagePreviewProjection {
  qualification: ResolvedStageQualification
  /** Rust TournamentStage 的原始快照；不把內部 teams／doubleState 縮窄解析。 */
  plannedStage: JsonObject
}

export interface QualifiedStageCreateProjection {
  qualification: ResolvedStageQualification
  /** Rust TournamentSnapshot 的原始快照；不在 TypeScript 重建賽程。 */
  currentTournament: JsonObject
}

export interface StageConfig {
  stageId: string
  format: TournamentFormat
  teams: SeededTeam[]
  randomSeed: number
  matchFormat: MatchFormat
  clockConfig: ClockConfig
  swissRounds: number | null
  swissPolicy: SwissStandingPolicy | null
}

export interface TournamentConfig {
  tournamentId: string
  stage: StageConfig
}

export type RoundKind = 'main' | 'winners' | 'losers' | 'grandFinal' | 'grandFinalReset'
export type MatchStatus = 'pending' | 'inProgress' | 'completed' | 'cancelled'

export interface ScheduledMatch {
  matchId: string
  stageId: string
  roundNumber: number
  sequence: number
  kind: RoundKind
  participants: string[]
  status: MatchStatus
  result: MatchResult | null
  cancellationReason: string | null
  started: boolean
}

export interface TournamentRound {
  roundNumber: number
  kind: RoundKind
  matches: ScheduledMatch[]
}

export interface Standing {
  teamId: string
  matchPoints: number
  wins: number
  draws: number
  losses: number
  gameWins: number
  gameLosses: number
  netGameWins: number
  opponentWinRateBasisPoints: number | null
  dropped: boolean
  rank: number | null
}

export interface TournamentStage {
  config: StageConfig
  status: 'active' | 'finished'
  rounds: TournamentRound[]
  standings: Standing[]
  corrections: JsonValue[]
  lastScheduleError: string | null
  scheduleGeneration: number
  supersededRounds: TournamentRound[]
}

export interface TournamentSnapshot {
  tournamentId: string
  stages: TournamentStage[]
}

export function isJsonValue(value: unknown): value is JsonValue {
  if (value === null || typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean') {
    return true
  }
  if (Array.isArray(value)) return value.every(isJsonValue)
  if (typeof value !== 'object') return false
  return Object.values(value).every(isJsonValue)
}

export function isJsonObject(value: unknown): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    && Object.values(value).every(isJsonValue)
}

/** 新可信設定邊界只收完整 JSON 資料；不觸發 getter 或忽略 Symbol／隱藏欄位。 */
export function isExactCompetitionJson(value: unknown): value is JsonValue {
  const ancestors = new Set<object>()
  const inspect = (input: unknown): boolean => {
    if (input === null || typeof input === 'string' || typeof input === 'boolean') return true
    if (typeof input === 'number') return Number.isFinite(input)
    if (typeof input !== 'object' || ancestors.has(input)) return false
    const array = Array.isArray(input)
    if (!array && Object.getPrototypeOf(input) !== Object.prototype
      && Object.getPrototypeOf(input) !== null) return false
    ancestors.add(input)
    const keys = Reflect.ownKeys(input)
    if (array && (keys.length !== input.length + 1
      || !keys.includes('length'))) return false
    for (const key of keys) {
      const field = Object.getOwnPropertyDescriptor(input, key)
      if (typeof key !== 'string' || !field || !Object.hasOwn(field, 'value')) return false
      if (array && key === 'length') continue
      if (!field.enumerable || (array && (!/^(0|[1-9][0-9]*)$/.test(key)
        || Number(key) >= input.length)) || !inspect(field.value)) return false
    }
    ancestors.delete(input)
    return true
  }
  try {
    return inspect(value)
  } catch {
    return false
  }
}

export function emptyCompetitionServerState(): CompetitionServerState {
  return { registration: null, clock: null, tournament: null, matches: {} }
}
