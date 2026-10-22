import { expect, test } from '@playwright/test'

const story = (name: string) => `/iframe.html?id=${encodeURIComponent('大廳-完整大廳畫面')}--${name}&viewMode=story`

for (const width of [1366, 390]) {
  test(`lobby entries and create dialog stay reachable at ${width}`, async ({ page }) => {
    await page.setViewportSize({ width, height: 844 })
    await page.goto(story('light-lobby'))
    await expect(page.getByRole('heading', { name: '房間', exact: true })).toBeVisible()
    const publicRooms = await page.getByRole('region', { name: '公開房間' }).boundingBox()
    const myRooms = await page.getByRole('region', { name: '我的房間' }).boundingBox()
    if (width > 900) expect(publicRooms!.y).toBe(myRooms!.y)
    else expect(myRooms!.y).toBeGreaterThan(publicRooms!.y + publicRooms!.height)
    expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBe(0)
    const create = page.getByRole('button', { name: '建立房間', exact: true })
    await create.click()
    const dialog = page.getByRole('dialog', { name: '建立房間' })
    await expect(dialog.getByLabel('房間名稱')).toBeFocused()
    await dialog.getByRole('button', { name: /團隊對戰/ }).click()
    await expect(dialog.getByRole('button', { name: /團隊對戰/ })).toHaveAttribute('aria-pressed', 'true')
    await dialog.getByRole('button', { name: '取消' }).click()
    await expect(create).toBeFocused()
    expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBe(0)
  })
}
