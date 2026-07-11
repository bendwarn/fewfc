import type { Element } from '../types/fewfc'

const elementGlyphs = {
  Metal: '金',
  Wood: '木',
  Water: '水',
  Fire: '火',
  Earth: '土',
} as const satisfies Record<Element, string>

export type ElementGlyph = typeof elementGlyphs[Element]

export function cardElementGlyph(element: Element): ElementGlyph
export function cardElementGlyph(element: null | undefined): ''
export function cardElementGlyph(element: Element | null | undefined): ElementGlyph | ''
export function cardElementGlyph(element: Element | null | undefined): ElementGlyph | '' {
  return element ? elementGlyphs[element] : ''
}

export function cardElementClass(element: Element | null | undefined): string {
  return element ? `element-${element}` : ''
}
