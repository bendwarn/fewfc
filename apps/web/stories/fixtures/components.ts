import { emptyPublicState } from '#shared/game-room'
import type { PublicCard, PublicGameState } from '~/types/fewfc'

export const exampleCards: PublicCard[] = [
  { id: 1, label: '火 3', element: 'Fire', level: 3, secretStrategies: [] },
  { id: 2, label: '水 2', element: 'Water', level: 2, secretStrategies: [] },
  { id: 3, label: '木 1', element: 'Wood', level: 1, secretStrategies: [] },
]

export function createExampleGameState(): PublicGameState {
  return emptyPublicState(['alice', 'bob'])
}

export function createFourPlayerGameState(): PublicGameState {
  const players = ['alice', 'bob', 'carol', 'dave']
  const state = emptyPublicState(players)
  state.players = [
    { id: 'alice', team: 'sun' },
    { id: 'bob', team: 'moon' },
    { id: 'carol', team: 'sun' },
    { id: 'dave', team: 'moon' },
  ]
  state.hp = [{ team: 'sun', hp: 150 }, { team: 'moon', hp: 132 }]
  state.initialHp = [{ team: 'sun', hp: 150 }, { team: 'moon', hp: 150 }]
  state.hands = players.map((player, index) => ({
    player,
    cards: index === 0
      ? { kind: 'known', cards: exampleCards.slice(0, 2) }
      : { kind: 'hidden', count: 5 },
  }))
  state.currentPlayer = 'bob'
  state.phase = 'Action'
  state.environment = 'Fire'
  return state
}
