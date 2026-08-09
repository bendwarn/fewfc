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

test('landing layout keeps its hero readable without horizontal overflow', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 })
  await page.goto('/')

  const desktopLayout = await page.evaluate(() => {
    const copy = document.querySelector('.landing-hero-copy')?.getBoundingClientRect()
    const visual = document.querySelector('.hero-visual')?.getBoundingClientRect()
    return {
      fitsViewport: document.documentElement.scrollWidth <= window.innerWidth,
      columnsAreSeparated: Boolean(copy && visual && copy.right <= visual.left),
    }
  })
  expect(desktopLayout).toEqual({ fitsViewport: true, columnsAreSeparated: true })

  await page.setViewportSize({ width: 375, height: 812 })
  await expect(page.getByRole('link', { name: '五行戰鬥牌官方網站' }).first()).toBeVisible()
  await expect(page.getByRole('button', { name: /^外觀：/ })).toBeVisible()
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
})

test('login hero copy stays above the decorative orbit when they overlap', async ({ page }) => {
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
