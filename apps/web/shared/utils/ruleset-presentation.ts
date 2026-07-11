import type { RuleModuleSpec } from '../../app/types/fewfc'
import { presentationForRuleModule } from './rule-modules'

export function ruleModuleLabel(id: string): string {
  return presentationForRuleModule(id).label
}

export function presentRoomRuleDifferences(
  catalog: readonly RuleModuleSpec[],
  enabledRuleModules: readonly string[],
): string {
  const enabled = new Set(enabledRuleModules)
  const disabled = catalog
    .filter(module => !enabled.has(module.id))
    .map(module => ruleModuleLabel(module.id))
  return disabled.length ? `停用：${disabled.join('、')}` : ''
}
