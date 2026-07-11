import type { Page } from '@playwright/test'
import { createPublicRoom, expect, joinListedRoom, loginAsGuests, test } from './fixtures'

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
    await loginAsGuests(pages)

    const roomName = `可用行動測試 ${Date.now()}`
    await createPublicRoom(host, roomName)
    await expect(host.getByLabel('英雄學派')).toBeChecked()
    await host.getByLabel('五方傳說').uncheck()
    await host.getByLabel('個人牌組').uncheck()

    await joinListedRoom(guest, roomName)
    await guest.getByRole('button', { name: '準備 →' }).click()
    const startButton = host.getByRole('button', { name: '開始遊戲 →' })
    await expect(startButton).toBeEnabled()
    await startButton.click()
    await Promise.all(pages.map(page => expect(page.locator('.setup-reveal')).toBeHidden()))

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
