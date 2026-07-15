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

test('imports, exports, and rejects malformed personal-deck drafts', async ({ page }) => {
  await page.context().grantPermissions(['clipboard-read', 'clipboard-write'])
  await page.getByRole('button', { name: /旅人-/ }).click()
  await page.getByRole('button', { name: '個人牌組', exact: true }).click()

  await page.getByLabel('牌組名稱').fill('尚未儲存的匯入牌組')
  await page.getByRole('button', { name: '匯入牌組' }).click()
  const dialog = page.getByRole('dialog', { name: '匯入牌組' })
  await expect(dialog).toBeVisible()
  await expect(dialog.getByLabel('牌組張數')).toBeFocused()
  await page.keyboard.press('Shift+Tab')
  await expect(dialog.getByRole('button', { name: '套用' })).toBeFocused()
  await page.keyboard.press('Tab')
  await expect(dialog.getByLabel('牌組張數')).toBeFocused()
  await dialog.getByLabel('牌組張數').fill(`4\t1\t1\t3\t1
4\t1\t1\t2\t2
4\t4\t4\t3\t3
4\t1\t1\t3\t3
4\t1\t1\t1\t3`)
  await dialog.getByRole('button', { name: '套用' }).click()

  const expectedCounts = [
    4, 1, 1, 3, 1,
    4, 1, 1, 2, 2,
    4, 4, 4, 3, 3,
    4, 1, 1, 3, 3,
    4, 1, 1, 1, 3,
  ]
  const elements = ['金', '木', '水', '火', '土']
  for (const [index, count] of expectedCounts.entries()) {
    const element = elements[Math.floor(index / 5)]
    const level = (index % 5) + 1
    await expect(page.getByLabel(`${element}${level}級張數`)).toHaveText(String(count))
  }
  await expect(page.getByLabel('牌組名稱')).toHaveValue('尚未儲存的匯入牌組')

  await page.getByRole('button', { name: '匯出牌組' }).click()
  await expect(page.getByText('牌組已複製到剪貼簿。')).toBeVisible()
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(
    `4\t1\t1\t3\t1
4\t1\t1\t2\t2
4\t4\t4\t3\t3
4\t1\t1\t3\t3
4\t1\t1\t1\t3`,
  )

  await page.getByRole('button', { name: '匯入牌組' }).click()
  await dialog.getByLabel('牌組張數').fill('4 1 1 3 not-a-number')
  await dialog.getByRole('button', { name: '套用' }).click()
  await expect(dialog.getByRole('alert')).toContainText('每個值只能是一個 0 至 9 的十進位數字')
  await expect(page.getByLabel('金1級張數')).toHaveText('4')
  await expect(page.getByLabel('牌組名稱')).toHaveValue('尚未儲存的匯入牌組')
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
