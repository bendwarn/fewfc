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

async function waitForPlayableActions(page: Page) {
  return page.waitForResponse(response => (
    response.url().includes('/commands')
    && response.request().method() === 'POST'
    && (response.request().postData() ?? '').includes('"type":"playableActions"')
    && response.ok()
  ))
}

test('Echo defaults on and normalizes every Advanced Rule dependency', async ({ page }) => {
  test.setTimeout(180_000)
  await page.goto('/login')
  await page.getByRole('button', { name: '以訪客身份遊玩' }).click()
  await page.getByRole('button', { name: '建立房間', exact: true }).click()
  await page.getByLabel('房間名稱').fill(`迴響測試 ${Date.now()}`)
  await page.getByRole('button', { name: '建立房間 →' }).click()

  await expect(page.getByLabel('主題規則‧迴響')).toBeChecked()

  await page.getByLabel('進階規則‧英雄學派').uncheck()
  await expect(page.getByLabel('主題規則‧迴響')).not.toBeChecked()

  await page.getByLabel('主題規則‧迴響').check()
  await expect(page.getByLabel('進階規則‧星辰圖記')).toBeChecked()
  await expect(page.getByLabel('進階規則‧英雄學派')).toBeChecked()
  await expect(page.getByLabel('進階規則‧五方傳說')).toBeChecked()
})

test('Pure Fire target choice is private, accessible, and reconnectable', async ({ browser }) => {
  test.setTimeout(180_000)
  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()

  try {
    await Promise.all([loginAsGuest(host), loginAsGuest(guest)])
    const roomName = `淨火選擇測試 ${Date.now()}`
    await createPublicRoom(host, roomName)
    await guest.locator('.public-room-list button').filter({ hasText: roomName }).click()
    await guest.getByRole('button', { name: '準備 →' }).click()
    await host.getByRole('button', { name: '開始遊戲 →' }).click()
    await expect(host.getByRole('region', { name: '啟用規則' })).toBeVisible()

    const roomId = new URL(host.url()).pathname.split('/').pop()
    const seeded = await host.evaluate(async (id) => {
      const response = await fetch(`/api/games/${id}/test-echo`, { method: 'POST' })
      return {
        ok: response.ok,
        body: await response.text(),
      }
    }, roomId)
    expect(seeded, seeded.body).toMatchObject({ ok: true })

    await host.reload()
    await expect(host.getByRole('heading', { name: '淨火：選擇玩家' })).toBeVisible()
    const targets = host.getByLabel('選擇玩家').getByRole('button')
    await expect(targets).toHaveCount(2)

    await guest.reload()
    await expect(guest.getByText('淨火：選擇玩家', { exact: true })).toBeVisible()
    await expect(guest.getByLabel('選擇玩家')).toHaveCount(0)

    const [answer] = await Promise.all([
      host.waitForResponse(response => (
        response.url().endsWith(`/api/games/${roomId}/commands`)
        && response.request().postDataJSON()?.action?.type === 'answerEffectChoiceTyped'
      )),
      targets.nth(1).click(),
    ])
    expect(answer.ok()).toBe(true)
    await expect(host.getByText('淨火', { exact: true })).toBeVisible()
    await expect(host.getByText(/迴響 · 第 \d+ 回合/)).toBeVisible()
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})

test('Echo action detail shows the delayed Echo policy on the battlefield', async ({ browser }) => {
  test.setTimeout(180_000)
  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const page = await hostContext.newPage()
  const guest = await guestContext.newPage()

  try {
    await Promise.all([loginAsGuest(page), loginAsGuest(guest)])
    const roomName = `迴響行動詳情 ${Date.now()}`
    await createPublicRoom(page, roomName)
    await guest.locator('.public-room-list button').filter({ hasText: roomName }).click()
    await guest.getByRole('button', { name: '準備 →' }).click()
    await page.getByRole('button', { name: '開始遊戲 →' }).click()
    await expect(page.getByRole('region', { name: '啟用規則' })).toBeVisible()

    const roomId = new URL(page.url()).pathname.split('/').pop()
    const seeded = await page.evaluate(async (id) => {
      const response = await fetch(`/api/games/${id}/test-echo`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ mode: 'actionDetail' }),
      })
      return {
        ok: response.ok,
        body: await response.json() as { fixtureCards?: number[], error?: string },
      }
    }, roomId)
    expect(seeded.ok, JSON.stringify(seeded.body)).toBe(true)
    expect(seeded.body.fixtureCards?.length).toBe(2)

    await page.reload()
    await expect(page.getByRole('region', { name: '啟用規則' })).toBeVisible()

    for (const cardId of seeded.body.fixtureCards ?? []) {
      const response = waitForPlayableActions(page)
      await page.locator(`[data-card-id="${cardId}"]`).click()
      await response
    }

    const pureFire = page.getByRole('button', { name: '變徵‧淨火' })
    await expect(pureFire).toBeVisible()
    await pureFire.hover()
    const detail = page.locator('.action-detail')
    await expect(detail).toContainText('不需支付迴響代價並自動排定迴響')
    await expect(detail).toContainText('自己下次回合開始')
    await expect(detail).toContainText('不視為新的陣法')
    await expect(detail).toContainText('不會再次排定迴響')
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})
