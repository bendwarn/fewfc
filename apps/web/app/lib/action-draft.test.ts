import assert from 'node:assert/strict'
import { test } from 'node:test'
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
    preparedProfessionAbilities: [],
    spirits: [],
    previousTurnFormation: null,
    ...overrides,
  }
}

test('Action Draft selection is local and toggles independently of Pending Choice', () => {
  assert.deepEqual(toggleActionDraftCard([], 7), [7])
  assert.deepEqual(toggleActionDraftCard([7], 7), [])
})

test('Action Draft survives equivalent refreshes but clears when canonical context advances', () => {
  assert.deepEqual(reconcileActionDraft([7], state(), state()), [7])
  assert.deepEqual(reconcileActionDraft([7], state(), state({ turnNumber: 2 })), [])
  assert.deepEqual(reconcileActionDraft([7], state(), state({ currentPlayer: 'bob' })), [])
})
