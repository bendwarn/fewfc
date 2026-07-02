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
  await expect(host).toHaveURL(/\/rooms\/[0-9a-f-]+$/)
}

async function joinListedRoom(page: Page, roomName: string) {
  const room = page.locator('.public-room-list button').filter({ hasText: roomName })
  await expect(room).toContainText('精靈：啟用')
  await room.click()
  await expect(page).toHaveURL(/\/rooms\/[0-9a-f-]+$/)
}

test('Spirit defaults on and keeps its Advanced Rule dependencies coherent', async ({ browser }) => {
  test.setTimeout(180_000)

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()

  try {
    await Promise.all([loginAsGuest(host), loginAsGuest(guest)])
    const roomName = `精靈規則測試 ${Date.now()}`
    await createPublicRoom(host, roomName)

    await expect(host.getByLabel('主題規則‧精靈')).toBeChecked()
    await expect(host.getByLabel('進階規則‧星辰圖記')).toBeChecked()
    await expect(host.getByLabel('進階規則‧英雄學派')).toBeChecked()
    await expect(host.getByLabel('進階規則‧五方傳說')).toBeChecked()

    await joinListedRoom(guest, roomName)
    await guest.getByRole('button', { name: '準備 →' }).click()

    await host.getByLabel('進階規則‧星辰圖記').uncheck()
    await expect(host.getByLabel('主題規則‧精靈')).not.toBeChecked()
    await expect(guest.getByLabel('主題規則‧精靈')).not.toBeChecked()
    await expect(guest.getByRole('button', { name: '準備 →' })).toBeVisible()

    await host.getByLabel('主題規則‧精靈').check()
    await expect(host.getByLabel('進階規則‧星辰圖記')).toBeChecked()
    await expect(host.getByLabel('進階規則‧英雄學派')).toBeChecked()
    await expect(host.getByLabel('進階規則‧五方傳說')).toBeChecked()

    const roomId = new URL(host.url()).pathname.split('/').pop()
    const invalidStatus = await host.evaluate(async (id) => {
      const response = await fetch(`/api/games/${id}/rules`, {
        method: 'PUT',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ enabledRuleModules: ['spirit'] }),
      })
      return response.status
    }, roomId)
    expect(invalidStatus).toBe(400)

    await guest.getByRole('button', { name: '準備 →' }).click()
    await host.getByRole('button', { name: '開始遊戲 →' }).click()

    await Promise.all([host, guest].map(async (page) => {
      await expect(page.getByRole('region', { name: '啟用規則' }))
        .toContainText('主題規則‧精靈')
    }))

    await guest.reload()
    await expect(guest.getByRole('region', { name: '啟用規則' }))
      .toContainText('主題規則‧精靈')
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})

test('a Spirit Skill is usable from the Ability panel and survives reconnect', async ({ browser }) => {
  test.setTimeout(180_000)

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()

  try {
    await Promise.all([loginAsGuest(host), loginAsGuest(guest)])
    const roomName = `精靈技能測試 ${Date.now()}`
    await createPublicRoom(host, roomName)
    await joinListedRoom(guest, roomName)
    await guest.getByRole('button', { name: '準備 →' }).click()
    await host.getByRole('button', { name: '開始遊戲 →' }).click()
    await expect(host.getByRole('region', { name: '啟用規則' })).toBeVisible()

    const roomId = new URL(host.url()).pathname.split('/').pop()
    const seeded = await host.evaluate(async (id) => {
      const response = await fetch(`/api/games/${id}/test-spirit`, { method: 'POST' })
      return {
        ok: response.ok,
        status: response.status,
        body: await response.text(),
      }
    }, roomId)
    expect(seeded, seeded.body).toMatchObject({ ok: true })

    await host.reload()
    await expect(host.locator('.spirit-status')).toContainText('精靈 · 金精靈 · 靈力 2 / 6')
    const flyingBlade = host
      .getByRole('region', { name: '能力' })
      .getByRole('button', { name: /^飛刃：/ })
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
    await guest.reload()
    await expect(guest.getByText(/使用「飛刃」，靈力由 2 變為 0/)).toBeVisible()
    await host.reload()
    await expect(host.getByText(/的金精靈已破除/)).toBeVisible()
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})
