import type {
  LocalGameResponse,
  PlayerId,
  PlayableFormation,
  PublicGameState,
  RecordedDecision,
} from '../app/types/fewfc'

export type GameRoomStatus = 'Waiting' | 'Active' | 'Finished' | 'Dissolved'
export type GameRoomAccess = 'private' | 'public'
export type GameRoomCapacity = 2 | 4

export interface GameRoomMember {
  userId: string
  displayName: string
  player: PlayerId
  ready: boolean
  connected: boolean
  owner: boolean
}

export interface GameRoomMetadata {
  schemaVersion: 2
  gameId: string
  name: string
  access: GameRoomAccess
  capacity: GameRoomCapacity
  ruleset: 'fewfc-base'
  players: PlayerId[]
  members: GameRoomMember[]
  status: GameRoomStatus
  createdAt: string
  updatedAt: string
}

interface StoredGameRoomMember extends Partial<GameRoomMember> {
  userId: string
  player: PlayerId
  ready: boolean
}

interface StoredGameRoomMetadata extends Omit<
  GameRoomMetadata,
  'schemaVersion' | 'name' | 'capacity' | 'members'
> {
  schemaVersion: 1 | 2
  name?: string
  capacity?: GameRoomCapacity
  members: StoredGameRoomMember[]
}

export function normalizeGameRoomMetadata(
  stored: GameRoomMetadata | StoredGameRoomMetadata,
): GameRoomMetadata {
  const ownerIndex = stored.members.findIndex((member) => member.owner === true)
  const resolvedOwnerIndex = ownerIndex >= 0 ? ownerIndex : 0
  const capacity: GameRoomCapacity = stored.capacity === 4 || stored.players.length === 4 ? 4 : 2

  return {
    schemaVersion: 2,
    gameId: stored.gameId,
    name: stored.name?.trim() || stored.gameId,
    access: stored.access,
    capacity,
    ruleset: stored.ruleset,
    players: stored.players,
    members: stored.members.map((member, index) => {
      const owner = index === resolvedOwnerIndex

      return {
        userId: member.userId,
        displayName: member.displayName?.trim() || member.player,
        player: member.player,
        ready: owner ? false : member.ready,
        connected: member.connected ?? false,
        owner,
      }
    }),
    status: stored.status,
    createdAt: stored.createdAt,
    updatedAt: stored.updatedAt,
  }
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
  schemaVersion: 4
  sequence: number
  firstPlayer: PlayerId
  deckSeed: string
  setup: RulesGameSetup
  rulesRecord: RecordedDecision[]
}

export interface RulesGameSetup {
  players: Array<{
    id: PlayerId
    team: string
  }>
  turnOrder: PlayerId[]
}

export type OnlineGameAction =
  | { type: 'start'; firstPlayer?: PlayerId; deckSeed?: string }
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
      actorName: string
      access?: GameRoomAccess
      capacity?: GameRoomCapacity
      name?: string
    }
  | {
      type: 'joinGame'
      actorUserId: string
      actorName: string
    }
  | {
      type: 'toggleReady'
      actorUserId: string
    }
  | {
      type: 'leaveGame'
      actorUserId: string
    }
  | {
      type: 'removePlayer'
      actorUserId: string
      targetUserId: string
    }
  | {
      type: 'dissolveGame'
      actorUserId: string
    }
  | {
      type: 'startGame'
      actorUserId: string
    }
  | {
      type: 'resetGame'
      actorUserId: string
    }
  | {
      type: 'cancelPendingCommand'
      actorUserId: string
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
  canCancelPendingCommand: boolean
}

export interface RulesEngineResult extends LocalGameResponse {
  playableFormations: PlayableFormation[]
}

export type GameRoomSocketMessage =
  | {
      type: 'roomState'
      data: GameRoomResponse
    }
  | {
      type: 'roomDissolved'
      gameId: string
    }

export interface PlayerNotification {
  id: string
  gameId: string
  kind: 'gameStarted' | 'yourTurn' | 'roomChanged' | 'removed' | 'dissolved'
  message: string
  createdAt: string
}

export type PlayerNotificationSocketMessage =
  | {
      type: 'notification'
      data: PlayerNotification
    }
  | {
      type: 'roomsChanged'
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
    previousTurnFormation: null,
  }
}
