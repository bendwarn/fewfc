import { expect, test } from '@playwright/test'

const fourPlayerStory = '/iframe.html?id=%E5%B0%8D%E5%B1%80-%E6%88%B0%E5%A0%B4%E6%A3%8B%E7%9B%A4--four-player-player-view&viewMode=story'
const twoPlayerStory = '/iframe.html?id=%E5%B0%8D%E5%B1%80-%E6%88%B0%E5%A0%B4%E6%A3%8B%E7%9B%A4--two-player-live&viewMode=story'
const visualStory = (name: string) => `/iframe.html?id=${encodeURIComponent('對局-完整戰場畫面')}--${name}&viewMode=story`

for (const viewport of [{ width: 1672, height: 941 }, { width: 1366, height: 768 }, { width: 390, height: 844 }]) {
  test(`composed battlefield keeps selected cards and actions reachable at ${viewport.width}`, async ({ page }) => {
    await page.setViewportSize(viewport)
    await page.goto(visualStory('wood-two-player-visual'))
    const capsuleParts = page.locator('.turn-capsule-heading > *')
    await expect(capsuleParts).toHaveCount(3)
    const centers = await capsuleParts.evaluateAll(es => es.map(e => {
      const rect = e.getBoundingClientRect()
      return rect.y + rect.height / 2
    }))
    expect(Math.max(...centers) - Math.min(...centers)).toBeLessThan(2)
    for (const group of await page.locator('.battle-action-group').all()) {
      const label = await group.locator('.battle-action-label').boundingBox()
      const options = await group.locator('.battle-action-options').boundingBox()
      expect(label!.x).toBeGreaterThanOrEqual(options!.x + options!.width)
    }
    const hand = page.locator('.seat-bottom .seat-hand')
    const cards = hand.locator('.playing-card')
    await expect(cards).toHaveCount(5)
    await expect(page.locator('.seat-top .compact-hand-count')).toHaveCount(0)
    const firstBack = await page.locator('.seat-top .seat-hand-backs img').first().boundingBox()
    const ownFirstCard = await cards.first().boundingBox()
    expect(Math.abs(firstBack!.x - ownFirstCard!.x)).toBeLessThan(1)
    if (viewport.width > 900) {
      const opponent = await page.locator('.seat-top').boundingBox()
      const formation = await page.locator('.formation-field').boundingBox()
      const actions = await page.locator('.action-dock-slot').boundingBox()
      expect(opponent!.x).toBe(formation!.x)
      expect(opponent!.width).toBe(formation!.width)
      expect(actions!.y).toBe(opponent!.y)
      expect(actions!.x).toBeGreaterThan(opponent!.x + opponent!.width)
      expect(actions!.y + actions!.height).toBe(formation!.y + formation!.height)
    }
    expect(await hand.evaluate(element => element.scrollWidth - element.clientWidth)).toBe(0)
    const identity = await page.locator('.seat-bottom .player-identity').boundingBox()
    const firstCard = await cards.first().boundingBox()
    expect(firstCard!.x - identity!.x - identity!.width).toBeLessThanOrEqual(14)
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

for (const width of [1024, 1366, 1672]) {
  test(`desktop long hand fits without scrolling at ${width}`, async ({ page }) => {
    await page.setViewportSize({ width, height: 941 })
    await page.goto(visualStory('long-hand-visual'))
    const hand = page.locator('.seat-bottom .seat-hand')
    const cards = hand.locator('.playing-card')
    await expect(cards).toHaveCount(10)
    expect(await hand.evaluate(element => element.scrollWidth - element.clientWidth)).toBe(0)
    await cards.last().click()
    await expect(cards.last()).toHaveClass(/selected/)
    const handBox = await hand.boundingBox()
    const lastCard = await cards.last().boundingBox()
    expect(lastCard!.x + lastCard!.width).toBeLessThanOrEqual(handBox!.x + handBox!.width)
    expect(await hand.evaluate(element => element.scrollLeft)).toBe(0)
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
  await expect(opponents.getByText(/^手牌 \d+$/)).toHaveCount(2)
  await expect(opponents.locator('.seat-hand-backs')).toHaveCount(1)

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

test('desktop two-player story aligns the opponent with the formation and keeps the own hand full width', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 720 })
  await page.goto(twoPlayerStory)

  const battlefield = page.locator('.battlefield')
  await expect(battlefield).not.toHaveClass(/four-player/)
  await expect(battlefield.locator('.seat-left, .seat-right')).toHaveCount(0)

  const board = await battlefield.boundingBox()
  const top = await battlefield.locator('.seat-top').boundingBox()
  const formation = await battlefield.locator('.formation-field').boundingBox()
  const actions = await battlefield.locator('.action-dock-slot').boundingBox()
  const bottom = await battlefield.locator('.seat-bottom').boundingBox()
  expect(board).not.toBeNull()
  expect(top).not.toBeNull()
  expect(bottom).not.toBeNull()
  expect(top!.width).toBe(formation!.width)
  expect(top!.x).toBe(formation!.x)
  expect(actions!.y).toBe(top!.y)
  expect(actions!.height).toBeGreaterThan(formation!.height)
  expect(bottom!.width).toBeGreaterThan(board!.width * .85)
  expect(top!.y).toBeLessThan(bottom!.y)

  const overflow = await page.evaluate(() => ({
    horizontal: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    vertical: document.documentElement.scrollHeight - window.innerHeight,
  }))
  expect(overflow.horizontal).toBeLessThanOrEqual(0)
  expect(overflow.vertical).toBeLessThanOrEqual(1)
})

for (const width of [1366, 390]) {
  test(`expanded opponent card backs stay within the fixed player row at ${width}`, async ({ page }) => {
    await page.setViewportSize({ width, height: 844 })
    await page.goto(visualStory('expanded-opponent-hand'))
    const opponents = page.locator('.seat-top')
    const active = opponents.filter({ has: page.locator('.seat-hand-backs') })
    await expect(active).toHaveCount(1)
    await expect(active).toHaveAttribute('aria-current', 'true')
    await expect(active.locator('.seat-hand-backs img')).toHaveCount(14)
    await expect(active.locator('.compact-hand-count')).toHaveCount(0)
    await expect(opponents.filter({ hasNot: page.locator('.seat-hand-backs') }).locator('.compact-hand-count')).toHaveCount(2)
    const before = await opponents.evaluateAll(es => es.map(e => {
      const r = e.getBoundingClientRect()
      return { name: e.getAttribute('aria-label')?.split('，')[0], y: r.y, height: r.height }
    }))
    expect(new Set(before.map(r => r.height)).size).toBe(1)
    const seat = await active.boundingBox()
    const backs = await active.locator('.seat-hand-backs').boundingBox()
    const identity = await active.locator('.player-identity').boundingBox()
    expect(backs!.x - identity!.x - identity!.width).toBeLessThanOrEqual(6)
    expect(backs!.x).toBeGreaterThanOrEqual(identity!.x + identity!.width)
    expect(backs!.y).toBeGreaterThanOrEqual(seat!.y)
    expect(backs!.y + backs!.height).toBeLessThanOrEqual(seat!.y + seat!.height)
    expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBe(0)
    if (width > 900) {
      await page.getByRole('button', { name: '切換回合提示' }).click()
      await expect(opponents.filter({ has: page.locator('.seat-hand-backs') })).toHaveCount(0)
      await expect(page.locator('.seat-bottom')).toHaveClass(/is-expanded/)
      const after = await opponents.evaluateAll(es => es.map(e => {
        const r = e.getBoundingClientRect()
        return { name: e.getAttribute('aria-label')?.split('，')[0], y: r.y, height: r.height }
      }))
      expect(after).toEqual(before)
    }
  })
}

test('two-player seats stay expanded across turns and the current player capsule loops', async ({ page }) => {
  await page.setViewportSize({ width: 1366, height: 768 })
  await page.goto(visualStory('two-player-always-expanded'))
  await expect(page.locator('.player-seat.is-expanded')).toHaveCount(2)
  await expect(page.locator('.seat-top .seat-hand-backs img')).toHaveCount(4)
  const capsule = page.locator('.turn-capsule')
  await expect(capsule).toHaveCSS('animation-iteration-count', 'infinite')
  await page.getByRole('button', { name: '切換回合提示' }).click()
  await expect(page.locator('.player-seat.is-expanded')).toHaveCount(2)
  await expect(page.locator('.seat-top .seat-hand-backs img')).toHaveCount(4)
  await expect(capsule).toHaveCSS('animation-name', 'none')
})

test('desktop header reveals on proximity or keyboard focus without moving the battlefield', async ({ page }) => {
  await page.setViewportSize({ width: 1366, height: 768 })
  await page.goto(visualStory('desktop-reveal-header'))
  const header = page.locator('.site-header')
  const board = page.locator('.battlefield')
  await page.mouse.move(600, 400)
  await expect(header).toHaveCSS('opacity', '0')
  const before = await board.boundingBox()
  await page.mouse.move(600, 2)
  await expect(header).toHaveCSS('opacity', '1')
  expect(await board.boundingBox()).toEqual(before)
  await page.mouse.move(600, 400)
  await expect(header).toHaveCSS('opacity', '0')
  await header.getByRole('button', { name: '回到首頁' }).focus()
  await expect(header).toHaveCSS('opacity', '1')
  expect(await board.boundingBox()).toEqual(before)
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await expect(page.locator('.turn-capsule')).toHaveCSS('animation-name', 'none')
})

test('mobile header remains visible above the battlefield', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 })
  await page.goto(visualStory('desktop-reveal-header'))
  const header = page.locator('.site-header')
  await expect(header).toHaveCSS('opacity', '1')
  const headerBox = await header.boundingBox()
  const boardBox = await page.locator('.battlefield').boundingBox()
  expect(boardBox!.y).toBeGreaterThanOrEqual(headerBox!.y + headerBox!.height)
})
