import type { PublicCard } from '../types/fewfc'

export const DISCARD_ELEMENTS = ['金', '木', '水', '火', '土'] as const
export const DISCARD_LEVELS = [1, 2, 3, 4, 5] as const

export type DiscardElement = typeof DISCARD_ELEMENTS[number]
export type DiscardLevel = typeof DISCARD_LEVELS[number]

export interface DiscardCompositionCell {
  element: DiscardElement
  level: DiscardLevel
  count: number
}

export interface DiscardCompositionRow {
  level: DiscardLevel
  cells: DiscardCompositionCell[]
}

export function buildDiscardComposition(cards: PublicCard[]): DiscardCompositionRow[] {
  const counts = new Map<string, number>()
  const elementLabels: Record<string, DiscardElement> = {
    Metal: '金',
    Wood: '木',
    Water: '水',
    Fire: '火',
    Earth: '土',
  }

  for (const card of cards) {
    const element = card.element ? elementLabels[card.element] : undefined
    const level = card.level

    if (!element || !DISCARD_LEVELS.includes(level as DiscardLevel)) {
      continue
    }

    const key = compositionKey(element, level as DiscardLevel)
    counts.set(key, (counts.get(key) ?? 0) + 1)
  }

  return DISCARD_LEVELS.map(level => ({
    level,
    cells: DISCARD_ELEMENTS.map(element => ({
      element,
      level,
      count: counts.get(compositionKey(element, level)) ?? 0,
    })),
  }))
}

function compositionKey(element: DiscardElement, level: DiscardLevel): string {
  return `${element}-${level}`
}
