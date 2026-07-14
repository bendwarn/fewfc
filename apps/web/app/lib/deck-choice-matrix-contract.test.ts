import { describe, expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'

const appSource = readFileSync(new URL('../app.vue', import.meta.url), 'utf8')

describe('deck card choice presentation', () => {
  test('every deck-search choice uses the element-by-level matrix', () => {
    const listBasedDeckChoices = [
      'aria-label="選擇牌組牌"',
      'aria-label="牽羊牌組牌"',
      'aria-label="選擇棄牌"',
      'aria-label="牽羊棄牌"',
    ]

    for (const ariaLabel of listBasedDeckChoices) {
      expect(appSource.includes(`<div class="choice-cards" ${ariaLabel}>`)).toBe(false)
    }

    const matrixLabels = [
      '初始錦囊牌組矩陣',
      '連環錦囊牌組矩陣',
      '連環觸發牌組矩陣',
      '牽羊牌組矩陣',
      '牽羊回收矩陣',
      '連環牽羊牌組矩陣',
      '連環牽羊回收矩陣',
      '商調‧鳴金牌組矩陣',
    ]

    for (const label of matrixLabels) {
      expect(appSource.match(new RegExp(`label="${label}"`, 'g'))).toHaveLength(1)
    }

    expect(appSource).toContain("state.pendingChoice.presentation.type === 'echoRingingMetalDeckCard'")
  })
})
