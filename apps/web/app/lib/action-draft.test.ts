import { expect, test } from 'bun:test'
import type { PublicGameState } from '../types/fewfc'
import { reconcileActionDraft, toggleActionDraftCard } from './action-draft'

function state(overrides: Partial<PublicGameState> = {}): PublicGameState {
  return {
    enabledRuleModules: [],
    status: 'InProgress',
    turnNumber: 1,
    phase: 'ActiveEffects',
    currentPlayer: 'alice',
    players: [],
    turnOrder: ['alice', 'bob'],
    hp: [],
    hands: [],
    discard: [],
    playerDecks: [],
    playerDiscards: [],
    pouches: [],
    initialPouchSelection: null,
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
    winnerTeam: null,
    gameConclusion: null,
    terminalResolution: null,
    professions: [],
    professionCatalog: [],
    cardInterpretations: [],
    spirits: [],
    previousTurnFormation: null,
    lastCompletedTurnDiscards: [],
    ...overrides,
  }
}

test('Action Draft selection is local and toggles independently of Pending Choice', () => {
  expect(toggleActionDraftCard([], 7)).toStrictEqual([7])
  expect(toggleActionDraftCard([7], 7)).toStrictEqual([])
})

test('Action Draft survives equivalent refreshes but clears when canonical context advances', () => {
  expect(reconcileActionDraft([7], state(), state())).toStrictEqual([7])
  expect(reconcileActionDraft([7], state(), state({ turnNumber: 2 }))).toStrictEqual([])
  expect(reconcileActionDraft([7], state(), state({ currentPlayer: 'bob' }))).toStrictEqual([])
})
