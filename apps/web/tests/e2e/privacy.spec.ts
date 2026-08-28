import { expect, test } from '@playwright/test'

test('privacy policy is public and exposes contact and provider policies', async ({ page }) => {
  await page.goto('/privacy')

  await expect(page).toHaveURL('/privacy')
  await expect(page).toHaveTitle('隱私權政策｜五行戰鬥牌')
  await expect(page.getByRole('heading', { level: 1, name: '隱私權政策' })).toBeVisible()
  await expect(page.getByText('我們不出售個人資料，也不將 Google 或 GitHub 提供的資料用於廣告投放。')).toBeVisible()

  await expect(page.getByRole('link', { name: 'rexkimta@gmail.com' })).toHaveAttribute('href', 'mailto:rexkimta@gmail.com')
  await expect(page.getByRole('link', { name: 'Google 隱私權政策' })).toHaveAttribute('href', 'https://policies.google.com/privacy')
  await expect(page.getByRole('link', { name: 'GitHub 一般隱私聲明' })).toHaveAttribute(
    'href',
    'https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement',
  )
  await expect(page.locator('link[rel="canonical"]')).toHaveAttribute('href', 'https://fewfc.febcg.workers.dev/privacy')
})
