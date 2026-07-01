const availableRuleModules = [
  'star',
  'hero-schools',
  'discard-retrieval',
  'personal-deck',
  'five-directions-legend',
]

export function normalizeServerRuleModules(modules: unknown): string[] {
  if (!Array.isArray(modules)) return [...availableRuleModules]
  return availableRuleModules.filter(module => modules.includes(module))
}
