import type { CompetitionCorrectionMatch } from './competition-ruling-correction'

export interface CaptainNextGameChoice {
  matchRevision: number
  gameNumber: number
  teams: [string, string]
}

/** 只決定網站是否顯示操作；提交時仍由 CompetitionRoom 重驗敗隊隊長與目前局號。 */
export function captainNextGameChoice(
  view: CompetitionCorrectionMatch | null,
  participantTeamId: string | null,
  isCaptain: boolean,
): CaptainNextGameChoice | null {
  const match = view?.match
  if (!isCaptain || !participantTeamId || !view || !match
    || match.status !== 'awaitingFirstPlayer'
    || !match.nextGame || match.nextGame.gameNumber < 2
    || match.nextGame.kind.type !== 'ordinaryNewGame'
    || match.nextGame.firstPlayer.type !== 'pending') return null

  const latestGame = match.games[match.games.length - 1]
  if (!latestGame) return null
  const ruling = match.rulings.find(record => record.rulingRecordId === latestGame.rulingRecordId
    && record.gameRecordId === latestGame.gameRecordId)?.ruling
  if (!ruling) return null

  let losingTeam: string | null = null
  if (ruling.type === 'timeoutLoss' || ruling.type === 'eliminationDrawLoss') {
    losingTeam = ruling.loser
  } else if (ruling.type === 'gameAccepted' && ruling.outcome !== 'draw') {
    const winner = ruling.outcome.winner
    losingTeam = match.config.teams.find(team => team !== winner) ?? null
  }
  if (losingTeam !== participantTeamId) return null

  return {
    matchRevision: view.matchRevision,
    gameNumber: match.nextGame.gameNumber,
    teams: [match.config.teams[0], match.config.teams[1]],
  }
}

export interface CaptainNextGameCommand {
  selectedFirstPlayerTeam: string
  expectedMatchRevision: number
  commandId: string
}

export interface CaptainNextGameAcknowledgement {
  gameNumber: number
  gameId: string
  status: 'pending' | 'provisioned' | 'blocked'
}

export function parseCaptainNextGameAcknowledgement(
  input: unknown,
  competitionId: string,
  matchId: string,
  command: CaptainNextGameCommand,
): CaptainNextGameAcknowledgement {
  const value = record(input)
  if (!value || !exactKeys(value, [
    'competitionId', 'matchId', 'matchRevision', 'gameNumber', 'gameId', 'commandId', 'status',
  ]) || value.competitionId !== competitionId || value.matchId !== matchId
    || value.matchRevision !== command.expectedMatchRevision + 1
    || value.commandId !== command.commandId
    || !positiveInteger(value.gameNumber) || value.gameNumber < 2
    || typeof value.gameId !== 'string' || !value.gameId.trim() || value.gameId !== value.gameId.trim()
    || !['pending', 'provisioned', 'blocked'].includes(value.status as string)) {
    throw new Error('下一局回應格式無法確認。')
  }
  return {
    gameNumber: value.gameNumber as number,
    gameId: value.gameId as string,
    status: value.status as CaptainNextGameAcknowledgement['status'],
  }
}

export function isCaptainNextGameCommand(value: unknown): value is CaptainNextGameCommand {
  const command = record(value)
  return Boolean(command && exactKeys(command, [
    'selectedFirstPlayerTeam', 'expectedMatchRevision', 'commandId',
  ]) && typeof command.selectedFirstPlayerTeam === 'string'
    && command.selectedFirstPlayerTeam.trim() === command.selectedFirstPlayerTeam
    && command.selectedFirstPlayerTeam.length > 0
    && positiveInteger(command.expectedMatchRevision)
    && command.expectedMatchRevision < Number.MAX_SAFE_INTEGER
    && typeof command.commandId === 'string' && command.commandId.trim() === command.commandId
    && command.commandId.length > 0)
}

function record(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null
}

function exactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Reflect.ownKeys(value)
  return actual.length === keys.length
    && actual.every((key): key is string => typeof key === 'string')
    && keys.every(key => Object.hasOwn(value, key))
}

function positiveInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value > 0
}
