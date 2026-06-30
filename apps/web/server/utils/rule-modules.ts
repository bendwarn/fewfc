const availableRuleModules = ['discard-retrieval', 'personal-deck']

export function normalizeServerRuleModules(modules: unknown): string[] {
  if (!Array.isArray(modules)) return [...availableRuleModules]
  return availableRuleModules.filter(module => modules.includes(module))
}
