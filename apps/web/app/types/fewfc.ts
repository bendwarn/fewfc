export type PlayerId = string
export type TeamId = string
export type ViewerId = PlayerId | 'observer'
export type CardInstanceId = number
export type StarKind = 'Metal' | 'Wood' | 'Water' | 'Fire' | 'Earth'
export type SpiritKind = 'Metal' | 'Wood' | 'Water' | 'Fire' | 'Earth' | 'Evil' | 'Death'
export type Element = 'Metal' | 'Wood' | 'Water' | 'Fire' | 'Earth'
export type SecretStrategy =
  | 'GoldenCicada'
  | 'StealTheBeam'
  | 'MuddyWaters'
  | 'WatchTheFire'
  | 'LureTheTigerAway'
  | 'ReturnSoul'
  | 'SheepStealing'
  | 'DarkCrossing'
  | 'DeceiveHeaven'
  | 'Retreat'

export interface StarElementSubstitution {
  card: CardInstanceId
  printedElement: Element
  interpretedElement: Element
}

export interface PublicPlayer {
  id: PlayerId
  team: TeamId
}

export interface TeamHp {
  team: TeamId
  hp: number
}

export type PublicCardRefs =
  | {
      kind: 'known'
      cards: PublicCard[]
    }
  | {
      kind: 'hidden'
      count: number
    }
  | {
      kind: 'partiallyKnown'
      cards: Array<PublicCard | null>
    }

export interface PublicPlayerHand {
  player: PlayerId
  cards: PublicCardRefs
}

export interface PublicCoveredPassive {
  owner: PlayerId
  formationId: string | null
  cards: PublicCardRefs
  starSubstitution: StarElementSubstitution | null
}

export interface PublicPreviousTurnFormation {
  player: PlayerId
  formationId: string | null
  formationName: string | null
  cards: PublicCardRefs
}

export interface PublicPendingChoice {
  player: PlayerId
  purpose: string
  kind: string
  cards: PublicCard[]
  requiredCount: number
  minimumCount: number
  maximumCount: number
  players: PlayerId[]
  formations: string[]
  environments: Element[]
  canDecline: boolean
}

export type EffectChoiceAnswer =
  | { type: 'cards'; cards: CardInstanceId[] }
  | { type: 'player'; player: PlayerId }
  | { type: 'formation'; formationId: string }
  | { type: 'environment'; environment: Element }
  | { type: 'decline' }

export interface PublicPendingRandomness {
  requestId: string
  deck: string
  cardCount: number
}

export interface PublicCard {
  id: CardInstanceId
  label: string
}

export interface PublicGameState {
  enabledRuleModules: string[]
  status: 'Preparing' | 'InProgress' | 'Finished'
  turnNumber: number
  phase: 'TurnStart' | 'Main' | 'TurnDraw' | 'TurnDrawDiscardChoice' | 'TurnEnd'
  currentPlayer: PlayerId | null
  players: PublicPlayer[]
  turnOrder: PlayerId[]
  hp: TeamHp[]
  hands: PublicPlayerHand[]
  discard: PublicCard[]
  playerDecks: Array<{ player: PlayerId; cards: PublicCardRefs }>
  playerDiscards: Array<{ player: PlayerId; cards: PublicCard[] }>
  pouches: Array<{ owner: PlayerId; card: PublicCard | null }>
  preparationPlayer: PlayerId | null
  coveredPassives: PublicCoveredPassive[]
  counterEffects: Array<{ owner: PlayerId; effectId: string; effectName: string }>
  pendingChoice: PublicPendingChoice | null
  pendingRandomness: PublicPendingRandomness | null
  shields: Array<{ player: PlayerId; value: number }>
  statuses: Array<{
    id: string
    owner: { kind: 'player' | 'team'; id: string }
    kind: string
  }>
  jianghuStates: Array<{
    owner: PlayerId
    kind: 'ThousandBlades' | 'SnowTreading' | 'Poison'
    remainingTurns: number
    expiresOnTurn: number | null
  }>
  limitedUses: Array<{
    owner: PlayerId
    key: string
    remaining: number
    maximum: number
  }>
  confluenceCardObligations: Array<{
    owner: PlayerId
    card: CardInstanceId | null
    allowProfessionFormation: boolean
  }>
  scheduledEchoes: Array<{
    player: PlayerId
    melodyId: string
    dueTurnNumber: number
  }>
  flowStates: Array<{
    player: PlayerId
    layers: number
  }>
  formationSuppressions: Array<{
    source: PlayerId
    target: PlayerId
    formationId: string
    expiresOnTurnNumber: number
  }>
  scheduledPlantEarth: Array<{
    player: PlayerId
    dueTurnNumber: number
  }>
  environment: 'Metal' | 'Wood' | 'Water' | 'Fire' | 'Earth' | null
  teamStars: Array<{ team: TeamId; star: StarKind }>
  starHistories: Array<{ player: PlayerId; stars: StarKind[] }>
  fiveStarAlignment: { player: PlayerId; team: TeamId } | null
  professions: Array<{
    player: PlayerId
    id: string
    name: string
    abilities: string[]
  }>
  professionCatalog: Array<{
    id: string
    name: string
    requirement: string
    parentName: string | null
    inheritance: string
    abilities: string[]
    formations: Array<{
      name: string
      summary: string
    }>
  }>
  preparedProfessionAbilities: Array<{
    player: PlayerId
    abilityId: string
    card: CardInstanceId
    element: Element
    level: number
    allowedFormationScope: string[]
  }>
  spirits: Array<{
    player: PlayerId
    spirit: SpiritKind
    power: number
  }>
  previousTurnFormation: PublicPreviousTurnFormation | null
}

export interface PublicGameEvent {
  id: string
  eventType: string
  title: string
  summary: string
}

export type PlayableAction =
  | {
      type: 'performFormation'
      id: string
      name: string
      category: 'Attack' | 'Spell'
      summary: string
      cards: CardInstanceId[]
      starSubstitution: StarElementSubstitution | null
      matchOption: {
        role: string
        card: CardInstanceId
        slots: number
        preview: string | null
      } | null
    }
  | {
      type: 'changeProfession'
      id: string
      name: string
      summary: string
      cards: CardInstanceId[]
    }
  | {
      type: 'activateProfessionAbility'
      id: string
      name: string
      summary: string
      cards: CardInstanceId[]
      targetCard: CardInstanceId | null
      declaredElement: Element | null
      declaredLevel: number | null
    }
  | {
      type: 'useSpiritSkill'
      id: string
      name: string
      summary: string
      cards: CardInstanceId[]
      selectedCard: CardInstanceId | null
      declaredLevel: number | null
    }

export type RecordedDecision = unknown

export interface LocalGameResponse {
  record: RecordedDecision[]
  state: PublicGameState
  events: PublicGameEvent[]
  playableActions: PlayableAction[]
  interaction: {
    canPass: boolean
    hasOptionalEffect: boolean
    canRetrieveDiscard: boolean
    canChooseInitialPouch: boolean
    canTriggerPouch: boolean
  }
  trustedRandomCandidates?: CardInstanceId[]
  pendingRandomnessRequest?: {
    requestId: string
    deck: 'Shared' | { Player: PlayerId }
    continuationId: string
    currentOrder: CardInstanceId[]
  }
}
