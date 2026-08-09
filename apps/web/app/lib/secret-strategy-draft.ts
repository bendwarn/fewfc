import type {
  CardInstanceId,
  PlayerId,
  SecretStrategyOption,
  StarKind,
} from '../types/fewfc'

export interface SecretStrategyDraftSelection {
  targetPlayer?: PlayerId | null
  star?: StarKind | null
  breakStar?: boolean
  retreat?: CardInstanceId | 'clearEnvironment' | null
}

export interface SecretStrategyDraftAction {
  strategy: SecretStrategyOption['strategy']
  options: {
    targetPlayer?: PlayerId
    star?: StarKind
    breakStar?: boolean
    discardCard?: CardInstanceId
  }
}

/**
 * Converts local Secret Strategy input into the exact command payload only
 * after it satisfies the server-projected choices. Cancelling is represented
 * by discarding this result, so it cannot submit a command.
 */
export function secretStrategyDraftAction(
  draft: SecretStrategyOption,
  selection: SecretStrategyDraftSelection,
): SecretStrategyDraftAction | undefined {
  if (draft.input === 'targetPlayer') {
    if (!selection.targetPlayer || !draft.targetPlayers.includes(selection.targetPlayer)) return undefined
    return { strategy: draft.strategy, options: { targetPlayer: selection.targetPlayer } }
  }

  if (draft.input === 'star') {
    if (!selection.star) return undefined
    const allowed = selection.breakStar ? draft.breakStars : draft.stars
    if (!allowed.includes(selection.star)) return undefined
    return {
      strategy: draft.strategy,
      options: { star: selection.star, breakStar: Boolean(selection.breakStar) },
    }
  }

  if (draft.input === 'retreat') {
    if (selection.retreat === 'clearEnvironment') {
      return { strategy: draft.strategy, options: {} }
    }
    if (selection.retreat === null || selection.retreat === undefined || !draft.handCards.includes(selection.retreat)) {
      return undefined
    }
    return { strategy: draft.strategy, options: { discardCard: selection.retreat } }
  }

  return undefined
}
