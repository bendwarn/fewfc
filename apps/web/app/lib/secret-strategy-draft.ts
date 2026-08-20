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
 * 只有在符合伺服器投影的選擇後，才將本機 Secret Strategy 輸入轉換為精確的
 * 命令負載。取消會以丟棄此結果表示，因此不可能提交命令。
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
