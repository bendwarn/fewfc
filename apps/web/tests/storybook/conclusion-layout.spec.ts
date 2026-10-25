import { expect, test } from '@playwright/test'

const story = (name: string) => `/iframe.html?id=${encodeURIComponent('對局-完整戰場畫面')}--${name}&viewMode=story`

for (const viewport of [{ width: 1440, height: 900 }, { width: 1366, height: 768 }, { width: 1024, height: 720 }, { width: 760, height: 612 }, { width: 390, height: 844 }, { width: 360, height: 640 }, { width: 320, height: 640 }]) {
  for (const theme of ['terminal-victory', 'terminal-victory-dark']) {
    test(`conclusion stays centered and readable: ${theme} at ${viewport.width}`, async ({ page }, testInfo) => {
      await page.setViewportSize(viewport)
      await page.goto(story(theme))
      const panel = page.getByRole('region', { name: '對局結算' })
      await expect(panel).toHaveCount(1)
      await expect(panel.getByRole('heading', { name: '我方 勝利' })).toBeVisible()
      await expect.poll(() => panel.evaluate(el => el.getAnimations().every(animation => animation.playState === 'finished'))).toBe(true)
      await expect(panel.getByRole('button', { name: '返回房間' })).toBeInViewport({ ratio: 1 })
      const board = await page.locator('.battlefield').boundingBox()
      const result = await panel.boundingBox()
      const center = await page.locator('.board-center').boundingBox()
      const top = await page.locator('.seat-top').boundingBox()
      const bottom = await page.locator('.seat-bottom').boundingBox()
      expect(Math.abs(result!.x + result!.width / 2 - board!.x - board!.width / 2)).toBeLessThan(2)
      expect(result!.y).toBeGreaterThanOrEqual(top!.y + top!.height)
      expect(result!.y + result!.height).toBeLessThanOrEqual(bottom!.y)
      for (const bounds of [board!, center!]) {
        expect(result!.x).toBeGreaterThanOrEqual(bounds.x - 1)
        expect(result!.y).toBeGreaterThanOrEqual(bounds.y - 1)
        expect(result!.x + result!.width).toBeLessThanOrEqual(bounds.x + bounds.width + 1)
        expect(result!.y + result!.height).toBeLessThanOrEqual(bounds.y + bounds.height + 1)
      }
      for (const card of await page.locator('.seat-bottom .playing-card').all()) {
        const box = await card.boundingBox()
        expect(box!.y).toBeGreaterThanOrEqual(bottom!.y)
        expect(box!.y + box!.height).toBeLessThanOrEqual(bottom!.y + bottom!.height)
      }
      const heading = await panel.locator('h2').boundingBox()
      const reason = await panel.locator('.result-reason').boundingBox()
      const summary = await panel.locator('.result-summary').boundingBox()
      const actions = await panel.locator('.result-actions').boundingBox()
      expect(heading!.y + heading!.height).toBeLessThanOrEqual(reason!.y)
      expect(reason!.y + reason!.height).toBeLessThanOrEqual(summary!.y)
      expect(summary!.y + summary!.height).toBeLessThanOrEqual(actions!.y)
      const cards = await panel.locator('.formation-card').all()
      expect(cards).toHaveLength(3)
      const texts = await panel.locator('small, strong, h2, .result-reason, .result-actions').all()
      for (const card of cards) {
        const cardBox = (await card.boundingBox())!
        for (const text of texts) {
          const textBox = (await text.boundingBox())!
          expect(cardBox.x + cardBox.width <= textBox.x || textBox.x + textBox.width <= cardBox.x
            || cardBox.y + cardBox.height <= textBox.y || textBox.y + textBox.height <= cardBox.y).toBe(true)
        }
      }
      for (let index = 1; index < cards.length; index += 1) {
        const before = await cards[index - 1]!.boundingBox()
        const after = await cards[index]!.boundingBox()
        expect(after!.x).toBeGreaterThanOrEqual(before!.x + before!.width)
      }
      const pile = await page.locator('.shared-discard-dock').boundingBox()
      expect(pile!.x + pile!.width <= result!.x || pile!.y + pile!.height <= result!.y).toBe(true)
      expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBe(0)
      expect(await page.locator('.battlefield').evaluate(el => el.scrollWidth - el.clientWidth)).toBe(0)
      expect(await page.locator('.terminal-resolution').evaluate(el => el.scrollHeight - el.clientHeight)).toBe(0)
      expect(await panel.evaluate(el => el.scrollHeight - el.clientHeight)).toBe(0)
      await page.screenshot({ path: testInfo.outputPath('conclusion.png'), fullPage: true })
    })
  }
}

test('distinct conclusions and perspectives retain their summary and next action', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 })
  for (const [name, heading, summary, action] of [
    ['terminal-draw', '平局', '我方、對方 的生命值歸零', '返回房間'],
    ['terminal-direct-victory', '我方 勝利', '達成五星連珠', '返回賽事'],
    ['terminal-automatic', '我方 勝利', '中毒效果', '返回房間'],
    ['terminal-discard-retrieval', '我方 勝利', '取回 小華 的棄牌', '返回房間'],
    ['terminal-without-resolution', '我方 勝利', '對方 的生命值歸零', '返回房間'],
    ['terminal-observer', '先手方 勝利', '1 張蓋牌', '離開觀戰'],
    ['terminal-replay', '對方 勝利', '烈焰陣', ''],
  ]) {
    await page.goto(story(name!))
    const panel = page.getByRole('region', { name: '對局結算' })
    await expect(panel.getByRole('heading', { name: heading, exact: true })).toBeVisible()
    await expect(panel.getByText(summary!, { exact: true })).toBeVisible()
    if (action) await expect(panel.getByRole('button', { name: action })).toBeInViewport()
    if (name === 'terminal-replay') {
      expect(await panel.evaluate(el => getComputedStyle(el).animationName)).toBe('none')
    }
  }
})

test('reduced motion shows a settled conclusion immediately', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await page.goto(story('terminal-victory-dark'))
  const panel = page.getByRole('region', { name: '對局結算' })
  await expect(panel).toBeVisible()
  for (const element of [panel, panel.locator('h2')]) {
    expect(await element.evaluate(el => getComputedStyle(el).animationName)).toBe('none')
  }
})

for (const width of [1366, 390]) {
  test(`replay stepping and existing record navigation work at ${width}`, async ({ page }) => {
    await page.setViewportSize({ width, height: 844 })
    await page.goto(`/iframe.html?id=${encodeURIComponent('頁面-完整功能頁')}--replay-finished&viewMode=story`)
    const panel = page.getByRole('region', { name: '對局結算' })
    await expect(panel).toBeVisible()
    if (width > 900) {
      await expect(page.locator('.event-panel').getByRole('heading', { name: '戰局紀錄' })).toBeVisible()
    } else {
      const trigger = page.locator('.replay-mobile-record-summary')
      await trigger.click()
      const dialog = page.getByRole('dialog', { name: '戰局紀錄' })
      await expect(dialog.getByRole('button', { name: '關閉戰局紀錄' })).toBeFocused()
      await page.keyboard.press('Escape')
      await expect(trigger).toBeFocused()
    }
    await page.getByRole('button', { name: '上一步', exact: true }).click()
    await expect(page.getByRole('region', { name: '對局結算' })).toHaveCount(0)
    await page.getByRole('button', { name: '最後一步', exact: true }).click()
    await expect(panel).toBeVisible()
    expect(await panel.evaluate(el => getComputedStyle(el).animationName)).toBe('none')
  })
}
