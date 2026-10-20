import { expect, test } from '@playwright/test'

const fourPlayerStory = '/iframe.html?id=%E5%B0%8D%E5%B1%80-%E6%88%B0%E5%A0%B4%E6%A3%8B%E7%9B%A4--four-player-player-view&viewMode=story'
const twoPlayerStory = '/iframe.html?id=%E5%B0%8D%E5%B1%80-%E6%88%B0%E5%A0%B4%E6%A3%8B%E7%9B%A4--two-player-live&viewMode=story'
const visualStory = (name: string) => `/iframe.html?id=${encodeURIComponent('對局-完整戰場畫面')}--${name}&viewMode=story`

for (const viewport of [{ width: 1672, height: 941 }, { width: 1366, height: 768 }, { width: 390, height: 844 }]) {
  test(`composed battlefield keeps selected cards and actions reachable at ${viewport.width}`, async ({ page }) => {
    await page.setViewportSize(viewport)
    await page.goto(visualStory('wood-two-player-visual'))
    const hand = page.locator('.seat-bottom .seat-hand')
    const cards = hand.locator('.playing-card')
    await expect(cards).toHaveCount(5)
    await cards.nth(2).click()
    await expect(cards.nth(2)).toHaveClass(/selected/)

    // 選牌上浮後仍在手牌捲動區內，焦點與插畫不會被裁掉。
    await expect.poll(async () => {
      const selected = await cards.nth(2).boundingBox()
      const area = await hand.boundingBox()
      return Boolean(selected && area && selected.y >= area.y
        && selected.y + selected.height <= area.y + area.height
        && selected.y + selected.height <= viewport.height)
    }).toBe(true)

    const action = page.getByRole('button', { name: '轉職', exact: true })
    await action.scrollIntoViewIfNeeded()
    await expect(action).toBeInViewport()
    const ownSeat = await page.locator('.seat-bottom').boundingBox()
    const actionBox = await action.boundingBox()
    expect(actionBox!.y + actionBox!.height).toBeLessThanOrEqual(ownSeat!.y)
    expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBe(0)
  })
}

test('long hands can scroll to and select the last card without moving the board', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 })
  await page.goto(visualStory('long-hand-visual'))
  const hand = page.locator('.seat-bottom .seat-hand')
  const lastCard = hand.locator('.playing-card').last()
  const before = await page.locator('.seat-bottom').boundingBox()
  await lastCard.click()
  await expect(lastCard).toHaveClass(/selected/)
  await expect(lastCard).toBeInViewport()
  expect(await hand.evaluate(element => element.scrollLeft)).toBeGreaterThan(0)
  expect(await page.locator('.seat-bottom').boundingBox()).toEqual(before)
})

test('mobile four-player story keeps three compact opponents and own hand in the viewport', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 })
  await page.goto(fourPlayerStory)

  const battlefield = page.locator('.battlefield')
  await expect(battlefield).toHaveClass(/four-player/)

  const opponents = battlefield.locator('.player-seat:not(.seat-bottom)')
  await expect(opponents).toHaveCount(3)
  const opponentBoxes = await opponents.evaluateAll(elements => elements.map((element) => {
    const rect = element.getBoundingClientRect()
    return { x: rect.x, y: rect.y, height: rect.height }
  }))
  expect(new Set(opponentBoxes.map(candidate => Math.round(candidate.y))).size).toBe(1)
  expect(new Set(opponentBoxes.map(candidate => Math.round(candidate.height))).size).toBe(1)
  expect(opponentBoxes[0]!.x).toBeLessThan(opponentBoxes[1]!.x)
  expect(opponentBoxes[1]!.x).toBeLessThan(opponentBoxes[2]!.x)
  await expect(opponents.locator('.avatar:visible')).toHaveCount(0)
  await expect(opponents.getByText(/^手牌 \d+$/)).toHaveCount(3)

  const center = await battlefield.locator('.center-stack').boundingBox()
  const ownSeat = await battlefield.locator('.seat-bottom').boundingBox()
  expect(center).not.toBeNull()
  expect(ownSeat).not.toBeNull()
  expect(center!.y + center!.height).toBeLessThanOrEqual(ownSeat!.y + 1)
  await expect(battlefield.locator('.seat-bottom .seat-hand .playing-card')).not.toHaveCount(0)

  const overflow = await page.evaluate(() => ({
    pageHorizontal: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    pageVertical: document.documentElement.scrollHeight - window.innerHeight,
    boardHorizontal: document.querySelector('.battlefield')!.scrollWidth
      - document.querySelector('.battlefield')!.clientWidth,
    boardVertical: document.querySelector('.battlefield')!.scrollHeight
      - document.querySelector('.battlefield')!.clientHeight,
  }))
  expect(overflow.pageHorizontal).toBeLessThanOrEqual(0)
  expect(overflow.pageVertical).toBeLessThanOrEqual(1)
  expect(overflow.boardHorizontal).toBeLessThanOrEqual(0)
  expect(overflow.boardVertical).toBeLessThanOrEqual(0)
})

test('desktop two-player story keeps both seats in full-width stacked bands', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 720 })
  await page.goto(twoPlayerStory)

  const battlefield = page.locator('.battlefield')
  await expect(battlefield).not.toHaveClass(/four-player/)
  await expect(battlefield.locator('.seat-left, .seat-right')).toHaveCount(0)

  const board = await battlefield.boundingBox()
  const top = await battlefield.locator('.seat-top').boundingBox()
  const bottom = await battlefield.locator('.seat-bottom').boundingBox()
  expect(board).not.toBeNull()
  expect(top).not.toBeNull()
  expect(bottom).not.toBeNull()
  expect(top!.width).toBeGreaterThan(board!.width * .85)
  expect(bottom!.width).toBeGreaterThan(board!.width * .85)
  expect(top!.y).toBeLessThan(bottom!.y)

  const overflow = await page.evaluate(() => ({
    horizontal: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    vertical: document.documentElement.scrollHeight - window.innerHeight,
  }))
  expect(overflow.horizontal).toBeLessThanOrEqual(0)
  expect(overflow.vertical).toBeLessThanOrEqual(1)
})
