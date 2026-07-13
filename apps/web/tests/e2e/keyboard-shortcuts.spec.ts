import { activePlayerPage, expect, loginAsGuests, startTwoPlayerMatch, test } from './fixtures'

test('Command+K submits AdvanceAutomatic for the active player', async ({ browser }) => {
  test.setTimeout(180_000)

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()

  try {
    await loginAsGuests([host, guest])
    await startTwoPlayerMatch(host, guest, `快捷鍵測試 ${Date.now()}`)
    const active = await activePlayerPage([host, guest])
    const command = active.waitForResponse(response => (
      response.url().includes('/commands')
      && response.request().method() === 'POST'
      && response.request().postDataJSON()?.action?.type === 'advanceAutomatic'
    ))

    await active.keyboard.press('Meta+k')

    expect((await command).ok()).toBe(true)
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})
