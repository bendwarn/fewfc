import type { RuleVersion } from '../game-room'
import type { RuleModuleSpec } from '../../app/types/fewfc'

export function modulesForVersion(catalog: readonly RuleModuleSpec[], version: RuleVersion): RuleModuleSpec[] {
  return catalog.filter(module => version === '5.16'
    ? module.id !== 'totem-formation'
    : module.id !== 'echo' && module.id !== 'tribulation')
}
