import { expect, test } from 'bun:test'
import type { Element } from '../types/fewfc'
import {
  CARD_BACK_IMAGE_PATH,
  cardElementClass,
  cardElementGlyph,
  cardFaceImagePath,
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

test('card faces map printed facts to optimized public assets', () => {
  expect(cardFaceImagePath('Metal', 1)).toBe('/cards/metal-1.webp')
  expect(cardFaceImagePath('Earth', 5)).toBe('/cards/earth-5.webp')
  expect(cardFaceImagePath('Fire', 0)).toBeNull()
  expect(cardFaceImagePath(null, 3)).toBeNull()
  expect(CARD_BACK_IMAGE_PATH).toBe('/cards/back.webp')
})
