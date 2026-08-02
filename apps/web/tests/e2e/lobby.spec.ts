import {
  expect,
  reloadAppRoute,
  seedDevelopmentScenario,
  signInAnonymously,
  setupFastWaitingRoom,
  startTwoPlayerMatch,
  test,
} from './fixtures'

test('the lobby lists rooms before showing room settings', async ({ page }) => {
  await expect(page.getByRole('heading', { name: '公開房間' })).toBeVisible()
  await expect(page.getByRole('heading', { name: '我的房間' })).toBeVisible()

  const createRoom = page.getByRole('button', { name: '建立房間', exact: true })
  await createRoom.click()

  const dialog = page.getByRole('dialog', { name: '建立房間' })
  await expect(dialog).toBeVisible()
  await expect(page.getByLabel('房間名稱')).toBeFocused()

  await page.keyboard.press('Escape')
  await expect(dialog).toBeHidden()
  await expect(createRoom).toBeFocused()
})

test('all-enabled rooms omit a rule summary and list only disabled differences', async ({ page }) => {
  const roomName = `規則摘要 ${Date.now()}`
  const created = await page.context().request.post('/api/games', {
    data: { name: roomName, access: 'public', capacity: 2 },
  })
  expect(created.ok()).toBe(true)
  const body = await created.json() as {
    gameId: string
    metadata: { enabledRuleModules: string[] }
  }
  expect(body.metadata.enabledRuleModules).toContain('pouch')

  await reloadAppRoute(page)
  const room = page.locator('.my-rooms-card')
    .locator('.public-room-list button').filter({ hasText: roomName })
  await expect(room).toBeVisible()
  await expect(room).not.toContainText('停用：')

  const updated = await page.context().request.put(`/api/games/${body.gameId}/rules`, {
    data: {
      enabledRuleModules: body.metadata.enabledRuleModules.filter(module => module !== 'pouch'),
    },
  })
  expect(updated.ok()).toBe(true)
  await reloadAppRoute(page)
  await expect(room).toContainText('停用：錦囊')
})

test('the lobby lists joined public rooms only in my rooms', async ({ browser }) => {
  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const roomName = `已加入公開房間 ${Date.now()}`

  try {
    await Promise.all([
      signInAnonymously(hostContext, 'lobby host'),
      signInAnonymously(guestContext, 'lobby guest'),
    ])
    const host = await hostContext.newPage()
    const guest = await guestContext.newPage()
    await Promise.all([
      host.goto('/rooms', { waitUntil: 'domcontentloaded' }),
      guest.goto('/rooms', { waitUntil: 'domcontentloaded' }),
    ])
    const created = await host.context().request.post('/api/games', {
      data: { name: roomName, access: 'public', capacity: 2 },
    })
    expect(created.ok()).toBe(true)
    const body = await created.json() as {
      gameId: string
      invitation: { roomCode: string }
    }

    const hostPublicRooms = host.locator('.public-rooms-card').filter({
      has: host.getByRole('heading', { name: '公開房間', exact: true }),
    })
    const hostMyRooms = host.locator('.my-rooms-card')
    await reloadAppRoute(host)
    await expect(hostPublicRooms.locator('button').filter({ hasText: roomName })).toHaveCount(0)
    const hostRoom = hostMyRooms.locator('button').filter({ hasText: roomName })
    await expect(hostRoom).toHaveCount(1)
    await expect(hostRoom).toContainText(body.gameId.slice(0, 8))
    await expect(hostRoom).not.toContainText(body.invitation.roomCode)

    const guestPublicRooms = guest.locator('.public-rooms-card').filter({
      has: guest.getByRole('heading', { name: '公開房間', exact: true }),
    })
    const guestMyRooms = guest.locator('.my-rooms-card')
    await reloadAppRoute(guest)
    const guestRoom = guestPublicRooms.locator('button').filter({ hasText: roomName })
    await expect(guestRoom).toBeVisible()
    await expect(guestRoom).toContainText(body.invitation.roomCode)
    const joined = guest.waitForResponse(response => (
      response.url().endsWith(`/api/games/${body.gameId}/join`)
      && response.request().method() === 'POST'
    ))
    await guestRoom.click()
    expect((await joined).ok()).toBe(true)
    await expect(guest).toHaveURL(/\/rooms\/[0-9a-f-]+$/)

    await guest.goto('/rooms')
    await expect(guestPublicRooms.locator('button').filter({ hasText: roomName })).toHaveCount(0)
    await expect(guestMyRooms.locator('button').filter({ hasText: roomName })).toHaveCount(1)
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})

test('desktop waiting room uses a compact two-column layout inside the battlefield', async ({ browser }) => {
  const room = await setupFastWaitingRoom(browser, {
    roomName: `桌面等待層 ${Date.now()}`,
  })
  const { page } = room

  try {
    await page.setViewportSize({ width: 1280, height: 720 })
    const waitingOverlay = page.locator('.waiting-overlay')
    const waitingPanel = page.locator('.waiting-panel')
    const waitingMain = page.locator('.waiting-main')
    const waitingSide = page.locator('.waiting-side')
    const startButton = page.getByRole('button', { name: '開始遊戲 →' })
    await expect(waitingOverlay).toHaveCSS('overflow-y', 'auto')
    await startButton.scrollIntoViewIfNeeded()

    const [panelBox, mainBox, sideBox, buttonBox] = await Promise.all([
      waitingPanel.boundingBox(),
      waitingMain.boundingBox(),
      waitingSide.boundingBox(),
      startButton.boundingBox(),
    ])
    expect(panelBox).not.toBeNull()
    expect(mainBox).not.toBeNull()
    expect(sideBox).not.toBeNull()
    expect(buttonBox).not.toBeNull()
    expect(panelBox!.width).toBeGreaterThanOrEqual(800)
    expect(sideBox!.x).toBeGreaterThanOrEqual(mainBox!.x + mainBox!.width)
    expect(buttonBox!.x).toBeGreaterThanOrEqual(sideBox!.x)
    expect(buttonBox!.y).toBeGreaterThanOrEqual(0)
    expect(buttonBox!.y + buttonBox!.height).toBeLessThanOrEqual(720)

    await page.setViewportSize({ width: 390, height: 844 })
    const [mobilePanelBox, mobileMainBox, mobileSideBox] = await Promise.all([
      waitingPanel.boundingBox(),
      waitingMain.boundingBox(),
      waitingSide.boundingBox(),
    ])
    expect(mobilePanelBox).not.toBeNull()
    expect(mobileMainBox).not.toBeNull()
    expect(mobileSideBox).not.toBeNull()
    expect(mobilePanelBox!.width).toBeLessThanOrEqual(390 - 32)
    expect(mobileSideBox!.y).toBeGreaterThanOrEqual(mobileMainBox!.y + mobileMainBox!.height)
  } finally {
    await room.close()
  }
})
