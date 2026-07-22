import { expect, test, type Locator } from '@playwright/test'

async function stackingAtOverlap(target: Locator, orbit: Locator) {
  return await target.evaluate((targetElement, orbitElement) => {
    const targetRect = targetElement.getBoundingClientRect()
    const orbitRect = orbitElement.getBoundingClientRect()
    const left = Math.max(targetRect.left, orbitRect.left)
    const right = Math.min(targetRect.right, orbitRect.right)
    const top = Math.max(targetRect.top, orbitRect.top)
    const bottom = Math.min(targetRect.bottom, orbitRect.bottom)

    if (left >= right || top >= bottom) {
      return { overlaps: false, targetAboveOrbit: false }
    }

    const elements = document.elementsFromPoint(
      left + (right - left) / 2,
      top + (bottom - top) / 2,
    )

    return {
      overlaps: true,
      targetAboveOrbit: elements.indexOf(targetElement) < elements.indexOf(orbitElement),
    }
  }, await orbit.elementHandle())
}

test('login hero copy is never obscured by the decorative orbit', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 720 })
  await page.goto('/login')

  const orbit = page.locator('.element-orbit')
  for (const target of [
    page.locator('.hero-copy h1'),
    page.locator('.hero-description'),
  ]) {
    await expect(target).toBeVisible()
    const stacking = await stackingAtOverlap(target, orbit)
    expect(!stacking.overlaps || stacking.targetAboveOrbit).toBe(true)
  }
})
