export type PlayerId = string
export type TeamId = string
export type ViewerId = PlayerId | 'observer'
export type CardInstanceId = number
export type StarKind = 'Metal' | 'Wood' | 'Water' | 'Fire' | 'Earth'
export type SpiritKind = 'Metal' | 'Wood' | 'Water' | 'Fire' | 'Earth'
export type Element = 'Metal' | 'Wood' | 'Water' | 'Fire' | 'Earth'

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
  kind: string
  cards: PublicCard[]
  requiredCount: number
}

export interface PublicCard {
  id: CardInstanceId
  label: string
}

export interface PublicGameState {
  enabledRuleModules: string[]
  status: 'InProgress' | 'Finished'
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
  coveredPassives: PublicCoveredPassive[]
  counterEffects: Array<{ owner: PlayerId; effectId: string; effectName: string }>
  pendingChoice: PublicPendingChoice | null
  shields: Array<{ player: PlayerId; value: number }>
  statuses: Array<{
    id: string
    owner: { kind: 'player' | 'team'; id: string }
    kind: string
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
  }
}
