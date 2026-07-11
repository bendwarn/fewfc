import assert from 'node:assert/strict'
import { test } from 'node:test'
import type { Element } from '../types/fewfc'
import { cardElementClass, cardElementGlyph } from './card-face-presentation'

test('card faces use stable element classes and concise glyphs', () => {
  const expected: Array<[Element, string]> = [
    ['Metal', '金'],
    ['Wood', '木'],
    ['Water', '水'],
    ['Fire', '火'],
    ['Earth', '土'],
  ]

  for (const [element, glyph] of expected) {
    assert.equal(cardElementClass(element), `element-${element}`)
    assert.equal(cardElementGlyph(element), glyph)
  }
  assert.equal(cardElementClass(null), '')
  assert.equal(cardElementGlyph(null), '')
})
