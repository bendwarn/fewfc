import { createPublicRoom, expect, joinListedRoom, loginAsGuests, test } from './fixtures'

test('Jianghu defaults on, preserves dependencies, and survives reconnect', async ({ browser }) => {
  test.setTimeout(180_000)

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()

  try {
    await loginAsGuests([host, guest])
    const roomName = `江湖規則測試 ${Date.now()}`
    await createPublicRoom(host, roomName)

    await expect(host.getByLabel('主題規則‧江湖')).toBeChecked()
    await expect(host.getByLabel('進階規則‧星辰圖記')).toBeChecked()
    await expect(host.getByLabel('進階規則‧英雄學派')).toBeChecked()
    await expect(host.getByLabel('進階規則‧五方傳說')).toBeChecked()

    await joinListedRoom(guest, roomName)
    await guest.getByRole('button', { name: '準備 →' }).click()

    await host.getByLabel('進階規則‧英雄學派').uncheck()
    await expect(host.getByLabel('主題規則‧江湖')).not.toBeChecked()
    await expect(guest.getByLabel('主題規則‧江湖')).not.toBeChecked()

    await host.getByLabel('主題規則‧江湖').check()
    await expect(host.getByLabel('進階規則‧星辰圖記')).toBeChecked()
    await expect(host.getByLabel('進階規則‧英雄學派')).toBeChecked()
    await expect(host.getByLabel('進階規則‧五方傳說')).toBeChecked()

    await guest.getByRole('button', { name: '準備 →' }).click()
    await host.getByRole('button', { name: '開始遊戲 →' }).click()
    await expect(host.getByRole('region', { name: '啟用規則' }))
      .toContainText('主題規則‧江湖')

    await guest.reload()
    await expect(guest.getByRole('region', { name: '啟用規則' }))
      .toContainText('主題規則‧江湖')
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})
