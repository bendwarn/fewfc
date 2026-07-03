import { expect, test } from '@playwright/test'

test('Confluence Generation defaults on and keeps Advanced dependencies coherent', async ({ page }) => {
  test.setTimeout(180_000)
  await page.goto('/login')
  await page.getByRole('button', { name: '以訪客身份遊玩' }).click()
  await page.getByRole('button', { name: '建立房間', exact: true }).click()
  await page.getByLabel('房間名稱').fill(`匯流世代測試 ${Date.now()}`)
  await page.getByRole('button', { name: '建立房間 →' }).click()

  await expect(page.getByLabel('主題規則‧匯流世代')).toBeChecked()
  await page.getByLabel('進階規則‧星辰圖記').uncheck()
  await expect(page.getByLabel('主題規則‧匯流世代')).not.toBeChecked()

  await page.getByLabel('主題規則‧匯流世代').check()
  await expect(page.getByLabel('進階規則‧星辰圖記')).toBeChecked()
  await expect(page.getByLabel('進階規則‧英雄學派')).toBeChecked()
  await expect(page.getByLabel('進階規則‧五方傳說')).toBeChecked()
})
