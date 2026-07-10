import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import {
  buildDiscardComposition,
  DISCARD_ELEMENTS,
  DISCARD_LEVELS,
} from './discard-composition'

describe('buildDiscardComposition', () => {
  test('returns all 25 definitions in stable row and column order', () => {
    const rows = buildDiscardComposition([])

    assert.deepEqual(rows.map(row => row.level), DISCARD_LEVELS)
    assert.equal(rows.flatMap(row => row.cells).length, 25)

    for (const row of rows) {
      assert.deepEqual(row.cells.map(cell => cell.element), DISCARD_ELEMENTS)
      assert.ok(row.cells.every(cell => cell.level === row.level && cell.count === 0))
    }
  })

  test('groups matching card instances by element and level', () => {
    const rows = buildDiscardComposition([
      { id: 1, label: '顯示名稱不參與判定', element: 'Metal', level: 1, secretStrategies: [] },
      { id: 2, label: '另一個名稱', element: 'Metal', level: 1, secretStrategies: [] },
      { id: 3, label: '無數字名稱', element: 'Fire', level: 5, secretStrategies: [] },
      { id: 4, label: '無法辨識', element: null, level: null, secretStrategies: [] },
    ])

    const cells = rows.flatMap(row => row.cells)
    assert.equal(cells.find(cell => cell.element === '金' && cell.level === 1)?.count, 2)
    assert.equal(cells.find(cell => cell.element === '火' && cell.level === 5)?.count, 1)
    assert.equal(cells.reduce((total, cell) => total + cell.count, 0), 3)
  })
})
