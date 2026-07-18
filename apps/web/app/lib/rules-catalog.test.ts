import { describe, expect, test } from 'bun:test'
import { createRulesCatalogCache } from './rules-catalog'

describe('Rules Catalog cache', () => {
  test('shares one authoritative catalog request between concurrent page callers', async () => {
    let requests = 0
    const catalog = { deckComposition: { elements: [] } }
    const cache = createRulesCatalogCache(async () => {
      requests += 1
      return catalog
    })

    expect(await Promise.all([cache.load(), cache.load(), cache.load()])).toEqual([catalog, catalog, catalog])
    expect(requests).toBe(1)
  })

  test('allows a later page to retry after a failed catalog request', async () => {
    let attempts = 0
    const cache = createRulesCatalogCache(async () => {
      attempts += 1
      if (attempts === 1) throw new Error('offline')
      return { deckComposition: { elements: [] } }
    })

    await expect(cache.load()).rejects.toThrow('offline')
    await expect(cache.load()).resolves.toEqual({ deckComposition: { elements: [] } })
    expect(attempts).toBe(2)
  })
})
