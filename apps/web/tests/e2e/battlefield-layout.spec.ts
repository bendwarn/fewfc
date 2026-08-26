import { type Locator, type Page } from '@playwright/test'
import {
  expect,
  setupFastFourPlayerGame,
  setupFastTwoPlayerGame,
  test,
} from './fixtures'

async function box(locator: Locator) {
  const value = await locator.boundingBox()
  expect(value).not.toBeNull()
  return value!
}

async function expectViewportLocked(page: Page) {
  const overflow = await page.evaluate(() => ({
    horizontal: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    vertical: document.documentElement.scrollHeight - window.innerHeight,
    battlefieldHorizontal: document.querySelector('.battlefield')!.scrollWidth
      - document.querySelector('.battlefield')!.clientWidth,
    battlefieldVertical: document.querySelector('.battlefield')!.scrollHeight
      - document.querySelector('.battlefield')!.clientHeight,
  }))
  expect(overflow.horizontal).toBeLessThanOrEqual(0)
  expect(overflow.vertical).toBeLessThanOrEqual(1)
  expect(overflow.battlefieldHorizontal).toBeLessThanOrEqual(0)
  expect(overflow.battlefieldVertical).toBeLessThanOrEqual(0)
}

async function expectEventGroupTitleSingleLine(page: Page, feedSelector: string) {
  const title = page.locator(`${feedSelector} .event-group-title strong`).first()
  await expect(title).toBeVisible()
  const metrics = await title.evaluate((element) => {
    const style = getComputedStyle(element)
    const rect = element.getBoundingClientRect()
    return { height: rect.height, lineHeight: Number.parseFloat(style.lineHeight) }
  })
  expect(metrics.height).toBeLessThanOrEqual(metrics.lineHeight * 1.5)
}

async function activePlayerPage(pages: readonly Page[]) {
  await expect.poll(async () => Math.max(...await Promise.all(
    pages.map(page => page.locator('.turn-controls').count()),
  ))).toBe(1)
  for (const page of pages) {
    if (await page.locator('.turn-controls').count()) return page
  }
  throw new Error('No active player page')
}

async function expectCompactFourPlayerTable(page: Page, width: number, height: number, expectActions = false) {
  await page.setViewportSize({ width, height })
  const battlefield = page.locator('.battlefield')
  await expect(battlefield).toHaveClass(/four-player/)

  const opponents = battlefield.locator('.player-seat:not(.seat-bottom)')
  await expect(opponents).toHaveCount(3)
  const opponentBoxes = await opponents.evaluateAll(elements => elements.map((element) => {
    const rect = element.getBoundingClientRect()
    return { x: rect.x, y: rect.y, width: rect.width, height: rect.height }
  }))
  expect(new Set(opponentBoxes.map(candidate => Math.round(candidate.y))).size).toBe(1)
  expect(new Set(opponentBoxes.map(candidate => Math.round(candidate.height))).size).toBe(1)
  expect(opponentBoxes[0]!.x).toBeLessThan(opponentBoxes[1]!.x)
  expect(opponentBoxes[1]!.x).toBeLessThan(opponentBoxes[2]!.x)
  await expect(opponents.locator('.avatar:visible')).toHaveCount(0)
  await expect(opponents.getByText(/^\u624b\u724c \d+$/)).toHaveCount(3)

  const center = await box(battlefield.locator('.center-stack'))
  const ownSeat = await box(battlefield.locator('.seat-bottom'))
  expect(center.y + center.height).toBeLessThanOrEqual(ownSeat.y + 1)
  await expect(battlefield.locator('.seat-bottom .seat-hand .playing-card')).not.toHaveCount(0)
  if (expectActions) {
    const actions = battlefield.locator('.turn-controls')
    await expect(actions).toBeVisible()
    const actionBox = await box(battlefield.locator('.action-dock'))
    const formationBox = await box(battlefield.locator('.formation-field'))
    expect(actionBox.x).toBeGreaterThanOrEqual(formationBox.x + formationBox.width - 1)
    expect(Math.abs(actionBox.y - formationBox.y)).toBeLessThanOrEqual(1)
    expect(Math.abs(actionBox.height - formationBox.height)).toBeLessThanOrEqual(1)
    const scrollState = await battlefield.locator('.action-dock').evaluate(element => ({
      overflowY: getComputedStyle(element).overflowY,
      clientHeight: element.clientHeight,
      scrollHeight: element.scrollHeight,
    }))
    expect(scrollState.overflowY).toBe('auto')
    expect(scrollState.clientHeight).toBeGreaterThan(0)
    expect(scrollState.scrollHeight).toBeGreaterThanOrEqual(scrollState.clientHeight)
  }

  const actingSeat = battlefield.locator('.player-seat[aria-current="true"]')
  await expect(actingSeat).toHaveCount(1)
  const actingStyle = await actingSeat.evaluate(element => ({
    outline: getComputedStyle(element).outlineStyle,
    shadow: getComputedStyle(element).boxShadow,
  }))
  expect(actingStyle.outline).not.toBe('none')
  expect(actingStyle.shadow).not.toBe('none')
  const eventSummary = page.locator('.mobile-event-summary')
  await expect(eventSummary).toBeVisible()
  expect((await box(eventSummary)).height).toBeLessThanOrEqual(40)
  await eventSummary.click()
  const eventSheet = page.getByRole('dialog', { name: '戰局紀錄' })
  await expect(eventSheet).toBeVisible()
  await expect(eventSheet.getByText(/^第 1 回合・/)).toBeVisible()
  await expectEventGroupTitleSingleLine(page, '.event-sheet-feed')
  await eventSheet.getByRole('button', { name: '關閉戰局紀錄' }).click()
  await expect(eventSummary).toBeFocused()
  await expectViewportLocked(page)
}

test('four-player table keeps desktop compass seats and one-row compact opponents', async ({ browser }) => {
  test.setTimeout(60_000)
  const game = await setupFastFourPlayerGame(browser, {
    roomName: `四人牌桌排版 ${Date.now()}`,
  })

  try {
    const desktop = game.pages[0]!
    await desktop.setViewportSize({ width: 1440, height: 900 })
    const battlefield = desktop.locator('.battlefield')
    const center = await box(battlefield.locator('.center-stack'))
    const top = await box(battlefield.locator('.seat-top'))
    const bottom = await box(battlefield.locator('.seat-bottom'))
    const left = await box(battlefield.locator('.seat-left'))
    const right = await box(battlefield.locator('.seat-right'))
    expect(left.x + left.width).toBeLessThanOrEqual(center.x + 1)
    expect(right.x).toBeGreaterThanOrEqual(center.x + center.width - 1)
    expect(top.y + top.height).toBeLessThanOrEqual(center.y + 1)
    expect(bottom.y).toBeGreaterThanOrEqual(center.y + center.height - 1)
    expect(center.width).toBeGreaterThan(300)
    await expectEventGroupTitleSingleLine(desktop, '.game-sidebar .event-feed')
    await expectViewportLocked(desktop)

    const activePage = await activePlayerPage(game.pages)
    await expectCompactFourPlayerTable(activePage, 360, 640, true)
    await expectCompactFourPlayerTable(game.pages.find(page => page !== activePage)!, 390, 844)
  } finally {
    await game.close()
  }
})

test('two-player desktop table uses full-width stacked bands', async ({ browser }) => {
  const game = await setupFastTwoPlayerGame(browser, {
    roomName: `兩人牌桌排版 ${Date.now()}`,
  })

  try {
    const page = game.host
    await page.setViewportSize({ width: 1280, height: 720 })
    const battlefield = page.locator('.battlefield')
    await expect(battlefield).not.toHaveClass(/four-player/)
    await expect(battlefield.locator('.seat-left, .seat-right')).toHaveCount(0)
    const board = await box(battlefield)
    const top = await box(battlefield.locator('.seat-top'))
    const bottom = await box(battlefield.locator('.seat-bottom'))
    expect(top.width).toBeGreaterThan(board.width * .85)
    expect(bottom.width).toBeGreaterThan(board.width * .85)
    expect(top.y).toBeLessThan(bottom.y)
    await expectViewportLocked(page)
  } finally {
    await game.close()
  }
})
