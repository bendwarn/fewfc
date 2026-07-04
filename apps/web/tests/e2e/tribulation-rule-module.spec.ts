import { expect, test, type Page } from '@playwright/test'

async function loginAsGuest(page: Page) {
  await page.goto('/login')
  await page.getByRole('button', { name: '以訪客身份遊玩' }).click()
  await expect(page).toHaveURL(/\/rooms(?:\?.*)?$/)
}

async function createPublicRoom(host: Page, roomName: string) {
  await host.getByRole('button', { name: '建立房間', exact: true }).click()
  await host.getByLabel('房間名稱').fill(roomName)
  await host.getByRole('button', { name: '公開房間', exact: true }).click()
  await host.getByRole('button', { name: '建立房間 →' }).click()
}

test('Tribulation defaults on and normalizes every Advanced Rule dependency', async ({ page }) => {
  test.setTimeout(180_000)
  await loginAsGuest(page)
  await page.getByRole('button', { name: '建立房間', exact: true }).click()
  await page.getByLabel('房間名稱').fill(`天劫測試 ${Date.now()}`)
  await page.getByRole('button', { name: '建立房間 →' }).click()

  await expect(page.getByLabel('主題規則‧天劫')).toBeChecked()
  await page.getByLabel('進階規則‧英雄學派').uncheck()
  await expect(page.getByLabel('主題規則‧天劫')).not.toBeChecked()

  await page.getByLabel('主題規則‧天劫').check()
  await expect(page.getByLabel('進階規則‧星辰圖記')).toBeChecked()
  await expect(page.getByLabel('進階規則‧英雄學派')).toBeChecked()
  await expect(page.getByLabel('進階規則‧五方傳說')).toBeChecked()
})

test('Earth Rending Environment choice is private, accessible, and reconnectable', async ({ browser }) => {
  test.setTimeout(180_000)
  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()

  try {
    await Promise.all([loginAsGuest(host), loginAsGuest(guest)])
    const roomName = `裂地崩山選擇測試 ${Date.now()}`
    await createPublicRoom(host, roomName)
    await guest.locator('.public-room-list button').filter({ hasText: roomName }).click()
    await guest.getByRole('button', { name: '準備 →' }).click()
    await host.getByRole('button', { name: '開始遊戲 →' }).click()
    await expect(host.getByRole('region', { name: '啟用規則' })).toBeVisible()

    const roomId = new URL(host.url()).pathname.split('/').pop()
    const seeded = await host.evaluate(async (id) => {
      const response = await fetch(`/api/games/${id}/test-tribulation`, { method: 'POST' })
      return {
        ok: response.ok,
        body: await response.text(),
      }
    }, roomId)
    expect(seeded, seeded.body).toMatchObject({ ok: true })

    await host.reload()
    await expect(host.getByRole('heading', {
      name: '裂地崩山：選擇環境或環行牌',
    })).toBeVisible()
    const environments = host.getByLabel('選擇環境').getByRole('button')
    await expect(environments).toHaveCount(5)

    await guest.reload()
    await expect(guest.getByText('裂地崩山：選擇環境或環行牌', {
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
