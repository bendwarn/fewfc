import { describe, expect, test } from 'bun:test'
import type { PublicCard } from '../types/fewfc'
import { isLegalChainTrigger, sheepReturnCards } from './pouch-choice'

const pouch: PublicCard = {
  id: 1,
  label: '金一',
  element: 'Metal',
  level: 1,
  secretStrategies: [],
}

describe('isLegalChainTrigger', () => {
  test('requires both a different element and a different level', () => {
    expect(isLegalChainTrigger(pouch, { ...pouch, id: 2, level: 2 })).toBe(false)
    expect(isLegalChainTrigger(pouch, { ...pouch, id: 3, element: 'Wood' })).toBe(false)
    expect(isLegalChainTrigger(pouch, {
      ...pouch,
      id: 4,
      element: 'Wood',
      level: 2,
    })).toBe(true)
  })

  test('rejects the pouch itself and cards without rule facts', () => {
    expect(isLegalChainTrigger(pouch, pouch)).toBe(false)
    expect(isLegalChainTrigger(pouch, {
      ...pouch,
      id: 5,
      element: null,
      level: null,
    })).toBe(false)
  })
})

describe('sheepReturnCards', () => {
  const discardCard = { ...pouch, id: 2, label: '棄牌' }
  const selectedDeckCard = { ...pouch, id: 3, label: '剛棄掉的牌' }
  const unselectedDeckCard = { ...pouch, id: 4, label: '仍在牌組的牌' }

  test('offers the selected Deck Cards after the discard step', () => {
    expect(sheepReturnCards(
      [discardCard.id, selectedDeckCard.id, unselectedDeckCard.id],
      [discardCard],
      [selectedDeckCard, unselectedDeckCard],
      [selectedDeckCard.id],
    )).toEqual([discardCard, selectedDeckCard])
  })

  test('does not offer an unselected or server-rejected Deck Card', () => {
    expect(sheepReturnCards(
      [discardCard.id],
      [discardCard],
      [selectedDeckCard],
      [selectedDeckCard.id],
    )).toEqual([discardCard])
  })
})
