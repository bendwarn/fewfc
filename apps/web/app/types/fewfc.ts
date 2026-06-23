export type PlayerId = 'alice' | 'bob'
export type TeamId = `team:${PlayerId}`
export type ViewerId = PlayerId | 'observer'

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
      cards: string[]
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
  formationId: string
  cards: PublicCardRefs
}

export interface PublicPendingChoice {
  player: PlayerId
  kind: string
}

export interface PublicGameState {
  status: 'InProgress' | 'Finished'
  turnNumber: number
  phase: 'MainPhase' | 'TurnDrawDiscardChoice'
  currentPlayer: PlayerId
  players: PublicPlayer[]
  turnOrder: PlayerId[]
  hp: TeamHp[]
  hands: PublicPlayerHand[]
  discard: string[]
  coveredPassives: PublicCoveredPassive[]
  pendingChoice: PublicPendingChoice | null
  shields: Array<{ player: PlayerId; value: number }>
  statuses: Array<{ id: string; owner: string; kind: string }>
}

export interface PublicGameEvent {
  id: string
  type: string
  summary: string
}

export interface PlayableFormation {
  id: string
  name: string
  category: 'Attack' | 'Spell'
  summary: string
}
