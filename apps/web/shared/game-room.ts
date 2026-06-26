import type {
  LocalGameResponse,
  PlayerId,
  PlayableFormation,
  PublicGameState,
  RecordedDecision,
} from '../app/types/fewfc'

export type GameRoomStatus = 'Active' | 'Finished'

export interface GameRoomMember {
  userId: string
  player: PlayerId
}

export interface GameRoomMetadata {
  schemaVersion: 1
  gameId: string
  ruleset: 'fewfc-base'
  players: PlayerId[]
  members: GameRoomMember[]
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
  schemaVersion: 2
  sequence: number
  rulesRecord: RecordedDecision[]
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
      actorUserId: string
      players?: PlayerId[]
    }
  | {
      type: 'getState'
      actorUserId: string
    }
  | {
      type: 'submitCommand'
      commandId: string
      actorUserId: string
      action: OnlineGameAction
    }

export interface GameRoomResponse extends Omit<LocalGameResponse, 'record'> {
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
