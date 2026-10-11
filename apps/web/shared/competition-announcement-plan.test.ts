import { expect, test } from 'bun:test'
import {
  buildCompetitionAnnouncementPlan,
  parseCompetitionAnnouncementPlan,
  type CompetitionAnnouncementStageDraft,
} from './competition-announcement-plan'

function firstStage(overrides: Partial<CompetitionAnnouncementStageDraft> = {}): CompetitionAnnouncementStageDraft {
  return {
    stageId: 'stage-1', format: 'roundRobin', matchGames: '3', mainMinutes: '0', byoyomiSeconds: '30',
    swissRounds: '3', swissMatchPoints: { win: '3', draw: '1', loss: '0', bye: '3' },
    swissPolicy: {
      drawCredit: { numerator: '1', denominator: '2' },
      opponentWinRateFloorPercent: { mode: 'none', value: '' },
      opponentWinRateCapPercent: { mode: 'none', value: '' },
      excludeByesFromOpponentWinRate: 'no', tieMode: 'none', tieBreakers: [],
    },
    sourceSlots: [],
    ...overrides,
  }
}

test('builds one immutable announcement plan with minutes, fixed byoyomi options and visible scoring', () => {
  const plan = buildCompetitionAnnouncementPlan([firstStage()])
  expect(plan).toEqual({
    initialSeedPolicy: 'randomDrawAfterRegistrationDeadline',
    swissPairingPolicy: 'publishedRulesWithPersistedRandomSeed',
    rankingPolicy: 'matchPointsThenOpponentWinsThenOpponentLosses',
    exactRankingTiePolicy: 'organizerOrUnconflictedRefereeWithPublicReason',
    stages: [{
      stageId: 'stage-1', format: 'roundRobin', matchFormat: { type: 'fixedGames', value: 3 },
      clockConfig: { mainTimeMs: 0, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
      swissRounds: null, swissPolicy: null, qualification: { type: 'allEligibleTeams' },
    }],
  })
})

test('records cross-stage source slots and complete swiss policy before registration closes', () => {
  const plan = buildCompetitionAnnouncementPlan([
    firstStage({ format: 'swiss', swissRounds: '5', swissMatchPoints: { win: '4', draw: '2', loss: '1', bye: '3' } }),
    firstStage({ stageId: 'stage-2', format: 'singleElimination', matchGames: '2', sourceSlots: [
      { sourceStageId: 'stage-1', sourceType: 'uniqueStandingRank', rank: '1', destinationSeed: '1' },
      { sourceStageId: 'stage-1', sourceType: 'uniqueStandingRank', rank: '2', destinationSeed: '2' },
    ] }),
  ])
  expect(plan.stages[0]).toMatchObject({
    format: 'swiss', swissRounds: 5, swissPolicy: { matchPoints: { win: 4, draw: 2, loss: 1, bye: 3 } },
  })
  expect(plan.stages[1]).toMatchObject({
    format: 'singleElimination', matchFormat: { type: 'firstToWins', value: 2 },
    qualification: { type: 'sourceSlots', slots: [
      { sourceStageId: 'stage-1', source: { type: 'uniqueStandingRank', rank: 1 }, destinationSeed: 1 },
      { sourceStageId: 'stage-1', source: { type: 'uniqueStandingRank', rank: 2 }, destinationSeed: 2 },
    ] },
  })
})

test('rejects unresolved, repeated or incompatible qualification sources and unsupported clocks', () => {
  const invalid = [
    [firstStage({ stageId: 'stage-1', format: 'singleElimination' }), firstStage({
      stageId: 'stage-2', sourceSlots: [{ sourceStageId: 'stage-1', sourceType: 'uniqueStandingRank', rank: '1', destinationSeed: '1' }],
    })],
    [firstStage({}), firstStage({ stageId: 'stage-2', sourceSlots: [
      { sourceStageId: 'stage-1', sourceType: 'uniqueStandingRank', rank: '1', destinationSeed: '1' },
      { sourceStageId: 'stage-1', sourceType: 'uniqueStandingRank', rank: '1', destinationSeed: '2' },
    ] })],
    [firstStage({ byoyomiSeconds: '90' })],
    [firstStage({ mainMinutes: '-1' })],
    [firstStage({ format: 'swiss', swissRounds: '0' })],
  ]
  for (const stages of invalid) expect(() => buildCompetitionAnnouncementPlan(stages)).toThrow()
})

test('rejects values that the Rust u32 announcement contract cannot store', () => {
  expect(() => buildCompetitionAnnouncementPlan([firstStage({
    format: 'swiss', swissRounds: '4294967296',
  })])).toThrow()
  expect(() => buildCompetitionAnnouncementPlan([firstStage({
    format: 'swiss', swissMatchPoints: { win: '3', draw: '1', loss: '4294967296', bye: '3' },
  })])).toThrow()

  const plan = buildCompetitionAnnouncementPlan([firstStage({ format: 'swiss' })])
  expect(parseCompetitionAnnouncementPlan({
    ...plan,
    stages: [{ ...plan.stages[0]!, swissRounds: 4_294_967_296 }],
  })).toBeNull()
})

test('the public plan parser accepts only the declared ranking and qualification schema', () => {
  const plan = buildCompetitionAnnouncementPlan([firstStage()])
  expect(parseCompetitionAnnouncementPlan(plan)).toEqual(plan)
  expect(parseCompetitionAnnouncementPlan({ ...plan, hiddenTieBreaker: 'seedOrder' })).toBeNull()
  expect(parseCompetitionAnnouncementPlan({ ...plan, rankingPolicy: 'pointsThenHealth' })).toBeNull()
  expect(parseCompetitionAnnouncementPlan({
    ...plan,
    stages: [{ ...plan.stages[0], clockConfig: { ...plan.stages[0]!.clockConfig, actionTimeMs: null } }],
  })).toBeNull()
})
