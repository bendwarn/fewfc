import {
  hasValidRuleModuleDependencies,
  normalizeRuleModules,
} from '../../shared/utils/rule-modules'

export function normalizeServerRuleModules(modules: unknown): string[] {
  return normalizeRuleModules(modules)
}

export function hasValidServerRuleModuleDependencies(modules: unknown): boolean {
  return hasValidRuleModuleDependencies(modules)
}
