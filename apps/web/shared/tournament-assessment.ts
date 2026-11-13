import { isExactCompetitionJson } from './competition'

export type TeamPlacement =
  | { type: 'ranked', rank: number }
  | { type: 'noPlacement' }
  | { type: 'unresolved' }

export type PlacementExplanation =
  | { type: 'singleFinalDoubleLoss' | 'singleBronzeDoubleLoss' | 'singleBronzeByeAfterFinalDoubleLoss' | 'singleSemifinalDoubleLossChampionAndBronzeBye' | 'singleBronzeByeAfterVacancy' | 'doubleNoChampion' }
  | { type: 'doubleGrandFinalDoubleLoss' | 'doubleResetDoubleLoss' | 'doubleWinnersChampionBye' | 'doubleLosersChampionBye', thirdPlaceDecided: boolean }
  | { type: 'eliminationPlaceVacant', rank: number }

export interface TournamentStageAssessment {
  stageId: string
  settled: boolean
  placements: Array<{ teamId: string, placement: TeamPlacement }>
  vacancies: Array<{ rank: number, reason: 'noQualifiedParticipants' | 'finalDoubleLoss' | 'bronzeDoubleLoss' | 'eliminationDoubleLoss' | 'vacantFinalPath' }>
  explanations: PlacementExplanation[]
}

export interface TournamentAssessment {
  tournamentId: string
  ready: boolean
  stages: TournamentStageAssessment[]
}

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function exact(value: unknown, keys: string[]): value is Record<string, unknown> {
  if (!record(value)) return false
  const actual = Reflect.ownKeys(value)
  return actual.length === keys.length && keys.every(key => {
    const descriptor = Object.getOwnPropertyDescriptor(value, key)
    return descriptor !== undefined && descriptor.enumerable && 'value' in descriptor
  })
}

function positive(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value > 0
}

const plainExplanations = new Set([
  'singleFinalDoubleLoss', 'singleBronzeDoubleLoss', 'singleBronzeByeAfterFinalDoubleLoss',
  'singleSemifinalDoubleLossChampionAndBronzeBye', 'singleBronzeByeAfterVacancy', 'doubleNoChampion',
])
const thirdPlaceExplanations = new Set([
  'doubleGrandFinalDoubleLoss', 'doubleResetDoubleLoss', 'doubleWinnersChampionBye', 'doubleLosersChampionBye',
])

/** 精確驗證 Rust facts 與同一份原始 Tournament 的身分、順序和排名。 */
export function parseTournamentStageAssessments(input: unknown, sourceStages: unknown): TournamentStageAssessment[] | null {
  if (!Array.isArray(input) || !isExactCompetitionJson(input)
    || !Array.isArray(sourceStages) || input.length !== sourceStages.length) return null
  const result: TournamentStageAssessment[] = []
  const stageIds = new Set<string>()
  for (let index = 0; index < input.length; index += 1) {
    const facts = input[index]
    const source = sourceStages[index]
    if (!exact(facts, ['stageId', 'settled', 'placements', 'vacancies', 'explanations'])
      || !record(source) || !record(source.config) || typeof source.config.stageId !== 'string'
      || !source.config.stageId || facts.stageId !== source.config.stageId || stageIds.has(source.config.stageId)
      || !['active', 'finished'].includes(source.status as string) || !Array.isArray(source.config.teams)
      || typeof facts.settled !== 'boolean'
      || facts.settled && source.status !== 'finished'
      || !Array.isArray(source.standings) || !Array.isArray(facts.placements)
      || facts.placements.length !== source.standings.length
      || !Array.isArray(facts.vacancies) || !Array.isArray(facts.explanations)) return null
    stageIds.add(source.config.stageId)
    const configuredTeamIds = new Set<string>()
    for (const team of source.config.teams) {
      if (!record(team) || typeof team.teamId !== 'string' || !team.teamId || configuredTeamIds.has(team.teamId)) return null
      configuredTeamIds.add(team.teamId)
    }
    if (configuredTeamIds.size !== source.standings.length) return null
    const teamIds = new Set<string>()
    const ranks = new Set<number>()
    for (let rowIndex = 0; rowIndex < facts.placements.length; rowIndex += 1) {
      const fact = facts.placements[rowIndex]
      const standing = source.standings[rowIndex]
      if (!exact(fact, ['teamId', 'placement']) || !record(standing)
        || typeof fact.teamId !== 'string' || !fact.teamId || fact.teamId !== standing.teamId
        || teamIds.has(fact.teamId) || !configuredTeamIds.has(fact.teamId) || !record(fact.placement)) return null
      teamIds.add(fact.teamId)
      const placement = fact.placement
      if (placement.type === 'ranked') {
        if (!exact(placement, ['type', 'rank']) || !positive(placement.rank)
          || placement.rank !== standing.rank) return null
        ranks.add(placement.rank)
      } else if (placement.type === 'noPlacement' || placement.type === 'unresolved') {
        if (!exact(placement, ['type']) || standing.rank !== null
          || (placement.type === 'noPlacement') !== facts.settled) return null
      } else return null
    }
    const vacantRanks = new Set<number>()
    const vacancyReasons = new Map<number, unknown>()
    for (const vacancy of facts.vacancies) {
      if (!facts.settled || !exact(vacancy, ['rank', 'reason']) || !positive(vacancy.rank)
        || ranks.has(vacancy.rank) || vacantRanks.has(vacancy.rank)
        || !['noQualifiedParticipants', 'finalDoubleLoss', 'bronzeDoubleLoss', 'eliminationDoubleLoss', 'vacantFinalPath'].includes(vacancy.reason as string)) return null
      vacantRanks.add(vacancy.rank)
      vacancyReasons.set(vacancy.rank, vacancy.reason)
    }
    const explanationKeys = new Set<string>()
    for (const explanation of facts.explanations) {
      if (!facts.settled || !record(explanation) || typeof explanation.type !== 'string') return null
      let key = explanation.type
      if (plainExplanations.has(explanation.type)) {
        if (!exact(explanation, ['type'])) return null
      } else if (thirdPlaceExplanations.has(explanation.type)) {
        if (!exact(explanation, ['type', 'thirdPlaceDecided']) || typeof explanation.thirdPlaceDecided !== 'boolean') return null
      } else if (explanation.type === 'eliminationPlaceVacant') {
        if (!exact(explanation, ['type', 'rank']) || !positive(explanation.rank)
          || vacancyReasons.get(explanation.rank) !== 'eliminationDoubleLoss') return null
        key = `${explanation.type}:${explanation.rank}`
      } else return null
      if (explanationKeys.has(key)) return null
      explanationKeys.add(key)
    }
    result.push(structuredClone(facts) as unknown as TournamentStageAssessment)
  }
  return result
}

export function parseTournamentAssessment(input: unknown, tournament: unknown): TournamentAssessment | null {
  if (!exact(input, ['tournamentId', 'ready', 'stages']) || !record(tournament)
    || typeof tournament.tournamentId !== 'string' || input.tournamentId !== tournament.tournamentId
    || typeof input.ready !== 'boolean') return null
  const stages = parseTournamentStageAssessments(input.stages, tournament.stages)
  if (!stages || !stages.length || input.ready !== stages.every(stage => stage.settled)) return null
  return { tournamentId: input.tournamentId as string, ready: input.ready, stages }
}
