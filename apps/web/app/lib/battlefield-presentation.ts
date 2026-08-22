import type {
  PlayerId,
  PublicCardRefs,
  PublicGameState,
  PublicLastCompletedTurnDiscard,
} from '../types/fewfc'

type BattlefieldPileState = Pick<PublicGameState,
  | 'deckCount'
  | 'discard'
  | 'playerDecks'
  | 'playerDiscards'
  | 'lastCompletedTurnDiscards'
  | 'turnNumber'>

export interface PresentedPileCounts {
  deck: number
  discard: number
}

export function publicCardRefCount(cards: PublicCardRefs | undefined): number {
  if (!cards) return 0
  return cards.kind === 'hidden' ? cards.count : cards.cards.length
}

export function personalPileCounts(
  state: BattlefieldPileState,
  player: PlayerId,
): PresentedPileCounts {
  return {
    deck: publicCardRefCount(state.playerDecks.find(entry => entry.player === player)?.cards),
    discard: state.playerDiscards.find(entry => entry.player === player)?.cards.length ?? 0,
  }
}

export function sharedPileCounts(state: BattlefieldPileState): PresentedPileCounts {
  return {
    deck: state.deckCount ?? 0,
    discard: state.discard.length,
  }
}

export function lastCompletedTurnDiscardForPlayer(
  state: BattlefieldPileState,
  player: PlayerId,
): PublicLastCompletedTurnDiscard | null {
  return state.lastCompletedTurnDiscards.find(entry => entry.player === player) ?? null
}

export function latestSharedTurnDiscard(
  state: BattlefieldPileState,
): PublicLastCompletedTurnDiscard | null {
  const latestCompletedTurn = state.turnNumber - 1
  if (latestCompletedTurn < 0) return null
  return state.lastCompletedTurnDiscards.find(
    entry => entry.turnNumber === latestCompletedTurn,
  ) ?? null
}
