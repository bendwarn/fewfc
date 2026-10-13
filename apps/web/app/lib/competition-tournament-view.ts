export type TournamentFormatView = 'singleElimination' | 'roundRobin' | 'doubleElimination' | 'swiss'
export type TournamentRoundKindView = 'main' | 'winners' | 'losers' | 'grandFinal' | 'grandFinalReset'
export type TournamentStageStatusView = 'active' | 'finished'
export type TournamentMatchStatusView = 'pending' | 'inProgress' | 'completed' | 'cancelled'
export type TournamentMatchPurposeView = 'standard' | 'bronze'
export type SwissTieBreakerView =
  | 'matchPoints'
  | 'headToHeadMatchPoints'
  | 'opponentWinRate'
  | 'netGameWins'
  | 'seedOrder'

export interface TournamentViewSeededTeam {
  teamId: string
  seed: number
}

export interface TournamentViewSwissPolicy {
  matchPoints: { win: number, draw: number, loss: number, bye: number }
  drawCredit: { numerator: number, denominator: number }
  opponentWinRateFloorPercent: number | null
  opponentWinRateCapPercent: number | null
  excludeByesFromOpponentWinRate: boolean
  tieBreakers: SwissTieBreakerView[]
}

export interface TournamentViewStageConfig {
  stageId: string
  format: TournamentFormatView
  teams: TournamentViewSeededTeam[]
  randomSeed: number
  matchFormat: { type: 'fixedGames' | 'firstToWins', value: number }
  clockConfig: {
    mainTimeMs: number | null
    actionTimeMs: number | null
    outOfTurnChoiceTimeMs: number | null
  }
  swissRounds: number | null
  swissPolicy: TournamentViewSwissPolicy | null
}

export type TournamentViewMatchResult =
  | { type: 'doubleLoss' }
  | { type: 'noShow', absentTeams: string[], winnerId: string | null }
  | { type: 'win', winnerId: string, loserId: string, winnerGames: number, loserGames: number }
  | { type: 'draw', firstGames: number, secondGames: number }
  | { type: 'bye', teamId: string }
  | { type: 'forfeit', winnerId: string, loserId: string }

export interface TournamentViewMatch {
  matchId: string
  stageId: string
  roundNumber: number
  sequence: number
  kind: TournamentRoundKindView
  purpose: TournamentMatchPurposeView
  participants: string[]
  status: TournamentMatchStatusView
  result: TournamentViewMatchResult | null
  cancellationReason: string | null
  started: boolean
}

export interface TournamentViewRound {
  roundNumber: number
  kind: TournamentRoundKindView
  matches: TournamentViewMatch[]
}

export interface TournamentViewStanding {
  teamId: string
  matchPoints: string
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

export interface TournamentViewStage {
  config: TournamentViewStageConfig
  status: TournamentStageStatusView
  rounds: TournamentViewRound[]
  standings: TournamentViewStanding[]
  hasScheduleWarning: boolean
  correctionCount: number
  scheduleGeneration: number
  supersededRounds: TournamentViewRound[]
}

export interface CompetitionTournamentViewData {
  competitionId: string
  revision: number
  tournamentId: string
  stages: TournamentViewStage[]
}

export interface CompetitionTournamentViewLoadError {
  kind: 'notReady' | 'forbidden' | 'error'
  message: string
}

export class CompetitionTournamentViewProtocolError extends Error {
  constructor() {
    super('賽程資料格式無法辨識。')
    this.name = 'CompetitionTournamentViewProtocolError'
  }
}

/** 將 GET 回應中的 unknown 精確縮成 UI 白名單；不把公開 JSON cast 成共享 DTO。 */
export function parseCompetitionTournamentViewResponse(
  input: unknown,
  expectedCompetitionId: string,
): CompetitionTournamentViewData {
  try {
    const expectedId = normalizeId(expectedCompetitionId)
    const envelope = exactRecord(input, ['competitionId', 'revision', 'projection'])
    if (!expectedId || expectedId !== expectedCompetitionId || !envelope
      || envelope.competitionId !== expectedId || !isPositiveSafeInteger(envelope.revision)) {
      throw new CompetitionTournamentViewProtocolError()
    }

    const projection = exactRecord(envelope.projection, ['ok', 'operation', 'public'])
    if (!projection || projection.ok !== true || projection.operation !== 'tournamentView') {
      throw new CompetitionTournamentViewProtocolError()
    }
    const publicValue = exactRecord(projection.public, ['tournamentId', 'stages'])
    if (!publicValue || publicValue.tournamentId !== expectedId) {
      throw new CompetitionTournamentViewProtocolError()
    }
    const stages = parseArray(publicValue.stages, parseStage)
    if (!stages || stages.length === 0
      || new Set(stages.map(stage => stage.config.stageId)).size !== stages.length) {
      throw new CompetitionTournamentViewProtocolError()
    }

    return {
      competitionId: expectedId,
      revision: envelope.revision,
      tournamentId: expectedId,
      stages,
    }
  } catch {
    throw new CompetitionTournamentViewProtocolError()
  }
}

/** 所有錯誤只轉成固定文案；不讀取或回傳 server message。 */
export function presentCompetitionTournamentViewError(
  error: unknown,
): CompetitionTournamentViewLoadError {
  if (error instanceof CompetitionTournamentViewProtocolError) {
    return { kind: 'error', message: '賽程資料格式不正確，請重新查詢。' }
  }

  const status = errorStatus(error)
  const code = errorCode(error)
  if (status === 409 && code === 'notReady') {
    return { kind: 'notReady', message: '目前尚未有可顯示賽程。' }
  }
  if (status === 401) {
    return { kind: 'error', message: '登入狀態已失效，請重新登入後再查詢。' }
  }
  if (status === 403) {
    return { kind: 'forbidden', message: '目前帳號沒有這場賽事的賽程讀取權限。' }
  }
  if (status === 404) {
    return { kind: 'error', message: '找不到這場賽事，請確認賽事 ID。' }
  }
  if (status === 502) {
    return { kind: 'error', message: '賽程資料格式不正確，請稍後重試。' }
  }
  if (status === 503 || (status !== null && status >= 500)) {
    return { kind: 'error', message: '賽事服務暫時無法使用，請稍後重試。' }
  }
  if (status === 409) {
    return { kind: 'error', message: '賽程狀態暫時無法確認，請重新查詢。' }
  }
  return { kind: 'error', message: '目前無法查詢賽程，請稍後重試。' }
}

export function tournamentFormatLabel(format: TournamentFormatView): string {
  return {
    singleElimination: '單敗淘汰',
    roundRobin: '循環賽',
    doubleElimination: '雙敗淘汰',
    swiss: '瑞士制',
  }[format]
}

export function tournamentRoundKindLabel(kind: TournamentRoundKindView): string {
  return {
    main: '主輪',
    winners: '勝部',
    losers: '敗部',
    grandFinal: '總決賽',
    grandFinalReset: '總決賽重置',
  }[kind]
}

export function tournamentStageStatusLabel(status: TournamentStageStatusView): string {
  return status === 'active' ? '進行中' : '階段已結束'
}

export function tournamentMatchStatusLabel(status: TournamentMatchStatusView): string {
  return {
    pending: '待開始',
    inProgress: '進行中',
    completed: '已完成',
    cancelled: '已取消',
  }[status]
}

export function tournamentMatchResultLabel(
  result: TournamentViewMatchResult | null,
  teamLabel: (teamId: string) => string = () => '該隊',
): string {
  if (result === null) return '尚無賽果'
  switch (result.type) {
    case 'doubleLoss': return '雙方判負（不推定勝者）'
    case 'noShow':
      return result.winnerId === null
        ? `雙方因賽前未完成報到判負（${result.absentTeams.map(teamLabel).join('、')}）`
        : `${result.absentTeams.map(teamLabel).join('、')} 因賽前未完成報到判負；${teamLabel(result.winnerId)} 勝出`
    case 'win': return `${result.winnerId} 勝出 ${result.winnerGames}–${result.loserGames}`
    case 'draw': return `和局 ${result.firstGames}–${result.secondGames}`
    case 'bye': return `${result.teamId} 輪空`
    case 'forfeit': return `${result.winnerId} 棄權判定勝出（對手 ${result.loserId}）`
  }
}

export function tournamentTimeLimitLabel(milliseconds: number | null): string {
  if (milliseconds === null) return '不限時'
  if (milliseconds % 1000 === 0) return `${milliseconds / 1000} 秒`
  return `${milliseconds.toLocaleString('zh-TW')} 毫秒`
}

export function swissTieBreakerLabel(value: SwissTieBreakerView): string {
  return {
    matchPoints: '賽事積分',
    headToHeadMatchPoints: '同分隊伍交手積分',
    opponentWinRate: '對手勝率',
    netGameWins: '淨勝局',
    seedOrder: '種子順序',
  }[value]
}

/** 只辨識服務已結清的目前單敗決賽；不由排名或被替代的輪次推定冠軍。 */
export function singleEliminationDoubleLossFinal(stage: TournamentViewStage): TournamentViewMatch | null {
  if (stage.config.format !== 'singleElimination' || stage.status !== 'finished'
    || stage.hasScheduleWarning) return null

  const finalRound = stage.rounds[stage.rounds.length - 1]
  if (!finalRound || finalRound.kind !== 'main' || finalRound.matches.length !== 1) return null
  const finalMatch = finalRound.matches[0]
  if (!finalMatch || finalMatch.kind !== 'main' || finalMatch.participants.length !== 2
    || finalMatch.status !== 'completed' || finalMatch.result?.type !== 'doubleLoss') return null

  // 已結清的前輪雙負或輪空仍是事實，不當作未決案件或另造勝者。
  const previousRoundsClosed = stage.rounds.slice(0, -1).every(round =>
    round.matches.every(match => match.status === 'completed' || match.status === 'cancelled'))
  return previousRoundsClosed ? finalMatch : null
}

function parseStage(input: unknown): TournamentViewStage | null {
  const value = exactRecord(input, [
    'config', 'status', 'rounds', 'standings', 'corrections', 'lastScheduleError',
    'scheduleGeneration', 'supersededRounds', 'teams', 'doubleState',
  ])
  if (!value || (value.status !== 'active' && value.status !== 'finished')
    || !(value.lastScheduleError === null || typeof value.lastScheduleError === 'string')
    || !isNonNegativeSafeInteger(value.scheduleGeneration)) return null
  const status = value.status
  const lastScheduleError = value.lastScheduleError
  const scheduleGeneration = value.scheduleGeneration

  const config = parseStageConfig(value.config)
  if (!config) return null
  const teamIds = new Set(config.teams.map(team => team.teamId))
  if (!validTeamEntries(value.teams, config.teams)
    || !validDoubleState(value.doubleState, config.format, teamIds)) return null
  const rounds = parseArray(value.rounds, round => parseRound(round, config.stageId, teamIds, config.format))
  const supersededRounds = parseArray(value.supersededRounds, round => parseRound(round, config.stageId, teamIds, config.format))
  const standings = parseArray(value.standings, standing => parseStanding(standing, teamIds))
  const corrections = parseArray(value.corrections, parseCorrection)
  if (!rounds || !supersededRounds || !standings || !corrections
    || new Set(standings.map(standing => standing.teamId)).size !== standings.length
    || new Set(rounds.map(round => `${round.kind}:${round.roundNumber}`)).size !== rounds.length
    || !validBronzeRounds(rounds, config.format, true)
    || !validBronzeRounds(supersededRounds, config.format, false)) return null
  const allMatches = [...rounds, ...supersededRounds].flatMap(round => round.matches)
  if (new Set(allMatches.map(match => match.matchId)).size !== allMatches.length) return null

  return {
    config,
    status,
    rounds,
    standings,
    hasScheduleWarning: lastScheduleError !== null,
    correctionCount: corrections.length,
    scheduleGeneration,
    supersededRounds,
  }
}

/** 接受核心既有 checkpoint 欄位但不輸出至 UI；不從它推算配對、勝者或名次。 */
function validTeamEntries(input: unknown, teams: readonly TournamentViewSeededTeam[]): boolean {
  const entries = exactRecord(input, teams.map(team => team.teamId))
  if (!entries) return false
  return teams.every(team => {
    const entry = exactRecord(entries[team.teamId], ['seed', 'dropped', 'hadBye'])
    return entry !== null && entry.seed === team.seed
      && typeof entry.dropped === 'boolean' && typeof entry.hadBye === 'boolean'
  })
}

function validDoubleState(
  input: unknown,
  format: TournamentFormatView,
  teamIds: ReadonlySet<string>,
): boolean {
  if (format !== 'doubleElimination') return input === null
  const state = exactRecord(input, [
    'phase', 'winnersPool', 'losersWaiting', 'lowerPool',
    'winnersChampion', 'losersChampion', 'grandFinalWinner',
  ])
  if (!state || (state.phase !== 'winners' && state.phase !== 'losers'
    && state.phase !== 'grandFinal' && state.phase !== 'grandFinalReset'
    && state.phase !== 'finished')) return false
  for (const key of ['winnersPool', 'losersWaiting', 'lowerPool']) {
    const pool = parseArray(state[key], value =>
      isNonEmptyString(value) && teamIds.has(value) ? value : null)
    if (!pool || new Set(pool).size !== pool.length) return false
  }
  return ['winnersChampion', 'losersChampion', 'grandFinalWinner'].every(key => {
    const teamId = state[key]
    return teamId === null || (isNonEmptyString(teamId) && teamIds.has(teamId))
  })
}

function parseStageConfig(input: unknown): TournamentViewStageConfig | null {
  const value = exactRecord(input, [
    'stageId', 'format', 'teams', 'randomSeed', 'matchFormat', 'clockConfig', 'swissRounds', 'swissPolicy',
  ])
  if (!value || !isNonEmptyString(value.stageId) || !isTournamentFormat(value.format)
    || !isNonNegativeSafeInteger(value.randomSeed)) return null

  const teams = parseArray(value.teams, parseSeededTeam)
  const matchFormat = parseMatchFormat(value.matchFormat)
  const clockConfig = parseClockConfig(value.clockConfig)
  const swissRounds = value.swissRounds === null
    ? null
    : isNonNegativeSafeInteger(value.swissRounds) ? value.swissRounds : undefined
  let swissPolicy: TournamentViewSwissPolicy | null
  if (value.swissPolicy === null) {
    swissPolicy = null
  } else {
    const parsedPolicy = parseSwissPolicy(value.swissPolicy)
    if (!parsedPolicy) return null
    swissPolicy = parsedPolicy
  }
  if (!teams || teams.length < 2 || !matchFormat || !clockConfig || swissRounds === undefined
    || new Set(teams.map(team => team.teamId)).size !== teams.length
    || new Set(teams.map(team => team.seed)).size !== teams.length) return null

  const elimination = value.format === 'singleElimination' || value.format === 'doubleElimination'
  if ((elimination && matchFormat.type !== 'firstToWins')
    || (!elimination && matchFormat.type !== 'fixedGames')) return null
  if (value.format === 'swiss' && (swissRounds === null || swissRounds < 1 || swissPolicy === null)) return null

  return {
    stageId: value.stageId,
    format: value.format,
    teams,
    randomSeed: value.randomSeed,
    matchFormat,
    clockConfig,
    swissRounds,
    swissPolicy,
  }
}

function parseSeededTeam(input: unknown): TournamentViewSeededTeam | null {
  const value = exactRecord(input, ['teamId', 'seed'])
  if (!value || !isNonEmptyString(value.teamId) || !isPositiveSafeInteger(value.seed)) return null
  return { teamId: value.teamId, seed: value.seed }
}

function parseMatchFormat(input: unknown): TournamentViewStageConfig['matchFormat'] | null {
  const value = exactRecord(input, ['type', 'value'])
  if (!value || (value.type !== 'fixedGames' && value.type !== 'firstToWins')
    || !isPositiveSafeInteger(value.value)) return null
  return { type: value.type, value: value.value }
}

function parseClockConfig(input: unknown): TournamentViewStageConfig['clockConfig'] | null {
  const value = exactRecord(input, ['mainTimeMs', 'actionTimeMs', 'outOfTurnChoiceTimeMs'])
  if (!value) return null
  const mainTimeMs = parseTimeLimit(value.mainTimeMs)
  const actionTimeMs = parseTimeLimit(value.actionTimeMs)
  const outOfTurnChoiceTimeMs = parseTimeLimit(value.outOfTurnChoiceTimeMs)
  if (mainTimeMs === undefined || actionTimeMs === undefined || outOfTurnChoiceTimeMs === undefined) return null
  return { mainTimeMs, actionTimeMs, outOfTurnChoiceTimeMs }
}

function parseTimeLimit(value: unknown): number | null | undefined {
  if (value === null) return null
  return isNonNegativeSafeInteger(value) ? value : undefined
}

function parseSwissPolicy(input: unknown): TournamentViewSwissPolicy | null {
  const value = exactRecord(input, [
    'matchPoints', 'drawCredit', 'opponentWinRateFloorPercent', 'opponentWinRateCapPercent',
    'excludeByesFromOpponentWinRate', 'tieBreakers',
  ])
  if (!value || typeof value.excludeByesFromOpponentWinRate !== 'boolean') return null
  const matchPoints = exactRecord(value.matchPoints, ['win', 'draw', 'loss', 'bye'])
  const drawCredit = exactRecord(value.drawCredit, ['numerator', 'denominator'])
  const tieBreakers = parseArray(value.tieBreakers, parseTieBreaker)
  const floor = parsePercent(value.opponentWinRateFloorPercent)
  const cap = parsePercent(value.opponentWinRateCapPercent)
  if (!matchPoints || !drawCredit || !tieBreakers
    || !isNonNegativeSafeInteger(matchPoints.win)
    || !isNonNegativeSafeInteger(matchPoints.draw)
    || !isNonNegativeSafeInteger(matchPoints.loss)
    || !isNonNegativeSafeInteger(matchPoints.bye)
    || !isNonNegativeSafeInteger(drawCredit.numerator)
    || !isPositiveSafeInteger(drawCredit.denominator)
    || drawCredit.numerator > drawCredit.denominator
    || floor === undefined || cap === undefined) return null

  return {
    matchPoints: {
      win: matchPoints.win,
      draw: matchPoints.draw,
      loss: matchPoints.loss,
      bye: matchPoints.bye,
    },
    drawCredit: { numerator: drawCredit.numerator, denominator: drawCredit.denominator },
    opponentWinRateFloorPercent: floor,
    opponentWinRateCapPercent: cap,
    excludeByesFromOpponentWinRate: value.excludeByesFromOpponentWinRate,
    tieBreakers,
  }
}

function parsePercent(value: unknown): number | null | undefined {
  if (value === null) return null
  return isNonNegativeSafeInteger(value) && value <= 100 ? value : undefined
}

function parseTieBreaker(value: unknown): SwissTieBreakerView | null {
  switch (value) {
    case 'matchPoints':
    case 'headToHeadMatchPoints':
    case 'opponentWinRate':
    case 'netGameWins':
    case 'seedOrder':
      return value
    default:
      return null
  }
}

function parseRound(
  input: unknown,
  expectedStageId: string,
  teamIds: ReadonlySet<string>,
  format: TournamentFormatView,
): TournamentViewRound | null {
  const value = exactRecord(input, ['roundNumber', 'kind', 'matches'])
  if (!value || !isPositiveSafeInteger(value.roundNumber) || !isRoundKind(value.kind)) return null
  const roundNumber = value.roundNumber
  const kind = value.kind
  const matches = parseArray(value.matches, match =>
    parseMatch(match, expectedStageId, roundNumber, kind, teamIds, format))
  if (!matches) return null
  return { roundNumber, kind, matches }
}

function parseMatch(
  input: unknown,
  expectedStageId: string,
  expectedRoundNumber: number,
  expectedKind: TournamentRoundKindView,
  teamIds: ReadonlySet<string>,
  format: TournamentFormatView,
): TournamentViewMatch | null {
  const hasPurpose = isPlainRecord(input) && Object.hasOwn(input, 'purpose')
  const value = exactRecord(input, [
    'matchId', 'stageId', 'roundNumber', 'sequence', 'kind', 'participants', 'status',
    'result', 'cancellationReason', 'started', ...(hasPurpose ? ['purpose'] : []),
  ])
  if (!value || !isNonEmptyString(value.matchId) || !isNonEmptyString(value.stageId)
    || value.stageId !== expectedStageId || !isPositiveSafeInteger(value.roundNumber)
    || value.roundNumber !== expectedRoundNumber || !isNonNegativeSafeInteger(value.sequence)
    || !isRoundKind(value.kind) || value.kind !== expectedKind || !isMatchStatus(value.status)
    || (hasPurpose && (value.purpose !== 'bronze'
      || format !== 'singleElimination' || value.kind !== 'main'))
    || !(value.cancellationReason === null || typeof value.cancellationReason === 'string')
    || typeof value.started !== 'boolean') return null

  const participants = parseArray(value.participants, participant =>
    isNonEmptyString(participant) && teamIds.has(participant) ? participant : null)
  if (!participants || participants.length < 1 || participants.length > 2
    || new Set(participants).size !== participants.length) return null
  let result: TournamentViewMatchResult | null = null
  if (value.result !== null) {
    const parsedResult = parseMatchResult(value.result, new Set(participants))
    if (!parsedResult) return null
    if (parsedResult.type === 'noShow' && (value.status !== 'completed' || value.started)) return null
    result = parsedResult
  }

  const matchId = value.matchId
  const roundNumber = value.roundNumber
  const sequence = value.sequence
  const kind = value.kind
  const status = value.status
  const cancellationReason = value.cancellationReason
  const started = value.started

  return {
    matchId,
    stageId: expectedStageId,
    roundNumber,
    sequence,
    kind,
    purpose: hasPurpose ? 'bronze' : 'standard',
    participants,
    status,
    result,
    cancellationReason,
    started,
  }
}

function validBronzeRounds(
  rounds: readonly TournamentViewRound[],
  format: TournamentFormatView,
  requireLatest: boolean,
): boolean {
  for (const [index, round] of rounds.entries()) {
    const bronzeMatches = round.matches.filter(match => match.purpose === 'bronze')
    if (!bronzeMatches.length) continue
    const [bronzeMatch] = bronzeMatches
    const championshipMatches = round.matches.filter(match => match.purpose === 'standard')
    if (format !== 'singleElimination' || round.kind !== 'main'
      || bronzeMatches.length !== 1 || round.matches.length !== 2
      || championshipMatches.length !== 1 || bronzeMatch?.participants.length !== 2
      || championshipMatches[0]?.participants.length !== 2
      || bronzeMatch.participants.some(team => championshipMatches[0]?.participants.includes(team))) {
      return false
    }
    if (requireLatest && index !== rounds.length - 1) return false
  }
  return true
}

function parseMatchResult(
  input: unknown,
  participants?: ReadonlySet<string>,
): TournamentViewMatchResult | null {
  const value = asPlainRecord(input)
  if (!value || typeof value.type !== 'string') return null
  switch (value.type) {
    case 'doubleLoss':
      return exactRecord(value, ['type']) && (!participants || participants.size === 2)
        ? { type: 'doubleLoss' }
        : null
    case 'noShow': {
      const result = exactRecord(value, ['type', 'absentTeams', 'winnerId'])
      if (!result || !Array.isArray(result.absentTeams) || result.absentTeams.length < 1
        || result.absentTeams.length > 2 || result.absentTeams.some(team => !isNonEmptyString(team))
        || new Set(result.absentTeams).size !== result.absentTeams.length
        || (participants && (participants.size !== 2 || result.absentTeams.some(team => !participants.has(team))))) {
        return null
      }
      const winnerId = result.winnerId
      if (result.absentTeams.length === 2) {
        if (winnerId !== null || (participants && result.absentTeams.some(team => !participants.has(team)))) return null
      } else if (!isNonEmptyString(winnerId) || result.absentTeams.includes(winnerId)
        || (participants && !participants.has(winnerId))) {
        return null
      }
      return { type: 'noShow', absentTeams: [...result.absentTeams], winnerId }
    }
    case 'win': {
      const result = exactRecord(value, ['type', 'winnerId', 'loserId', 'winnerGames', 'loserGames'])
      if (!result || !isNonEmptyString(result.winnerId) || !isNonEmptyString(result.loserId)
        || result.winnerId === result.loserId
        || !isNonNegativeSafeInteger(result.winnerGames) || !isNonNegativeSafeInteger(result.loserGames)
        || (participants && (!participants.has(result.winnerId) || !participants.has(result.loserId)))) return null
      return {
        type: 'win',
        winnerId: result.winnerId,
        loserId: result.loserId,
        winnerGames: result.winnerGames,
        loserGames: result.loserGames,
      }
    }
    case 'draw': {
      const result = exactRecord(value, ['type', 'firstGames', 'secondGames'])
      if (!result || !isNonNegativeSafeInteger(result.firstGames) || !isNonNegativeSafeInteger(result.secondGames)
        || (participants && participants.size !== 2)) return null
      return { type: 'draw', firstGames: result.firstGames, secondGames: result.secondGames }
    }
    case 'bye': {
      const result = exactRecord(value, ['type', 'teamId'])
      if (!result || !isNonEmptyString(result.teamId)
        || (participants && (participants.size !== 1 || !participants.has(result.teamId)))) return null
      return { type: 'bye', teamId: result.teamId }
    }
    case 'forfeit': {
      const result = exactRecord(value, ['type', 'winnerId', 'loserId'])
      if (!result || !isNonEmptyString(result.winnerId) || !isNonEmptyString(result.loserId)
        || result.winnerId === result.loserId
        || (participants && (!participants.has(result.winnerId) || !participants.has(result.loserId)))) return null
      return { type: 'forfeit', winnerId: result.winnerId, loserId: result.loserId }
    }
    default:
      return null
  }
}

function parseStanding(input: unknown, teamIds: ReadonlySet<string>): TournamentViewStanding | null {
  const value = exactRecord(input, [
    'teamId', 'matchPoints', 'wins', 'draws', 'losses', 'gameWins', 'gameLosses',
    'netGameWins', 'opponentWinRateBasisPoints', 'dropped', 'rank',
  ])
  if (!value || !isNonEmptyString(value.teamId) || !teamIds.has(value.teamId)
    || !isU64DecimalString(value.matchPoints)
    || !isNonNegativeSafeInteger(value.wins)
    || !isNonNegativeSafeInteger(value.draws)
    || !isNonNegativeSafeInteger(value.losses)
    || !isNonNegativeSafeInteger(value.gameWins)
    || !isNonNegativeSafeInteger(value.gameLosses)
    || !isSafeInteger(value.netGameWins)
    || !(value.opponentWinRateBasisPoints === null
      || (isNonNegativeSafeInteger(value.opponentWinRateBasisPoints) && value.opponentWinRateBasisPoints <= 10_000))
    || typeof value.dropped !== 'boolean'
    || !(value.rank === null || isPositiveSafeInteger(value.rank))) return null

  const teamId = value.teamId
  const matchPoints = value.matchPoints
  const wins = value.wins
  const draws = value.draws
  const losses = value.losses
  const gameWins = value.gameWins
  const gameLosses = value.gameLosses
  const netGameWins = value.netGameWins
  const opponentWinRateBasisPoints = value.opponentWinRateBasisPoints
  const dropped = value.dropped
  const rank = value.rank

  return {
    teamId,
    matchPoints,
    wins,
    draws,
    losses,
    gameWins,
    gameLosses,
    netGameWins,
    opponentWinRateBasisPoints,
    dropped,
    rank,
  }
}

function parseCorrection(input: unknown): true | null {
  const value = exactRecord(input, ['matchId', 'previousResult', 'replacementResult', 'reason'])
  if (!value || !isNonEmptyString(value.matchId) || typeof value.reason !== 'string'
    || !parseMatchResult(value.previousResult) || !parseMatchResult(value.replacementResult)) return null
  return true
}

function parseArray<T>(input: unknown, parse: (item: unknown) => T | null): T[] | null {
  if (!Array.isArray(input) || Object.getPrototypeOf(input) !== Array.prototype) return null
  const ownKeys = Reflect.ownKeys(input)
  if (ownKeys.length !== input.length + 1 || ownKeys.some(key => typeof key !== 'string')) return null
  const lengthDescriptor = Object.getOwnPropertyDescriptor(input, 'length')
  if (!lengthDescriptor || lengthDescriptor.enumerable) return null

  const result: T[] = []
  for (let index = 0; index < input.length; index += 1) {
    const descriptor = Object.getOwnPropertyDescriptor(input, String(index))
    if (!descriptor || !descriptor.enumerable || !('value' in descriptor)) return null
    const parsed = parse(descriptor.value)
    if (parsed === null) return null
    result.push(parsed)
  }
  return result
}

function exactRecord(input: unknown, expectedKeys: readonly string[]): Record<string, unknown> | null {
  const value = asPlainRecord(input)
  if (!value) return null
  const ownKeys = Reflect.ownKeys(value)
  if (ownKeys.length !== expectedKeys.length
    || ownKeys.some(key => typeof key !== 'string')
    || !expectedKeys.every(key => Object.hasOwn(value, key))) return null
  for (const key of expectedKeys) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key)
    if (!descriptor || !descriptor.enumerable || !('value' in descriptor)) return null
  }
  return value
}

function asPlainRecord(input: unknown): Record<string, unknown> | null {
  return isPlainRecord(input) ? input : null
}

function isPlainRecord(input: unknown): input is Record<string, unknown> {
  if (typeof input !== 'object' || input === null || Array.isArray(input)) return false
  const prototype = Object.getPrototypeOf(input)
  return prototype === Object.prototype || prototype === null
}

function normalizeId(input: unknown): string | null {
  if (typeof input !== 'string') return null
  const normalized = input.trim()
  return normalized ? normalized : null
}

function isTournamentFormat(value: unknown): value is TournamentFormatView {
  return value === 'singleElimination' || value === 'roundRobin'
    || value === 'doubleElimination' || value === 'swiss'
}

function isRoundKind(value: unknown): value is TournamentRoundKindView {
  return value === 'main' || value === 'winners' || value === 'losers'
    || value === 'grandFinal' || value === 'grandFinalReset'
}

function isMatchStatus(value: unknown): value is TournamentMatchStatusView {
  return value === 'pending' || value === 'inProgress' || value === 'completed' || value === 'cancelled'
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0
}

function isSafeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value)
}

function isNonNegativeSafeInteger(value: unknown): value is number {
  return isSafeInteger(value) && value >= 0
}

function isU64DecimalString(value: unknown): value is string {
  return typeof value === 'string'
    && /^(0|[1-9][0-9]*)$/.test(value)
    && (value.length < 20 || (value.length === 20 && value <= '18446744073709551615'))
}

function isPositiveSafeInteger(value: unknown): value is number {
  return isSafeInteger(value) && value > 0
}

function errorStatus(error: unknown): number | null {
  const statusCode = errorValue(error, 'statusCode')
  if (isSafeInteger(statusCode)) return statusCode
  const status = errorValue(error, 'status')
  return isSafeInteger(status) ? status : null
}

function errorCode(error: unknown): string | null {
  const data = errorValue(error, 'data')
  const record = asPlainRecord(data)
  // ofetch 的 data 是 H3 HTTP error JSON，真正的固定錯誤碼位於其 data。
  const nested = record && asPlainRecord(errorValue(record, 'data'))
  const code = nested ? errorValue(nested, 'code') : record && errorValue(record, 'code')
  return typeof code === 'string' ? code : null
}

function errorValue(input: unknown, key: string): unknown {
  if (typeof input !== 'object' || input === null) return undefined
  // FetchError 用 getter 提供 status/data；不讀 raw message，異常 getter 亦不向外反射。
  try {
    return Reflect.get(input, key)
  } catch {
    return undefined
  }
}
