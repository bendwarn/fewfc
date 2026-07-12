import { expect, test } from 'bun:test'
import type { PublicGameState } from '../types/fewfc'
import { reconcileActionDraft, toggleActionDraftCard } from './action-draft'

function state(overrides: Partial<PublicGameState> = {}): PublicGameState {
  return {
    enabledRuleModules: [],
    status: 'InProgress',
    turnNumber: 1,
    phase: 'Main',
    currentPlayer: 'alice',
    players: [],
    turnOrder: ['alice', 'bob'],
    hp: [],
    hands: [],
    discard: [],
    playerDecks: [],
    playerDiscards: [],
    pouches: [],
    preparationPlayer: null,
    coveredPassives: [],
    counterEffects: [],
    pendingChoice: null,
    pendingRandomness: null,
    shields: [],
    statuses: [],
    jianghuStates: [],
    limitedUses: [],
    confluenceCardObligations: [],
    scheduledEchoes: [],
    flowStates: [],
    formationSuppressions: [],
    scheduledPlantEarth: [],
    environment: null,
    teamStars: [],
    starHistories: [],
    fiveStarAlignment: null,
    professions: [],
    professionCatalog: [],
    cardInterpretations: [],
    spirits: [],
    previousTurnFormation: null,
    ...overrides,
  }
}

test('Action Draft selection is local and toggles independently of Pending Choice', () => {
  expect(toggleActionDraftCard([], 7)).toEqual([7])
  expect(toggleActionDraftCard([7], 7)).toEqual([])
})

test('Action Draft survives equivalent refreshes but clears when canonical context advances', () => {
  expect(reconcileActionDraft([7], state(), state())).toEqual([7])
  expect(reconcileActionDraft([7], state(), state({ turnNumber: 2 }))).toEqual([])
  expect(reconcileActionDraft([7], state(), state({ currentPlayer: 'bob' }))).toEqual([])
})
