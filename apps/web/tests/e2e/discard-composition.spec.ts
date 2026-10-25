import { waitForGameCommand } from './realtime-helpers'
import { type Page } from '@playwright/test'
import {
  expect,
  reloadFastGameRoute,
  setupFastTwoPlayerGame,
  test,
} from './fixtures'

const discardDialog = (page: Page) => page.locator('#discard-composition')
const discardTrigger = (page: Page, count: number) => page.getByRole('button', {
  name: new RegExp(`查看共用棄牌詳情。目前棄牌 ${count} 張`),
})
const matrixElements = [
  ['金', 'Metal'],
  ['木', 'Wood'],
  ['水', 'Water'],
  ['火', 'Fire'],
  ['土', 'Earth'],
] as const

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

async function beginSingleCardTurn(pages: Page[]) {
  const page = await activePlayerPage(pages)
  const cards = page.locator('.playing-card:enabled:not(.hidden)')
  const labels = await cards.evaluateAll(elements => (
    elements.map(element => element.getAttribute('aria-label') ?? '')
  ))
  const highestLevelIndex = labels.reduce((bestIndex, label, index) => {
    const level = Number(label.match(/\d+$/)?.[0] ?? 0)
    const bestLevel = Number(labels[bestIndex]?.match(/\d+$/)?.[0] ?? 0)
    return level > bestLevel ? index : bestIndex
  }, 0)

  await cards.nth(highestLevelIndex).click()
  const formation = page.locator('.action-panel .action-candidates button:not(.skip-action)').first()
  await expect(formation).toBeVisible()
  const command = waitForGameCommand(page)
  await formation.click()
  expect((await command).ok).toBe(true)

  let outcomePage: Page | undefined
  await expect.poll(async () => {
    for (const candidate of pages) {
      if (await candidate.locator('.choice-overlay').isVisible()) {
        outcomePage = candidate
        return true
      }
    }
    for (const candidate of pages) {
      if (await candidate.locator('.result-panel').isVisible()) {
        outcomePage = candidate
        return true
      }
    }
    return false
  }).toBe(true)

  return outcomePage!
}

async function finishSingleCardTurn(page: Page) {
  if (await page.locator('.result-panel').isVisible()) {
    return true
  }

  const choice = page.locator('.choice-cards button:enabled').first()
  await expect(choice).toBeVisible()
  const command = waitForGameCommand(page)
  await choice.click()
  expect((await command).ok).toBe(true)
  await expect(page.locator('.choice-overlay')).toBeHidden()
  return await page.locator('.result-panel').isVisible()
}

async function expectDiscardTotal(pages: Page[], count: number) {
  await Promise.all(pages.map(page => (
    expect(discardTrigger(page, count)).toBeVisible()
  )))
}

async function expectCompositionTotal(page: Page, total: number) {
  const dialog = discardDialog(page)
  await expect(dialog).toBeVisible()
  await expect(dialog.getByRole('heading')).toHaveText(new RegExp(`^目前棄牌 ${total} · 牌庫 \\d+$`))
  const counts = await dialog.locator('tbody td strong').allTextContents()
  expect(counts.map(Number).reduce((sum, count) => sum + count, 0)).toBe(total)
}

async function expectDiscardMatrixMatchesGameState(page: Page) {
  const gameId = new URL(page.url()).pathname.split('/').at(-1)
  if (!gameId) throw new Error(`No game id in ${page.url()}`)
  const response = await page.context().request.get(`/api/games/${gameId}`)
  const body = await response.text()
  if (!response.ok()) {
    throw new Error(`GET /api/games/${gameId} failed with HTTP ${response.status()}: ${body}`)
  }
  const discard = (JSON.parse(body) as {
    state: { discard: Array<{ element: string | null; level: number | null }> }
  }).state.discard

  for (const [rowIndex, [elementLabel, element]] of matrixElements.entries()) {
    const row = discardDialog(page).locator('tbody tr').nth(rowIndex)
    await expect(row.getByRole('rowheader')).toHaveText(elementLabel)
    for (let level = 1; level <= 5; level += 1) {
      const expectedCount = discard.filter(card => card.element === element && card.level === level).length
      await expect(row.locator('td').nth(level - 1).locator('strong')).toHaveText(String(expectedCount))
    }
  }
}

test('players can inspect a synchronized discard composition throughout a match', async ({ browser }) => {
  // 完整棄牌對局包含多次回合操作與跨分頁同步，完整套件負載下超過預設 30 秒。
  test.setTimeout(90_000)
  const game = await setupFastTwoPlayerGame(browser, {
    roomName: `同步棄牌測試 ${Date.now()}`,
    disabledRuleModules: ['five-directions-legend', 'personal-deck'],
  })
  const { pages, gameId } = game

  try {
    await Promise.all(pages.map(async (page) => {
      await expect(page.locator('.seat-bottom .playing-card:not(.hidden)')).not.toHaveCount(0)
      await expect(page.locator('.seat-top').getByRole('img', { name: /的手牌 \d+ 張$/ })).toBeVisible()
      await expect(page.locator('.seat-top .playing-card')).toHaveCount(0)
    }))
    let active = await beginSingleCardTurn(pages)
    const hiddenChoiceObserver = pages.find(page => page !== active)!
    await expect(active.locator('.choice-overlay')).toBeVisible()
    await expect(hiddenChoiceObserver.locator('.choice-overlay')).toBeHidden()
    await reloadFastGameRoute(active, gameId)
    await expect(active.locator('.choice-overlay')).toBeVisible()
    expect(await finishSingleCardTurn(active)).toBe(false)
    await expectDiscardTotal(pages, 2)

    const observer = pages.find(page => page !== active)!
    const trigger = discardTrigger(observer, 2)
    await trigger.click()
    await expectCompositionTotal(observer, 2)
    await expectDiscardMatrixMatchesGameState(observer)

    await discardDialog(observer).getByRole('button', { name: '關閉詳情' }).click()
    await expect(discardDialog(observer)).toBeHidden()
    const nextActive = await activePlayerPage(pages)
    const pendingObserver = pages.find(page => page !== nextActive)!
    await discardTrigger(pendingObserver, 2).click()
    await expect(discardDialog(pendingObserver)).toBeVisible()

    active = await beginSingleCardTurn(pages)
    expect(active).toBe(nextActive)
    await expect(discardDialog(pendingObserver)).toBeVisible()
    await expect(discardTrigger(pendingObserver, 3)).toHaveAttribute('aria-disabled', 'true')
    await expect(discardDialog(pendingObserver).getByRole('heading'))
      .toHaveText(/^目前棄牌 3 · 牌庫 \d+$/)
    expect(await finishSingleCardTurn(active)).toBe(false)
    await expectDiscardTotal(pages, 4)
    await expect(discardDialog(pendingObserver).getByRole('heading'))
      .toHaveText(/^目前棄牌 4 · 牌庫 \d+$/)
    await discardDialog(pendingObserver).getByRole('button', { name: '關閉詳情' }).click()
    await expect(discardDialog(pendingObserver)).toBeHidden()
  } finally {
    await game.close()
  }
})
