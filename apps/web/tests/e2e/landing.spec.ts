import { expect, test } from '@playwright/test'

test('public homepage exposes metadata and navigates to login', async ({ page }) => {
  await page.goto('/')

  await expect(page).toHaveTitle('五行戰鬥牌')
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
