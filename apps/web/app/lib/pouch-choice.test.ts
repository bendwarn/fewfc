import { describe, expect, test } from 'bun:test'
import type { PublicCard } from '../types/fewfc'
import { sheepReturnCards } from './pouch-choice'

const pouch: PublicCard = {
  id: 1,
  label: '金一',
  element: 'Metal',
  level: 1,
  secretStrategies: [],
}

/*
舊 `isLegalChainTrigger` assertion claim inventory：
- 不同行、不同級、非同一卡及完整印製值才可觸發。
- 替代證據是 Rust `pouch::chain_decision_plan` 的完整 envelope 驗證，以及
  `pouch_secret_strategy_validation_failures_are_atomic_before_reveal_or_choice_made`。
  前端不再重做這些規則，僅提交封閉 Decision。
*/

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
