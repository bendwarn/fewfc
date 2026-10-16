import type { RuleModuleSpec } from '~/types/fewfc'

export const ruleModuleCatalog: RuleModuleSpec[] = [
  { id: 'discard-retrieval', category: 'optional', defaultEnabled: true, dependencies: [] },
  { id: 'personal-deck', category: 'optional', defaultEnabled: true, dependencies: [] },
  { id: 'five-directions-legend', category: 'advanced', defaultEnabled: true, dependencies: [] },
  { id: 'star', category: 'advanced', defaultEnabled: true, dependencies: [] },
  { id: 'hero-schools', category: 'advanced', defaultEnabled: true, dependencies: [] },
  { id: 'spirit', category: 'theme', defaultEnabled: true, dependencies: ['star', 'five-directions-legend', 'hero-schools'] },
  { id: 'dark-glimmer', category: 'theme', defaultEnabled: true, dependencies: ['spirit'] },
  { id: 'pouch', category: 'theme', defaultEnabled: true, dependencies: ['personal-deck', 'spirit'] },
]

export const allRuleModulesEnabled = ruleModuleCatalog.map(module => module.id)
