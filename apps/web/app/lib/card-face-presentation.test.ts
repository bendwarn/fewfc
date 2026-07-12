import { expect, test } from 'bun:test'
import type { Element } from '../types/fewfc'
import {
  cardElementClass,
  cardElementGlyph,
  type ElementGlyph,
} from './card-face-presentation'

test('card faces use stable element classes and concise glyphs', () => {
  const expected: Array<[Element, ElementGlyph]> = [
    ['Metal', '金'],
    ['Wood', '木'],
    ['Water', '水'],
    ['Fire', '火'],
    ['Earth', '土'],
  ]

  for (const [element, glyph] of expected) {
    expect(cardElementClass(element)).toBe(`element-${element}`)
    expect(cardElementGlyph(element)).toBe(glyph)
  }
  expect(cardElementClass(null)).toBe('')
  expect(cardElementGlyph(null)).toBe('')
})
