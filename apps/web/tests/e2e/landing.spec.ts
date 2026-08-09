import { expect, test } from '@playwright/test'

test('public homepage presents the game, metadata, and login entry', async ({ page }) => {
  await page.goto('/')

  await expect(page).toHaveTitle('五行戰鬥牌')
  await expect(page.getByRole('heading', { level: 1, name: '以牌為陣， 決勝五行。' })).toBeVisible()
  await expect.poll(() => page.evaluate(() => (
    getComputedStyle(document.querySelector('#hero-title')!).color === getComputedStyle(document.body).color
  ))).toBe(true)
  await expect(page.getByRole('heading', { name: '三步開局，每手都是選擇' })).toBeVisible()
  await expect(page.getByRole('heading', { name: '一組牌，展開層層戰術' })).toBeVisible()
  await expect(page.getByRole('list', { name: '五行卡牌展示' }).getByRole('listitem')).toHaveCount(5)

  const officialSite = page.getByRole('link', { name: '五行戰鬥牌官方網站' }).first()
  await expect(officialSite).toHaveAttribute('href', 'https://www.cfecards.org/')
  await expect(officialSite).toHaveAttribute('target', '_blank')

  await expect(page.locator('meta[name="robots"]')).toHaveAttribute('content', 'noindex, nofollow')
  await expect(page.locator('link[rel="canonical"]')).toHaveAttribute('href', 'https://fewfc.febcg.workers.dev/')
  await expect(page.locator('meta[property="og:image"]')).toHaveAttribute('content', 'https://fewfc.febcg.workers.dev/og-image.png')
  await expect(page.locator('link[rel="icon"][type="image/svg+xml"]')).toHaveAttribute('href', '/favicon.svg')
  await expect(page.locator('link[rel="apple-touch-icon"]')).toHaveAttribute('href', '/apple-touch-icon.png')
  const structuredData = JSON.parse(await page.locator('script[type="application/ld+json"]').textContent() ?? '{}')
  expect(structuredData['@graph'].map((entry: { '@type': string }) => entry['@type'])).toEqual(['WebSite', 'VideoGame'])

  await page.getByRole('link', { name: '立即遊玩' }).first().click()
  await expect(page).toHaveURL('/login')
  await expect(page).toHaveTitle('登入｜五行戰鬥牌')
})
