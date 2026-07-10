import { rulesEngine } from './rules-engine'

export async function resolveServerRuleModules(modules: unknown): Promise<string[]> {
  const candidate = Array.isArray(modules)
    ? modules.filter((module): module is string => typeof module === 'string')
    : undefined
  return (await rulesEngine().resolveRuleModules(candidate)).modules
}
