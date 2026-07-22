import { expect, setupFastWaitingRoom, test } from './fixtures'

test('Confluence Generation defaults on and keeps Advanced dependencies coherent', async ({ browser }) => {
  const room = await setupFastWaitingRoom(browser, {
    roomName: `匯流世代測試 ${Date.now()}`,
  })
  const { page } = room

  try {
    await expect(page.getByLabel('匯流世代')).toBeChecked()
    await page.getByLabel('星辰圖記').uncheck()
    await expect(page.getByLabel('匯流世代')).not.toBeChecked()

    await page.getByLabel('匯流世代').check()
    await expect(page.getByLabel('星辰圖記')).toBeChecked()
    await expect(page.getByLabel('英雄學派')).toBeChecked()
    await expect(page.getByLabel('五方傳說')).toBeChecked()
  } finally {
    await room.close()
  }
})
