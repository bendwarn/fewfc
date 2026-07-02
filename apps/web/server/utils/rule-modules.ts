const availableRuleModules = [
  'star',
  'hero-schools',
  'discard-retrieval',
  'personal-deck',
  'five-directions-legend',
  'spirit',
]
const spiritDependencies = ['star', 'hero-schools', 'five-directions-legend']

export function normalizeServerRuleModules(modules: unknown): string[] {
  if (!Array.isArray(modules)) return [...availableRuleModules]
  const normalized = availableRuleModules.filter(module => modules.includes(module))
  if (
    normalized.includes('spirit')
    && !spiritDependencies.every(module => normalized.includes(module))
  ) {
    return normalized.filter(module => module !== 'spirit')
  }
  return normalized
}

export function hasValidServerRuleModuleDependencies(modules: unknown): boolean {
  if (!Array.isArray(modules) || !modules.includes('spirit')) return true
  return spiritDependencies.every(module => modules.includes(module))
}
