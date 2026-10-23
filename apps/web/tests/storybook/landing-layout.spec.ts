import { expect, test } from '@playwright/test'

const story = (title: string, variant: string) => `/iframe.html?id=${encodeURIComponent(title)}--${variant}&viewMode=story`

for (const theme of ['light', 'dark']) {
  for (const width of [1440, 1024, 390]) {
    test(`homepage content and entries fit ${theme} at ${width}`, async ({ page }) => {
      await page.setViewportSize({ width, height: 1000 })
      await page.emulateMedia({ reducedMotion: 'reduce' })
      await page.goto(story('頁面-首頁', theme))
      await expect(page.getByRole('heading', { level: 1 })).toBeVisible()
      await expect(page.getByRole('link', { name: '立即遊玩', exact: true })).toHaveAttribute('href', '/login')
      await expect(page.getByRole('link', { name: '五行戰鬥牌官方網站', exact: true }).first()).toBeVisible()
      await expect(page.getByRole('button', { name: /^外觀：/ })).toBeVisible()
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
      if (width > 920) {
        const copy = await page.locator('.landing-hero-copy').boundingBox()
        const visual = await page.locator('.hero-visual').boundingBox()
        expect(copy!.x + copy!.width).toBeLessThanOrEqual(visual!.x)
      }
      for (const card of await page.locator('.hero-card').all()) {
        const box = await card.boundingBox()
        expect(box!.x).toBeGreaterThanOrEqual(0)
        expect(box!.x + box!.width).toBeLessThanOrEqual(width)
      }
      await page.getByRole('link', { name: '了解玩法', exact: true }).click()
      await expect(page.getByRole('heading', { name: '三步開局，每手都是選擇' })).toBeInViewport()
      const cards = page.getByRole('list', { name: '五行卡牌展示' })
      await cards.scrollIntoViewIfNeeded()
      await expect(cards.getByRole('listitem')).toHaveCount(5)
      expect(await cards.evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true)
    })
  }
}

for (const width of [1366, 390]) {
  test(`shared player menu and notifications remain reachable at ${width}`, async ({ page }) => {
    await page.setViewportSize({ width, height: 844 })
    await page.goto(story('共用-玩家導覽與通知', 'dark'))
    const trigger = page.getByRole('button', { name: '小明的玩家選單' })
    await trigger.click()
    await expect(page.getByRole('button', { name: '帳號設定', exact: true })).toBeInViewport()
    await expect(page.getByRole('button', { name: '登出', exact: true })).toBeInViewport()
    await trigger.press('Escape')
    await expect(trigger).toBeFocused()
    await expect(trigger).toHaveAttribute('aria-expanded', 'false')
    await expect(page.getByRole('button', { name: /好友切磋：輪到你的回合/ })).toBeInViewport()
    await expect(page.getByRole('status')).toHaveText('你已離開原來的房間。')
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
  })
}
