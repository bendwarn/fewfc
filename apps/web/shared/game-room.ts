import type {
  Element,
  LocalGameResponse,
  PlayerId,
  PlayableAction,
  PublicGameState,
  RecordedDecision,
} from '../app/types/fewfc'
export interface PlayerDeckList {
  name: string
  cards: string[]
}

export type GameRoomStatus = 'Waiting' | 'Active' | 'Finished' | 'Dissolved'
export type GameRoomAccess = 'private' | 'public'
export type GameRoomCapacity = 2 | 4
export const AVAILABLE_RULE_MODULES = [
  'star',
  'hero-schools',
  'discard-retrieval',
  'personal-deck',
  'five-directions-legend',
  'spirit',
] as const
export const DEFAULT_RULE_MODULES = [...AVAILABLE_RULE_MODULES]
const SPIRIT_DEPENDENCIES = ['star', 'hero-schools', 'five-directions-legend'] as const

export function normalizeRuleModules(modules: unknown): string[] {
  if (!Array.isArray(modules)) return [...DEFAULT_RULE_MODULES]
  const normalized = AVAILABLE_RULE_MODULES.filter(module => modules.includes(module))
  if (
    normalized.includes('spirit')
    && !SPIRIT_DEPENDENCIES.every(module => normalized.includes(module))
  ) {
    return normalized.filter(module => module !== 'spirit')
  }
  return normalized
}

export interface GameRoomMember {
  userId: string
  displayName: string
  player: PlayerId
  ready: boolean
  connected: boolean
  owner: boolean
}

export interface GameRoomMetadata {
  schemaVersion: 3
  gameId: string
  name: string
  access: GameRoomAccess
  capacity: GameRoomCapacity
  ruleset: 'fewfc-base'
  enabledRuleModules: string[]
  players: PlayerId[]
  members: GameRoomMember[]
  status: GameRoomStatus
  createdAt: string
  updatedAt: string
}

export interface GameRoomInvitation {
  roomCode: string
  inviteToken: string
}

export function invitationCredentialMatches(
  invitation: GameRoomInvitation | undefined,
  credential: Extract<GameRoomRequest, { type: 'joinGame' }>['credential'],
): boolean {
  if (!invitation || !credential?.value) {
    return false
  }

  return credential.type === 'token'
    ? credential.value === invitation.inviteToken
    : credential.value.toUpperCase() === invitation.roomCode
}

interface StoredGameRoomMember extends Partial<GameRoomMember> {
  userId: string
  player: PlayerId
  ready: boolean
}

interface StoredGameRoomMetadata extends Omit<
  GameRoomMetadata,
  'schemaVersion' | 'name' | 'capacity' | 'members' | 'enabledRuleModules'
> {
  schemaVersion: 1 | 2 | 3
  name?: string
  capacity?: GameRoomCapacity
  members: StoredGameRoomMember[]
  enabledRuleModules?: string[]
}

export function normalizeGameRoomMetadata(
  stored: GameRoomMetadata | StoredGameRoomMetadata,
): GameRoomMetadata {
  const ownerIndex = stored.members.findIndex((member) => member.owner === true)
  const resolvedOwnerIndex = ownerIndex >= 0 ? ownerIndex : 0
  const capacity: GameRoomCapacity = stored.capacity === 4 || stored.players.length === 4 ? 4 : 2

  return {
    schemaVersion: 3,
    gameId: stored.gameId,
    name: stored.name?.trim() || stored.gameId,
    access: stored.access,
    capacity,
    ruleset: stored.ruleset,
    enabledRuleModules: stored.enabledRuleModules ?? [],
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
  schemaVersion: 5
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
  enabledRuleModules: string[]
  deckLists: Array<PlayerDeckList & { player: PlayerId }>
  initialHp?: Array<{
    team: string
    hp: number
  }>
}

export type OnlineGameAction =
  | { type: 'start'; firstPlayer?: PlayerId; deckSeed?: string }
  | { type: 'refresh' }
  | { type: 'advanceAutomatic' }
  | { type: 'passAction' }
  | {
      type: 'performFormation'
      player: PlayerId
      formationId: string
      cards: number[]
      starSubstitutionCard?: number
      matchOptionRole?: string
      matchOptionCard?: number
      matchOptionSlots?: number
    }
  | { type: 'changeProfession'; player: PlayerId; professionId: string; cards: number[] }
  | {
      type: 'activateProfessionAbility'
      player: PlayerId
      abilityId: string
      cards: number[]
      targetCard?: number
      declaredElement?: Element
      declaredLevel?: number
    }
  | {
      type: 'useSpiritSkill'
      player: PlayerId
      skill: string
      selectedCard?: number
      declaredLevel?: number
    }
  | { type: 'chooseTurnDiscard'; player: PlayerId; card: number }
  | { type: 'answerEffectChoice'; player: PlayerId; cards: number[] }
  | { type: 'retrievePreviousTurnDiscard'; player: PlayerId }
  | { type: 'playableActions'; player: PlayerId; cards: number[] }

export function requiresPendingCommandDraft(
  action: OnlineGameAction,
  pendingChoiceKind: string | undefined,
): action is Extract<OnlineGameAction, { type: 'performFormation' }> {
  return action.type === 'performFormation' && pendingChoiceKind === 'EffectGenerated'
}

export function continuesPendingCommandDraft(
  pendingChoiceKind: string | undefined,
): boolean {
  return pendingChoiceKind === 'EffectGenerated'
}

export type GameRoomRequest =
  | {
      type: 'createGame'
      gameId: string
      actorUserId: string
      actorName: string
      access?: GameRoomAccess
      capacity?: GameRoomCapacity
      name?: string
      enabledRuleModules?: string[]
      invitation: GameRoomInvitation
    }
  | {
      type: 'joinGame'
      actorUserId: string
      actorName: string
      credential?: {
        type: 'code' | 'token'
        value: string
      }
    }
  | {
      type: 'toggleReady'
      actorUserId: string
      deckList: PlayerDeckList
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
      deckList: PlayerDeckList
    }
  | {
      type: 'updateRuleModules'
      actorUserId: string
      enabledRuleModules: string[]
    }
  | {
      type: 'resetGame'
      actorUserId: string
    }
  | {
      type: 'seedEndgameFixture'
      actorUserId: string
    }
  | {
      type: 'seedHeroSchoolsFixture'
      actorUserId: string
    }
  | {
      type: 'seedSpiritFixture'
      actorUserId: string
      spirit?: 'Metal' | 'Fire'
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
  invitation?: GameRoomInvitation
  lockedDeckName?: string
}

export interface RulesEngineResult extends LocalGameResponse {
  playableActions: PlayableAction[]
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
    enabledRuleModules: [],
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
      hp: players.length === 2 ? 100 : 150,
    })),
    hands: players.map((player) => ({
      player,
      cards: {
        kind: 'hidden',
        count: player === players[0] ? 4 : 5,
      },
    })),
    discard: [],
    playerDecks: [],
    playerDiscards: [],
    coveredPassives: [],
    counterEffects: [],
    pendingChoice: null,
    shields: [],
    statuses: [],
    environment: null,
    teamStars: [],
    starHistories: [],
    fiveStarAlignment: null,
    professions: [],
    professionCatalog: [],
    preparedProfessionAbilities: [],
    spirits: [],
    previousTurnFormation: null,
  }
}
