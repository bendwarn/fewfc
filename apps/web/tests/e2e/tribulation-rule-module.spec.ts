import {
  createPublicRoom,
  createRoom,
  expect,
  joinListedRoom,
  loginAsGuests,
  seedDevelopmentScenario,
  startTwoPlayerMatch,
  test,
} from './fixtures'

test('Tribulation defaults on and normalizes every Advanced Rule dependency', async ({ page }) => {
  test.setTimeout(180_000)
  await createRoom(page, `天劫測試 ${Date.now()}`)

  await expect(page.getByLabel('天劫')).toBeChecked()
  await page.getByLabel('英雄學派').uncheck()
  await expect(page.getByLabel('天劫')).not.toBeChecked()

  await page.getByLabel('天劫').check()
  await expect(page.getByLabel('星辰圖記')).toBeChecked()
  await expect(page.getByLabel('英雄學派')).toBeChecked()
  await expect(page.getByLabel('五方傳說')).toBeChecked()
})

test('Earth Rending Environment choice is private, accessible, and reconnectable', async ({ browser }) => {
  test.setTimeout(180_000)
  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()

  try {
    await loginAsGuests([host, guest])
    const roomName = `裂地崩山選擇測試 ${Date.now()}`
    const roomId = await startTwoPlayerMatch(host, guest, roomName)
    await seedDevelopmentScenario(host, { name: 'tribulation-earth-rending' })

    await host.reload()
    await expect(host.getByRole('heading', {
      name: '裂地崩山：選擇要轉移的環境',
    })).toBeVisible()
    const environments = host.getByLabel('選擇環境').getByRole('button')
    await expect(environments).toHaveCount(5)

    await guest.reload()
    await expect(guest.getByText('裂地崩山：選擇要轉移的環境', {
      exact: true,
    })).toBeVisible()
    await expect(guest.getByLabel('選擇環境')).toHaveCount(0)

    const [answer] = await Promise.all([
      host.waitForResponse(response => (
        response.url().endsWith(`/api/games/${roomId}/commands`)
        && response.request().postDataJSON()?.action?.type === 'answerEffectChoiceTyped'
      )),
      host.getByRole('button', { name: '選擇環境 火行' }).click(),
    ])
    expect(answer.ok()).toBe(true)

    await host.reload()
    await expect(host.getByText('裂地崩山', { exact: true }).first()).toBeVisible()
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})
