import { createPublicRoom, expect, joinListedRoom, loginAsGuests, test } from './fixtures'

test('new rooms enable Five Directions Legend and hide the environment until one exists', async ({ browser }) => {

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()

  try {
    await loginAsGuests([host, guest])

    const roomName = `五方傳說測試 ${Date.now()}`
    await createPublicRoom(host, roomName)
    await expect(host.getByLabel('五方傳說')).toBeChecked()

    await joinListedRoom(guest, roomName)
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
