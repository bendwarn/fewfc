import { createRoom, expect, joinListedRoom, loginAsGuests, test } from './fixtures'

test('account menu opens the valid built-in personal deck editor', async ({ page }) => {
  await page.getByRole('button', { name: /旅人-/ }).click()
  await page.getByRole('button', { name: '個人牌組', exact: true }).click()

  await expect(page).toHaveURL('/deck')
  await expect(page.getByRole('heading', { name: '個人牌組' })).toBeVisible()
  await expect(page.locator('.deck-validation')).toContainText('60 / 60 張')
  await expect(page.locator('.deck-validation')).toContainText('170 / 170 級')
  await expect(page.locator('.deck-validation')).toContainText('目前使用內建預組')

  await page.getByRole('button', { name: '減少金1級' }).click()
  await expect(page.getByRole('button', { name: '儲存牌組' })).toBeDisabled()
  await page.getByRole('button', { name: '重設為預組' }).click()
  await expect(page.locator('.deck-validation')).toContainText('60 / 60 張')
})

test('default room rules lock preconstructed decks and start personal piles', async ({ browser }) => {
  test.setTimeout(300_000)

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()

  try {
    await loginAsGuests([host, guest])
    const roomName = `個人牌組測試 ${Date.now()}`

    await createRoom(host, roomName)
    await expect(host.getByLabel('棄牌回收')).toBeChecked()
    await expect(host.getByLabel('個人牌組')).toBeChecked()

    await joinListedRoom(guest, roomName)

    await expect(guest.getByLabel('棄牌回收')).toBeChecked()
    await expect(guest.getByLabel('個人牌組')).toBeChecked()
    await expect(guest.getByLabel('五方傳說')).toBeChecked()
    await expect(guest.getByLabel('個人牌組')).toBeDisabled()

    await guest.getByRole('button', { name: '準備 →' }).click()
    await expect(guest.getByRole('button', { name: '取消準備 →' })).toBeVisible()
    await expect(guest.getByText('本局使用：五行均衡預組')).toBeVisible()
    await host.getByRole('button', { name: '開始遊戲 →' }).click()

    await expect(host.locator('.setup-reveal')).toBeHidden()
    await expect(host.locator('.player-identity').filter({ hasText: /牌庫 5[56] · 棄牌 0/ })).toHaveCount(2)
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})
