import type {
  LocalGameResponse,
  PlayerId,
  PlayableFormation,
  PublicGameEvent,
  PublicGameState,
  RecordedDecision,
  ViewerId,
} from '../app/types/fewfc'

export type GameRoomStatus = 'Active' | 'Finished'

export interface GameRoomMetadata {
  schemaVersion: 1
  gameId: string
  ruleset: 'fewfc-base'
  players: PlayerId[]
  status: GameRoomStatus
  createdAt: string
  updatedAt: string
}

export interface StoredGameEvent {
  sequence: number
  type: string
  commandId?: string
  actor?: PlayerId
  payload: unknown
  createdAt: string
}

export interface GameRoomSnapshot {
  sequence: number
  rulesRecord: RecordedDecision[]
  publicState: PublicGameState
  publicEvents: PublicGameEvent[]
}

export type OnlineGameAction =
  | { type: 'start' }
  | { type: 'refresh' }
  | { type: 'advanceAutomatic' }
  | { type: 'passAction' }
  | { type: 'performFormation'; player: PlayerId; formationId: string; cards: number[] }
  | { type: 'chooseTurnDiscard'; player: PlayerId; card: number }
  | { type: 'answerEffectChoice'; player: PlayerId; cards: number[] }
  | { type: 'playableFormations'; player: PlayerId; cards: number[] }

export type GameRoomRequest =
  | {
      type: 'createGame'
      gameId: string
      viewer?: ViewerId
      players?: PlayerId[]
    }
  | {
      type: 'getState'
      viewer?: ViewerId
    }
  | {
      type: 'submitCommand'
      commandId: string
      viewer?: ViewerId
      action: OnlineGameAction
    }

export interface GameRoomResponse extends LocalGameResponse {
  gameId: string
  metadata: GameRoomMetadata
}

export interface RulesEngineResult extends LocalGameResponse {
  playableFormations: PlayableFormation[]
}

export function emptyPublicState(players: PlayerId[] = ['alice', 'bob']): PublicGameState {
  return {
    status: 'InProgress',
    turnNumber: 1,
    phase: 'TurnStart',
    currentPlayer: null,
    players: players.map((player) => ({
      id: player,
      team: `team:${player}`,
    })),
    turnOrder: players,
    hp: players.map((player) => ({
      team: `team:${player}`,
      hp: 20,
    })),
    hands: players.map((player) => ({
      player,
      cards: {
        kind: 'hidden',
        count: player === players[0] ? 4 : 5,
      },
    })),
    discard: [],
    coveredPassives: [],
    pendingChoice: null,
    shields: [],
    statuses: [],
  }
}
