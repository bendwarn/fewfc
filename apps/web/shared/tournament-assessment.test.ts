import { describe, expect, test } from 'bun:test'
import { parseTournamentAssessment } from './tournament-assessment'

function fixture() {
  return {
    tournament: {
      tournamentId: 'event',
      stages: [{ config: { stageId: 'stage', teams: [{ teamId: 'a' }, { teamId: 'b' }] },
        status: 'finished', standings: [{ teamId: 'a', rank: 3 }, { teamId: 'b', rank: null }] }],
    },
    facts: {
      tournamentId: 'event', ready: true,
      stages: [{ stageId: 'stage', settled: true,
        placements: [{ teamId: 'a', placement: { type: 'ranked', rank: 3 } },
          { teamId: 'b', placement: { type: 'noPlacement' } }],
        vacancies: [{ rank: 1, reason: 'finalDoubleLoss' }],
        explanations: [{ type: 'doubleResetDoubleLoss', thirdPlaceDecided: true }],
      }],
    },
  }
}

describe('tournament assessment wire contract', () => {
  test('accepts facts that cover the exact source in source order', () => {
    const { facts, tournament } = fixture()
    expect(JSON.stringify(parseTournamentAssessment(facts, tournament))).toEqual(JSON.stringify(facts))
  })

  test('rejects foreign identities, missing teams, changed ranks and unknown facts', () => {
    const mutations: Array<(data: ReturnType<typeof fixture>) => void> = [
      data => { data.facts.tournamentId = 'other' },
      data => { data.facts.stages[0]!.stageId = 'other' },
      data => { data.facts.stages[0]!.placements[0]!.teamId = 'b' },
      data => { data.facts.stages[0]!.placements.pop() },
      data => { data.facts.stages[0]!.placements[0]!.placement.rank = 1 },
      data => { data.facts.stages[0]!.vacancies[0]!.rank = 3 },
      data => { data.facts.stages[0]!.explanations[0]!.type = 'unknown' },
      data => { data.facts.ready = false },
      data => { data.tournament.stages[0]!.config.teams.pop() },
    ]
    for (const mutate of mutations) {
      const data = fixture()
      mutate(data)
      expect(parseTournamentAssessment(data.facts, data.tournament)).toBeNull()
    }
  })

  test('does not accept noPlacement while a stage is unresolved', () => {
    const data = fixture()
    data.facts.ready = false
    data.facts.stages[0]!.settled = false
    data.facts.stages[0]!.vacancies = []
    data.facts.stages[0]!.explanations = []
    expect(parseTournamentAssessment(data.facts, data.tournament)).toBeNull()
  })

  test('copies accepted facts and rejects hidden or accessor properties without evaluating them', () => {
    const data = fixture()
    const parsed = parseTournamentAssessment(data.facts, data.tournament)
    data.facts.stages[0]!.placements[0]!.teamId = 'changed'
    expect(parsed?.stages[0]?.placements[0]?.teamId).toBe('a')
    const hidden = fixture()
    Object.defineProperty(hidden.facts.stages[0]!.vacancies, 'secret', { value: 'private' })
    expect(parseTournamentAssessment(hidden.facts, hidden.tournament)).toBeNull()
    const getter = fixture()
    let evaluated = false
    Object.defineProperty(getter.facts.stages[0]!, 'secret', { enumerable: true, get() { evaluated = true; return 'private' } })
    expect(parseTournamentAssessment(getter.facts, getter.tournament)).toBeNull()
    expect(evaluated).toBe(false)
  })

  test('permits distinct explicitly vacant exit ranks and rejects unexplained rank references', () => {
    const data = fixture()
    const stage = data.facts.stages[0]!
    stage.vacancies = [{ rank: 5, reason: 'eliminationDoubleLoss' }, { rank: 9, reason: 'eliminationDoubleLoss' }]
    stage.explanations = [
      { type: 'eliminationPlaceVacant', rank: 5 },
      { type: 'eliminationPlaceVacant', rank: 9 },
    ] as unknown as typeof stage.explanations
    expect(parseTournamentAssessment(data.facts, data.tournament)).not.toBeNull()
    stage.vacancies.pop()
    expect(parseTournamentAssessment(data.facts, data.tournament)).toBeNull()
  })

  test('accepts the bronze double-loss reason with the two explicit places', () => {
    const data = fixture()
    data.tournament.stages[0]!.standings = [{ teamId: 'a', rank: 1 }, { teamId: 'b', rank: 2 }]
    data.facts.stages[0]!.placements = [
      { teamId: 'a', placement: { type: 'ranked', rank: 1 } },
      { teamId: 'b', placement: { type: 'ranked', rank: 2 } },
    ]
    data.facts.stages[0]!.vacancies = [{ rank: 3, reason: 'bronzeDoubleLoss' }, { rank: 4, reason: 'bronzeDoubleLoss' }]
    data.facts.stages[0]!.explanations = [{ type: 'singleBronzeDoubleLoss' }] as unknown as typeof data.facts.stages[0]['explanations']
    expect(parseTournamentAssessment(data.facts, data.tournament)).not.toBeNull()
  })
})
