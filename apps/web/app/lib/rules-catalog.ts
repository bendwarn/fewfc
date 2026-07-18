import type { RulesCatalog } from '~/types/fewfc'

export interface RulesCatalogCache<T> {
  load(): Promise<T>
  clear(): void
}

export function createRulesCatalogCache<T>(loadCatalog: () => Promise<T>): RulesCatalogCache<T> {
  let value: T | undefined
  let pending: Promise<T> | undefined

  return {
    async load() {
      if (value !== undefined) {
        return value
      }

      pending ??= loadCatalog().then(
        (catalog) => {
          value = catalog
          return catalog
        },
        (error) => {
          pending = undefined
          throw error
        },
      )

      return await pending
    },
    clear() {
      value = undefined
      pending = undefined
    },
  }
}

let sharedCatalog: RulesCatalogCache<RulesCatalog> | undefined

export function useRulesCatalog() {
  const catalog = useState<RulesCatalog | null>('rules-catalog', () => null)
  const loading = useState<boolean>('rules-catalog-loading', () => false)
  const error = useState<unknown>('rules-catalog-error', () => null)

  sharedCatalog ??= createRulesCatalogCache(() => $fetch<RulesCatalog>('/api/rules/catalog'))

  async function load(): Promise<RulesCatalog> {
    loading.value = true
    error.value = null
    try {
      const value = await sharedCatalog!.load()
      catalog.value = value
      return value
    } catch (reason) {
      error.value = reason
      throw reason
    } finally {
      loading.value = false
    }
  }

  return { catalog, loading, error, load }
}
