import { expect, test, type Page } from '@playwright/test'

async function loginAsGuest(page: Page) {
  await page.goto('/login')
  await page.getByRole('button', { name: '以訪客身份遊玩' }).click()
  await expect(page).toHaveURL(/\/rooms(?:\?.*)?$/)
}

test('a Profession change and activated ability survive public reconnect', async ({ browser }) => {
  test.setTimeout(180_000)

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()

  try {
    await Promise.all([host, guest].map(loginAsGuest))
    const roomName = `英雄學派測試 ${Date.now()}`
    await host.getByRole('button', { name: '建立房間', exact: true }).click()
    await host.getByLabel('房間名稱').fill(roomName)
    await host.getByRole('button', { name: '公開房間', exact: true }).click()
    await host.getByRole('button', { name: '建立房間 →' }).click()
    await host.getByLabel('進階規則‧星辰圖記').uncheck()
    await host.getByLabel('進階規則‧五方傳說').uncheck()
    await host.getByLabel('棄牌回收').uncheck()
    await host.getByLabel('個人牌組').uncheck()

    const listedRoom = guest.locator('.public-room-list button').filter({ hasText: roomName })
    await expect(listedRoom).toBeVisible()
    await listedRoom.click()
    await guest.getByRole('button', { name: '準備 →' }).click()
    await host.getByRole('button', { name: '開始遊戲 →' }).click()
    await expect(host.getByRole('region', { name: '啟用規則' })).toBeVisible()

    const roomId = new URL(host.url()).pathname.split('/').pop()
    const seeded = await host.evaluate(async (id) => {
      const response = await fetch(`/api/games/${id}/test-hero-schools`, { method: 'POST' })
      return {
        ok: response.ok,
        status: response.status,
        body: await response.text(),
      }
    }, roomId)
    expect(seeded, seeded.body).toMatchObject({ ok: true })

    await expect(host.locator('.profession-badge')).toContainText('幻術師')
    await expect(guest.locator('.profession-badge')).toContainText('幻術師')
    await host.reload()
    const selectable = host.locator('.playing-card:enabled:not(.hidden)')
    await expect(selectable).toHaveCount(5)
    await selectable.nth(0).click()
    await selectable.nth(1).click()
    const illusion = host
      .getByRole('region', { name: '能力' })
      .getByRole('button', { name: /^幻術：/ })
      .first()
    await expect(illusion).toBeVisible()
    const [activationResponse] = await Promise.all([
      host.waitForResponse(response => (
        response.url().endsWith(`/api/games/${roomId}/commands`)
        && response.request().postDataJSON()?.action?.type === 'activateProfessionAbility'
      )),
      illusion.click(),
    ])
    expect(activationResponse.ok()).toBe(true)

    await host.reload()
    await expect(host.locator('.prepared-ability')).toContainText('已準備 · 幻術')
    await guest.reload()
    await expect(guest.locator('.prepared-ability')).toContainText('已準備 · 幻術')
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})
