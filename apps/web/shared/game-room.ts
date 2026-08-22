import type {
  Element,
  ChoiceAnswer,
  LocalGameResponse,
  PlayerId,
  PlayableAction,
  PublicGameState,
  RecordedDecision,
  PassActionReason,
  SecretStrategy,
  StarKind,
} from '../app/types/fewfc'
import type { DevelopmentScenario } from './development-scenarios'
export interface PlayerDeckList {
  name: string
  cards: string[]
}

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
  schemaVersion: 4
  gameId: string
  /** 只有房間等待下一個遊戲實例時才是未定義。 */
  gameInstanceId?: string
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
  schemaVersion: 1 | 2 | 3 | 4
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
    schemaVersion: 4,
    gameId: stored.gameId,
    gameInstanceId: stored.gameInstanceId,
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
  transactionId?: string
  actor?: PlayerId
  payload: unknown
  createdAt: string
}

/**
 * 作用中遊戲實例唯一權威且持久的檢查點。
 * 交易中繼資料特意不包含此記錄的另一份副本。
 */
export interface GameRecord {
  schemaVersion: 6
  gameInstanceId: string
  sequence: number
  firstPlayer: PlayerId
  deckSeed: string
  setup: RulesGameSetup
  rulesRecord: RecordedDecision[]
  finishedAt?: string
}

export interface SavableReplay {
  replayId: string
  sourceGameId: string
  finishedAt: string
}

/** 僅供標準記錄使用的物件：絕不要將它放入瀏覽器房間回應。 */
export interface CompletedReplayDraft extends SavableReplay {
  schemaVersion: 1
  setup: RulesGameSetup
  record: RecordedDecision[]
  players: Array<{ player: PlayerId; displayName: string }>
  originalUserIds: string[]
  roomName: string
  result: unknown
  firstPlayer: PlayerId
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
  | { type: 'passAction'; reason: PassActionReason }
  | { type: 'chooseInitialPouch'; player: PlayerId; card: number }
  | {
      type: 'triggerSecretStrategy'
      player: PlayerId
      strategy: SecretStrategy
      targetPlayer?: PlayerId
      star?: StarKind
      breakStar?: boolean
      discardCard?: number
      deckCards?: number[]
      discardCards?: number[]
    }
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
  | { type: 'answerChoice'; player: PlayerId; choiceId: number; answer: ChoiceAnswer }
  | { type: 'retrievePreviousTurnDiscard'; player: PlayerId }
  | { type: 'playableActions'; player: PlayerId; cards: number[] }

export function isOnlineGameAction(value: unknown): value is OnlineGameAction {
  if (!value || typeof value !== 'object' || !('type' in value)) {
    return false
  }
  return [
    'start',
    'refresh',
    'advanceAutomatic',
    'passAction',
    'chooseInitialPouch',
    'triggerSecretStrategy',
    'performFormation',
    'changeProfession',
    'activateProfessionAbility',
    'useSpiritSkill',
    'answerChoice',
    'retrievePreviousTurnDiscard',
    'playableActions',
  ].includes(String(value.type))
}

export interface TrustedRandomnessRequest {
  requestId: string
  operation:
    | { type: 'deckShuffle'; deck: 'Shared' | { Player: PlayerId } }
    | { type: 'discardShuffle'; pile: 'Shared' | { Player: PlayerId }; placement: 'Bottom' }
  continuation:
    | { type: 'base'; kind: 'turnDraw' }
    | { type: 'spirit'; kind: { deathOmen: { player: PlayerId } } }
    | { type: 'echo'; kind: 'ringingMetalRecycleDiscard' | 'ringingMetalPostSearch' }
    | { type: 'hero'; kind: 'revelation' }
    | { type: 'confluence'; kind: 'clearWindTenThousandMiles' }
    | {
        type: 'pouch'
        kind:
          | 'initialShuffle'
          | 'chainRecycle'
          | { sheepStealingRecycle: { sourceCard: number } }
          | { sheepStealing: { sourceCard: number; owner: PlayerId | null } }
      }
    | { type: 'tribulation'; kind: 'rustedForestDiscardShuffle' | 'rustedForestShuffle' }
  currentOrder: number[]
}

export interface TrustedRandomnessAction {
  type: 'resolveRandomness'
  requestId: string
  shuffledOrder: number[]
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
      type: 'getCompletedReplayDraft'
      actorUserId: string
    }
  | {
      type: 'seedDevelopmentScenario'
      actorUserId: string
      scenario: DevelopmentScenario
    }
  | {
      type: 'inspectDevelopmentRecord'
      actorUserId: string
      commandId: string
    }
  | {
      type: 'getState'
      actorUserId: string
    }
  | {
      type: 'submitCommand'
      commandId: string
      gameInstanceId: string
      transactionId: string
      actorUserId: string
      action: OnlineGameAction
    }

export interface RulesNeedsRandomnessResult {
  type: 'needsRandomness'
  record: RecordedDecision[]
  request: TrustedRandomnessRequest
}

export interface RulesReadyResult extends LocalGameResponse {
  type: 'ready'
}

export type RulesEngineResult = RulesNeedsRandomnessResult | RulesReadyResult

export function requireReadyRulesResult(result: RulesEngineResult): RulesReadyResult {
  switch (result.type) {
    case 'ready':
      return result
    case 'needsRandomness':
      throw new Error(`Rules Engine unexpectedly requested randomness: ${result.request.requestId}`)
  }
}

export interface GameRoomResponse extends Omit<
  RulesReadyResult,
  'type' | 'record' | 'trustedRandomCandidates'
> {
  gameId: string
  metadata: GameRoomMetadata
  invitation?: GameRoomInvitation
  lockedDeckName?: string
  savableReplay?: SavableReplay
  /** 只有標準遊戲記錄等待延續時才存在。 */
  activeTransactionId?: string
  /** 只要回應由目前的標準遊戲記錄支援，就會存在。 */
  activeGameVersion?: {
    gameInstanceId: string
    recordSequence: number
  }
  receipt?: CommandReceipt
}

export type CommandTransactionStatus = 'awaitingChoice' | 'awaitingRandomness' | 'complete'

export interface CommandReceipt {
  schemaVersion: 1
  commandId: string
  transactionId: string
  gameInstanceId: string
  actorUserId: string
  actor: PlayerId
  payloadIdentity: string
  outcome: 'accepted' | 'rejected'
  /** 對已接受的命令存在；它是遊戲記錄的檢查點序列。 */
  committedSequence?: number
  /** 對確定性的驗證失敗存在；它不會推進標準序列。 */
  observedSequence?: number
  transactionStatus?: CommandTransactionStatus
  errorCode?: string
  createdAt: string
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
    deckCount: 0,
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
    professions: [],
    professionCatalog: [],
    cardInterpretations: [],
    spirits: [],
    previousTurnFormation: null,
    lastCompletedTurnDiscards: [],
  }
}
