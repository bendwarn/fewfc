import { expect, setupFastWaitingRoom, test } from './fixtures'

test('Dark Glimmer defaults on and enables Spirit transitively', async ({ browser }) => {
  const room = await setupFastWaitingRoom(browser, {
    roomName: `黑暗微光測試 ${Date.now()}`,
  })
  const { page } = room

  try {
    await expect(page.getByLabel('黑暗微光')).toBeChecked()
    await expect(page.getByLabel('精靈')).toBeChecked()

    await page.getByLabel('精靈').uncheck()
    await expect(page.getByLabel('黑暗微光')).not.toBeChecked()

    await page.getByLabel('黑暗微光').check()
    await expect(page.getByLabel('精靈')).toBeChecked()
    await expect(page.getByLabel('星辰圖記')).toBeChecked()
    await expect(page.getByLabel('英雄學派')).toBeChecked()
    await expect(page.getByLabel('五方傳說')).toBeChecked()
  } finally {
    await room.close()
  }
})
