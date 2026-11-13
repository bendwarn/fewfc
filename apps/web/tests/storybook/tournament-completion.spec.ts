import { expect, test } from '@playwright/test'

const scenarios = [
  ['pending', '賽果：尚無賽果'],
  ['single-final-vacancy', '決賽雙負，冠軍空缺'],
  ['single-bronze-vacancy', '季軍賽雙負，第三名與第四名空缺'],
  ['single-semifinal-vacancy', '半決賽雙負後，唯一勝者輪空取得冠軍；正常敗者輪空取得季軍，亞軍空缺'],
  ['double-final-vacancy', '總決賽雙負，冠軍與亞軍空缺；保留已決出的季軍'],
  ['double-reset-vacancy', '重置賽雙負，冠軍與亞軍空缺；保留已決出的季軍'],
  ['zero-and-one-team', '階段已取消'],
  ['settled-then-active', '決賽雙負，冠軍空缺'],
  ['single-final-vacancy-dark', '決賽雙負，冠軍空缺'],
  ['double-reset-vacancy-dark', '重置賽雙負，冠軍與亞軍空缺；保留已決出的季軍'],
] as const

for (const viewport of [{ width: 1440, height: 900 }, { width: 390, height: 844 }]) {
  for (const [story, text] of scenarios) {
    test(`${story} at ${viewport.width}px`, async ({ page }, testInfo) => {
      await page.setViewportSize(viewport)
      await page.goto(`/iframe.html?id=${encodeURIComponent(`競賽-目前賽程--${story}`)}&viewMode=story`)
      await expect(page.getByText(text, { exact: true })).toBeVisible()
      await expect(page.locator('article.stage-card').first()).toBeVisible()
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
      if (story.startsWith('single-final') || story.startsWith('double-final') || story.startsWith('double-reset')) {
        const stage = page.locator('article.stage-card').first()
        await expect(stage.locator('.standing-card').filter({ has: page.getByRole('heading', { name: 'team-a', exact: true }) }).locator('.rank-label')).toHaveCount(0)
        await expect(stage.getByText('第 3 名', { exact: true })).toBeVisible()
      }
      if (story === 'settled-then-active') {
        await expect(page.getByText('排名未確定', { exact: true })).toHaveCount(2)
        await expect(page.getByText('本場賽事已完成', { exact: true })).toHaveCount(0)
      }
      if (story === 'single-bronze-vacancy') {
        const stage = page.locator('article.stage-card').first()
        await expect(stage.locator('.standing-card').filter({ has: page.getByRole('heading', { name: 'team-c', exact: true }) }).locator('.rank-label')).toHaveCount(0)
        await expect(stage.getByText('第 1 名', { exact: true })).toBeVisible()
      }
      if (story === 'single-semifinal-vacancy') {
        await expect(page.getByText('第 4 名因雙負空缺', { exact: true })).toBeVisible()
      }
      if (['single-final-vacancy', 'single-bronze-vacancy', 'single-final-vacancy-dark', 'double-reset-vacancy-dark'].includes(story)) {
        await page.screenshot({ path: testInfo.outputPath(`${story}-${viewport.width}.png`), fullPage: true })
      }
    })
  }
}
