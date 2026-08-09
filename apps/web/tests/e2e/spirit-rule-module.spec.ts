import {
  expect,
  reloadFastGameRoute,
  seedDevelopmentScenario,
  setupFastTwoPlayerGame,
  test,
} from './fixtures'

test('a Spirit Skill is usable from the Ability panel and survives reconnect', async ({ browser }) => {
  const game = await setupFastTwoPlayerGame(browser, {
    roomName: `精靈技能測試 ${Date.now()}`,
  })
  const { host, guest, gameId: roomId } = game

  try {
    await expect(host.getByLabel('棄牌堆').locator('.discard-pile')).toHaveCount(2)
    await expect(host.locator('.discard-position-top')).toHaveCount(1)
    await expect(host.locator('.discard-position-bottom')).toHaveCount(1)
    const discardOverlapsFormation = await host.locator('.board-center').evaluate((center) => {
      const formation = center.querySelector('.formation-field')!.getBoundingClientRect()
      return [...center.querySelectorAll('.discard-pile')].some((pile) => {
        const box = pile.getBoundingClientRect()
        return box.left < formation.right
          && box.right > formation.left
          && box.top < formation.bottom
          && box.bottom > formation.top
      })
    })
    expect(discardOverlapsFormation).toBe(false)

    await seedDevelopmentScenario(host, { name: 'spirit-skill' })

    await reloadFastGameRoute(host, roomId)
    await expect(host.locator('.spirit-status')).toContainText('精靈 · 金精靈 · 靈力 2 / 6')
    const flyingBlade = host
      .getByRole('region', { name: '能力' })
      .getByRole('button', { name: /^飛刃/ })
    await expect(flyingBlade).toBeVisible()
    const selectedCard = host.locator('.playing-card:enabled:not(.hidden)').first()
    await Promise.all([
      host.waitForResponse(response => (
        response.url().endsWith(`/api/games/${roomId}/commands`)
        && response.request().postDataJSON()?.action?.type === 'playableActions'
      )),
      selectedCard.click(),
    ])
    await Promise.all([
      host.waitForResponse(response => (
        response.url().endsWith(`/api/games/${roomId}/commands`)
        && response.request().postDataJSON()?.action?.type === 'playableActions'
      )),
      selectedCard.click(),
    ])
    await expect(flyingBlade).toBeVisible()
    const [skillResponse] = await Promise.all([
      host.waitForResponse(response => (
        response.url().endsWith(`/api/games/${roomId}/commands`)
        && response.request().postDataJSON()?.action?.type === 'useSpiritSkill'
      )),
      flyingBlade.click(),
    ])
    expect(skillResponse.ok()).toBe(true)

    await expect(host.getByText('使用精靈技能', { exact: true })).toBeVisible()
    await expect(host.getByText(/使用「飛刃」，靈力由 2 變為 0/)).toBeVisible()
    await reloadFastGameRoute(guest, roomId)
    await expect(guest.getByText(/使用「飛刃」，靈力由 2 變為 0/)).toBeVisible()
    await reloadFastGameRoute(host, roomId)
    await expect(host.getByText(/的金精靈已破除/)).toBeVisible()
  } finally {
    await game.close()
  }
})
