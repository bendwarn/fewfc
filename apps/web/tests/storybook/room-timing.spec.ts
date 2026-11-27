import { expect, test } from '@playwright/test'

const story = (name: string) => `/iframe.html?id=${encodeURIComponent('頁面-正式房間')}--${name}&viewMode=story`

test('等待房間顯示設定的操作時限', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 })
  await page.goto(story('timed-waiting'))
  await expect(page.getByText('限時模式 · 每次操作 60 秒')).toBeVisible()
})

test('選擇初始錦囊時也可讀取剩餘時間', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 })
  await page.goto(story('timed-preparation'))
  const choice = page.getByRole('dialog', { name: '選擇初始錦囊' })
  await expect(choice.getByRole('timer', { name: '剩餘操作時間' })).toBeVisible()
})

for (const width of [1366, 390]) {
  test(`對局在 ${width}px 顯示操作玩家及剩餘時間`, async ({ page }) => {
    await page.setViewportSize({ width, height: 844 })
    await page.goto(story('timed-playing'))
    await expect(page.getByText('等待 小明 操作')).toBeVisible()
    const timer = page.getByRole('timer', { name: '剩餘操作時間' })
    await expect(timer).toHaveText(/0:5[78]/)
    await expect(timer).toHaveText(/0:5[56]/)
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
  })
}
