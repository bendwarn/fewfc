import { expect, test } from '@playwright/test'

test('privacy policy has a public route and page metadata', async ({ page }) => {
  await page.goto('/privacy')

  await expect(page).toHaveURL('/privacy')
  await expect(page).toHaveTitle('隱私權政策｜五行戰鬥牌')
  await expect(page.locator('link[rel="canonical"]')).toHaveAttribute('href', 'https://fewfc.febcg.workers.dev/privacy')
})
