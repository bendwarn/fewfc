import { expect, setupFastFourPlayerGame, test } from './fixtures'

test('four-player room projects the authoritative turn and round record', async ({ browser }) => {
  test.setTimeout(60_000)
  const game = await setupFastFourPlayerGame(browser, {
    roomName: `四人牌局投影 ${Date.now()}`,
  })

  try {
    const page = game.pages[0]!
    await expect(page.locator('.player-seat')).toHaveCount(4)
    await expect(page.locator('.player-seat[aria-current="true"]')).toHaveCount(1)

    const eventFeed = page.locator('.game-sidebar .event-feed')
    await expect(eventFeed.locator('.event-group-title strong').first()).toBeVisible()
    await expect(eventFeed).toContainText(/第 1 回合/u)
  } finally {
    await game.close()
  }
})
