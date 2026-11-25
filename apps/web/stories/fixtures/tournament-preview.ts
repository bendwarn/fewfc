// 固定的公開 DTO 畫面資料，沿用賽程契約測試的資料形狀。
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

export function tournamentPreview() {
  return {
    competitionId: 'competition-a',
    revision: 9,
    projection: {
      ok: true,
      operation: 'tournamentView',
      public: {
        tournamentId: 'competition-a',
        stages: [stageFixture('roundRobin', 'round-robin-stage')],
      },
      stageFacts: [{
        stageId: 'round-robin-stage', settled: false,
        placements: [
          { teamId: 'team-a', placement: { type: 'unresolved' } },
          { teamId: 'team-b', placement: { type: 'unresolved' } },
        ],
        vacancies: [], explanations: [],
      }],
    },
  }
}

export type TournamentCompletionScenario = 'pending' | 'rankingTie' | 'singleFinal' | 'singleBronze' | 'singleSemifinal' | 'doubleFinal' | 'doubleReset' | 'zeroAndOne' | 'laterActive'

/** 展示層固定 facts；淘汰路徑由 Rust 測試另行驗證。 */
export function tournamentCompletionPreview(scenario: TournamentCompletionScenario) {
  const match = (stageId: string, roundNumber: number, sequence: number, kind: string,
    participants: string[], result: Record<string, unknown>, purpose?: 'bronze') => ({
    matchId: `${stageId}-r${roundNumber}-m${sequence}`, stageId, roundNumber, sequence, kind,
    ...(purpose ? { purpose } : {}), participants, status: 'completed', result,
    cancellationReason: null, started: result.type !== 'bye' && result.type !== 'noShow',
  })
  const win = (winnerId: string, loserId: string) => ({ type: 'win', winnerId, loserId, winnerGames: 1, loserGames: 0 })
  if (scenario === 'rankingTie') {
    const preview = tournamentPreview()
    const stage = stageFixture('roundRobin', 'round-robin-stage')
    return {
      ...preview,
      projection: { ...preview.projection, public: { ...preview.projection.public, stages: [{
        ...stage,
        config: { ...stage.config, matchFormat: { type: 'fixedGames', value: 1 } },
        status: 'finished',
        rounds: [{ roundNumber: 1, kind: 'main', matches: [
          match(stage.config.stageId, 1, 1, 'main', ['team-a', 'team-b'], { type: 'draw', firstGames: 0, secondGames: 0 }),
        ] }],
        standings: stage.standings.map(row => ({ ...row, matchPoints: '1', draws: 1 })),
      }] } },
    }
  }
  const pending: any = stageFixture('singleElimination', 'pending-stage')
  const pendingFact = { stageId: 'pending-stage', settled: false, placements: [
    { teamId: 'team-a', placement: { type: 'unresolved' } },
    { teamId: 'team-b', placement: { type: 'unresolved' } },
  ], vacancies: [], explanations: [] }
  const result: any = {
    competitionId: 'competition-a', revision: 12,
    projection: { ok: true, operation: 'tournamentView', public: { tournamentId: 'competition-a', stages: [] }, stageFacts: [] },
  }
  if (scenario === 'pending') {
    result.projection.public.stages = [pending]
    result.projection.stageFacts = [pendingFact]
    return result
  }
  if (scenario === 'zeroAndOne') {
    const zero: any = stageFixture('singleElimination', 'cancelled-stage')
    Object.assign(zero, { config: { ...zero.config, teams: [] }, status: 'finished', rounds: [], standings: [], teams: {},
      resolution: { type: 'cancelled', reason: 'noQualifiedParticipants' } })
    const one: any = stageFixture('singleElimination', 'sole-winner-stage')
    Object.assign(one, { config: { ...one.config, teams: [one.config.teams[0]] }, status: 'finished', rounds: [],
      standings: [{ ...one.standings[0], rank: 1 }], teams: { 'team-a': one.teams['team-a'] },
      resolution: { type: 'winnerWithoutMatch', teamId: 'team-a' } })
    result.projection.public.stages = [zero, one]
    result.projection.stageFacts = [
      { stageId: 'cancelled-stage', settled: true, placements: [],
        vacancies: [{ rank: 1, reason: 'noQualifiedParticipants' }], explanations: [] },
      { stageId: 'sole-winner-stage', settled: true,
        placements: [{ teamId: 'team-a', placement: { type: 'ranked', rank: 1 } }], vacancies: [], explanations: [] },
    ]
  } else {
    const double = scenario === 'doubleFinal' || scenario === 'doubleReset'
    const stageId = double ? 'double-stage' : 'single-stage'
    const stage: any = stageFixture(double ? 'doubleElimination' : 'singleElimination', stageId)
    const teamIds = ['team-a', 'team-b', 'team-c', 'team-d']
    stage.config.teams = teamIds.map((teamId, index) => ({ teamId, seed: index + 1 }))
    stage.config.matchFormat.value = 1
    stage.teams = Object.fromEntries(teamIds.map((teamId, index) => [teamId, { seed: index + 1, dropped: false, hadBye: false }]))
    stage.status = 'finished'
    const semifinal = scenario === 'singleSemifinal'
    const bronzeDoubleLoss = scenario === 'singleBronze'
    const ranks = semifinal ? [null, null, 1, 3] : bronzeDoubleLoss ? [1, 2, null, null] : [null, null, 3, 4]
    const scores: Array<[number, number, number]> = semifinal ? [[0, 1, 0], [0, 1, 0], [1, 0, 0], [0, 1, 1]]
      : bronzeDoubleLoss ? [[2, 0, 0], [1, 1, 1], [0, 2, 1], [0, 2, 1]]
      : double ? [[2, scenario === 'doubleReset' ? 2 : 1, scenario === 'doubleReset' ? 1 : 0],
          [scenario === 'doubleReset' ? 3 : 2, 2, 1], [1, 2, 2], [0, 2, 2]]
        : [[1, 1, 0], [1, 1, 0], [1, 1, 1], [0, 2, 2]]
    stage.standings = teamIds.map((teamId, index) => {
      const [wins, losses, gameLosses] = scores[index]!
      return { ...stage.standings[0], teamId, matchPoints: String(wins * 3), wins, losses,
        gameWins: wins, gameLosses, netGameWins: wins - gameLosses,
        rank: ranks[index], dropped: bronzeDoubleLoss ? index >= 2 : index < 2 }
    })
    const facts: any = { stageId, settled: true,
      placements: teamIds.map((teamId, index) => ({ teamId, placement: ranks[index] === null
        ? { type: 'noPlacement' } : { type: 'ranked', rank: ranks[index] } })),
      vacancies: [{ rank: 2, reason: semifinal ? 'vacantFinalPath' : 'finalDoubleLoss' }], explanations: [] }
    if (semifinal) {
      stage.rounds = [{ roundNumber: 1, kind: 'main', matches: [
        match(stageId, 1, 1, 'main', ['team-a', 'team-b'], { type: 'doubleLoss' }),
        match(stageId, 1, 2, 'main', ['team-c', 'team-d'], win('team-c', 'team-d')),
      ] }]
      facts.vacancies.push({ rank: 4, reason: 'eliminationDoubleLoss' })
      facts.explanations = [{ type: 'eliminationPlaceVacant', rank: 4 },
        { type: 'singleSemifinalDoubleLossChampionAndBronzeBye' }]
    } else if (!double) {
      stage.rounds = [
        { roundNumber: 1, kind: 'main', matches: [
          match(stageId, 1, 1, 'main', ['team-a', 'team-c'], win('team-a', 'team-c')),
          match(stageId, 1, 2, 'main', ['team-b', 'team-d'], win('team-b', 'team-d')),
        ] },
        { roundNumber: 2, kind: 'main', matches: [
          match(stageId, 2, 1, 'main', ['team-a', 'team-b'], bronzeDoubleLoss ? win('team-a', 'team-b') : { type: 'doubleLoss' }),
          match(stageId, 2, 2, 'main', ['team-c', 'team-d'], bronzeDoubleLoss ? { type: 'doubleLoss' } : win('team-c', 'team-d'), 'bronze'),
        ] },
      ]
      if (bronzeDoubleLoss) {
        facts.vacancies = [{ rank: 3, reason: 'bronzeDoubleLoss' }, { rank: 4, reason: 'bronzeDoubleLoss' }]
        facts.explanations = [{ type: 'singleBronzeDoubleLoss' }]
      } else {
        facts.vacancies.unshift({ rank: 1, reason: 'finalDoubleLoss' })
        facts.explanations = [{ type: 'singleFinalDoubleLoss' }]
      }
    } else {
      const reset = scenario === 'doubleReset'
      stage.rounds = [
        { roundNumber: 1, kind: 'winners', matches: [
          match(stageId, 1, 1, 'winners', ['team-a', 'team-d'], win('team-a', 'team-d')),
          match(stageId, 1, 2, 'winners', ['team-b', 'team-c'], win('team-b', 'team-c')),
        ] },
        { roundNumber: 2, kind: 'losers', matches: [match(stageId, 2, 1, 'losers', ['team-c', 'team-d'], win('team-c', 'team-d'))] },
        { roundNumber: 3, kind: 'winners', matches: [match(stageId, 3, 1, 'winners', ['team-a', 'team-b'], win('team-a', 'team-b'))] },
        { roundNumber: 4, kind: 'losers', matches: [match(stageId, 4, 1, 'losers', ['team-c', 'team-b'], win('team-b', 'team-c'))] },
        { roundNumber: 5, kind: 'grandFinal', matches: [match(stageId, 5, 1, 'grandFinal', ['team-a', 'team-b'], reset ? win('team-b', 'team-a') : { type: 'doubleLoss' })] },
        ...(reset ? [{ roundNumber: 6, kind: 'grandFinalReset', matches: [match(stageId, 6, 1, 'grandFinalReset', ['team-a', 'team-b'], { type: 'doubleLoss' })] }] : []),
      ]
      stage.doubleState = { phase: 'finished', winnersPool: ['team-a'], losersWaiting: [], lowerPool: ['team-b'],
        winnersChampion: 'team-a', losersChampion: 'team-b', grandFinalWinner: null }
      facts.vacancies.unshift({ rank: 1, reason: 'finalDoubleLoss' })
      facts.explanations = [{ type: reset ? 'doubleResetDoubleLoss' : 'doubleGrandFinalDoubleLoss', thirdPlaceDecided: true }]
    }
    result.projection.public.stages = scenario === 'laterActive' ? [stage, pending] : [stage]
    result.projection.stageFacts = scenario === 'laterActive' ? [facts, pendingFact] : [facts]
  }
  if (scenario !== 'laterActive') result.publication = { completedAtTrustedTimeMs: 1_800_000_000_000, lockedDecks: [], replays: [] }
  return result
}

