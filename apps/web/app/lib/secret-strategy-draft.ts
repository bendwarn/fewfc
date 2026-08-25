import type {
  CardInstanceId,
  PlayerId,
  SecretStrategy,
  SecretStrategyDecision,
  SecretStrategyOption,
  StarKind,
} from '../types/fewfc'

export interface SecretStrategyDraftSelection {
  targetPlayer?: PlayerId | null
  star?: StarKind | null
  breakStar?: boolean
  retreat?: CardInstanceId | 'clearEnvironment' | null
}

/**
 * 瀏覽器只把目前草稿轉成封閉 Decision；候選是否合法、印製值與過期狀態一律
 * 由 Rules Engine 在提交時重驗。取消以丟棄結果表示，不會提交命令。
 */
export function secretStrategyDraftAction(
  draft: SecretStrategyOption,
  selection: SecretStrategyDraftSelection,
): SecretStrategyDecision | undefined {
  switch (draft.type) {
    case 'noInput':
      return { type: 'noInput', sourceCard: draft.sourceCard, strategy: draft.strategy }
    case 'targetPlayer':
      return selection.targetPlayer
        ? { type: 'targetPlayer', sourceCard: draft.sourceCard, targetPlayer: selection.targetPlayer }
        : undefined
    case 'star':
      return selection.star
        ? {
            type: 'star',
            sourceCard: draft.sourceCard,
            operation: selection.breakStar
              ? { type: 'break', star: selection.star }
              : { type: 'gain', star: selection.star },
          }
        : undefined
    case 'environment':
      if (selection.retreat === 'clearEnvironment') {
        return { type: 'environment', sourceCard: draft.sourceCard, operation: { type: 'clear' } }
      }
      return typeof selection.retreat === 'number'
        ? {
            type: 'environment',
            sourceCard: draft.sourceCard,
            operation: { type: 'transferByDiscard', card: selection.retreat },
          }
        : undefined
    case 'sheepStealing':
      return { type: 'sheepStealing', sourceCard: draft.sourceCard }
  }
}

export function secretStrategyOptionStrategy(option: SecretStrategyOption): SecretStrategy {
  switch (option.type) {
    case 'noInput': return option.strategy
    case 'targetPlayer': return 'LureTheTigerAway'
    case 'star': return 'DeceiveHeaven'
    case 'environment': return 'Retreat'
    case 'sheepStealing': return 'SheepStealing'
  }
}
