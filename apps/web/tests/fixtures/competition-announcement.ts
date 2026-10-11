import type { CompetitionAnnouncement, CompetitionAnnouncementPlan } from '../../shared/competition'
import type { CompetitionAnnouncementDraft } from '../../shared/competition-room'

const announcementPlan: CompetitionAnnouncementPlan = {
  initialSeedPolicy: 'randomDrawAfterRegistrationDeadline',
  swissPairingPolicy: 'publishedRulesWithPersistedRandomSeed',
  rankingPolicy: 'matchPointsThenOpponentWinsThenOpponentLosses',
  exactRankingTiePolicy: 'organizerOrUnconflictedRefereeWithPublicReason',
  stages: [{
    stageId: 'stage-1',
    format: 'roundRobin',
    matchFormat: { type: 'fixedGames', value: 1 },
    clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
    swissRounds: null,
    swissPolicy: null,
    qualification: { type: 'allEligibleTeams' },
  }],
}

const announcementDraft: CompetitionAnnouncementDraft = {
  ruleVersion: '5.17',
  ruleset: 'base',
  enabledRuleModules: [],
  teamSize: 2,
  registrationMode: 'public',
  personalDecks: 'disabled',
  registrationDeadline: 60_000,
  checkInDeadline: 120_000,
  plan: announcementPlan,
}

const announcement: CompetitionAnnouncement = {
  ...announcementDraft,
  organizer: 'organizer-1',
  initialHp: 20,
}

export function createCompetitionAnnouncementFixture(
  overrides: Partial<CompetitionAnnouncement> = {},
): CompetitionAnnouncement {
  return structuredClone({ ...announcement, ...overrides })
}

export function createCompetitionAnnouncementDraftFixture(
  overrides: Partial<CompetitionAnnouncementDraft> = {},
): CompetitionAnnouncementDraft {
  return structuredClone({ ...announcementDraft, ...overrides })
}

export function createCompetitionAnnouncementPlanFixture(): CompetitionAnnouncementPlan {
  return structuredClone(announcementPlan)
}
