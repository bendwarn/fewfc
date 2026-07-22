import { activePlayerPage, expect, setupFastTwoPlayerGame, test } from './fixtures'

test('Command+K submits AdvanceAutomatic for the active player', async ({ browser }) => {
  const game = await setupFastTwoPlayerGame(browser, {
    roomName: `快捷鍵測試 ${Date.now()}`,
  })
  const { host, guest, pages } = game

  try {
    const active = await activePlayerPage(pages)
    const command = active.waitForResponse(response => (
      response.url().includes('/commands')
      && response.request().method() === 'POST'
      && response.request().postDataJSON()?.action?.type === 'advanceAutomatic'
    ))

    await active.keyboard.press('Meta+k')

    expect((await command).ok()).toBe(true)
  } finally {
    await game.close()
  }
})
