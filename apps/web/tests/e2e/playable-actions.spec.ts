import type { Page } from '@playwright/test'
import { activePlayerPage, expect, setupFastTwoPlayerGame, test } from './fixtures'

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
  const game = await setupFastTwoPlayerGame(browser, {
    roomName: `可用行動測試 ${Date.now()}`,
    enabledRuleModules: ['discard-retrieval', 'star', 'hero-schools'],
  })
  const { host, guest, pages } = game

  try {
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
        expect(body.commandId).toMatch(/\S/)
        expect(body.gameInstanceId).toMatch(/\S/)
        expect(body.transactionId).toMatch(/\S/)
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
    await expect(active.locator('.action-error'))
      .toHaveText('無法取得可用行動，請稍後再試。')
    await expect(active.locator('.action-error')).not.toContainText('500')
    await expect(active.locator('.action-error')).not.toContainText('Internal Server Error')
  } finally {
    await game.close()
  }
})
