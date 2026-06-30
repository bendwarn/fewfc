import { expect, test, type Page } from '@playwright/test'

async function loginAsGuest(page: Page) {
  await page.goto('/login')
  await page.getByRole('button', { name: '以訪客身份遊玩' }).click()
  await expect(page).toHaveURL(/\/rooms(?:\?.*)?$/)
}

test('new rooms enable Five Directions Legend and expose the shared environment', async ({ browser }) => {
  test.setTimeout(180_000)

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()

  try {
    await Promise.all([loginAsGuest(host), loginAsGuest(guest)])

    const roomName = `五方傳說測試 ${Date.now()}`
    await host.getByRole('button', { name: '建立房間', exact: true }).click()
    const createDialog = host.getByRole('dialog', { name: '建立房間' })
    await expect(createDialog.getByText('規則模組')).toHaveCount(0)
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
      expect(host.locator('.environment-badge')).toHaveText('環境 · 無環境'),
      expect(guest.locator('.environment-badge')).toHaveText('環境 · 無環境'),
    ])
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})
