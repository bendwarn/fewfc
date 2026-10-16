import { ref } from 'vue'
import type { RulesCatalog } from '../../app/types/fewfc'

const catalogValue = { deckComposition: { cardDefinitions: [] } } as unknown as RulesCatalog
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
