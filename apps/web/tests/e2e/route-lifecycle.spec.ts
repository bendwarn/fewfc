import { expect as bareExpect, test as bareTest, type Page } from '@playwright/test'
import { expect, fastPageTest, test } from './fixtures'

async function createWaitingRoomViaRequest(page: Page, roomName: string): Promise<string> {
  const response = await page.context().request.post('/api/games', {
    data: { name: roomName, access: 'public', capacity: 2 },
  })
  const body = await response.text()
  if (!response.ok()) {
    throw new Error(`POST /api/games failed with HTTP ${response.status()}: ${body}`)
  }
  let result: { gameId?: unknown }
  try {
    result = JSON.parse(body) as { gameId?: unknown }
  } catch {
    throw new Error(`POST /api/games returned invalid JSON: ${body}`)
  }
  if (typeof result.gameId !== 'string' || !result.gameId) {
    throw new Error(`POST /api/games returned no gameId: ${body}`)
  }
  return result.gameId
}

bareTest('unauthenticated deep links preserve room invite and replay queries through login', async ({ page }) => {
  await page.goto('/rooms/room-7?invite=invite-token')
  await bareExpect(page).toHaveURL(/\/login/)
  bareExpect(new URL(page.url()).searchParams.get('redirect')).toBe('/rooms/room-7?invite=invite-token')
  await bareExpect(page.locator('.login-hero')).toBeVisible()
  await bareExpect(page.getByRole('navigation', { name: '帳號選單' })).toHaveCount(0)

  await page.goto('/replays/replay-9?step=12')
  await bareExpect(page).toHaveURL(/\/login/)
  bareExpect(new URL(page.url()).searchParams.get('redirect')).toBe('/replays/replay-9?step=12')
})

fastPageTest('Deck owns its route-local loading and retryable error outcomes', async ({ page }) => {
  let releaseDeckLoad: (() => void) | undefined
  const deckLoadGate = new Promise<void>((resolve) => { releaseDeckLoad = resolve })
  const deckUrl = '**/api/deck'
  await page.route(deckUrl, async (route) => {
    if (route.request().method() === 'GET') await deckLoadGate
    try {
      await route.continue()
    } catch (error) {
      if (!(error instanceof Error) || !error.message.includes('already handled')) throw error
    }
  })

  const navigation = page.goto('/deck')
  await expect(page.getByRole('heading', { name: '正在載入' })).toBeVisible()
  releaseDeckLoad?.()
  await navigation
  await expect(page.getByRole('button', { name: '匯入牌組' })).toBeVisible()
  await page.unroute(deckUrl)

  await page.route(deckUrl, async (route) => {
    if (route.request().method() === 'GET') {
      await route.fulfill({ status: 500, contentType: 'application/json', body: '{"statusMessage":"internal server error"}' })
      return
    }
    await route.continue()
  })
  await page.goto('/deck')
  await expect(page.getByRole('heading', { name: '無法載入牌組' })).toBeVisible()
  await expect(page.getByRole('button', { name: '重試' })).toBeVisible()
  await page.unroute(deckUrl)
})

fastPageTest('Deck owns its import dialog focus, cleans up its listener, and Replay routes render their own error outcome', async ({ page }) => {
  await page.goto('/deck')
  const importButton = page.getByRole('button', { name: '匯入牌組' })
  await importButton.click()
  await expect(page.getByRole('dialog', { name: '匯入牌組' })).toBeVisible()
  await expect(page.getByLabel('牌組張數')).toBeFocused()
  await page.keyboard.press('Escape')
  await expect(importButton).toBeFocused()

  await Promise.all([
    page.goto('/rooms', { waitUntil: 'domcontentloaded' }),
    page.context().request.get('/api/games'),
  ])
  const createRoomButton = page.getByRole('button', { name: '建立房間', exact: true })
  await createRoomButton.click()
  await expect(page.getByRole('dialog', { name: '建立房間' })).toBeVisible()
  await page.goto('/deck')
  const escapeWasPrevented = await page.evaluate(() => {
    const event = new KeyboardEvent('keydown', { key: 'Escape', cancelable: true })
    window.dispatchEvent(event)
    return event.defaultPrevented
  })
  bareExpect(escapeWasPrevented).toBe(false)

  await page.goto('/replays')
  await expect(page.getByRole('heading', { name: '重播紀錄' })).toBeVisible()
  await page.goto('/replays/missing-replay?step=0')
  await expect(page.getByText('找不到這個重播')).toBeVisible()
})

test('Lobby exposes its route-local loading state before rooms arrive', async ({ page }) => {
  let releaseRoomListLoad: (() => void) | undefined
  const roomListGate = new Promise<void>((resolve) => { releaseRoomListLoad = resolve })
  await page.route('**/api/games', async (route) => {
    if (route.request().method() === 'GET') await roomListGate
    await route.continue()
  })

  const navigation = page.goto('/rooms')
  await expect(page.getByRole('status').filter({ hasText: '正在載入房間…' })).toBeVisible()
  releaseRoomListLoad?.()
  await navigation
  await expect(page.getByRole('heading', { name: '公開房間' })).toBeVisible()
  await page.unroute('**/api/games')
})

test('Lobby navigation reloads Game state, resets parameter-local state, and tears down the room session before re-entry', async ({ page }) => {
  await page.setViewportSize({ width: 600, height: 900 })
  const gameId = await createWaitingRoomViaRequest(page, `路由生命週期 ${Date.now()}`)
  const nextGameId = await createWaitingRoomViaRequest(page, `路由生命週期下一局 ${Date.now()}`)
  await page.goto('/rooms')

  const openedSockets: string[] = []
  const closedSockets: string[] = []
  page.on('websocket', socket => {
    if (
      socket.url().includes(`/api/games/${gameId}/socket`)
      || socket.url().includes(`/api/games/${nextGameId}/socket`)
    ) {
      openedSockets.push(socket.url())
      socket.on('close', () => closedSockets.push(socket.url()))
    }
  })

  await page.locator('.my-rooms-card .public-room-list button').filter({ hasText: gameId.slice(0, 8) }).click()
  await expect(page).toHaveURL(new RegExp(`/rooms/${gameId}$`))
  await expect(page.locator('.waiting-overlay')).toBeVisible()
  await expect.poll(() => openedSockets.filter(url => url.includes(`/api/games/${gameId}/socket`)).length).toBe(1)

  await page.getByRole('button', { name: '完整紀錄' }).click()
  await expect(page.locator('.event-panel.expanded')).toBeVisible()
  await page.evaluate((path) => {
    history.replaceState(history.state, '', path)
    window.dispatchEvent(new PopStateEvent('popstate', { state: history.state }))
  }, `/rooms/${nextGameId}`)
  await expect(page).toHaveURL(new RegExp(`/rooms/${nextGameId}$`))
  await expect(page.locator('.waiting-overlay')).toBeVisible()
  await expect(page.locator('.event-panel.expanded')).toHaveCount(0)
  await expect.poll(() => openedSockets.some(url => url.includes(`/api/games/${nextGameId}/socket`))).toBe(true)

  await page.goto('/rooms')
  await expect(page.getByRole('heading', { name: '公開房間' })).toBeVisible()
  await expect.poll(() => closedSockets.some(url => url.includes(`/api/games/${nextGameId}/socket`))).toBe(true)

  await page.goto(`/rooms/${gameId}`)
  await expect(page.locator('.waiting-overlay')).toBeVisible()
  await expect.poll(() => openedSockets.filter(url => url.includes(`/api/games/${gameId}/socket`)).length).toBe(2)
})

fastPageTest('leaving a loading Game route cannot recreate its room session', async ({ page }) => {
  const gameId = await createWaitingRoomViaRequest(page, `離開載入中的房間 ${Date.now()}`)
  await page.goto('/rooms')

  const openedSockets: string[] = []
  page.on('websocket', socket => {
    if (socket.url().includes(`/api/games/${gameId}/socket`)) openedSockets.push(socket.url())
  })

  let releaseRoomFetch: (() => void) | undefined
  const roomFetchGate = new Promise<void>((resolve) => { releaseRoomFetch = resolve })
  const roomUrl = `**/api/games/${gameId}`
  await page.route(roomUrl, async (route) => {
    if (route.request().method() === 'GET') await roomFetchGate
    try {
      await route.continue()
    } catch (error) {
      if (!(error instanceof Error) || !error.message.includes('already handled')) throw error
    }
  })

  try {
    await page.locator('.my-rooms-card .public-room-list button').filter({ hasText: gameId.slice(0, 8) }).click()
    await expect(page).toHaveURL(new RegExp(`/rooms/${gameId}$`))
    await expect(page.getByRole('heading', { name: '正在載入' })).toBeVisible()

    await page.getByRole('button', { name: '回到首頁' }).click()
    await expect(page).toHaveURL(/\/rooms$/)
  } finally {
    releaseRoomFetch?.()
    await page.unroute(roomUrl)
  }

  await page.waitForTimeout(250)
  bareExpect(openedSockets).toHaveLength(0)
})
