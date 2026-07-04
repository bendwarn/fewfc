export const RULE_MODULE_SPECS = [
  {
    id: 'star',
    label: '進階規則‧星辰圖記',
    group: 'advanced',
    defaultEnabled: true,
    dependencies: [],
  },
  {
    id: 'hero-schools',
    label: '進階規則‧英雄學派',
    group: 'advanced',
    defaultEnabled: true,
    dependencies: [],
  },
  {
    id: 'discard-retrieval',
    label: '棄牌回收',
    group: 'gameplay',
    defaultEnabled: true,
    dependencies: [],
  },
  {
    id: 'personal-deck',
    label: '個人牌組',
    group: 'gameplay',
    defaultEnabled: true,
    dependencies: [],
  },
  {
    id: 'five-directions-legend',
    label: '進階規則‧五方傳說',
    group: 'advanced',
    defaultEnabled: true,
    dependencies: [],
  },
  {
    id: 'spirit',
    label: '主題規則‧精靈',
    group: 'theme',
    defaultEnabled: true,
    dependencies: ['star', 'five-directions-legend', 'hero-schools'],
  },
  {
    id: 'jianghu',
    label: '主題規則‧江湖',
    group: 'theme',
    defaultEnabled: true,
    dependencies: ['star', 'five-directions-legend', 'hero-schools'],
  },
  {
    id: 'confluence-generation',
    label: '主題規則‧匯流世代',
    group: 'theme',
    defaultEnabled: true,
    dependencies: ['star', 'five-directions-legend', 'hero-schools'],
  },
  {
    id: 'dark-glimmer',
    label: '主題規則‧黑暗微光',
    group: 'theme',
    defaultEnabled: true,
    dependencies: ['spirit'],
  },
  {
    id: 'echo',
    label: '主題規則‧迴響',
    group: 'theme',
    defaultEnabled: true,
    dependencies: ['star', 'five-directions-legend', 'hero-schools'],
  },
  {
    id: 'tribulation',
    label: '主題規則‧天劫',
    group: 'theme',
    defaultEnabled: true,
    dependencies: ['star', 'five-directions-legend', 'hero-schools'],
  },
] as const

export type RuleModuleId = typeof RULE_MODULE_SPECS[number]['id']
export type RuleModuleGroupId = typeof RULE_MODULE_SPECS[number]['group']

export const AVAILABLE_RULE_MODULES = RULE_MODULE_SPECS.map(module => module.id)
export const DEFAULT_RULE_MODULES = RULE_MODULE_SPECS
  .filter(module => module.defaultEnabled)
  .map(module => module.id)

const ruleModuleById = new Map<RuleModuleId, typeof RULE_MODULE_SPECS[number]>(
  RULE_MODULE_SPECS.map(module => [module.id, module]),
)

function isRuleModuleId(value: unknown): value is RuleModuleId {
  return typeof value === 'string' && ruleModuleById.has(value as RuleModuleId)
}

export function hasValidRuleModuleDependencies(modules: unknown): boolean {
  if (!Array.isArray(modules)) return true
  const enabled = new Set(modules.filter(isRuleModuleId))
  return [...enabled].every(module => (
    ruleModuleById.get(module)?.dependencies.every(dependency => enabled.has(dependency)) ?? false
  ))
}

export function normalizeRuleModules(modules: unknown): RuleModuleId[] {
  if (!Array.isArray(modules)) return [...DEFAULT_RULE_MODULES]
  const enabled = new Set(modules.filter(isRuleModuleId))
  let changed = true
  while (changed) {
    changed = false
    for (const module of [...enabled]) {
      const dependencies = ruleModuleById.get(module)?.dependencies ?? []
      if (!dependencies.every(dependency => enabled.has(dependency))) {
        enabled.delete(module)
        changed = true
      }
    }
  }
  return AVAILABLE_RULE_MODULES.filter(module => enabled.has(module))
}

export function enableRuleModule(
  modules: readonly string[],
  moduleId: string,
): RuleModuleId[] {
  const enabled = new Set(normalizeRuleModules(modules))
  if (!isRuleModuleId(moduleId)) return [...enabled]
  const addWithDependencies = (id: RuleModuleId) => {
    for (const dependency of ruleModuleById.get(id)?.dependencies ?? []) {
      addWithDependencies(dependency)
    }
    enabled.add(id)
  }
  addWithDependencies(moduleId)
  return AVAILABLE_RULE_MODULES.filter(module => enabled.has(module))
}

export function disableRuleModule(
  modules: readonly string[],
  moduleId: string,
): RuleModuleId[] {
  const enabled = new Set(normalizeRuleModules(modules))
  if (!isRuleModuleId(moduleId)) return [...enabled]
  enabled.delete(moduleId)
  let changed = true
  while (changed) {
    changed = false
    for (const enabledId of [...enabled]) {
      const dependencies = ruleModuleById.get(enabledId)?.dependencies ?? []
      if (!dependencies.every(dependency => enabled.has(dependency))) {
        enabled.delete(enabledId)
        changed = true
      }
    }
  }
  return AVAILABLE_RULE_MODULES.filter(module => enabled.has(module))
}
