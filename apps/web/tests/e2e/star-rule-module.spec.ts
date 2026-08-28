import { type Locator, type Page } from '@playwright/test'
import {
  battleRecordRuleEntry,
  expect,
  reloadFastGameRoute,
  seedDevelopmentScenario,
  setupFastFourPlayerGame,
  setupFastTwoPlayerGame,
  test,
} from './fixtures'

async function indexedModules(page: Page, roomName: string): Promise<string[]> {
  const response = await page.context().request.get('/api/games')
  const body = await response.text()
  if (!response.ok()) {
    throw new Error(`GET /api/games failed with HTTP ${response.status()}: ${body}`)
  }
  let result: { myRooms?: Array<{ name: string; enabledRuleModules: string[] }> }
  try {
    result = JSON.parse(body) as { myRooms?: Array<{ name: string; enabledRuleModules: string[] }> }
  } catch {
    throw new Error(`GET /api/games returned invalid JSON: ${body}`)
  }
  return result.myRooms?.find(room => room.name === roomName)?.enabledRuleModules ?? []
}

async function box(locator: Locator) {
  const value = await locator.boundingBox()
  expect(value).not.toBeNull()
  return value!
}

async function expectBattlefieldConclusion(page: Page, width: number, height: number) {
  await page.setViewportSize({ width, height })
  const battlefield = page.locator('.battlefield')
  const center = battlefield.locator('.board-center')
  const conclusion = center.locator('.result-panel')

  await expect(conclusion).toBeVisible()
  await expect(battlefield.locator('.result-panel')).toHaveCount(1)

  const [battlefieldBox, centerBox, conclusionBox] = await Promise.all([
    box(battlefield),
    box(center),
    box(conclusion),
  ])
  expect(conclusionBox.x).toBeGreaterThanOrEqual(centerBox.x - 1)
  expect(conclusionBox.y).toBeGreaterThanOrEqual(centerBox.y - 1)
  expect(conclusionBox.x + conclusionBox.width).toBeLessThanOrEqual(centerBox.x + centerBox.width + 1)
  expect(conclusionBox.y + conclusionBox.height).toBeLessThanOrEqual(centerBox.y + centerBox.height + 1)
  expect(conclusionBox.x).toBeGreaterThanOrEqual(battlefieldBox.x - 1)
  expect(conclusionBox.y).toBeGreaterThanOrEqual(battlefieldBox.y - 1)
  expect(conclusionBox.x + conclusionBox.width).toBeLessThanOrEqual(battlefieldBox.x + battlefieldBox.width + 1)
  expect(conclusionBox.y + conclusionBox.height).toBeLessThanOrEqual(battlefieldBox.y + battlefieldBox.height + 1)

  const terminalCards = conclusion.locator('.formation-cards .playing-card')
  const terminalCardBoxes = await terminalCards.evaluateAll(elements => elements.map((element) => {
    const rect = element.getBoundingClientRect()
    return { x: rect.x, y: rect.y, width: rect.width, height: rect.height }
  }))
  const conclusionTextBoxes = await conclusion.locator('small, strong, h2, .result-reason, .result-actions').evaluateAll(elements => elements.map((element) => {
    const rect = element.getBoundingClientRect()
    return { x: rect.x, y: rect.y, width: rect.width, height: rect.height }
  }))
  for (const cardBox of terminalCardBoxes) {
    for (const textBox of conclusionTextBoxes) {
      const overlaps = cardBox.x < textBox.x + textBox.width
        && cardBox.x + cardBox.width > textBox.x
        && cardBox.y < textBox.y + textBox.height
        && cardBox.y + cardBox.height > textBox.y
      expect(overlaps).toBe(false)
    }
  }

  const seatBoxes = await battlefield.locator('.player-seat').evaluateAll(elements => elements.map((element) => {
    const rect = element.getBoundingClientRect()
    return { x: rect.x, y: rect.y, width: rect.width, height: rect.height }
  }))
  for (const seatBox of seatBoxes) {
    const overlaps = conclusionBox.x < seatBox.x + seatBox.width
      && conclusionBox.x + conclusionBox.width > seatBox.x
      && conclusionBox.y < seatBox.y + seatBox.height
      && conclusionBox.y + conclusionBox.height > seatBox.y
    expect(overlaps).toBe(false)
  }

  const overflow = await page.evaluate(() => ({
    page: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    battlefield: document.querySelector('.battlefield')!.scrollWidth
      - document.querySelector('.battlefield')!.clientWidth,
  }))
  expect(overflow.page).toBeLessThanOrEqual(0)
  expect(overflow.battlefield).toBeLessThanOrEqual(0)
}

test('Star defaults on, survives reconnect, and is immutable after a two-player start', async ({ browser }) => {
  const roomName = `星辰預設測試 ${Date.now()}`
  const game = await setupFastTwoPlayerGame(browser, { roomName, activeMatch: false })
  const { host, guest, hostContext, gameId: roomId } = game

  try {
    await expect(host.getByRole('heading', { name: '選用規則' })).toBeVisible()
    await expect(host.getByRole('heading', { name: '進階規則' })).toBeVisible()
    await expect(host.getByRole('heading', { name: '主題規則' })).toBeVisible()
    await expect(host.getByLabel('星辰圖記')).toBeChecked()
    await expect(host.getByLabel('星辰圖記')).toBeEnabled()
    expect(await indexedModules(host, roomName)).toContain('star')

    await expect(guest.getByLabel('星辰圖記')).toBeChecked()
    await expect(guest.getByLabel('星辰圖記')).toBeDisabled()
    await game.start()

    await Promise.all([host, guest].map(async (page) => {
      const rules = await battleRecordRuleEntry(page)
      await expect(rules).toContainText('基礎規則')
      await expect(rules).toContainText('星辰圖記規則')
      await expect(page.locator('.player-identity').filter({ hasText: '召星' })).toHaveCount(0)
    }))

    const updateResponse = await hostContext.request.put(`/api/games/${roomId}/rules`, {
      data: { enabledRuleModules: [] },
    })
    expect(updateResponse.status()).toBe(409)

    await reloadFastGameRoute(guest, roomId)
    await expect(await battleRecordRuleEntry(guest)).toContainText('星辰圖記規則')
  } finally {
    await game.close()
  }
})

test('disabling Star invalidates readiness and locked decks while preserving Base play', async ({ browser }) => {
  const roomName = `星辰關閉測試 ${Date.now()}`
  const game = await setupFastTwoPlayerGame(browser, { roomName, activeMatch: false })
  const { host, guest, hostContext, gameId: roomId } = game

  try {
    await game.readyGuest()
    await expect(guest.getByText('本局使用：五行均衡預組')).toBeVisible()
    await host.getByLabel('星辰圖記').uncheck()

    await expect(guest.getByLabel('星辰圖記')).not.toBeChecked()
    await expect(guest.getByRole('button', { name: '準備 →' })).toBeVisible()
    await expect(guest.getByText('本局使用：五行均衡預組')).toHaveCount(0)
    expect(await indexedModules(host, roomName)).not.toContain('star')

    await reloadFastGameRoute(guest, roomId)
    await expect(guest.getByLabel('星辰圖記')).not.toBeChecked()
    await game.readyGuest()
    await game.start()

    await Promise.all([host, guest].map(async (page) => {
      await expect(await battleRecordRuleEntry(page))
        .not.toContainText('星辰圖記規則')
      await expect(page.locator('.player-identity').filter({ hasText: '召星' })).toHaveCount(0)
    }))

    const active = (await host.locator('.playing-card:enabled:not(.hidden)').count()) ? host : guest
    await active.locator('.playing-card:enabled:not(.hidden)').first().click()
    await expect(active.locator('.action-panel .action-candidates button:not(.skip-action)').first())
      .toBeVisible()

    const stateResponse = await hostContext.request.get(`/api/games/${roomId}`)
    const stateBody = await stateResponse.text()
    if (!stateResponse.ok()) {
      throw new Error(`GET /api/games/${roomId} failed with HTTP ${stateResponse.status()}: ${stateBody}`)
    }
    const publicState = (JSON.parse(stateBody) as {
      state: {
        enabledRuleModules: string[]
        teamStars: unknown[]
        starHistories: unknown[]
      }
    }).state
    expect(publicState.enabledRuleModules).not.toContain('star')
    expect(publicState.teamStars).toEqual([])
    expect(publicState.starHistories).toEqual([])
  } finally {
    await game.close()
  }
})

test('a four-player team room starts with one shared immutable Star configuration', async ({ browser }) => {
  const game = await setupFastFourPlayerGame(browser, {
    roomName: `星辰團隊測試 ${Date.now()}`,
  })
  const { pages } = game

  try {
    await Promise.all(pages.map(async (page) => {
      await expect(await battleRecordRuleEntry(page))
        .toContainText('星辰圖記規則')
      await expect(page.locator('.player-seat')).toHaveCount(4)
      await expect(page.locator('.seat-discard-control')).toHaveCount(4)
      await expect(page.locator('.discard-position-top')).toHaveCount(1)
      await expect(page.locator('.discard-position-left')).toHaveCount(1)
      await expect(page.locator('.discard-position-right')).toHaveCount(1)
      await expect(page.locator('.discard-position-bottom')).toHaveCount(1)
      await expect(page.locator('.player-identity').filter({ hasText: '召星' })).toHaveCount(0)
    }))
  } finally {
    await game.close()
  }
})

test('a Star endgame fixture finishes through normal UI play and resets with its rules', async ({ browser }) => {
  const game = await setupFastTwoPlayerGame(browser, {
    roomName: `星辰殘局測試 ${Date.now()}`,
  })
  const { host, guest, pages } = game

  try {
    await seedDevelopmentScenario(host, { name: 'star-endgame' })

    await Promise.all(pages.map(page => (
      expect(page.locator('.player-identity')).toContainText(['1 HP', '1 HP'])
    )))
    const active = (await host.locator('.playing-card:enabled:not(.hidden)').count()) ? host : guest
    await active.locator('.playing-card:enabled:not(.hidden)').first().click()
    const attack = active.locator(
      '.action-panel .action-candidates button[title*="進行五行攻擊"]',
    )
    await expect(attack).toBeVisible()
    const command = active.waitForResponse(response => (
      response.request().method() === 'POST'
      && new URL(response.url()).pathname.endsWith('/commands')
    ))
    await attack.click()
    expect((await command).ok()).toBe(true)

    await Promise.all(pages.map(async (page) => {
      await expect(page.locator('.battlefield .result-reason')).toContainText('生命值歸零')
      await expect(await battleRecordRuleEntry(page))
        .toContainText('星辰圖記規則')
    }))
    await expectBattlefieldConclusion(host, 1440, 900)
    await expectBattlefieldConclusion(host, 760, 612)
    await expectBattlefieldConclusion(guest, 390, 844)
    await expectBattlefieldConclusion(guest, 360, 640)

    await guest.getByRole('button', { name: '返回房間 →' }).click()
    await Promise.all(pages.map(async (page) => {
      await expect(page.getByLabel('星辰圖記')).toBeChecked()
      await expect(page.getByRole('heading', { name: '進階規則' })).toBeVisible()
    }))
  } finally {
    await game.close()
  }
})
