import { expect, test } from '@playwright/test'
import { setupFastTwoPlayerGame } from './fixtures'

test('appearance menu persists explicit themes and system mode stays live', async ({ page }) => {
  await page.goto('/login')

  const themeTrigger = page.getByRole('button', { name: /^外觀：/ })
  await expect(themeTrigger).toBeVisible()
  await themeTrigger.click()
  await page.getByRole('menuitemradio', { name: '亮色' }).click()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light')

  await page.reload()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light')
  await themeTrigger.click()
  await page.getByRole('menuitemradio', { name: '深色' }).click()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark')

  await page.emulateMedia({ colorScheme: 'dark' })
  await themeTrigger.click()
  await page.getByRole('menuitemradio', { name: '系統' }).click()
  await expect(page.locator('html')).not.toHaveAttribute('data-theme', /.+/)
  await expect.poll(() => page.evaluate(() => getComputedStyle(document.documentElement).colorScheme)).toBe('dark')

  await page.emulateMedia({ colorScheme: 'light' })
  await expect.poll(() => page.evaluate(() => getComputedStyle(document.documentElement).colorScheme)).toBe('light')
})

test('long pressing a card previews it without selecting it', async ({ browser }) => {
  const game = await setupFastTwoPlayerGame(browser)

  try {
    await expect.poll(async () => Math.max(...await Promise.all(game.pages.map(page => (
      page.locator('.playing-card:enabled:not(.hidden)').count()
    ))))).toBeGreaterThan(0)

    const page = (await Promise.all(game.pages.map(async page => ({
      page,
      count: await page.locator('.playing-card:enabled:not(.hidden)').count(),
    })))).find(candidate => candidate.count > 0)!.page
    const card = page.locator('.playing-card:enabled:not(.hidden)').first()

    await expect(card.locator('img')).toHaveAttribute('src', /^\/cards\//)
    await expect(card).toHaveAttribute('aria-pressed', 'false')
    await card.hover()
    await page.mouse.down()
    await expect(page.locator('.game-card-preview-layer')).toBeVisible()
    await page.mouse.up()
    await expect(page.locator('.game-card-preview-layer')).toHaveCount(0)
    await expect(card).toHaveAttribute('aria-pressed', 'false')

    await card.click()
    await expect(card).toHaveAttribute('aria-pressed', 'true')
  } finally {
    await game.close()
  }
})
