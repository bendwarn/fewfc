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
    },
  }
}

