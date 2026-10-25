import { expect, fastPageTest, setupFastTwoPlayerGame, test } from './fixtures'

fastPageTest('account menu navigates to the personal deck editor', async ({ page }) => {
  await page.getByRole('button', { name: /旅人-/ }).click()
  await page.getByRole('button', { name: '個人牌組', exact: true }).click()

  await expect(page).toHaveURL('/deck')
  await expect(page.getByRole('heading', { name: '個人牌組' })).toBeVisible()
})

test('default room rules lock preconstructed decks and start personal piles', async ({ browser }) => {
  const game = await setupFastTwoPlayerGame(browser, {
    roomName: `個人牌組測試 ${Date.now()}`,
    enabledRuleModules: ['discard-retrieval', 'personal-deck', 'five-directions-legend'],
    activeMatch: false,
  })
  const { host, guest } = game

  try {
    await expect(host.getByLabel('棄牌回收')).toBeChecked()
    await expect(host.getByLabel('個人牌組')).toBeChecked()

    await expect(guest.getByLabel('棄牌回收')).toBeChecked()
    await expect(guest.getByLabel('個人牌組')).toBeChecked()
    await expect(guest.getByLabel('五方傳說')).toBeChecked()
    await expect(guest.getByLabel('個人牌組')).toBeDisabled()

    await game.start()

    const pileCounts = host.locator('.seat-discard-control .discard-counts')
    await expect(pileCounts).toHaveCount(2)
    for (const text of await pileCounts.allTextContents()) expect(text).toMatch(/^0—5[56]$/)
  } finally {
    await game.close()
  }
})
