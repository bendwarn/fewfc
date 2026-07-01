import { expect, test, type Page } from '@playwright/test'

async function loginAsGuest(page: Page) {
  await page.goto('/login')
  await page.getByRole('button', { name: '以訪客身份遊玩' }).click()
  await expect(page).toHaveURL(/\/rooms(?:\?.*)?$/)
}

async function activePlayerPage(pages: Page[]) {
  await expect.poll(async () => {
    const counts = await Promise.all(pages.map(page => (
      page.locator('.playing-card:enabled:not(.hidden)').count()
    )))
    return Math.max(...counts)
  }).toBeGreaterThan(0)

  for (const page of pages) {
    if (await page.locator('.playing-card:enabled:not(.hidden)').count()) {
      return page
    }
  }

  throw new Error('No active player page')
}

async function expectVerticalPanels(page: Page) {
  const ability = page.getByRole('region', { name: '能力' })
  const action = page.getByRole('region', { name: '行動' })
  const abilityBox = await ability.boundingBox()
  const actionBox = await action.boundingBox()

  expect(abilityBox).not.toBeNull()
  expect(actionBox).not.toBeNull()
  expect(abilityBox!.y + abilityBox!.height).toBeLessThanOrEqual(actionBox!.y)
}

test('selected cards expose rule-backed actions in vertically ordered control panels', async ({ browser }) => {
  test.setTimeout(180_000)

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()
  const pages = [host, guest]

  try {
    await Promise.all(pages.map(loginAsGuest))

    const roomName = `可用行動測試 ${Date.now()}`
    await host.getByRole('button', { name: '建立房間', exact: true }).click()
    await host.getByLabel('房間名稱').fill(roomName)
    await host.getByRole('button', { name: '公開房間', exact: true }).click()
    await host.getByRole('button', { name: '建立房間 →' }).click()
    await expect(host.getByLabel('進階規則‧英雄學派')).toBeChecked()
    await host.getByLabel('進階規則‧五方傳說').uncheck()
    await host.getByLabel('個人牌組').uncheck()

    const listedRoom = guest.locator('.public-room-list button').filter({ hasText: roomName })
    await expect(listedRoom).toBeVisible()
    await listedRoom.click()
    await guest.getByRole('button', { name: '準備 →' }).click()
    const startButton = host.getByRole('button', { name: '開始遊戲 →' })
    await expect(startButton).toBeEnabled()
    await startButton.click()
    await Promise.all(pages.map(page => expect(page.locator('.setup-reveal')).toBeHidden()))
    await expect(host.getByRole('button', { name: '職業教學' })).toBeVisible()
    await host.getByRole('button', { name: '職業教學' }).click()
    const teaching = host.getByRole('dialog', { name: '英雄學派職業圖鑑' })
    await expect(teaching.locator('.profession-card')).toHaveCount(18)
    await expect(teaching).toContainText('升階後保留')
    await expect(teaching).toContainText('轉職後不保留原學派能力')
    await host.getByRole('button', { name: '關閉職業教學' }).click()

    const active = await activePlayerPage(pages)
    const ability = active.getByRole('region', { name: '能力' })
    const action = active.getByRole('region', { name: '行動' })

    await expect(ability).toContainText('不結束行動階段')
    await expect(ability).toContainText('目前沒有可用能力')
    await expect(action).toContainText('使用後結束行動階段')
    await expect(action).toContainText('選擇手牌以尋找可用行動')
    await expectVerticalPanels(active)

    await active.setViewportSize({ width: 377, height: 734 })
    await expectVerticalPanels(active)

    const commandUrl = '**/api/games/*/commands'
    await active.route(commandUrl, async (route) => {
      const body = route.request().postDataJSON()
      if (body?.action?.type === 'playableActions') {
        await new Promise(resolve => setTimeout(resolve, 300))
      }
      await route.continue()
    })

    const card = active.locator('.playing-card:enabled:not(.hidden)').first()
    await card.click()
    await expect(active.locator('.action-processing')).toHaveText('處理中')

    const formation = action.locator('.action-candidates button:not(.skip-action)').first()
    await expect(formation).toBeVisible()
    await formation.focus()
    await expect(formation).toBeFocused()
    await expect(active.locator('.action-detail')).toBeVisible()

    await active.unroute(commandUrl)
    await card.click()
    await active.route(commandUrl, async (route) => {
      const body = route.request().postDataJSON()
      if (body?.action?.type === 'playableActions') {
        await route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: JSON.stringify({ statusMessage: 'query failed' }),
        })
        return
      }
      await route.continue()
    })
    await card.click()
    await expect(active.locator('.action-error')).toBeVisible()
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})
