import {
  expect,
  reloadFastGameRoute,
  seedDevelopmentScenario,
  setupFastTwoPlayerGame,
  test,
} from './fixtures'

test('a Profession change and activated ability survive public reconnect', async ({ browser }) => {
  const game = await setupFastTwoPlayerGame(browser, {
    roomName: `英雄學派測試 ${Date.now()}`,
    enabledRuleModules: ['hero-schools'],
  })
  const { host, guest, gameId: roomId } = game

  try {
    await seedDevelopmentScenario(host, { name: 'hero-schools-transition' })

    await expect(host.locator('.profession-badge')).toContainText('幻術師')
    await expect(guest.locator('.profession-badge')).toContainText('幻術師')
    const selectable = host.locator('.playing-card:enabled:not(.hidden)')
    await expect(selectable).toHaveCount(5)
    await selectable.nth(0).click()
    await selectable.nth(1).click()
    const illusion = host
      .getByRole('region', { name: '能力' })
      .getByRole('button', { name: /^幻術：/ })
      .first()
    await expect(illusion).toBeVisible()
    const [activationResponse] = await Promise.all([
      host.waitForResponse(response => (
        response.url().endsWith(`/api/games/${roomId}/commands`)
        && response.request().postDataJSON()?.action?.type === 'activateProfessionAbility'
      )),
      illusion.click(),
    ])
    expect(activationResponse.ok()).toBe(true)

    await reloadFastGameRoute(host, roomId)
    await expect(host.locator('.event-feed')).toContainText('虛擬牌')
    await expect(host.locator('.card-interpretation')).toHaveCount(0)
    await reloadFastGameRoute(guest, roomId)
    await expect(guest.locator('.event-feed')).toContainText('虛擬牌')
    await expect(guest.locator('.card-interpretation')).toHaveCount(0)
  } finally {
    await game.close()
  }
})
