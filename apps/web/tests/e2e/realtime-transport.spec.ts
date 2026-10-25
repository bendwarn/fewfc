import { test, expect, setupFastTwoPlayerGame, seedDevelopmentScenario, signInAnonymously, gotoAppRoute, createPublicRoom } from './fixtures'
import { roomStateFrames, waitForGameCommand } from './realtime-helpers'

test('lobby creation, room entry and configuration work with business HTTP blocked', async ({ browser }) => {
  const context = await browser.newContext()
  try {
    await signInAnonymously(context, 'WS lobby')
    const page = await context.newPage()
    const forbidden: string[] = []
    await page.route(/\/api\/games(?:\/.*)?(?:\?.*)?$/, route => {
      forbidden.push(route.request().url())
      return route.abort()
    })
    await gotoAppRoute(page, '/rooms')
    await createPublicRoom(page, 'WS 客棧')
    await expect(page.locator('.waiting-overlay')).toBeVisible()
    await expect(page.locator('.connection-dot.connected')).toHaveCount(1)
    expect(forbidden).toEqual([])
  } finally { await context.close() }
})

test('WS play immediately accepts discard and synchronizes the opponent; reconnect restores state', async ({ browser }) => {
  const game = await setupFastTwoPlayerGame(browser, { roomName: 'WS 棄牌驗證', enabledRuleModules: ['hero-schools'], activeMatch: false })
  try {
    const forbidden: string[] = []
    for (const page of game.pages) {
      await page.route(/\/api\/games(?:\/.*)?$/, route => { forbidden.push(route.request().url()); return route.abort() })
    }
    await game.guest.getByRole('button', { name: '準備 →', exact: true }).click()
    await expect(game.host.getByRole('button', { name: '開始遊戲 →', exact: true })).toBeEnabled()
    await game.host.getByRole('button', { name: '開始遊戲 →', exact: true }).click()
    await expect(game.host.locator('.waiting-overlay')).toBeHidden()
    await seedDevelopmentScenario(game.host, { name: 'hero-schools-transition' })
    const hand = game.host.locator('.seat-bottom .playing-card:enabled:not(.hidden)')
    await expect(hand).toHaveCount(5)
    await hand.first().click()
    const formation = game.host.getByRole('region', { name: '行動' }).locator('.action-candidates button:not(.skip-action)').first()
    const played = waitForGameCommand(game.host, action => ['performFormation', 'changeProfession'].includes(action.type))
    await formation.click()
    const play = await played
    expect(play.ok).toBe(true)
    expect(play.data?.state.pendingChoice?.reason.type).toBe('turnDrawDiscard')
    expect(play.data).not.toHaveProperty('receipt')
    const choice = game.host.getByRole('dialog', { name: '選擇一張本回合抽到的牌捨棄' })
    const card = choice.locator('.playing-card').first()
    await expect(card).toBeEnabled()
    const answered = waitForGameCommand(game.host, action => action.type === 'answerChoice')
    await card.click()
    const answer = await answered
    expect(answer.ok).toBe(true)
    expect(answer.data?.state.currentPlayer).not.toBe(play.data?.state.currentPlayer)
    await expect(choice).toBeHidden()
    await expect(game.guest.locator('.seat-bottom .playing-card:enabled:not(.hidden)').first()).toBeEnabled()
    // 同一出牌的完整狀態只在 result 回到發起端；對手仍會收到 roomState。
    const playedVersion = JSON.stringify(play.data?.activeGameVersion)
    expect(play.data?.activeGameVersion).toBeDefined()
    expect(roomStateFrames(game.host).some(state => JSON.stringify(state.activeGameVersion) === playedVersion)).toBe(false)
    expect(roomStateFrames(game.guest).some(state => JSON.stringify(state.activeGameVersion) === playedVersion)).toBe(true)
    // 用同一個 commandId 重送已完成的出牌，必須回覆最新狀態而不重複執行。
    const retried = await game.host.evaluate(async ({ gameId, input }) => {
      const socket = new WebSocket(`${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}/api/games/${gameId}/socket`)
      return await new Promise<{ ok: boolean; data: { activeGameVersion: unknown } }>((resolve, reject) => {
        const requestId = crypto.randomUUID()
        const timer = setTimeout(() => { socket.close(); reject(new Error('WS retry timeout')) }, 10_000)
        socket.onopen = () => socket.send(JSON.stringify({ type: 'request', requestId, operation: 'command', input }))
        socket.onmessage = ({ data }) => {
          const result = JSON.parse(data)
          if (result.type === 'result' && result.requestId === requestId) {
            clearTimeout(timer)
            socket.close()
            resolve(result)
          }
        }
      })
    }, { gameId: game.gameId, input: play.input })
    expect(retried.ok).toBe(true)
    expect(retried.data.activeGameVersion).toEqual(answer.data?.activeGameVersion)
    await game.host.reload({ waitUntil: 'domcontentloaded' })
    await expect(game.host.locator('.connection-dot.connected')).toHaveCount(2)
    await expect(choice).toBeHidden()
    expect(forbidden).toEqual([])
  } finally { await game.close() }
})

test('WS rejects client identities and revalidates a session after logout', async ({ browser }) => {
  const context = await browser.newContext()
  try {
    await signInAnonymously(context, 'WS session')
    const page = await context.newPage()
    await gotoAppRoute(page, '/rooms')
    await page.evaluate(async () => {
      const socket = new WebSocket(`${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}/api/notifications/socket`)
      await new Promise<void>((resolve, reject) => {
        socket.onopen = () => resolve()
        socket.onerror = () => reject(new Error('WS connection failed'))
      })
      Object.assign(window, { realtimeTestSocket: socket })
    })
    const request = (input?: unknown) => page.evaluate(async (input) => {
      const socket = (window as unknown as { realtimeTestSocket: WebSocket }).realtimeTestSocket
      const requestId = crypto.randomUUID()
      return await new Promise<{ requestId: string; ok: boolean; error?: { statusCode: number; code: string } }>((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error('WS response timeout')), 10_000)
        const receive = ({ data }: MessageEvent) => {
          const message = JSON.parse(data)
          if (message.type !== 'result' || message.requestId !== requestId) return
          clearTimeout(timer)
          socket.removeEventListener('message', receive)
          resolve(message)
        }
        socket.addEventListener('message', receive)
        socket.send(JSON.stringify({ type: 'request', requestId, operation: 'list', input }))
      })
    }, input)
    expect(await request({ actorUserId: 'another-user' })).toMatchObject({
      ok: false, error: { statusCode: 400, code: 'invalidRequest' },
    })
    expect(await request()).toMatchObject({ ok: true })
    const logout = await page.evaluate(async () => {
      const response = await fetch('/api/auth/sign-out', {
        method: 'POST', headers: { 'content-type': 'application/json' }, body: '{}',
      })
      return { ok: response.ok, text: await response.text() }
    })
    expect(logout.ok, logout.text).toBe(true)
    expect(await request()).toMatchObject({
      ok: false, error: { statusCode: 401, code: 'sessionExpired' },
    })
  } finally { await context.close() }
})
