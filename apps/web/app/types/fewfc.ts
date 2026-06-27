export type PlayerId = string
export type TeamId = string
export type ViewerId = PlayerId | 'observer'
export type CardInstanceId = number

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

export interface PublicPlayerHand {
  player: PlayerId
  cards: PublicCardRefs
}

export interface PublicCoveredPassive {
  owner: PlayerId
  formationId: string | null
  cards: PublicCardRefs
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
}

export interface PublicCard {
  id: CardInstanceId
  label: string
}

export interface PublicGameState {
  status: 'InProgress' | 'Finished'
  turnNumber: number
  phase: 'TurnStart' | 'Main' | 'TurnDraw' | 'TurnDrawDiscardChoice' | 'TurnEnd'
  currentPlayer: PlayerId | null
  players: PublicPlayer[]
  turnOrder: PlayerId[]
  hp: TeamHp[]
  hands: PublicPlayerHand[]
  discard: PublicCard[]
  coveredPassives: PublicCoveredPassive[]
  pendingChoice: PublicPendingChoice | null
  shields: Array<{ player: PlayerId; value: number }>
  statuses: Array<{ id: string; owner: string; kind: string }>
  previousTurnFormation: PublicPreviousTurnFormation | null
}

export interface PublicGameEvent {
  id: string
  eventType: string
  title: string
  summary: string
}

export interface PlayableFormation {
  id: string
  name: string
  category: 'Attack' | 'Spell'
  summary: string
}

export type RecordedDecision = unknown

export interface LocalGameResponse {
  record: RecordedDecision[]
  state: PublicGameState
  events: PublicGameEvent[]
  playableFormations: PlayableFormation[]
  interaction: {
    canPass: boolean
    hasOptionalEffect: boolean
  }
}
