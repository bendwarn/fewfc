import { expect, test } from '@playwright/test'

async function loginAsGuest(page: import('@playwright/test').Page) {
  await page.goto('/login')
  await page.getByRole('button', { name: '以訪客身份遊玩' }).click()
  await expect(page).toHaveURL(/\/rooms(?:\?.*)?$/)
}

test('account menu opens the valid built-in personal deck editor', async ({ page }) => {
  await loginAsGuest(page)

  await page.getByRole('button', { name: /旅人-/ }).click()
  await page.getByRole('button', { name: '個人牌組' }).click()

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
    await Promise.all([loginAsGuest(host), loginAsGuest(guest)])
    const roomName = `個人牌組測試 ${Date.now()}`

    await host.getByRole('button', { name: '建立房間', exact: true }).click()
    const createDialog = host.getByRole('dialog', { name: '建立房間' })
    await expect(createDialog.getByText('規則模組')).toHaveCount(0)
    await host.getByLabel('房間名稱').fill(roomName)
    await host.getByRole('button', { name: '建立房間 →' }).click()
    await expect(host.getByLabel('棄牌回收')).toBeChecked()
    await expect(host.getByLabel('個人牌組')).toBeChecked()

    const listedRoom = guest.locator('.public-room-list button').filter({ hasText: roomName })
    await expect(listedRoom).toBeVisible()
    await listedRoom.click()

    await expect(guest.getByLabel('棄牌回收')).toBeChecked()
    await expect(guest.getByLabel('個人牌組')).toBeChecked()
    await expect(guest.getByLabel('進階規則‧五方傳說')).toBeChecked()
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
