import { expect, test } from '@playwright/test'

const pages = [
  ['login', '登入對戰'], ['deck', '個人牌組'], ['account', '帳號設定'],
  ['replays', '重播紀錄'], ['replay', '重播'], ['competition', '競賽對局・論劍'],
  ['competition-details', '競賽對局・論劍'], ['privacy', '隱私權政策'],
] as const

for (const width of [1366, 390]) {
  for (const theme of ['', '-dark']) {
    for (const [screen, heading] of pages) {
      test(`${screen}${theme} keeps content and controls usable at ${width}`, async ({ page }) => {
        await page.setViewportSize({ width, height: 900 })
        await page.goto(`/iframe.html?id=${encodeURIComponent('頁面-完整功能頁')}--${screen}${theme}&viewMode=story`)
        await expect(page.getByRole('heading', { name: heading, exact: true }).first()).toBeVisible()
        expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
        for (const control of await page.locator('main input:not([type=hidden]), main select, main textarea, main button').all()) {
          if (!await control.isVisible()) continue
          const box = await control.boundingBox()
          expect(box!.x).toBeGreaterThanOrEqual(0)
          expect(box!.x + box!.width).toBeLessThanOrEqual(width + 1)
        }
        if (screen === 'deck') {
          await expect(page.getByRole('textbox', { name: '牌組名稱' })).toHaveValue('五行均衡預組')
          await page.getByRole('button', { name: '增加金1級' }).click()
          await expect(page.getByLabel('金1級張數', { exact: true })).toHaveText('4')
          await expect(page.getByRole('button', { name: '儲存牌組' })).toBeDisabled()
        }
        if (screen === 'replays') await expect(page.getByRole('button', { name: '觀看', exact: true })).toHaveCount(2)
        if (screen === 'replay') {
          await page.getByRole('button', { name: '下一步', exact: true }).click()
          await expect(page.getByText('第 1 / 8 步', { exact: true })).toBeVisible()
        }
      })
    }
    test(`waiting room${theme} exposes every seat and start action at ${width}`, async ({ page }) => {
      await page.setViewportSize({ width, height: 844 })
      await page.goto(`/iframe.html?id=${encodeURIComponent('房間-等待室版面')}--four-players${theme}&viewMode=story`)
      await expect(page.getByRole('heading', { name: '好友切磋' })).toBeVisible()
      await page.getByRole('button', { name: '開始遊戲 →' }).scrollIntoViewIfNeeded()
      await expect(page.getByRole('button', { name: '開始遊戲 →' })).toBeInViewport()
      await expect(page.getByText('第四席', { exact: false })).toBeVisible()
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
    })
  }
}
