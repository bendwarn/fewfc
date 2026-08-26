import {
  expect,
  reloadFastGameRoute,
  seedDevelopmentScenario,
  setupFastTwoPlayerGame,
  test,
} from './fixtures'

test('Pure Fire target choice is private, accessible, and reconnectable', async ({ browser }) => {
  const game = await setupFastTwoPlayerGame(browser, {
    roomName: `淨火選擇測試 ${Date.now()}`,
  })
  const { host, guest, gameId: roomId } = game

  try {
    await seedDevelopmentScenario(host, { name: 'echo-pure-fire' })

    await reloadFastGameRoute(host, roomId)
    await expect(host.getByRole('heading', { name: '變徵‧淨火：選擇受影響玩家' })).toBeVisible()
    const targets = host.getByLabel('選擇玩家').getByRole('button')
    await expect(targets).toHaveCount(2)

    await reloadFastGameRoute(guest, roomId)
    await expect(guest.getByText('變徵‧淨火：選擇受影響玩家', { exact: true })).toBeVisible()
    await expect(guest.getByLabel('選擇玩家')).toHaveCount(0)

    const [answer] = await Promise.all([
      host.waitForResponse(response => (
        response.url().endsWith(`/api/games/${roomId}/commands`)
        && response.request().postDataJSON()?.action?.type === 'answerChoice'
      )),
      targets.nth(1).click(),
    ])
    expect(answer.ok(), await answer.text()).toBe(true)
    await expect(host.getByText('施展「變徵‧淨火」', { exact: true })).toBeVisible()
    await host.locator('.choice-overlay .choice-cards button:enabled').first().click()
    await expect(host.locator('.choice-overlay')).toBeHidden()
    const effectSummary = host.getByRole('button', { name: /^\s*效果 1\s*$/ })
    await effectSummary.click()
    const effectDetail = host.getByRole('dialog', { name: /的效果 1$/ })
    await expect(effectDetail.getByText(/迴響 · .* · 第 \d+ 回合/)).toBeVisible()
  } finally {
    await game.close()
  }
})
