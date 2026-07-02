import { expect, test, type Page } from '@playwright/test'

async function loginAsGuest(page: Page) {
  await page.goto('/login')
  await page.getByRole('button', { name: '以訪客身份遊玩' }).click()
  await expect(page).toHaveURL(/\/rooms(?:\?.*)?$/)
}

test('new rooms enable Five Directions Legend and hide the environment until one exists', async ({ browser }) => {
  test.setTimeout(180_000)

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()

  try {
    await Promise.all([loginAsGuest(host), loginAsGuest(guest)])

    const roomName = `五方傳說測試 ${Date.now()}`
    await host.getByRole('button', { name: '建立房間', exact: true }).click()
    await host.getByLabel('房間名稱').fill(roomName)
    await host.getByRole('button', { name: '公開房間', exact: true }).click()
    await host.getByRole('button', { name: '建立房間 →' }).click()
    await expect(host.getByLabel('進階規則‧五方傳說')).toBeChecked()

    const listedRoom = guest.locator('.public-room-list button').filter({ hasText: roomName })
    await expect(listedRoom).toBeVisible()
    await listedRoom.click()
    await guest.getByRole('button', { name: '準備 →' }).click()

    const startButton = host.getByRole('button', { name: '開始遊戲 →' })
    await expect(startButton).toBeEnabled()
    await startButton.click()

    await Promise.all([
      expect(host.locator('.formation-field-heading')).toHaveText('陣法區'),
      expect(guest.locator('.formation-field-heading')).toHaveText('陣法區'),
    ])
    await expect(host.locator('.environment-badge')).toHaveCount(0)
    await expect(guest.locator('.environment-badge')).toHaveCount(0)
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})
