import { describe, expect, test } from 'bun:test'
import {
  buildCardComposition,
  cardForCompositionSelection,
  CARD_ELEMENTS,
  CARD_LEVELS,
} from './card-composition'

describe('buildCardComposition', () => {
  test('returns all 25 definitions in stable row and column order', () => {
    const rows = buildCardComposition([])

    expect(rows.map(row => row.element)).toStrictEqual([...CARD_ELEMENTS])
    expect(rows.flatMap(row => row.cells).length).toBe(25)

    for (const row of rows) {
      expect(row.cells.map(cell => cell.level)).toStrictEqual([...CARD_LEVELS])
      expect(row.cells.every(cell => (
        cell.element === row.element && cell.count === 0 && cell.cardIds.length === 0
      ))).toBeTruthy()
    }
  })

  test('groups matching card instances by element and level', () => {
    const rows = buildCardComposition([
      { id: 1, label: '顯示名稱不參與判定', element: 'Metal', level: 1, secretStrategies: [] },
      { id: 2, label: '另一個名稱', element: 'Metal', level: 1, secretStrategies: [] },
      { id: 3, label: '無數字名稱', element: 'Fire', level: 5, secretStrategies: [] },
      { id: 4, label: '無法辨識', element: null, level: null, secretStrategies: [] },
    ])

    const cells = rows.flatMap(row => row.cells)
    expect(cells.find(cell => cell.element === '金' && cell.level === 1)?.count).toBe(2)
    expect(cells.find(cell => cell.element === '金' && cell.level === 1)?.cardIds).toStrictEqual([1, 2])
    expect(cells.find(cell => cell.element === '火' && cell.level === 5)?.count).toBe(1)
    expect(rows.find(row => row.element === '金')?.cells[0]?.cardIds).toStrictEqual([1, 2])
    expect(rows.find(row => row.element === '火')?.cells[4]?.cardIds).toStrictEqual([3])
    expect(cells.reduce((total, cell) => total + cell.count, 0)).toBe(3)
  })

  test('selects repeated instances from one matrix cell up to the limit', () => {
    expect(cardForCompositionSelection([1, 2, 3], [], 2, 'toggle')).toBe(1)
    expect(cardForCompositionSelection([1, 2, 3], [1], 2, 'toggle')).toBe(2)
    expect(cardForCompositionSelection([1, 2, 3], [1, 2], 2, 'toggle')).toBe(2)
    expect(cardForCompositionSelection([3], [1, 2], 2, 'toggle')).toBeUndefined()
  })

  test('reuses the selected instance for a replace-style matrix cell', () => {
    expect(cardForCompositionSelection([1, 2], [2], 1, 'replace')).toBe(2)
    expect(cardForCompositionSelection([1, 2], [], 1, 'replace')).toBe(1)
  })
})
