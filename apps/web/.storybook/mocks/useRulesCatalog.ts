import { ref } from 'vue'
import type { RulesCatalog } from '../../app/types/fewfc'
import rulesCatalogFixture from '../../stories/fixtures/rules-catalog.json'

// 固定使用已建置 Rust 規則引擎匯出的目錄，避免故事自行推定牌組規則。
const catalogValue = rulesCatalogFixture as RulesCatalog
const catalog = ref<RulesCatalog | null>(catalogValue)
const loading = ref(false)
const error = ref<unknown>(null)

export function useRulesCatalog() {
  return {
    catalog,
    loading,
    error,
    async load() {
      return catalogValue
    },
  }
}
