import { expect, setupFastTwoPlayerGame, test } from './fixtures'

test('new rooms enable Five Directions Legend and hide the environment until one exists', async ({ browser }) => {
  const game = await setupFastTwoPlayerGame(browser, {
    roomName: `五方傳說測試 ${Date.now()}`,
  })
  const { host, guest } = game

  try {
    await expect(host.getByRole('region', { name: '啟用規則' })).toContainText('五方傳說')

    await Promise.all([
      expect(host.locator('.formation-field-heading')).toHaveText('陣法區'),
      expect(guest.locator('.formation-field-heading')).toHaveText('陣法區'),
    ])
    await expect(host.locator('.environment-badge')).toHaveCount(0)
    await expect(guest.locator('.environment-badge')).toHaveCount(0)
  } finally {
    await game.close()
  }
})
