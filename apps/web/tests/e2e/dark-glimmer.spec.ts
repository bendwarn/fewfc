import { expect, test } from '@playwright/test'

test('Dark Glimmer defaults on and enables Spirit transitively', async ({ page }) => {
  test.setTimeout(180_000)
  await page.goto('/login')
  await page.getByRole('button', { name: '以訪客身份遊玩' }).click()
  await page.getByRole('button', { name: '建立房間', exact: true }).click()
  await page.getByLabel('房間名稱').fill(`黑暗微光測試 ${Date.now()}`)
  await page.getByRole('button', { name: '建立房間 →' }).click()

  await expect(page.getByLabel('主題規則‧黑暗微光')).toBeChecked()
  await expect(page.getByLabel('主題規則‧精靈')).toBeChecked()

  await page.getByLabel('主題規則‧精靈').uncheck()
  await expect(page.getByLabel('主題規則‧黑暗微光')).not.toBeChecked()

  await page.getByLabel('主題規則‧黑暗微光').check()
  await expect(page.getByLabel('主題規則‧精靈')).toBeChecked()
  await expect(page.getByLabel('進階規則‧星辰圖記')).toBeChecked()
  await expect(page.getByLabel('進階規則‧英雄學派')).toBeChecked()
  await expect(page.getByLabel('進階規則‧五方傳說')).toBeChecked()
})
