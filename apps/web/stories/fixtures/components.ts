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
