import { expect, test } from './fixtures'
import type { Element } from '../../app/types/fewfc'
import { cardElementClass, cardElementGlyph } from '../../app/lib/card-face-presentation'

test('compact card faces separate level and element with distinct element colors', async ({ page }) => {
  const faces = (['Metal', 'Wood', 'Water', 'Fire', 'Earth'] satisfies Element[])
    .map(element => ({
      className: cardElementClass(element),
      glyph: cardElementGlyph(element),
    }))

  await page.evaluate((cardFaces) => {
    const harness = document.createElement('div')
    harness.id = 'card-face-harness'
    harness.style.cssText = 'display:flex;align-items:flex-start;gap:24px;padding:24px'
    for (const [index, face] of cardFaces.entries()) {
      const seat = document.createElement('div')
      seat.className = `player-seat ${index === 0 ? 'seat-top' : index === 1 ? 'seat-left' : 'seat-bottom'}`
      const hand = document.createElement('div')
      hand.className = 'hand seat-hand'
      const card = document.createElement('button')
      card.className = `playing-card ${face.className}`
      card.setAttribute('aria-label', `${face.glyph} 1`)
      card.innerHTML = '<span class="card-level">1</span><span class="card-element">' + face.glyph + '</span>'
      hand.append(card)
      seat.append(hand)
      harness.append(seat)
    }
    document.body.append(harness)
  }, faces)

  const cards = page.locator('#card-face-harness .playing-card')
  await expect(cards).toHaveCount(5)
  await expect(cards.locator('.card-name')).toHaveCount(0)

  const presentation = await cards.evaluateAll(elements => elements.map((card) => {
    const level = card.querySelector('.card-level')!.getBoundingClientRect()
    const element = card.querySelector('.card-element')!.getBoundingClientRect()
    const style = getComputedStyle(card)
    const elementStyle = getComputedStyle(card.querySelector('.card-element')!)
    return {
      overlaps: level.left < element.right
        && level.right > element.left
        && level.top < element.bottom
        && level.bottom > element.top,
      background: style.backgroundColor,
      border: style.borderColor,
      elementColor: elementStyle.color,
    }
  }))

  expect(presentation.every(card => !card.overlaps)).toBe(true)
  expect(new Set(presentation.map(card => card.background)).size).toBe(5)
  expect(new Set(presentation.map(card => card.border)).size).toBe(5)
  expect(new Set(presentation.map(card => card.elementColor)).size).toBe(5)
})
