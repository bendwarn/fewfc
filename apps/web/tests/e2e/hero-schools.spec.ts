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
    let activationRequests = 0
    host.on('request', (request) => {
      if (
        request.url().endsWith(`/api/games/${roomId}/commands`)
        && request.postDataJSON()?.action?.type === 'activateProfessionAbility'
      ) {
        activationRequests += 1
      }
    })
    const selectable = host.locator('.playing-card:enabled:not(.hidden)')
    await expect(selectable).toHaveCount(5)
    await selectable.nth(0).click()
    await selectable.nth(1).click()
    const illusion = host
      .getByRole('region', { name: '能力' })
      .getByRole('button', { name: /^幻術/ })
    await expect(illusion).toHaveCount(1)
    await illusion.click()
    const virtualCardDialog = host.getByRole('dialog', { name: '幻術：選擇虛擬牌' })
    await expect(virtualCardDialog).toBeVisible()
    expect(activationRequests).toBe(0)

    await reloadFastGameRoute(host, roomId)
    await expect(virtualCardDialog).toHaveCount(0)
    expect(activationRequests).toBe(0)

    await selectable.nth(0).click()
    await selectable.nth(1).click()
    await expect(illusion).toHaveCount(1)
    await illusion.click()
    await expect(virtualCardDialog).toBeVisible()
    await host.keyboard.press('Escape')
    await expect(virtualCardDialog).toHaveCount(0)
    expect(activationRequests).toBe(0)
    await expect(illusion).toHaveCount(1)

    await illusion.click()
    await virtualCardDialog.getByRole('button', { name: '取消' }).click()
    await expect(virtualCardDialog).toHaveCount(0)
    expect(activationRequests).toBe(0)

    await illusion.click()
    const [activationResponse] = await Promise.all([
      host.waitForResponse(response => (
        response.url().endsWith(`/api/games/${roomId}/commands`)
        && response.request().postDataJSON()?.action?.type === 'activateProfessionAbility'
      )),
      virtualCardDialog.getByRole('button', { name: '幻術：火行 3 級' }).click(),
    ])
    expect(activationResponse.ok()).toBe(true)
    expect(activationResponse.request().postDataJSON()?.action).toMatchObject({
      abilityId: 'illusion',
      declaredElement: 'Fire',
      declaredLevel: 3,
    })
    await expect(virtualCardDialog).toHaveCount(0)
    expect(activationRequests).toBe(1)

    await reloadFastGameRoute(host, roomId)
    await expect(host.locator('.event-feed')).toContainText('虛擬牌')
    await expect(host.locator('.card-interpretation-badge')).toHaveCount(0)
    await reloadFastGameRoute(guest, roomId)
    await expect(guest.locator('.event-feed')).toContainText('虛擬牌')
    await expect(guest.locator('.card-interpretation-badge')).toHaveCount(0)
  } finally {
    await game.close()
  }
})
