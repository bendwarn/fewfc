import type {
  CompetitionMatchConfig, CompetitionRuling, EliminationContinuation,
  FirstPlayerIntent, GameConclusion, GameEndCause, GameStartKind, GameTerminalEvidence,
} from '../../shared/competition'

export interface CompetitionCorrectionScope {
  competitionId: string
  matchId: string
  sessionUserId: string
}

export interface CompetitionCorrectionCommand {
  commandId: string
  expectedMatchRevision: number
  correctionId: string
  gameRecordId: string
  replacementRulingRecordId: string
  replacementRuling: CompetitionRuling
  reason: string
}

interface GameRecord {
  gameNumber: number
  gameRecordId: string
  startKind: GameStartKind
  firstPlayer: string
  terminalEvidence: GameTerminalEvidence
  rulingRecordId: string
}
interface RulingRecord { rulingRecordId: string; gameRecordId: string; ruling: CompetitionRuling }
interface CorrectionRecord {
  correctionId: string; refereeId: string; gameRecordId: string; previousRulingRecordId: string
  replacementRulingRecordId: string; replacementRuling: CompetitionRuling; reason: string
}
type MatchOutcome =
  | { type: 'winner'; winnerId: string; loserId: string; score: [number, number] }
  | { type: 'draw'; score: [number, number] }
  | { type: 'doubleLoss'; teams: [string, string]; progression: { type: 'stageExit' } | { type: 'pendingReferee'; reason: string } }
  | { type: 'pendingReferee'; gameRecordId: string; reason: Extract<CompetitionRuling, { type: 'needsDecision' }>['reason'] }
interface MatchView {
  config: CompetitionMatchConfig
  status: 'awaitingFirstPlayer' | 'open' | 'awaitingReferee' | 'completed'
  score: [number, number]
  games: GameRecord[]
  rulings: RulingRecord[]
  corrections: CorrectionRecord[]
  outcome: MatchOutcome | null
  nextFirstPlayer: string | null
  nextGame: { gameNumber: number; kind: GameStartKind; firstPlayer: FirstPlayerIntent } | null
}
export interface CompetitionCorrectionMatch {
  competitionId: string
  matchId: string
  matchRevision: number
  match: MatchView | null
}
export interface CompetitionCorrectionCandidate { key: string; label: string; ruling: CompetitionRuling }

export class CompetitionCorrectionProtocolError extends Error {
  constructor() { super('交手回應無法安全驗證，請重新查詢。') }
}
function reject(): never { throw new CompetitionCorrectionProtocolError() }

function exact(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false
  const prototype = Object.getPrototypeOf(value)
  return (prototype === Object.prototype || prototype === null)
    && Reflect.ownKeys(value).length === keys.length
    && keys.every(key => {
      const descriptor = Object.getOwnPropertyDescriptor(value, key)
      return Boolean(descriptor?.enumerable && Object.hasOwn(descriptor, 'value'))
    })
}
function array(value: unknown): value is unknown[] {
  return Array.isArray(value) && Object.getPrototypeOf(value) === Array.prototype
    && Reflect.ownKeys(value).length === value.length + 1
    && Array.from({ length: value.length }, (_, index) => {
      const descriptor = Object.getOwnPropertyDescriptor(value, String(index))
      return Boolean(descriptor?.enumerable && Object.hasOwn(descriptor, 'value'))
    }).every(Boolean)
}
function id(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0 && value === value.trim()
}
function text(value: unknown): value is string { return typeof value === 'string' && value.trim().length > 0 }
function integer(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
}
function u16(value: unknown): value is number { return integer(value) && value <= 65535 }
function team(value: unknown, teams: readonly string[]): value is string { return id(value) && teams.includes(value) }
function pair(value: unknown, teams?: readonly string[]): [string, string] {
  if (!array(value) || value.length !== 2) reject()
  const [first, second] = value
  if (!id(first) || !id(second) || first === second || (teams && (!teams.includes(first) || !teams.includes(second)))) reject()
  return [first, second]
}
function score(value: unknown): [number, number] {
  if (!array(value) || value.length !== 2) reject()
  const [first, second] = value
  if (!u16(first) || !u16(second)) reject()
  return [first, second]
}
function unique(values: readonly string[]): boolean { return new Set(values).size === values.length }

export function createCompetitionCorrectionScope(competitionId: unknown, matchId: unknown, sessionUserId: unknown): CompetitionCorrectionScope | null {
  if (!text(competitionId) || !text(matchId) || !text(sessionUserId)) return null
  return { competitionId: competitionId.trim(), matchId: matchId.trim(), sessionUserId: sessionUserId.trim() }
}
export function competitionCorrectionPath(scope: CompetitionCorrectionScope): string {
  return `/api/competitions/${encodeURIComponent(scope.competitionId)}/matches/${encodeURIComponent(scope.matchId)}`
}

function firstPlayer(value: unknown, teams: readonly string[]): FirstPlayerIntent {
  if (exact(value, ['type', 'team']) && value.type === 'assigned' && team(value.team, teams)) return { type: 'assigned', team: value.team }
  if (exact(value, ['type', 'reason']) && value.type === 'pending' && text(value.reason)) return { type: 'pending', reason: value.reason }
  return reject()
}
function continuation(value: unknown, teams: readonly string[]): EliminationContinuation {
  if (!exact(value, ['teams']) || !array(value.teams) || value.teams.length !== 2) reject()
  const parsed = value.teams.map(item => {
    if (!exact(item, ['team', 'mainTimeRemainingMs']) || !team(item.team, teams)
      || !(item.mainTimeRemainingMs === null || integer(item.mainTimeRemainingMs))) reject()
    return { team: item.team, mainTimeRemainingMs: item.mainTimeRemainingMs }
  })
  if (!unique(parsed.map(item => item.team))) reject()
  return { teams: parsed }
}
function startKind(value: unknown, teams: readonly string[]): GameStartKind {
  if (exact(value, ['type']) && value.type === 'ordinaryNewGame') return { type: 'ordinaryNewGame' }
  if (exact(value, ['type', 'continuation']) && value.type === 'eliminationDrawContinuation') {
    return { type: 'eliminationDrawContinuation', continuation: continuation(value.continuation, teams) }
  }
  return reject()
}
function decisionReason(value: unknown): Extract<CompetitionRuling, { type: 'needsDecision' }>['reason'] {
  if (value === 'zeroTimeControl' || value === 'simultaneousByoyomiEntry'
    || value === 'unlimitedEliminationDraw' || value === 'zeroTimeEliminationDraw') return value
  return reject()
}
function ruling(value: unknown, teams: readonly string[]): CompetitionRuling {
  if (exact(value, ['type', 'loser']) && value.type === 'timeoutLoss' && team(value.loser, teams)) return { type: 'timeoutLoss', loser: value.loser }
  if (exact(value, ['type', 'teams']) && value.type === 'doubleTimeoutLoss') return { type: 'doubleTimeoutLoss', teams: pair(value.teams, teams) }
  if (exact(value, ['type', 'outcome']) && value.type === 'gameAccepted') {
    if (value.outcome === 'draw') return { type: 'gameAccepted', outcome: 'draw' }
    if (exact(value.outcome, ['winner']) && team(value.outcome.winner, teams)) return { type: 'gameAccepted', outcome: { winner: value.outcome.winner } }
  }
  if (exact(value, ['type', 'loser', 'reason']) && value.type === 'eliminationDrawLoss'
    && team(value.loser, teams) && value.reason === 'enteredByoyomiFirst') {
    return { type: 'eliminationDrawLoss', loser: value.loser, reason: 'enteredByoyomiFirst' }
  }
  if (exact(value, ['type', 'continuation']) && value.type === 'needsTiebreak') return { type: 'needsTiebreak', continuation: continuation(value.continuation, teams) }
  if (exact(value, ['type', 'reason']) && value.type === 'needsDecision') return { type: 'needsDecision', reason: decisionReason(value.reason) }
  return reject()
}
function conclusion(value: unknown, teams: readonly string[]): GameConclusion {
  if (!exact(value, ['outcome', 'causes']) || !array(value.causes) || value.causes.length === 0) reject()
  let outcome: GameConclusion['outcome']
  if (exact(value.outcome, ['type']) && value.outcome.type === 'draw') outcome = { type: 'draw' }
  else if (exact(value.outcome, ['type', 'team']) && value.outcome.type === 'winner' && team(value.outcome.team, teams)) outcome = { type: 'winner', team: value.outcome.team }
  else return reject()
  const causes: GameEndCause[] = value.causes.map(cause => {
    if (exact(cause, ['type', 'teams']) && cause.type === 'teamHpDepleted' && array(cause.teams)
      && cause.teams.length >= 1 && cause.teams.length <= 2) {
      const affected = cause.teams.map(item => { if (!team(item, teams)) reject(); return item })
      if (!unique(affected)) reject()
      return { type: 'teamHpDepleted', teams: affected }
    }
    if (exact(cause, ['type', 'rule', 'team']) && cause.type === 'directVictory' && text(cause.rule) && team(cause.team, teams)) {
      return { type: 'directVictory', rule: cause.rule, team: cause.team }
    }
    return reject()
  })
  return { outcome, causes }
}
function evidence(value: unknown, teams: readonly string[]): GameTerminalEvidence {
  if (exact(value, ['type', 'conclusion']) && value.type === 'canonicalGameEnded') {
    return { type: 'canonicalGameEnded', conclusion: conclusion(value.conclusion, teams) }
  }
  if (exact(value, ['type', 'timeoutId', 'expiries', 'observedAtMonotonicMs', 'clockCheckpointDigest']) && value.type === 'timeout'
    && id(value.timeoutId) && id(value.clockCheckpointDigest) && integer(value.observedAtMonotonicMs)
    && array(value.expiries) && value.expiries.length >= 1 && value.expiries.length <= 2) {
    const observedAtMonotonicMs = value.observedAtMonotonicMs
    const expiries = value.expiries.map(expiry => {
      if (!exact(expiry, ['team', 'deadlineMonotonicMs']) || !team(expiry.team, teams)
        || !integer(expiry.deadlineMonotonicMs) || expiry.deadlineMonotonicMs > observedAtMonotonicMs) reject()
      return { team: expiry.team, deadlineMonotonicMs: expiry.deadlineMonotonicMs }
    })
    if (!unique(expiries.map(item => item.team)) || (expiries.length === 2
      && expiries[0]?.deadlineMonotonicMs !== expiries[1]?.deadlineMonotonicMs)) reject()
    return { type: 'timeout', timeoutId: value.timeoutId, expiries, observedAtMonotonicMs, clockCheckpointDigest: value.clockCheckpointDigest }
  }
  return reject()
}
function config(value: unknown, matchId: string): CompetitionMatchConfig {
  if (!exact(value, ['matchId', 'teams', 'format', 'firstGame', 'referees']) || value.matchId !== matchId
    || !exact(value.format, ['type', 'value']) || !u16(value.format.value) || value.format.value < 1
    || !(value.format.type === 'fixedGames' || value.format.type === 'firstToWins') || !array(value.referees)) reject()
  const teams = pair(value.teams)
  const referees = value.referees.map(item => {
    if (!exact(item, ['refereeId', 'conflictedTeam']) || !id(item.refereeId)
      || !(item.conflictedTeam === null || team(item.conflictedTeam, teams))) reject()
    return { refereeId: item.refereeId, conflictedTeam: item.conflictedTeam }
  })
  if (!unique(referees.map(item => item.refereeId))) reject()
  return { matchId, teams, format: { type: value.format.type, value: value.format.value }, firstGame: firstPlayer(value.firstGame, teams), referees }
}
function outcome(value: unknown, teams: readonly string[]): MatchOutcome | null {
  if (value === null) return null
  if (exact(value, ['type', 'winnerId', 'loserId', 'score']) && value.type === 'winner'
    && team(value.winnerId, teams) && team(value.loserId, teams) && value.winnerId !== value.loserId) {
    return { type: 'winner', winnerId: value.winnerId, loserId: value.loserId, score: score(value.score) }
  }
  if (exact(value, ['type', 'score']) && value.type === 'draw') return { type: 'draw', score: score(value.score) }
  if (exact(value, ['type', 'teams', 'progression']) && value.type === 'doubleLoss'
    && exact(value.progression, ['type']) && value.progression.type === 'stageExit') {
    return { type: 'doubleLoss', teams: pair(value.teams, teams), progression: { type: 'stageExit' } }
  }
  if (exact(value, ['type', 'teams', 'progression']) && value.type === 'doubleLoss'
    && exact(value.progression, ['type', 'reason']) && value.progression.type === 'pendingReferee' && text(value.progression.reason)) {
    return { type: 'doubleLoss', teams: pair(value.teams, teams), progression: { type: 'pendingReferee', reason: value.progression.reason } }
  }
  if (exact(value, ['type', 'gameRecordId', 'reason']) && value.type === 'pendingReferee' && id(value.gameRecordId)) {
    return { type: 'pendingReferee', gameRecordId: value.gameRecordId, reason: decisionReason(value.reason) }
  }
  return reject()
}
function parseMatch(value: unknown, matchId: string): MatchView {
  if (!exact(value, ['config', 'status', 'score', 'games', 'rulings', 'corrections', 'outcome', 'nextFirstPlayer', 'nextGame'])
    || !(value.status === 'awaitingFirstPlayer' || value.status === 'open' || value.status === 'awaitingReferee' || value.status === 'completed')
    || !array(value.games) || !array(value.rulings) || !array(value.corrections)) reject()
  const parsedConfig = config(value.config, matchId)
  const teams = parsedConfig.teams
  const games: GameRecord[] = value.games.map(item => {
    if (!exact(item, ['gameNumber', 'gameRecordId', 'startKind', 'firstPlayer', 'terminalEvidence', 'rulingRecordId'])
      || !u16(item.gameNumber) || item.gameNumber < 1 || !id(item.gameRecordId) || !id(item.rulingRecordId) || !team(item.firstPlayer, teams)) reject()
    return { gameNumber: item.gameNumber, gameRecordId: item.gameRecordId, startKind: startKind(item.startKind, teams),
      firstPlayer: item.firstPlayer, terminalEvidence: evidence(item.terminalEvidence, teams), rulingRecordId: item.rulingRecordId }
  })
  const rulings: RulingRecord[] = value.rulings.map(item => {
    if (!exact(item, ['rulingRecordId', 'gameRecordId', 'ruling']) || !id(item.rulingRecordId) || !id(item.gameRecordId)) reject()
    return { rulingRecordId: item.rulingRecordId, gameRecordId: item.gameRecordId, ruling: ruling(item.ruling, teams) }
  })
  const corrections: CorrectionRecord[] = value.corrections.map(item => {
    if (!exact(item, ['correctionId', 'refereeId', 'gameRecordId', 'previousRulingRecordId', 'replacementRulingRecordId', 'replacementRuling', 'reason'])
      || !id(item.correctionId) || !id(item.refereeId) || !id(item.gameRecordId) || !id(item.previousRulingRecordId)
      || !id(item.replacementRulingRecordId) || !text(item.reason)) reject()
    return { correctionId: item.correctionId, refereeId: item.refereeId, gameRecordId: item.gameRecordId,
      previousRulingRecordId: item.previousRulingRecordId, replacementRulingRecordId: item.replacementRulingRecordId,
      replacementRuling: ruling(item.replacementRuling, teams), reason: item.reason }
  })
  const parsedScore = score(value.score)
  const parsedOutcome = outcome(value.outcome, teams)
  const nextFirstPlayer = value.nextFirstPlayer
  if (!(nextFirstPlayer === null || team(nextFirstPlayer, teams))) reject()
  let nextGame: MatchView['nextGame'] = null
  if (value.nextGame !== null) {
    if (!exact(value.nextGame, ['gameNumber', 'kind', 'firstPlayer']) || !u16(value.nextGame.gameNumber) || value.nextGame.gameNumber < 1) reject()
    nextGame = { gameNumber: value.nextGame.gameNumber, kind: startKind(value.nextGame.kind, teams), firstPlayer: firstPlayer(value.nextGame.firstPlayer, teams) }
    if ((nextGame.firstPlayer.type === 'assigned' ? nextGame.firstPlayer.team : null) !== nextFirstPlayer) reject()
  }
  // 驗證 DTO 的引用關係，不重播交手、不重算 score／勝者或下游賽程。
  if (!unique(games.map(item => item.gameRecordId)) || !unique(games.map(item => String(item.gameNumber)))
    || games.some((game, index) => game.gameNumber !== index + 1)
    || !unique(rulings.map(item => item.rulingRecordId)) || !unique(corrections.map(item => item.correctionId))) reject()
  const byRuling = new Map(rulings.map(item => [item.rulingRecordId, item]))
  const byGame = new Map(games.map(item => [item.gameRecordId, item]))
  for (const item of rulings) {
    const game = byGame.get(item.gameRecordId)
    if (!game) reject()
    // 只驗已保存終止證據與判定的配對，不重算耗時或勝者。
    const terminal = game.terminalEvidence
    const savedRuling = item.ruling
    if (terminal.type === 'timeout') {
      if (savedRuling.type === 'timeoutLoss') {
        if (terminal.expiries.length !== 1 || terminal.expiries[0]?.team !== savedRuling.loser) reject()
      } else if (savedRuling.type === 'doubleTimeoutLoss') {
        const lostTeams = savedRuling.teams
        if (terminal.expiries.length !== 2 || terminal.expiries.some(expiry => !lostTeams.includes(expiry.team))) reject()
      } else reject()
    } else if (savedRuling.type === 'timeoutLoss' || savedRuling.type === 'doubleTimeoutLoss') reject()
    // 已保存的 gameAccepted 也不能冒充另一個 canonical 結論；不是勝者選擇算法。
    if (item.ruling.type === 'gameAccepted') {
      if (game.terminalEvidence.type !== 'canonicalGameEnded') reject()
      const original = game.terminalEvidence.conclusion.outcome
      const accepted = item.ruling.outcome
      if (original.type === 'winner') {
        if (accepted === 'draw' || accepted.winner !== original.team) reject()
      } else if (accepted !== 'draw' || parsedConfig.format.type !== 'fixedGames') reject()
    }
  }
  for (const game of games) if (byRuling.get(game.rulingRecordId)?.gameRecordId !== game.gameRecordId) reject()
  const currentRuling = new Map<string, string>()
  const consumed = new Set<string>()
  for (const item of rulings) {
    if (!currentRuling.has(item.gameRecordId)) {
      currentRuling.set(item.gameRecordId, item.rulingRecordId)
      consumed.add(item.rulingRecordId)
    }
  }
  for (const item of corrections) {
    const previous = byRuling.get(item.previousRulingRecordId)
    const replacement = byRuling.get(item.replacementRulingRecordId)
    if (!previous || !replacement || !byGame.has(item.gameRecordId) || previous.gameRecordId !== item.gameRecordId || replacement.gameRecordId !== item.gameRecordId
      || currentRuling.get(item.gameRecordId) !== item.previousRulingRecordId || consumed.has(item.replacementRulingRecordId)
      || !parsedConfig.referees.some(referee => referee.refereeId === item.refereeId)
      || JSON.stringify(replacement.ruling) !== JSON.stringify(item.replacementRuling)) reject()
    consumed.add(item.replacementRulingRecordId)
    currentRuling.set(item.gameRecordId, item.replacementRulingRecordId)
  }
  if (consumed.size !== rulings.length || games.some(game => currentRuling.get(game.gameRecordId) !== game.rulingRecordId)) reject()
  if (parsedOutcome?.type === 'pendingReferee' && !byGame.has(parsedOutcome.gameRecordId)) reject()
  if (parsedOutcome?.type === 'winner' || parsedOutcome?.type === 'draw') {
    if (value.status !== 'completed' || parsedOutcome.score[0] !== parsedScore[0] || parsedOutcome.score[1] !== parsedScore[1]) reject()
  } else if (parsedOutcome?.type === 'doubleLoss') {
    if (parsedOutcome.progression.type === 'stageExit') {
      if (value.status !== 'completed') reject()
    } else if (value.status !== 'awaitingReferee') reject()
  } else if (value.status === 'completed') reject()
  if (parsedOutcome?.type === 'pendingReferee' && value.status !== 'awaitingReferee') reject()
  if ((value.status === 'completed' || value.status === 'awaitingReferee') && nextGame !== null) reject()
  if (value.status === 'completed' && nextFirstPlayer !== null) reject()
  if (value.status === 'awaitingReferee'
    && !(parsedOutcome?.type === 'pendingReferee'
      || (parsedOutcome?.type === 'doubleLoss' && parsedOutcome.progression.type === 'pendingReferee'))) reject()
  if (value.status === 'open' || value.status === 'awaitingFirstPlayer') {
    if (parsedOutcome !== null || nextGame === null || nextGame.gameNumber !== games.length + 1
      || (value.status === 'open') !== (nextGame.firstPlayer.type === 'assigned')) reject()
  }
  return { config: parsedConfig, status: value.status, score: parsedScore, games, rulings, corrections, outcome: parsedOutcome, nextFirstPlayer, nextGame }
}

const matchOperations = ['matchCreate', 'matchView', 'matchAssignFirstPlayer', 'matchSubmitGame', 'matchCorrectRuling']
const clockOperations = ['clockCreate', 'clockBeginPreparation', 'clockFinishPreparation', 'clockBeginAction', 'clockBeginOutOfTurnChoice',
  'clockObserve', 'clockEndWindow', 'clockPause', 'clockResume', 'clockNewGame', 'clockRuleGame', 'clockView']
// clock public 僅驗 JSON 容器後丟棄，既不保留隱藏資料，也不拿它補歷史判定事實。
function safeJson(value: unknown, seen = new Set<object>()): boolean {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return true
  if (typeof value === 'number') return Number.isFinite(value)
  if (typeof value !== 'object' || seen.has(value)) return false
  seen.add(value)
  let valid = false
  if (array(value)) valid = value.every(child => safeJson(child, seen))
  else if (value !== null && exact(value, Object.keys(value))) {
    const forbidden = ['private', 'authorizedPrivate', 'serverState', 'history', 'callerUserId', 'deck', 'decks', 'deckSnapshot', 'seed']
    valid = Object.entries(value).every(([key, child]) => !forbidden.includes(key) && safeJson(child, seen))
  }
  seen.delete(value)
  return valid
}
export function parseCompetitionCorrectionMatch(input: unknown, scope: CompetitionCorrectionScope): CompetitionCorrectionMatch {
  try {
    if (!exact(input, ['competitionId', 'matchId', 'matchRevision', 'matchProjection', 'clockProjection'])
      || input.competitionId !== scope.competitionId || input.matchId !== scope.matchId || !integer(input.matchRevision)) reject()
    const clock = input.clockProjection
    if (clock !== null && (!exact(clock, ['ok', 'operation', 'public']) || clock.ok !== true
      || typeof clock.operation !== 'string' || !clockOperations.includes(clock.operation) || !safeJson(clock.public))) reject()
    const projection = input.matchProjection
    let match: MatchView | null = null
    if (projection !== null) {
      if (!exact(projection, ['ok', 'operation', 'public']) || projection.ok !== true || typeof projection.operation !== 'string'
        || !matchOperations.includes(projection.operation) || !exact(projection.public, ['match'])) reject()
      match = parseMatch(projection.public.match, scope.matchId)
    }
    return { competitionId: scope.competitionId, matchId: scope.matchId, matchRevision: input.matchRevision, match }
  } catch { return reject() }
}

export function isAssignedCompetitionReferee(view: CompetitionCorrectionMatch | null, sessionUserId: string | null): boolean {
  return Boolean(sessionUserId && view?.match?.config.referees.some(item => item.refereeId === sessionUserId && item.conflictedTeam === null))
}
export function competitionCorrectionTeamLabel(teams: readonly string[], value: string): string {
  if (value === teams[0]) return '甲隊'
  if (value === teams[1]) return '乙隊'
  return '未能確認隊伍'
}
export function competitionRulingLabel(value: CompetitionRuling, teams: readonly string[]): string {
  switch (value.type) {
    case 'timeoutLoss': return `${competitionCorrectionTeamLabel(teams, value.loser)}超時判負`
    case 'doubleTimeoutLoss': return '雙方超時判負，後續待裁判處理'
    case 'gameAccepted': return value.outcome === 'draw' ? '採納原始結論，和局' : `採納原始結論，${competitionCorrectionTeamLabel(teams, value.outcome.winner)}獲勝`
    case 'eliminationDrawLoss': return `${competitionCorrectionTeamLabel(teams, value.loser)}依先進讀秒的和局政策判負`
    case 'needsTiebreak': return '和局需加賽，沿用已保存的主時間'
    case 'needsDecision': return {
      zeroTimeControl: '零時間控制待裁判裁定', simultaneousByoyomiEntry: '同時進入讀秒，待裁判裁定',
      unlimitedEliminationDraw: '不限時淘汰和局，待裁判裁定', zeroTimeEliminationDraw: '零時間淘汰和局，待裁判裁定',
    }[value.reason]
  }
}
export function competitionCorrectionStatusLabel(view: CompetitionCorrectionMatch): string {
  if (!view.match) return '目前沒有可顯示的交手紀錄'
  return { awaitingFirstPlayer: '等待確認先手', open: '交手進行中', awaitingReferee: '等待裁判處理', completed: '交手已完成' }[view.match.status]
}
export function competitionTerminalEvidenceLabel(value: GameTerminalEvidence, teams: readonly string[]): string {
  if (value.type === 'timeout') return `權威超時終止證據：${value.expiries.map(item => competitionCorrectionTeamLabel(teams, item.team)).join('、')}`
  return value.conclusion.outcome.type === 'draw' ? '原始遊戲結論：和局' : `原始遊戲結論：${competitionCorrectionTeamLabel(teams, value.conclusion.outcome.team)}獲勝`
}
export function competitionCorrectionCandidates(view: CompetitionCorrectionMatch, gameRecordId: string): CompetitionCorrectionCandidate[] {
  const match = view.match
  const game = match?.games.find(item => item.gameRecordId === gameRecordId)
  if (!match || !game) return []
  const candidates = match.rulings.filter(item => item.gameRecordId === gameRecordId).map((item, index) => ({
    key: `saved:${item.rulingRecordId}`, label: `已保存判定 ${index + 1}：${competitionRulingLabel(item.ruling, match.config.teams)}`,
    ruling: ruling(item.ruling, match.config.teams),
  }))
  if (game.terminalEvidence.type === 'canonicalGameEnded') {
    const original = game.terminalEvidence.conclusion.outcome
    let accepted: CompetitionRuling | null = null
    if (original.type === 'winner') accepted = { type: 'gameAccepted', outcome: { winner: original.team } }
    else if (match.config.format.type === 'fixedGames') accepted = { type: 'gameAccepted', outcome: 'draw' }
    if (accepted) candidates.push({ key: 'canonical', label: `採納原始遊戲結論：${competitionRulingLabel(accepted, match.config.teams)}`, ruling: accepted })
  }
  return candidates
}
function freezeTree(value: unknown): void {
  if (typeof value !== 'object' || value === null) return
  for (const child of Object.values(value)) freezeTree(child)
  Object.freeze(value)
}
/** 三個 ID 只於送出建立一次；草稿、view 與後續 GET 不共享可修改的巢狀物件。 */
export function buildCompetitionRulingCorrection(view: CompetitionCorrectionMatch, scope: CompetitionCorrectionScope,
  gameRecordId: string, candidateKey: string, reason: string, uuid: () => string = () => crypto.randomUUID()): Readonly<CompetitionCorrectionCommand> {
  if (view.competitionId !== scope.competitionId || view.matchId !== scope.matchId || !isAssignedCompetitionReferee(view, scope.sessionUserId)
    || !integer(view.matchRevision) || view.matchRevision >= Number.MAX_SAFE_INTEGER || !text(reason)) reject()
  const candidate = competitionCorrectionCandidates(view, gameRecordId).find(item => item.key === candidateKey)
  if (!candidate) reject()
  const commandId = uuid(), correctionId = uuid(), replacementRulingRecordId = uuid()
  if (!id(commandId) || !id(correctionId) || !id(replacementRulingRecordId) || !unique([commandId, correctionId, replacementRulingRecordId])) reject()
  const body = { commandId, expectedMatchRevision: view.matchRevision, correctionId, gameRecordId, replacementRulingRecordId,
    replacementRuling: candidate.ruling, reason: reason.trim() }
  freezeTree(body)
  return body
}
export function parseCompetitionCorrectionAck(input: unknown, scope: CompetitionCorrectionScope, command: Readonly<CompetitionCorrectionCommand>): { matchRevision: number; idempotent: boolean } {
  try {
    if (!exact(input, ['competitionId', 'matchId', 'matchRevision', 'commandId', 'idempotent'])
      || input.competitionId !== scope.competitionId || input.matchId !== scope.matchId || input.commandId !== command.commandId
      || !integer(input.matchRevision) || input.matchRevision !== command.expectedMatchRevision + 1 || typeof input.idempotent !== 'boolean') reject()
    return { matchRevision: input.matchRevision, idempotent: input.idempotent }
  } catch { return reject() }
}
export function competitionCorrectionErrorStatus(error: unknown): number | undefined {
  try {
    if (typeof error !== 'object' || error === null) return undefined
    const value = 'statusCode' in error ? error.statusCode : 'status' in error ? error.status : undefined
    return integer(value) ? value : undefined
  } catch { return undefined }
}
function protocolError(error: unknown): boolean {
  try { return error instanceof CompetitionCorrectionProtocolError } catch { return false }
}
/** 一旦結果未知，GET／4xx 永遠不能代替 matching receipt ACK。 */
export function keepCompetitionCorrectionIntent(wasUnknown: boolean, error: unknown): boolean {
  if (wasUnknown || protocolError(error)) return true
  const status = competitionCorrectionErrorStatus(error)
  return ![400, 401, 403, 404, 409, 422].some(value => value === status)
}
export function competitionCorrectionErrorMessage(error: unknown, phase: 'read' | 'mutation' = 'mutation'): string {
  if (phase === 'read') {
    if (protocolError(error)) return '交手回應無法安全驗證，未顯示投影；請重新查詢。'
    switch (competitionCorrectionErrorStatus(error)) {
      case 400: return '交手查詢資料不正確，請核對後再試。'
      case 401: return '請重新登入後查詢交手紀錄。'
      case 403: return '你目前沒有查詢交手紀錄的權限，請確認登入狀態。'
      case 404: return '找不到這場賽事或交手，請向主辦方確認。'
      case 409: return '目前無法取得交手紀錄，請稍後重新查詢。'
      default: return '交手服務暫時無法使用，請手動重新查詢。'
    }
  }
  if (protocolError(error)) return '交手服務回應無法安全驗證，請重新查詢；更正命令如已送出，其結果仍未確認。'
  switch (competitionCorrectionErrorStatus(error)) {
    case 400: return '更正或查詢資料不正確，請核對後再試。'
    case 401: return '請重新登入後再操作；重新登入不代表先前更正已取消。'
    case 403: return '伺服器未授權這次操作；裁判指派不代表目前仍有有效授權。'
    case 404: return '找不到這場賽事、交手或對局；不能據此判定先前更正未提交。'
    case 409: return '目前交手或賽程狀態不允許這次更正，請重新查詢；沒有自動重送或改動命令。'
    case 422: return '賽事核心拒絕這次判定更正；原始終局證據不會由本頁改寫。'
    default: return '賽事服務暫時無法處理，請手動重試；已送出的更正結果可能尚未確認。'
  }
}
