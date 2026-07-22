import { expect, reloadFastGameRoute, setupFastTwoPlayerGame, test } from './fixtures'

test('Jianghu defaults on, preserves dependencies, and survives reconnect', async ({ browser }) => {
  const game = await setupFastTwoPlayerGame(browser, {
    roomName: `江湖規則測試 ${Date.now()}`,
    enabledRuleModules: ['five-directions-legend', 'star', 'hero-schools', 'jianghu'],
    activeMatch: false,
  })
  const { host, guest, gameId: roomId } = game

  try {
    await expect(host.getByLabel('江湖')).toBeChecked()
    await expect(host.getByLabel('星辰圖記')).toBeChecked()
    await expect(host.getByLabel('英雄學派')).toBeChecked()
    await expect(host.getByLabel('五方傳說')).toBeChecked()

    await host.getByLabel('英雄學派').uncheck()
    await expect(host.getByLabel('江湖')).not.toBeChecked()
    await expect(guest.getByLabel('江湖')).not.toBeChecked()

    await host.getByLabel('江湖').check()
    await expect(host.getByLabel('星辰圖記')).toBeChecked()
    await expect(host.getByLabel('英雄學派')).toBeChecked()
    await expect(host.getByLabel('五方傳說')).toBeChecked()

    await game.start()
    await expect(host.getByRole('region', { name: '啟用規則' }))
      .toContainText('江湖')

    await reloadFastGameRoute(guest, roomId)
    await expect(guest.getByRole('region', { name: '啟用規則' }))
      .toContainText('江湖')
  } finally {
    await game.close()
  }
})
