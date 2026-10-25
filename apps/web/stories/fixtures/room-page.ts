import type { GameRoomResponse } from '../../shared/game-room'
import type { PlayableAction, PublicCard, SecretStrategyOption } from '~/types/fewfc'
import { createExampleGameState, exampleCards } from './components'

export type RoomPageScenario = 'actions' | 'discard' | 'initial-pouch' | 'chain' | 'card-choice' | 'pouch-ability'

const formation: PlayableAction = {
  type: 'performFormation', commandRole: 'action', id: 'confluence:imprisoning-array', name: '禁錮法陣', category: 'Spell',
  cards: [1], starSubstitution: null, matchOption: null,
  detail: { consequences: [{ type: 'immediateEffect', certainty: 'guaranteed', effect: {
    type: 'resolveFormationEffect', effect: { type: 'preventOtherPlayersFromActingOrDrawing', durationTurns: 1 },
  } }] },
}

export function createRoomPageFixture(scenario: RoomPageScenario = 'actions') {
  const state = createExampleGameState()
  state.status = 'InProgress'
  state.currentPlayer = 'alice'
  state.phase = 'Action'
  state.environment = 'Wood'
  state.turnNumber = 1
  if (scenario === 'discard') {
    state.discard = exampleCards.slice(0, 2)
    state.lastCompletedTurnDiscards = [{ player: 'bob', card: exampleCards[0]!, turnNumber: 1 }]
  }
  state.hands = [{ player: 'alice', cards: { kind: 'known', cards: exampleCards } }, { player: 'bob', cards: { kind: 'hidden', count: 3 } }]
  const deckCards: PublicCard[] = [
    { id: 21, label: '金 2', element: 'Metal', level: 2, secretStrategies: [] },
    { id: 24, label: '木 3', element: 'Wood', level: 3, secretStrategies: [] },
  ]
  state.playerDecks = [{ player: 'alice', cards: { kind: 'known', cards: deckCards } }, { player: 'bob', cards: { kind: 'hidden', count: 24 } }]
  if (scenario === 'initial-pouch') {
    state.status = 'Preparing'
    state.initialPouchSelection = { remainingPlayers: ['alice', 'bob'] }
  }
  if (scenario === 'chain') {
    const option: SecretStrategyOption = {
      type: 'sheepStealing', sourceCard: 24,
      detail: { consequences: [{ type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'triggerSecretStrategy', effect: 'swapDeckAndDiscard' } }] },
    }
    state.pendingChoice = { visibility: 'visible', choiceId: 1, player: 'alice', reason: { type: 'chain' }, choice: {
      type: 'chain', pouchOwners: ['alice'], deckCards, strategyOptions: [option],
    } }
  }
  if (scenario === 'card-choice') {
    state.pendingChoice = { visibility: 'visible', choiceId: 2, player: 'alice', reason: { type: 'turnDrawDiscard' }, choice: {
      type: 'card', cards: exampleCards, minimum: 1, maximum: 1, canDecline: false,
    } }
  }
  const playableActions: PlayableAction[] = []
  if (scenario === 'pouch-ability') {
    state.pouches = [{ owner: 'alice', card: deckCards[0]! }]
    playableActions.push({ type: 'triggerSecretStrategy', commandRole: 'activeEffect', option: {
      type: 'noInput', sourceCard: 21, strategy: 'GoldenCicada',
      detail: { consequences: [{ type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'triggerSecretStrategy', effect: 'protectTriggeringPlayer' } }] },
    } })
  }
  const response: GameRoomResponse = {
    gameId: 'story-room', state, playableActions, interaction: { canChooseInitialPouch: scenario === 'initial-pouch' },
    metadata: {
      schemaVersion: 5, gameId: 'story-room', gameInstanceId: 'story-game', name: '故事對局', access: 'public', capacity: 2,
      ruleVersion: '5.16', ruleset: 'fewfc-base', enabledRuleModules: [], players: ['alice', 'bob'], observers: [], status: 'Active',
      createdAt: '2026-10-25T00:00:00Z', updatedAt: '2026-10-25T00:00:00Z',
      members: ['alice', 'bob'].map((player, index) => ({ player, userId: index === 0 ? 'player-story' : 'player-bob', displayName: index === 0 ? '小明' : '小華', owner: index === 0, ready: true, connected: true })),
    },
    battleRecord: { preparation: { entries: [] }, turns: [{ turnNumber: 1, title: '第 1 回合・小明', entries: [{ id: 'start', title: '回合開始', summary: '選擇本回合的行動。' }] }] },
  }
  return { response, offeredActions: [formation] }
}
