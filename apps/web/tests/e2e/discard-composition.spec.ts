import { expect, test, type Page } from '@playwright/test'

const discardDialog = (page: Page) => page.getByRole('dialog', { name: '棄牌內容' })
const discardTrigger = (page: Page, count: number) => page.getByRole('button', {
  name: `查看棄牌內容，共 ${count} 張`,
})

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
  const formation = page.locator('.formation-candidates button:not(.skip-action)').first()
  await expect(formation).toBeVisible()
  await formation.click()

  await expect.poll(async () => (
    await page.locator('.choice-overlay').isVisible()
    || await page.locator('.result-panel').isVisible()
  )).toBe(true)

  return page
}

async function finishSingleCardTurn(page: Page) {
  if (await page.locator('.result-panel').isVisible()) {
    return true
  }

  const choice = page.locator('.choice-cards button:enabled').first()
  await expect(choice).toBeVisible()
  await choice.click()
  await expect(page.locator('.choice-overlay')).toBeHidden()
  return await page.locator('.result-panel').isVisible()
}

async function expectDiscardTotal(pages: Page[], count: number) {
  await Promise.all(pages.map(page => (
    expect(discardTrigger(page, count)).toBeVisible()
  )))
}

async function expectStableComposition(page: Page, total: number) {
  const dialog = discardDialog(page)
  await expect(dialog).toBeVisible()
  await expect(dialog.getByRole('columnheader')).toHaveText([
    '等級',
    '金',
    '木',
    '水',
    '火',
    '土',
  ])
  await expect(dialog.getByRole('rowheader')).toHaveText(['1', '2', '3', '4', '5'])
  await expect(dialog.getByRole('cell')).toHaveCount(25)

  const counts = await dialog.locator('tbody td strong').allTextContents()
  expect(counts.map(Number).reduce((sum, count) => sum + count, 0)).toBe(total)
}

test('an empty discard pile reports zero cards and cannot be opened', async ({ page }) => {
  await loginAsGuest(page)

  await page.getByRole('button', { name: '建立房間', exact: true }).click()
  await expect(page.getByRole('dialog', { name: '建立房間' })).toBeVisible()
  await page.getByLabel('房間名稱').fill(`棄牌測試 ${Date.now()}`)
  await page.getByLabel('個人牌組').uncheck()
  await page.getByRole('button', { name: '建立房間 →' }).click()
  await expect(page).toHaveURL(/\/rooms\/[0-9a-f-]+$/)

  const trigger = discardTrigger(page, 0)
  await expect(trigger).toHaveAttribute('aria-disabled', 'true')
  await trigger.click({ force: true })
  await expect(discardDialog(page)).toBeHidden()
})

test('players can inspect a synchronized discard composition throughout a match', async ({ browser }) => {
  test.setTimeout(300_000)

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()
  const pages = [host, guest]

  try {
    await Promise.all(pages.map(loginAsGuest))

    const roomName = `同步棄牌測試 ${Date.now()}`
    await host.getByRole('button', { name: '建立房間', exact: true }).click()
    await expect(host.getByRole('dialog', { name: '建立房間' })).toBeVisible()
    await host.getByLabel('房間名稱').fill(roomName)
    await host.getByRole('button', { name: '公開房間', exact: true }).click()
    await host.getByLabel('個人牌組').uncheck()
    await host.getByRole('button', { name: '建立房間 →' }).click()
    await expect(host).toHaveURL(/\/rooms\/[0-9a-f-]+$/)

    const listedRoom = guest.locator('.public-room-list button').filter({ hasText: roomName })
    await expect(listedRoom).toBeVisible()
    await listedRoom.click()
    await expect(guest).toHaveURL(/\/rooms\/[0-9a-f-]+$/)
    expect(new URL(host.url()).pathname).toBe(new URL(guest.url()).pathname)

    await guest.getByRole('button', { name: '準備 →' }).click()
    const startButton = host.getByRole('button', { name: '開始遊戲 →' })
    await expect(startButton).toBeEnabled()
    await startButton.click()
    await Promise.all(pages.map(page => expect(page.locator('.setup-reveal')).toBeHidden()))

    await expect(host.locator('.event-panel')).not.toContainText('隱藏')
    await expect(host.locator('.zone-summary')).toHaveCount(0)
    await expect(host.locator('.selection-count')).toHaveCount(0)
    await expect(host.locator('.game-page')).not.toContainText('伏牌')

    const desktopTextSizes = await host.locator('.game-page').evaluate((gamePage) => {
      const selectors = [
        '.event-feed span',
        '.event-feed p',
        '.player-identity small',
      ]
      return selectors.map((selector) => (
        Number.parseFloat(getComputedStyle(gamePage.querySelector(selector)!).fontSize)
      ))
    })
    expect(desktopTextSizes.every(size => size >= 11)).toBe(true)

    const handCards = host.locator('.seat-bottom .playing-card')
    const handBoxes = await handCards.evaluateAll(cards => (
      cards.map(card => {
        const box = card.getBoundingClientRect()
        return { left: box.left, right: box.right }
      }).sort((left, right) => left.left - right.left)
    ))
    for (let index = 1; index < handBoxes.length; index += 1) {
      expect(handBoxes[index - 1]!.right).toBeLessThanOrEqual(handBoxes[index]!.left)
    }

    let active = await beginSingleCardTurn(pages)
    await expect(active.getByRole('button', { name: '返回重選' })).toHaveCount(0)
    const choiceOverlayStyle = await active.locator('.choice-overlay').evaluate((overlay) => {
      const style = getComputedStyle(overlay)
      return {
        background: style.backgroundColor,
        backdropFilter: style.backdropFilter,
      }
    })
    const overlayAlpha = Number(choiceOverlayStyle.background.match(/[\d.]+\)$/)?.[0]?.slice(0, -1) ?? 1)
    expect(overlayAlpha).toBeLessThanOrEqual(0.35)
    expect(choiceOverlayStyle.backdropFilter).toBe('none')

    expect(await finishSingleCardTurn(active)).toBe(false)
    await expectDiscardTotal(pages, 2)

    const observer = pages.find(page => page !== active)!
    const trigger = discardTrigger(observer, 2)
    await trigger.click()
    await expectStableComposition(observer, 2)

    await discardDialog(observer).getByRole('heading', { name: '棄牌內容' }).click()
    await expect(discardDialog(observer)).toBeHidden()
    await expect(trigger).toBeFocused()

    await trigger.click()
    await trigger.click()
    await expect(discardDialog(observer)).toBeHidden()
    await expect(trigger).toBeFocused()

    await trigger.click()
    await observer.keyboard.press('Escape')
    await expect(discardDialog(observer)).toBeHidden()
    await expect(trigger).toBeFocused()

    await trigger.click()
    await observer.locator('.deck-pile').click()
    await expect(discardDialog(observer)).toBeHidden()
    await expect(trigger).toBeFocused()

    await observer.setViewportSize({ width: 377, height: 734 })
    await trigger.click()
    await expectStableComposition(observer, 2)
    const mobileBox = await discardDialog(observer).boundingBox()
    expect(mobileBox).not.toBeNull()
    expect(Math.abs(mobileBox!.x + mobileBox!.width / 2 - 377 / 2)).toBeLessThanOrEqual(2)
    expect(Math.abs(mobileBox!.y + mobileBox!.height / 2 - 734 / 2)).toBeLessThanOrEqual(2)
    expect(mobileBox!.x).toBeGreaterThanOrEqual(0)
    expect(mobileBox!.y).toBeGreaterThanOrEqual(0)
    expect(mobileBox!.x + mobileBox!.width).toBeLessThanOrEqual(377)
    expect(mobileBox!.y + mobileBox!.height).toBeLessThanOrEqual(734)
    await discardDialog(observer).click()

    await observer.setViewportSize({ width: 1280, height: 720 })
    await trigger.click()
    const desktopBox = await discardDialog(observer).boundingBox()
    const triggerBox = await trigger.boundingBox()
    expect(desktopBox).not.toBeNull()
    expect(triggerBox).not.toBeNull()
    expect(desktopBox!.y + desktopBox!.height).toBeLessThanOrEqual(triggerBox!.y)
    expect(desktopBox!.x).toBeGreaterThanOrEqual(0)
    expect(desktopBox!.x + desktopBox!.width).toBeLessThanOrEqual(1280)
    await observer.keyboard.press('Escape')

    const nextActive = await activePlayerPage(pages)
    const pendingObserver = pages.find(page => page !== nextActive)!
    await discardTrigger(pendingObserver, 2).click()
    await expect(discardDialog(pendingObserver)).toBeVisible()

    active = await beginSingleCardTurn(pages)
    expect(active).toBe(nextActive)
    await expect(discardDialog(pendingObserver)).toBeHidden()
    await expect(discardTrigger(pendingObserver, 3)).toHaveAttribute('aria-disabled', 'true')
    await discardTrigger(pendingObserver, 3).click({ force: true })
    await expect(discardDialog(pendingObserver)).toBeHidden()
    expect(await finishSingleCardTurn(active)).toBe(false)
    await expectDiscardTotal(pages, 4)

    let finished = false
    for (let turn = 0; turn < 40 && !finished; turn += 1) {
      active = await beginSingleCardTurn(pages)
      finished = await finishSingleCardTurn(active)
    }
    expect(finished).toBe(true)
    await Promise.all(pages.map(page => expect(page.locator('.result-panel')).toBeVisible()))
    await Promise.all(pages.map(async (page) => {
      await expect(page.locator('.turn-badge')).toHaveCount(0)
      await expect(page.locator('.player-seat.acting')).toHaveCount(0)
    }))

    await Promise.all(pages.map(page => (
      expect(page.getByRole('button', { name: '查看棄牌', exact: true })).toHaveCount(0)
    )))
    const finalPage = pages.find(page => page !== active)!
    const finalTrigger = finalPage.locator('.discard-pile-trigger')
    const finalTotal = Number(
      (await finalTrigger.getAttribute('aria-label'))?.match(/\d+/)?.[0] ?? 0,
    )
    expect(finalTotal).toBeGreaterThan(0)
    await finalTrigger.click()
    await expectStableComposition(finalPage, finalTotal)
    await discardDialog(finalPage).click()

    const resetPage = pages.find(page => page !== finalPage)!
    await resetPage.getByRole('button', { name: '返回房間 →' }).click()
    await expect(discardTrigger(finalPage, 0)).toHaveAttribute('aria-disabled', 'true')
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})
