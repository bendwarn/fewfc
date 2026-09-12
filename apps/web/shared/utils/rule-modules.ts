import type { RuleModuleSpec } from '../../app/types/fewfc'

export const RULE_MODULE_PRESENTATION = {
  star: { label: '星辰圖記' },
  'hero-schools': { label: '英雄學派' },
  'discard-retrieval': { label: '棄牌回收' },
  'personal-deck': { label: '個人牌組' },
  'five-directions-legend': { label: '五方傳說' },
  spirit: { label: '精靈' },
  jianghu: { label: '江湖' },
  'confluence-generation': { label: '匯流世代' },
  'dark-glimmer': { label: '黑暗微光' },
  echo: { label: '迴響' },
  tribulation: { label: '天劫' },
  pouch: { label: '錦囊' },
  'totem-formation': { label: '圖騰法陣' },
} as const

export interface RuleModulePolicy {
  modules: RuleModuleSpec[]
  defaults: string[]
  normalize(modules: unknown): string[]
  hasValidDependencies(modules: unknown): boolean
  enable(modules: readonly string[], moduleId: string): string[]
  disable(modules: readonly string[], moduleId: string): string[]
}

export function createRuleModulePolicy(modules: readonly RuleModuleSpec[]): RuleModulePolicy {
  const ordered = modules.map(module => ({ ...module, dependencies: [...module.dependencies] }))
  const byId = new Map(ordered.map(module => [module.id, module]))
  const isRuleModuleId = (value: unknown): value is string => (
    typeof value === 'string' && byId.has(value)
  )

  const normalize = (value: unknown): string[] => {
    if (!Array.isArray(value)) {
      return ordered.filter(module => module.defaultEnabled).map(module => module.id)
    }
    const enabled = new Set(value.filter(isRuleModuleId))
    let changed = true
    while (changed) {
      changed = false
      for (const id of [...enabled]) {
        const dependencies = byId.get(id)?.dependencies ?? []
        if (!dependencies.every(dependency => enabled.has(dependency))) {
          enabled.delete(id)
          changed = true
        }
      }
    }
    return ordered.filter(module => enabled.has(module.id)).map(module => module.id)
  }

  const addWithDependencies = (enabled: Set<string>, id: string) => {
    for (const dependency of byId.get(id)?.dependencies ?? []) {
      addWithDependencies(enabled, dependency)
    }
    if (byId.has(id)) enabled.add(id)
  }

  return {
    modules: ordered,
    defaults: normalize(undefined),
    normalize,
    hasValidDependencies(value) {
      if (!Array.isArray(value)) return true
      const enabled = new Set(value.filter(isRuleModuleId))
      return [...enabled].every(id => (
        byId.get(id)?.dependencies.every(dependency => enabled.has(dependency)) ?? false
      ))
    },
    enable(value, moduleId) {
      const enabled = new Set(normalize(value))
      addWithDependencies(enabled, moduleId)
      return ordered.filter(module => enabled.has(module.id)).map(module => module.id)
    },
    disable(value, moduleId) {
      const enabled = new Set(normalize(value))
      enabled.delete(moduleId)
      return normalize([...enabled])
    },
  }
}

export function presentationForRuleModule(id: string): {
  label: string
} {
  return RULE_MODULE_PRESENTATION[id as keyof typeof RULE_MODULE_PRESENTATION]
    ?? { label: '其他規則' }
}
