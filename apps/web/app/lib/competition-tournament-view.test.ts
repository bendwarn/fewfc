import { expect, test } from 'bun:test'
import {
  parseCompetitionTournamentViewResponse,
  doubleEliminationVacancySummary,
  presentCompetitionTournamentViewError,
  singleEliminationDoubleLossFinal,
  singleEliminationVacancySummary,
  tournamentFormatLabel,
  tournamentMatchResultLabel,
  tournamentStageResolutionLabel,
  tournamentStageStatusLabel,
  tournamentTimeLimitLabel,
} from './competition-tournament-view'

function stageFixture(format: string, stageId: string) {
  return {
    config: {
      stageId,
      format,
      teams: [
        { teamId: 'team-a', seed: 1 },
        { teamId: 'team-b', seed: 2 },
      ],
      randomSeed: 77,
      matchFormat: { type: format === 'roundRobin' || format === 'swiss' ? 'fixedGames' : 'firstToWins', value: 2 },
      clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 15_000 },
      swissRounds: format === 'swiss' ? 3 : null,
      swissPolicy: format === 'swiss'
        ? {
            matchPoints: { win: 3, draw: 1, loss: 0, bye: 3 },
            drawCredit: { numerator: 1, denominator: 2 },
            opponentWinRateFloorPercent: 25,
            opponentWinRateCapPercent: 75,
            excludeByesFromOpponentWinRate: true,
            tieBreakers: ['matchPoints', 'opponentWinRate', 'seedOrder'],
          }
        : null,
    },
    status: 'active',
    rounds: [{
      roundNumber: 1,
      kind: format === 'doubleElimination' ? 'winners' : 'main',
      matches: [{
        matchId: `${stageId}-match-1`,
        stageId,
        roundNumber: 1,
        sequence: 1,
        kind: format === 'doubleElimination' ? 'winners' : 'main',
        participants: ['team-a', 'team-b'],
        status: 'pending',
        result: null,
        cancellationReason: null,
        started: false,
      }],
    }],
    standings: [
      {
        teamId: 'team-a', matchPoints: '0', wins: 0, draws: 0, losses: 0,
        gameWins: 0, gameLosses: 0, netGameWins: 0,
        opponentWinRateBasisPoints: null, opponentWinScore: '0', opponentLossScore: '0', dropped: false, rank: null,
      },
      {
        teamId: 'team-b', matchPoints: '0', wins: 0, draws: 0, losses: 0,
        gameWins: 0, gameLosses: 0, netGameWins: 0,
        opponentWinRateBasisPoints: null, opponentWinScore: '0', opponentLossScore: '0', dropped: false, rank: null,
      },
    ],
    resolution: null,
    rankingTieDecisions: [],
    corrections: [],
    lastScheduleError: null,
    scheduleGeneration: 0,
    supersededRounds: [],
    // 欄位依 Rust Stage／TeamEntry／DoubleState 的 camelCase 序列化契約建立。
    teams: {
      'team-a': { seed: 1, dropped: false, hadBye: false },
      'team-b': { seed: 2, dropped: false, hadBye: false },
    },
    doubleState: format === 'doubleElimination'
      ? {
          phase: 'winners',
          winnersPool: ['team-a', 'team-b'],
          losersWaiting: [],
          lowerPool: [],
          winnersChampion: null,
          losersChampion: null,
          grandFinalWinner: null,
        }
      : null,
  }
}

function response() {
  return {
    competitionId: 'competition-a',
    revision: 9,
    projection: {
      ok: true,
      operation: 'tournamentView',
      public: {
        tournamentId: 'competition-a',
        stages: [
          stageFixture('singleElimination', 'single-stage'),
          stageFixture('roundRobin', 'round-robin-stage'),
          stageFixture('doubleElimination', 'double-stage'),
          stageFixture('swiss', 'swiss-stage'),
        ],
      },
    },
  }
}

function responseWithStageOverrides(stageIndex: number, overrides: Record<string, unknown>): unknown {
  const base = response()
  const stages = base.projection.public.stages.map((stage, index) =>
    index === stageIndex ? { ...stage, ...overrides } : stage)
  return {
    ...base,
    projection: {
      ...base.projection,
      public: { ...base.projection.public, stages },
    },
  }
}

function responseWithMatchOverrides(stageIndex: number, overrides: Record<string, unknown>): unknown {
  const base = response()
  const stages = base.projection.public.stages.map((stage, index) => {
    if (index !== stageIndex) return stage
    const rounds = stage.rounds.map((round, roundIndex) => roundIndex === 0
      ? {
          ...round,
          matches: round.matches.map((match, matchIndex) =>
            matchIndex === 0 ? { ...match, ...overrides } : match),
        }
      : round)
    return { ...stage, rounds }
  })
  return {
    ...base,
    projection: {
      ...base.projection,
      public: { ...base.projection.public, stages },
    },
  }
}

function responseWithStandingOverrides(stageIndex: number, overrides: Record<string, unknown>): unknown {
  const base = response()
  const stages = base.projection.public.stages.map((stage, index) => index === stageIndex
    ? { ...stage, standings: stage.standings.map((standing, standingIndex) =>
        standingIndex === 0 ? { ...standing, ...overrides } : standing) }
    : stage)
  return {
    ...base,
    projection: {
      ...base.projection,
      public: { ...base.projection.public, stages },
    },
  }
}

function singleEliminationBronzeResponse(): unknown {
  const fixture = response()
  const stage = fixture.projection.public.stages[0]
  if (!stage) throw new Error('missing single-elimination fixture')
  const stageId = stage.config.stageId
  const teamIds = ['team-a', 'team-b', 'team-c', 'team-d']
  const makeMatch = (
    matchId: string,
    roundNumber: number,
    sequence: number,
    participants: string[],
    status: 'pending' | 'completed',
    purpose?: 'bronze',
    result: unknown = null,
  ) => ({
    matchId, stageId, roundNumber, sequence, kind: 'main', ...(purpose ? { purpose } : {}), participants,
    status, result, cancellationReason: null, started: status === 'completed',
  })
  const rows = teamIds.map(teamId => ({
    teamId, matchPoints: '3', wins: 1, draws: 0, losses: 0, gameWins: 2, gameLosses: 0,
    netGameWins: 2, opponentWinRateBasisPoints: null, opponentWinScore: '0', opponentLossScore: '0', dropped: false, rank: null,
  }))
  Object.assign(stage, {
    config: { ...stage.config, teams: teamIds.map((teamId, index) => ({ teamId, seed: index + 1 })) },
    teams: Object.fromEntries(teamIds.map((teamId, index) => [teamId, {
      seed: index + 1, dropped: false, hadBye: false,
    }])),
    standings: rows,
    rounds: [
      {
        roundNumber: 1,
        kind: 'main',
        matches: [
          makeMatch('single-stage-g0-r1-m1', 1, 1, ['team-a', 'team-b'], 'completed', undefined,
            { type: 'win', winnerId: 'team-a', loserId: 'team-b', winnerGames: 2, loserGames: 0 }),
          makeMatch('single-stage-g0-r1-m2', 1, 2, ['team-c', 'team-d'], 'completed', undefined,
            { type: 'win', winnerId: 'team-c', loserId: 'team-d', winnerGames: 2, loserGames: 0 }),
        ],
      },
      {
        roundNumber: 2,
        kind: 'main',
        matches: [
          makeMatch('single-stage-g0-r2-m1', 2, 1, ['team-a', 'team-c'], 'pending'),
          makeMatch('single-stage-g0-r2-m2', 2, 2, ['team-b', 'team-d'], 'pending', 'bronze'),
        ],
      },
    ],
  })
  return fixture
}

test('parses the exact current tournament projection without inferring rank from array position', () => {
  const parsed = parseCompetitionTournamentViewResponse(response(), 'competition-a')

  expect(parsed.competitionId).toBe('competition-a')
  expect(parsed.revision).toBe(9)
  expect(parsed.stages.map(stage => stage.config.format)).toEqual([
    'singleElimination', 'roundRobin', 'doubleElimination', 'swiss',
  ])
  expect(parsed.stages[0]?.standings[0]?.rank).toBeNull()
  expect(parsed.stages[0]?.standings[1]?.rank).toBeNull()
  expect(parsed.stages[0]?.rounds[0]?.matches[0]?.sequence).toBe(1)
  expect(parsed.stages[0]?.config.clockConfig.outOfTurnChoiceTimeMs).toBe(15_000)
  expect(parsed.stages[3]?.config.swissPolicy?.tieBreakers).toEqual([
    'matchPoints', 'opponentWinRate', 'seedOrder',
  ])
  expect(parsed.stages[2]).not.toHaveProperty('teams')
  expect(parsed.stages[2]).not.toHaveProperty('doubleState')
})

test('parses and labels qualified-stage winners and cancellations without matches', () => {
  const machineTeamId = '2f76d85c-512b-4ea6-9297-5a6953ed114c'
  const oneTeam = parseCompetitionTournamentViewResponse(responseWithStageOverrides(0, {
    config: {
      ...stageFixture('singleElimination', 'single-stage').config,
      teams: [{ teamId: machineTeamId, seed: 8 }],
    },
    status: 'finished',
    rounds: [],
    standings: [{ ...stageFixture('singleElimination', 'single-stage').standings[0], teamId: machineTeamId, rank: 1 }],
    teams: { [machineTeamId]: { seed: 8, dropped: false, hadBye: false } },
    resolution: { type: 'winnerWithoutMatch', teamId: machineTeamId },
  }), 'competition-a').stages[0]!
  expect(oneTeam.config.teams).toEqual([{ teamId: machineTeamId, seed: 8 }])
  const winnerMessage = tournamentStageResolutionLabel(oneTeam.resolution)
  expect(winnerMessage).toContain('直接成為階段勝者')
  expect(winnerMessage).toContain('遊戲')
  expect(winnerMessage).not.toContain(machineTeamId)
  expect(winnerMessage).not.toContain('Game')
  expect(oneTeam.rounds).toEqual([])

  const noTeams = parseCompetitionTournamentViewResponse(responseWithStageOverrides(0, {
    config: { ...stageFixture('singleElimination', 'single-stage').config, teams: [] },
    status: 'finished',
    rounds: [],
    standings: [],
    teams: {},
    resolution: { type: 'cancelled', reason: 'noQualifiedParticipants' },
  }), 'competition-a').stages[0]!
  expect(tournamentStageStatusLabel(noTeams.status, noTeams.resolution)).toBe('階段已取消')
  expect(tournamentStageResolutionLabel(noTeams.resolution)).toContain('無合格參賽者')
  expect(noTeams.rounds).toEqual([])
})

test('parses only the frozen publication payload for a completed tournament', () => {
  const publication = {
    completedAtTrustedTimeMs: 2_000,
    lockedDecks: [{ teamId: 'team-a', decks: [{ player: 'a-1', name: 'frozen', cards: ['card-a', 'card-b'] }] }],
    replays: [{ matchId: 'match-a', gameId: 'game-a', gameNumber: 1, replayId: 'competition:competition-a:game-a' }],
  }
  const parsed = parseCompetitionTournamentViewResponse({ ...response(), publication }, 'competition-a')
  expect(parsed.publication).toEqual(publication)

  for (const invalid of [
    { ...publication, privateState: {} },
    { ...publication, lockedDecks: [{ ...publication.lockedDecks[0], accountDeck: true }] },
    { ...publication, replays: [{ ...publication.replays[0], shuffleOrder: ['secret'] }] },
    { ...publication, completedAtTrustedTimeMs: -1 },
  ]) {
    expect(() => parseCompetitionTournamentViewResponse({ ...response(), publication: invalid }, 'competition-a'))
      .toThrow()
  }
})

test('preserves exact decimal-string Match Points through the full u64 range', () => {
  for (const matchPoints of ['8589934590', '18446744073709551615']) {
    const parsed = parseCompetitionTournamentViewResponse(
      responseWithStandingOverrides(0, { matchPoints }), 'competition-a',
    )
    expect(parsed.stages[0]?.standings[0]?.matchPoints).toBe(matchPoints)
  }

  for (const matchPoints of [8589934590, '018', '18446744073709551616']) {
    expect(() => parseCompetitionTournamentViewResponse(
      responseWithStandingOverrides(0, { matchPoints }), 'competition-a',
    )).toThrow()
  }
})

test('parses opponent scores and public tie decisions as exact decimal-string ranking facts', () => {
  const decision = {
    tiedTeamIds: ['team-a', 'team-b'],
    orderedTeamIds: ['team-b', 'team-a'],
    matchPoints: '18446744073709551615',
    opponentWinScore: '18446744073709551614',
    opponentLossScore: '0',
    decidedBy: { type: 'referee', userId: 'referee-1' },
    reason: '公開理由',
  }
  const parsed = parseCompetitionTournamentViewResponse(responseWithStageOverrides(1, {
    status: 'finished',
    standings: [
      { ...stageFixture('roundRobin', 'round-robin-stage').standings[0],
        matchPoints: decision.matchPoints, opponentWinScore: decision.opponentWinScore,
        opponentLossScore: decision.opponentLossScore, rank: 2 },
      { ...stageFixture('roundRobin', 'round-robin-stage').standings[1],
        matchPoints: decision.matchPoints, opponentWinScore: decision.opponentWinScore,
        opponentLossScore: decision.opponentLossScore, rank: 1 },
    ],
    rankingTieDecisions: [decision],
  }), 'competition-a')
  expect(parsed.stages[1]?.standings.map(row => [row.opponentWinScore, row.opponentLossScore])).toEqual([
    ['18446744073709551614', '0'], ['18446744073709551614', '0'],
  ])
  expect(parsed.stages[1]?.rankingTieDecisions[0]?.orderedTeamIds).toEqual(['team-b', 'team-a'])
  expect(parsed.stages[1]?.rankingTieDecisions[0]?.reason).toBe('公開理由')

  for (const malformed of [
    { ...decision, tiedTeamIds: ['team-b', 'team-a'] },
    { ...decision, orderedTeamIds: ['team-a', 'team-a'] },
    { ...decision, opponentWinScore: 18 },
    { ...decision, opponentLossScore: '18446744073709551616' },
    { ...decision, decidedBy: { type: 'player', userId: 'player-1' } },
    { ...decision, reason: '   ' },
    { ...decision, private: 'secret' },
  ]) {
    expect(() => parseCompetitionTournamentViewResponse(responseWithStageOverrides(1, {
      status: 'finished', rankingTieDecisions: [malformed],
    }), 'competition-a')).toThrow()
  }
  expect(() => parseCompetitionTournamentViewResponse(responseWithStandingOverrides(1, {
    opponentWinScore: '018',
  }), 'competition-a')).toThrow()
})

test('parses the bronze purpose only for the same-round single-elimination podium pair', () => {
  const parsed = parseCompetitionTournamentViewResponse(singleEliminationBronzeResponse(), 'competition-a')
  const finalRound = parsed.stages[0]?.rounds[1]
  expect(finalRound?.matches.map(match => match.purpose)).toEqual(['standard', 'bronze'])

  expect(() => parseCompetitionTournamentViewResponse(
    responseWithMatchOverrides(1, { purpose: 'bronze' }), 'competition-a',
  )).toThrow()
  expect(() => parseCompetitionTournamentViewResponse(
    responseWithMatchOverrides(0, { purpose: 'championship' }), 'competition-a',
  )).toThrow()
})

test('preserves server bye and double-loss states and never renders raw schedule or correction reasons', () => {
  const base = response()
  const stages = base.projection.public.stages.map((stage, stageIndex) => {
    if (stageIndex !== 2) return stage
    return {
      ...stage,
      lastScheduleError: 'private referee payload',
      resolution: null,
      rankingTieDecisions: [],
      corrections: [{
        matchId: 'double-stage-match-1',
        previousResult: { type: 'draw', firstGames: 1, secondGames: 1 },
        replacementResult: { type: 'doubleLoss' },
        reason: 'private correction note',
      }],
      rounds: stage.rounds.map((round, roundIndex) => roundIndex === 0
        ? {
            ...round,
            matches: round.matches.map((match, matchIndex) => matchIndex === 0
              ? { ...match, status: 'completed', result: { type: 'doubleLoss' }, started: true }
              : match),
          }
        : round),
    }
  })
  const combined = {
    ...base,
    projection: {
      ...base.projection,
      public: { ...base.projection.public, stages },
    },
  }
  const parsed = parseCompetitionTournamentViewResponse(combined, 'competition-a')
  const doubleLossStage = parsed.stages[2]
  const doubleLoss = doubleLossStage?.rounds[0]?.matches[0]?.result ?? null
  expect(doubleLossStage?.hasScheduleWarning).toBe(true)
  expect(doubleLossStage?.correctionCount).toBe(1)
  expect(tournamentMatchResultLabel(doubleLoss)).toBe('雙方判負（不推定勝者）')
  expect(JSON.stringify(parsed)).not.toContain('private referee payload')
  expect(JSON.stringify(parsed)).not.toContain('private correction note')

  const byeInput = responseWithMatchOverrides(3, {
    participants: ['team-a'],
    status: 'completed',
    result: { type: 'bye', teamId: 'team-a' },
    started: false,
  })
  const bye = parseCompetitionTournamentViewResponse(byeInput, 'competition-a')
    .stages[3]?.rounds[0]?.matches[0]?.result ?? null
  expect(tournamentMatchResultLabel(bye)).toBe('team-a 輪空')

  const win = parseCompetitionTournamentViewResponse(responseWithMatchOverrides(0, {
    status: 'completed',
    started: true,
    result: { type: 'win', winnerId: 'team-a', loserId: 'team-b', winnerGames: 2, loserGames: 1 },
  }), 'competition-a').stages[0]?.rounds[0]?.matches[0]?.result ?? null
  const draw = parseCompetitionTournamentViewResponse(responseWithMatchOverrides(1, {
    status: 'completed',
    started: true,
    result: { type: 'draw', firstGames: 1, secondGames: 1 },
  }), 'competition-a').stages[1]?.rounds[0]?.matches[0]?.result ?? null
  const forfeit = parseCompetitionTournamentViewResponse(responseWithMatchOverrides(0, {
    status: 'completed',
    result: { type: 'forfeit', winnerId: 'team-b', loserId: 'team-a' },
  }), 'competition-a').stages[0]?.rounds[0]?.matches[0]?.result ?? null
  expect(tournamentMatchResultLabel(win)).toBe('team-a 勝出 2–1')
  expect(tournamentMatchResultLabel(draw)).toBe('和局 1–1')
  expect(tournamentMatchResultLabel(forfeit)).toBe('team-b 棄權判定勝出（對手 team-a）')
})

test('parses structural double-elimination vacancies and explains a lower-final double loss', () => {
  const base = response()
  const stage = base.projection.public.stages[2]!
  const stageId = stage.config.stageId
  const teams = ['team-a', 'team-b', 'team-c', 'team-d']
  const standings = teams.map((teamId, index) => ({
    teamId,
    matchPoints: '0', wins: 0, draws: 0, losses: index === 1 || index === 2 ? 1 : 0,
    gameWins: 0, gameLosses: 0, netGameWins: 0,
    opponentWinRateBasisPoints: null, opponentWinScore: '0', opponentLossScore: '0',
    dropped: index === 1 || index === 2, rank: index === 0 ? 1 : index === 3 ? 4 : null,
  }))
  const winnersMatch = {
    matchId: `${stageId}-winners-final`, stageId, roundNumber: 1, sequence: 1, kind: 'winners',
    participants: ['team-a', 'team-d'], status: 'completed',
    result: { type: 'win', winnerId: 'team-a', loserId: 'team-d', winnerGames: 1, loserGames: 0 },
    cancellationReason: null, started: true,
  }
  const doubleLoss = {
    matchId: `${stageId}-lower-final`, stageId, roundNumber: 2, sequence: 1, kind: 'losers',
    participants: ['team-b', 'team-c'], status: 'completed', result: { type: 'doubleLoss' },
    cancellationReason: null, started: true,
  }
  const input = responseWithStageOverrides(2, {
    config: { ...stage.config, teams: teams.map((teamId, index) => ({ teamId, seed: index + 1 })) },
    teams: Object.fromEntries(teams.map((teamId, index) => [teamId, {
      seed: index + 1,
      dropped: index === 1 || index === 2,
      hadBye: false,
    }])),
    status: 'finished',
    rounds: [
      { roundNumber: 1, kind: 'winners', matches: [winnersMatch] },
      { roundNumber: 2, kind: 'losers', matches: [doubleLoss] },
    ],
    standings,
    doubleState: {
      phase: 'finished', winnersPool: ['team-a', null], losersWaiting: [], lowerPool: [null],
      winnersChampion: 'team-a', losersChampion: null, grandFinalWinner: 'team-a',
    },
  })
  const parsed = parseCompetitionTournamentViewResponse(input, 'competition-a')
  const doubleStage = parsed.stages[2]!
  expect(doubleStage).not.toHaveProperty('doubleState')
  expect(doubleEliminationVacancySummary(doubleStage))
    .toBe('勝部冠軍沿空位取得冠軍，亞軍與季軍空缺')

  const malformed = responseWithStageOverrides(2, {
    doubleState: { ...stage.doubleState, winnersPool: [null, 'unknown-team'] },
  })
  expect(() => parseCompetitionTournamentViewResponse(malformed, 'competition-a')).toThrow()
})

test('explains when the lower-bracket champion takes the vacant grand-final position', () => {
  const source = response().projection.public.stages[2]!
  const stageId = source.config.stageId
  const teams = Array.from({ length: 8 }, (_, index) => `team-${String.fromCharCode(97 + index)}`)
  const standings = teams.map((teamId, index) => ({
    teamId,
    matchPoints: '0', wins: 0, draws: 0, losses: index < 4 ? 1 : 0,
    gameWins: 0, gameLosses: 0, netGameWins: 0,
    opponentWinRateBasisPoints: null, opponentWinScore: '0', opponentLossScore: '0',
    dropped: index < 4, rank: index === 4 ? 1 : null,
  }))
  const doubleLossMatch = (sequence: number, participants: [string, string]) => ({
    matchId: `${stageId}-winners-${sequence}`, stageId, roundNumber: 3, sequence, kind: 'winners',
    participants, status: 'completed', result: { type: 'doubleLoss' }, cancellationReason: null, started: true,
  })
  const input = responseWithStageOverrides(2, {
    config: { ...source.config, teams: teams.map((teamId, index) => ({ teamId, seed: index + 1 })) },
    teams: Object.fromEntries(teams.map((teamId, index) => [teamId, {
      seed: index + 1,
      dropped: index < 4,
      hadBye: false,
    }])),
    status: 'finished',
    rounds: [
      { roundNumber: 3, kind: 'winners', matches: [
        doubleLossMatch(1, ['team-a', 'team-b']),
        doubleLossMatch(2, ['team-c', 'team-d']),
      ] },
      { roundNumber: 6, kind: 'losers', matches: [{
        matchId: `${stageId}-lower-final`, stageId, roundNumber: 6, sequence: 1, kind: 'losers',
        participants: ['team-e', 'team-f'], status: 'completed',
        result: { type: 'win', winnerId: 'team-e', loserId: 'team-f', winnerGames: 1, loserGames: 0 },
        cancellationReason: null, started: true,
      }] },
    ],
    standings,
    doubleState: {
      phase: 'finished', winnersPool: [null, null], losersWaiting: [], lowerPool: ['team-e'],
      winnersChampion: null, losersChampion: 'team-e', grandFinalWinner: 'team-e',
    },
  })
  const parsed = parseCompetitionTournamentViewResponse(input, 'competition-a')
  expect(doubleEliminationVacancySummary(parsed.stages[2]!))
    .toBe('敗部冠軍沿總決賽空位取得冠軍，亞軍與季軍空缺')
  expect(parsed.stages[2]!.standings.some(standing => standing.rank === 2)).toBe(false)
})

test('does not announce a vacancy for an ordinary finished double-elimination final', () => {
  const stage = response().projection.public.stages[2]!
  const finished = parseCompetitionTournamentViewResponse(responseWithStageOverrides(2, {
    status: 'finished',
    doubleState: { ...stage.doubleState!, phase: 'finished', grandFinalWinner: 'team-a' },
  }), 'competition-a').stages[2]!
  expect(doubleEliminationVacancySummary(finished)).toBeNull()
})

test('explains a semifinal double loss with a surviving champion and bronze bye', () => {
  const source = response().projection.public.stages[0]!
  const teams = ['team-a', 'team-b', 'team-c', 'team-d']
  const standings = teams.map((teamId, index) => ({
    teamId,
    matchPoints: '0', wins: 0, draws: 0, losses: index < 2 ? 1 : 0,
    gameWins: 0, gameLosses: 0, netGameWins: 0,
    opponentWinRateBasisPoints: null, opponentWinScore: '0', opponentLossScore: '0',
    dropped: index < 2, rank: index === 2 ? 1 : index === 3 ? 3 : null,
  }))
  const parsed = parseCompetitionTournamentViewResponse(responseWithStageOverrides(0, {
    config: { ...source.config, teams: teams.map((teamId, index) => ({ teamId, seed: index + 1 })) },
    teams: Object.fromEntries(teams.map((teamId, index) => [teamId, {
      seed: index + 1,
      dropped: index < 2,
      hadBye: false,
    }])),
    status: 'finished',
    rounds: [{
      roundNumber: 1, kind: 'main', matches: [
        { matchId: 'single-semi-dl', stageId: source.config.stageId, roundNumber: 1,
          sequence: 1, kind: 'main', participants: ['team-a', 'team-b'], status: 'completed',
          result: { type: 'doubleLoss' }, cancellationReason: null, started: true },
        { matchId: 'single-semi-win', stageId: source.config.stageId, roundNumber: 1,
          sequence: 2, kind: 'main', participants: ['team-c', 'team-d'], status: 'completed',
          result: { type: 'win', winnerId: 'team-c', loserId: 'team-d', winnerGames: 1, loserGames: 0 },
          cancellationReason: null, started: true },
      ],
    }],
    standings,
  }), 'competition-a')
  expect(singleEliminationVacancySummary(parsed.stages[0]!))
    .toBe('半決賽雙負後，唯一勝者輪空取得冠軍；正常敗者輪空取得季軍，亞軍空缺')
})

test('recognizes a true final double loss while keeping the completed bronze result', () => {
  const source = response().projection.public.stages[0]!
  const stageId = source.config.stageId
  const teams = ['team-a', 'team-b', 'team-c', 'team-d']
  const standings = teams.map((teamId, index) => ({
    teamId,
    matchPoints: '0', wins: 0, draws: 0, losses: index < 2 ? 1 : 0,
    gameWins: 0, gameLosses: 0, netGameWins: 0,
    opponentWinRateBasisPoints: null, opponentWinScore: '0', opponentLossScore: '0',
    dropped: index < 2, rank: index === 2 ? 3 : index === 3 ? 4 : null,
  }))
  const parsed = parseCompetitionTournamentViewResponse(responseWithStageOverrides(0, {
    config: { ...source.config, teams: teams.map((teamId, index) => ({ teamId, seed: index + 1 })) },
    teams: Object.fromEntries(teams.map((teamId, index) => [teamId, {
      seed: index + 1,
      dropped: index < 2,
      hadBye: false,
    }])),
    status: 'finished',
    rounds: [{ roundNumber: 2, kind: 'main', matches: [
      { matchId: 'single-final-dl', stageId, roundNumber: 2, sequence: 1, kind: 'main',
        participants: ['team-a', 'team-b'], status: 'completed', result: { type: 'doubleLoss' },
        cancellationReason: null, started: true },
      { matchId: 'single-bronze', stageId, roundNumber: 2, sequence: 2, kind: 'main', purpose: 'bronze',
        participants: ['team-c', 'team-d'], status: 'completed',
        result: { type: 'win', winnerId: 'team-c', loserId: 'team-d', winnerGames: 1, loserGames: 0 },
        cancellationReason: null, started: true },
    ] }],
    standings,
  }), 'competition-a')
  expect(singleEliminationDoubleLossFinal(parsed.stages[0]!)?.matchId).toBe('single-final-dl')
})

test('recognizes a true final double loss when the sole semifinal bronze candidate advanced by bye', () => {
  const source = response().projection.public.stages[0]!
  const stageId = source.config.stageId
  const teams = Array.from({ length: 8 }, (_, index) => `team-${String.fromCharCode(97 + index)}`)
  const match = (
    roundNumber: number,
    sequence: number,
    participants: string[],
    result: Record<string, unknown>,
  ) => ({
    matchId: `${stageId}-r${roundNumber}-m${sequence}`, stageId, roundNumber, sequence, kind: 'main',
    participants, status: 'completed', result, cancellationReason: null, started: result.type !== 'bye',
  })
  const standings = teams.map((teamId, index) => ({
    teamId, matchPoints: '0', wins: 0, draws: 0, losses: index === 2 || index === 4 || index === 5 || index === 6 ? 1 : 0,
    gameWins: 0, gameLosses: 0, netGameWins: 0,
    opponentWinRateBasisPoints: null, opponentWinScore: '0', opponentLossScore: '0',
    dropped: index === 0 || index === 1 || index === 3 || index === 7,
    rank: index === 2 ? 3 : index === 4 || index === 5 || index === 6 ? 5 : null,
  }))
  const parsed = parseCompetitionTournamentViewResponse(responseWithStageOverrides(0, {
    config: { ...source.config, teams: teams.map((teamId, index) => ({ teamId, seed: index + 1 })) },
    teams: Object.fromEntries(teams.map((teamId, index) => [teamId, {
      seed: index + 1, dropped: false, hadBye: index === 3,
    }])),
    status: 'finished',
    rounds: [
      { roundNumber: 1, kind: 'main', matches: [
        match(1, 1, [teams[0]!, teams[7]!], { type: 'doubleLoss' }),
        match(1, 2, [teams[3]!, teams[4]!], { type: 'win', winnerId: teams[3], loserId: teams[4], winnerGames: 1, loserGames: 0 }),
        match(1, 3, [teams[1]!, teams[6]!], { type: 'win', winnerId: teams[1], loserId: teams[6], winnerGames: 1, loserGames: 0 }),
        match(1, 4, [teams[2]!, teams[5]!], { type: 'win', winnerId: teams[2], loserId: teams[5], winnerGames: 1, loserGames: 0 }),
      ] },
      { roundNumber: 2, kind: 'main', matches: [
        match(2, 1, [teams[3]!], { type: 'bye', teamId: teams[3] }),
        match(2, 2, [teams[1]!, teams[2]!], { type: 'win', winnerId: teams[1], loserId: teams[2], winnerGames: 1, loserGames: 0 }),
      ] },
      { roundNumber: 3, kind: 'main', matches: [match(3, 1, [teams[3]!, teams[1]!], { type: 'doubleLoss' })] },
    ],
    standings,
  }), 'competition-a')
  const stage = parsed.stages[0]!
  expect(singleEliminationDoubleLossFinal(stage)?.matchId).toBe(`${stageId}-r3-m1`)
  expect(singleEliminationVacancySummary(stage))
    .toBe('決賽雙負後，唯一合格半決賽敗隊沿原季軍空位取得第三名；冠亞軍空缺')
  expect(stage.standings.find(standing => standing.teamId === teams[2])?.rank).toBe(3)
})

test('公開賽程顯示報到缺席原因與判負隊伍，且不把它標成已開局', () => {
  const noShowInput = responseWithMatchOverrides(0, {
    status: 'completed',
    started: false,
    result: { type: 'noShow', absentTeams: ['team-a'], winnerId: 'team-b' },
  })
  const parsed = parseCompetitionTournamentViewResponse(noShowInput, 'competition-a')
  expect(parsed).not.toBeNull()
  const result = parsed?.stages[0]?.rounds[0]?.matches[0]?.result ?? null
  expect(result).toEqual({ type: 'noShow', absentTeams: ['team-a'], winnerId: 'team-b' })
  expect(tournamentMatchResultLabel(result, teamId => teamId === 'team-a' ? '種子 1' : '種子 2'))
    .toBe('種子 1 因賽前未完成報到判負；種子 2 勝出')

  const doubleNoShowInput = responseWithMatchOverrides(0, {
    status: 'completed',
    started: false,
    result: { type: 'noShow', absentTeams: ['team-a', 'team-b'], winnerId: null },
  })
  const doubleNoShow = parseCompetitionTournamentViewResponse(doubleNoShowInput, 'competition-a')
    ?.stages[0]?.rounds[0]?.matches[0]?.result ?? null
  expect(tournamentMatchResultLabel(doubleNoShow, teamId => teamId === 'team-a' ? '種子 1' : '種子 2'))
    .toBe('雙方因賽前未完成報到判負（種子 1、種子 2）')

  expect(() => parseCompetitionTournamentViewResponse(responseWithMatchOverrides(0, {
    status: 'completed',
    started: true,
    result: { type: 'noShow', absentTeams: ['team-a'], winnerId: 'team-b' },
  }), 'competition-a')).toThrow()
})

test('projects a Swiss round without double-loss exits and marks both teams as exited', () => {
  const base = stageFixture('swiss', 'swiss-exit-stage')
  const teamIds = ['team-a', 'team-b', 'team-c', 'team-d', 'team-e', 'team-f']
  const exitedTeamIds = new Set(['team-a', 'team-b'])
  const match = (
    roundNumber: number,
    sequence: number,
    participants: string[],
    status: 'completed' | 'pending',
    result: Record<string, unknown> | null,
  ) => ({
    matchId: `swiss-exit-stage-r${roundNumber}-m${sequence}`,
    stageId: 'swiss-exit-stage',
    roundNumber,
    sequence,
    kind: 'main',
    participants,
    status,
    result,
    cancellationReason: null,
    started: status === 'completed',
  })
  const stage = {
    ...base,
    config: {
      ...base.config,
      teams: teamIds.map((teamId, index) => ({ teamId, seed: index + 1 })),
    },
    teams: Object.fromEntries(teamIds.map((teamId, index) => [teamId, {
      seed: index + 1,
      dropped: false,
      hadBye: false,
    }])),
    rounds: [
      {
        roundNumber: 1,
        kind: 'main',
        matches: [
          match(1, 1, ['team-a', 'team-b'], 'completed', { type: 'doubleLoss' }),
          match(1, 2, ['team-c', 'team-d'], 'completed', {
            type: 'win', winnerId: 'team-c', loserId: 'team-d', winnerGames: 1, loserGames: 0,
          }),
          match(1, 3, ['team-e', 'team-f'], 'completed', {
            type: 'win', winnerId: 'team-e', loserId: 'team-f', winnerGames: 1, loserGames: 0,
          }),
        ],
      },
      {
        roundNumber: 2,
        kind: 'main',
        matches: [
          match(2, 1, ['team-c', 'team-e'], 'pending', null),
          match(2, 2, ['team-d', 'team-f'], 'pending', null),
        ],
      },
    ],
    standings: teamIds.map(teamId => ({
      teamId,
      matchPoints: teamId === 'team-c' || teamId === 'team-e' ? '3' : '0',
      wins: teamId === 'team-c' || teamId === 'team-e' ? 1 : 0,
      draws: 0,
      losses: exitedTeamIds.has(teamId) || teamId === 'team-d' || teamId === 'team-f' ? 1 : 0,
      gameWins: teamId === 'team-c' || teamId === 'team-e' ? 1 : 0,
      gameLosses: teamId === 'team-d' || teamId === 'team-f' ? 1 : 0,
      netGameWins: 0,
      opponentWinRateBasisPoints: null,
      opponentWinScore: '0', opponentLossScore: '0', dropped: exitedTeamIds.has(teamId),
      rank: null,
    })),
  }
  const parsed = parseCompetitionTournamentViewResponse({
    competitionId: 'competition-a',
    revision: 10,
    projection: {
      ok: true,
      operation: 'tournamentView',
      public: { tournamentId: 'competition-a', stages: [stage] },
    },
  }, 'competition-a')
  const swissStage = parsed.stages[0]!
  const secondRoundParticipants = swissStage.rounds[1]?.matches.flatMap(match => match.participants) ?? []
  expect(swissStage.rounds[1]?.matches).toHaveLength(2)
  expect(secondRoundParticipants).toEqual(['team-c', 'team-e', 'team-d', 'team-f'])
  expect(secondRoundParticipants.some(teamId => exitedTeamIds.has(teamId))).toBe(false)
  for (const teamId of exitedTeamIds) {
    const standing = swissStage.standings.find(row => row.teamId === teamId)
    expect(standing).toMatchObject({ dropped: true, wins: 0, draws: 0, losses: 1 })
  }
  expect(tournamentMatchResultLabel(swissStage.rounds[0]?.matches[0]?.result ?? null))
    .toBe('雙方判負（不推定勝者）')
})

test('rejects mismatched identity, unsafe revisions, unknown fields, and unknown enum states', () => {
  const valid = response()
  expect(() => parseCompetitionTournamentViewResponse(valid, 'another-competition')).toThrow()
  expect(() => parseCompetitionTournamentViewResponse({ ...valid, revision: 1.5 }, 'competition-a')).toThrow()
  const wrongPublicId = response()
  wrongPublicId.projection.public.tournamentId = 'another-competition'
  expect(() => parseCompetitionTournamentViewResponse(wrongPublicId, 'competition-a')).toThrow()
  expect(() => parseCompetitionTournamentViewResponse({ ...valid, privateHistory: ['secret'] }, 'competition-a')).toThrow()
  expect(() => parseCompetitionTournamentViewResponse({ ...valid, [Symbol('hidden')]: true }, 'competition-a')).toThrow()
  expect(() => parseCompetitionTournamentViewResponse(
    responseWithStageOverrides(0, { status: 'finished-ish' }), 'competition-a',
  )).toThrow()
  expect(() => parseCompetitionTournamentViewResponse(
    responseWithStageOverrides(0, { privateRoster: ['member-a'] }), 'competition-a',
  )).toThrow()
  expect(() => parseCompetitionTournamentViewResponse(
    responseWithMatchOverrides(0, { status: 'queued' }), 'competition-a',
  )).toThrow()
  expect(() => parseCompetitionTournamentViewResponse(
    responseWithMatchOverrides(0, { result: { type: 'winner', winnerId: 'team-a' } }), 'competition-a',
  )).toThrow()
  expect(() => parseCompetitionTournamentViewResponse(
    responseWithMatchOverrides(0, { privateRoster: ['member-a'] }), 'competition-a',
  )).toThrow()
  expect(() => parseCompetitionTournamentViewResponse(responseWithStandingOverrides(0, { rank: '1' }), 'competition-a'))
    .toThrow()
  const singleConfig = response().projection.public.stages[0]?.config
  if (!singleConfig) throw new Error('missing single-elimination fixture config')
  expect(() => parseCompetitionTournamentViewResponse(responseWithStageOverrides(0, {
    config: { ...singleConfig, format: 'unknownFormat' },
  }), 'competition-a')).toThrow()
  expect(() => parseCompetitionTournamentViewResponse(responseWithMatchOverrides(0, {
    stageId: 'another-stage',
  }), 'competition-a')).toThrow()
})

test('rejects unsafe Swiss policy numbers and preserves only valid explicit display labels', () => {
  const swissConfig = response().projection.public.stages[3]?.config
  const singleConfig = response().projection.public.stages[0]?.config
  if (!swissConfig || !singleConfig) throw new Error('missing tournament config fixture')
  const invalidPolicy = responseWithStageOverrides(3, {
    config: {
      ...swissConfig,
      swissPolicy: {
        matchPoints: { win: 3, draw: 1, loss: 0, bye: 3 },
        drawCredit: { numerator: 1, denominator: 2 },
        opponentWinRateFloorPercent: '25',
        opponentWinRateCapPercent: 75,
        excludeByesFromOpponentWinRate: true,
        tieBreakers: ['matchPoints'],
      },
    },
  })
  expect(() => parseCompetitionTournamentViewResponse(invalidPolicy, 'competition-a')).toThrow()
  expect(() => parseCompetitionTournamentViewResponse(responseWithStageOverrides(0, {
    config: { ...singleConfig, swissPolicy: undefined },
  }), 'competition-a')).toThrow()
  expect(tournamentFormatLabel('doubleElimination')).toBe('雙敗淘汰')
  expect(tournamentTimeLimitLabel(null)).toBe('不限時')
  expect(tournamentTimeLimitLabel(1_250)).toBe('1,250 毫秒')
})

test('maps not-ready, forbidden, and malformed responses to fixed Chinese messages', () => {
  expect(presentCompetitionTournamentViewError({
    statusCode: 409,
    message: 'private response detail',
    data: { code: 'notReady', message: 'private response detail' },
  })).toEqual({ kind: 'notReady', message: '目前尚未有可顯示賽程。' })
  expect(presentCompetitionTournamentViewError({
    statusCode: 403,
    message: 'private response detail',
  })).toEqual({ kind: 'forbidden', message: '此賽事目前沒有可公開查閱的賽程。' })
  expect(presentCompetitionTournamentViewError({
    statusCode: 503,
    message: 'private response detail',
  }).message).not.toContain('private response detail')
})

test('recognizes ofetch accessor status and the actual nested H3 error data without exposing raw errors', () => {
  // 安裝的 ofetch 以 getter 提供 statusCode/data；H3 HTTP JSON 將 code 放在 data.data。
  const fetchError = new Error('private HTTP request detail')
  Object.defineProperties(fetchError, {
    statusCode: { get: () => 409 },
    data: { get: () => ({ statusCode: 409, data: { code: 'notReady', message: 'private checkpoint' } }) },
  })
  expect(presentCompetitionTournamentViewError(fetchError)).toEqual({
    kind: 'notReady', message: '目前尚未有可顯示賽程。',
  })
  const forbidden = new Error('private ACL detail')
  Object.defineProperty(forbidden, 'statusCode', { get: () => 403 })
  expect(presentCompetitionTournamentViewError(forbidden)).toEqual({
    kind: 'forbidden', message: '此賽事目前沒有可公開查閱的賽程。',
  })
  const badGetter = new Error('private getter detail')
  Object.defineProperty(badGetter, 'statusCode', { get: () => { throw new Error('private getter throws') } })
  expect(presentCompetitionTournamentViewError(badGetter).message).toBe('目前無法查詢賽程，請稍後重試。')
})

test('accepts actual serialized scheduler fields but rejects malformed or private checkpoint extras', () => {
  // 合法對照先證明完整 envelope 可通過，以下只改欲驗證的 checkpoint 欄位。
  expect(parseCompetitionTournamentViewResponse(response(), 'competition-a').stages).toHaveLength(4)
  const doubleStage = response().projection.public.stages[2]
  if (!doubleStage?.doubleState) throw new Error('missing serialized double-stage fixture')
  for (const overrides of [
    { teams: undefined },
    { teams: { ...doubleStage.teams, 'team-a': { seed: 1, dropped: false, hadBye: 'false' } } },
    { teams: { ...doubleStage.teams, 'team-a': { seed: 2, dropped: false, hadBye: false } } },
    { teams: { ...doubleStage.teams, 'team-a': { ...doubleStage.teams['team-a'], cards: ['secret'] } } },
    { doubleState: null },
    { doubleState: { ...doubleStage.doubleState, phase: 'unknownPhase' } },
    { doubleState: { ...doubleStage.doubleState, winnersPool: ['team-a', 'outsider-team'] } },
    { doubleState: { ...doubleStage.doubleState, winnersPool: ['team-a', 'team-a'] } },
    { doubleState: { ...doubleStage.doubleState, winnersChampion: 'outsider-team' } },
    { doubleState: { ...doubleStage.doubleState, privateRoster: ['member-a'] } },
  ]) {
    expect(() => parseCompetitionTournamentViewResponse(responseWithStageOverrides(2, overrides), 'competition-a')).toThrow()
  }
  expect(() => parseCompetitionTournamentViewResponse(responseWithStageOverrides(0, {
    doubleState: doubleStage.doubleState,
  }), 'competition-a')).toThrow()
})

test('accepts phase-specific double-elimination slot widths and keeps them inside the checkpoint', () => {
  const source = response().projection.public.stages[2]!
  if (!source.doubleState) throw new Error('missing double-elimination state fixture')
  const winnersState = { ...source.doubleState, winnersOutputSlotCount: 1 }
  const winners = parseCompetitionTournamentViewResponse(responseWithStageOverrides(2, {
    doubleState: winnersState,
  }), 'competition-a')
  expect(winners.stages[2]).not.toHaveProperty('doubleState')

  const lowerState = { ...source.doubleState, phase: 'losers', lowerOutputSlotCount: 1 }
  const lower = parseCompetitionTournamentViewResponse(responseWithStageOverrides(2, {
    doubleState: lowerState,
  }), 'competition-a')
  expect(lower.stages[2]).not.toHaveProperty('doubleState')

  const legacyState = { ...source.doubleState }
  const legacy = parseCompetitionTournamentViewResponse(responseWithStageOverrides(2, {
    doubleState: legacyState,
  }), 'competition-a')
  expect(legacy.stages[2]).not.toHaveProperty('doubleState')

  const finishedState = {
    ...source.doubleState,
    phase: 'finished',
    winnersChampion: 'team-a',
    grandFinalWinner: 'team-a',
  }
  expect(parseCompetitionTournamentViewResponse(responseWithStageOverrides(2, {
    doubleState: finishedState,
  }), 'competition-a').stages[2]).not.toHaveProperty('doubleState')

  for (const doubleState of [
    { ...winnersState, winnersOutputSlotCount: '1' },
    { ...winnersState, winnersOutputSlotCount: 0 },
    { ...winnersState, winnersOutputSlotCount: 2 },
    { ...lowerState, lowerOutputSlotCount: Number.MAX_SAFE_INTEGER + 1 },
    { ...winnersState, lowerOutputSlotCount: 1 },
    { ...lowerState, winnersOutputSlotCount: 1 },
    { ...source.doubleState, unrecognizedSlotWidth: 1 },
  ]) {
    expect(() => parseCompetitionTournamentViewResponse(responseWithStageOverrides(2, {
      doubleState,
    }), 'competition-a')).toThrow()
  }
})

test('guards consumed nested numbers and identity without coercion and copies public values', () => {
  const base = response()
  const singleStage = base.projection.public.stages[0]
  if (!singleStage) throw new Error('missing full tournament fixture')
  expect(parseCompetitionTournamentViewResponse(base, 'competition-a').stages).toHaveLength(4)
  for (const overrides of [
    { scheduleGeneration: -1 },
    { config: { ...singleStage.config, randomSeed: Number.MAX_SAFE_INTEGER + 1 } },
    { config: { ...singleStage.config, clockConfig: { ...singleStage.config.clockConfig, mainTimeMs: '300000' } } },
    { config: { ...singleStage.config, matchFormat: { type: 'firstToWins', value: 0 } } },
    { config: { ...singleStage.config, teams: [{ teamId: 'team-a', seed: 1 }, { teamId: 'team-b', seed: 1 }] } },
    { standings: singleStage.standings.map(standing => ({ ...standing, opponentWinRateBasisPoints: Infinity })) },
  ]) {
    expect(() => parseCompetitionTournamentViewResponse(responseWithStageOverrides(0, overrides), 'competition-a')).toThrow()
  }
  for (const overrides of [
    { roundNumber: 2 },
    { sequence: -1 },
    { participants: ['team-a', 'another-team'] },
    { result: { type: 'win', winnerId: 'team-a', loserId: 'another-team', winnerGames: 2, loserGames: 0 } },
    { started: 'false' },
  ]) {
    expect(() => parseCompetitionTournamentViewResponse(responseWithMatchOverrides(0, overrides), 'competition-a')).toThrow()
  }
  const parsed = parseCompetitionTournamentViewResponse(base, 'competition-a')
  const firstTeam = singleStage.config.teams[0]
  if (!firstTeam) throw new Error('missing configured fixture team')
  firstTeam.teamId = 'changed-after-parse'
  expect(parsed.stages[0]?.config.teams[0]?.teamId).toBe('team-a')
})
