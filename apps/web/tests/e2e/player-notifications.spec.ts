import { expect, test } from '@playwright/test'
import { playerNotificationStorageKey } from '../../app/lib/player-notifications'
import { setupFastTwoPlayerGame, setupFastWaitingRoom, signInAnonymously } from './fixtures'

test('retains a room dot after the prompt expires and reloads, consuming it only after successful entry', async ({ browser }) => {
  const roomName = `通知保存 ${Date.now()}`
  const room = await setupFastWaitingRoom(browser, { roomName, enabledRuleModules: [] })
  const visitor = await browser.newContext()
  const { page, gameId } = room

  try {
    await page.getByRole('button', { name: '回到首頁' }).click()
    await expect(page.getByRole('heading', { name: '房間', exact: true })).toBeVisible()
    await expect(page.locator('.connection')).toHaveText('已連線')
    await page.clock.install()

    await signInAnonymously(visitor, 'notification visitor')
    const joined = await visitor.request.post(`/api/games/${gameId}/join`, { data: {} })
    expect(joined.ok()).toBe(true)

    const prompt = page.getByRole('complementary', { name: '玩家通知' })
    const roomButton = page.locator('.my-rooms-card').getByRole('button').filter({ hasText: roomName })
    const dot = roomButton.getByRole('status', { name: '有房間通知' })
    await expect(prompt).toContainText(roomName)
    await expect(dot).toBeVisible()
    await page.clock.runFor(5000)
    await expect(prompt).toBeHidden()
    await expect(dot).toBeVisible()

    await page.reload({ waitUntil: 'domcontentloaded' })
    await expect(dot).toBeVisible()
    await expect(prompt).toBeHidden()

    // 房間載入與補做加入都失敗時，不應提早消耗通知。
    const roomApi = new RegExp(`/api/games/${gameId}(?:/join)?$`)
    await page.route(roomApi, route => route.fulfill({
      status: 403,
      contentType: 'application/json',
      body: JSON.stringify({ statusMessage: 'room access denied' }),
    }))
    await roomButton.click()
    await page.getByRole('button', { name: '返回房間大廳' }).click()
    await expect(dot).toBeVisible()
    await page.unroute(roomApi)

    await roomButton.click()
    await expect(page.locator('.waiting-overlay')).toBeVisible()
    await page.getByRole('button', { name: '回到首頁' }).click()
    await expect(roomButton).toBeVisible()
    await expect(dot).toHaveCount(0)

    const left = await visitor.request.post(`/api/games/${gameId}/leave`, { data: {} })
    expect(left.ok()).toBe(true)
    await expect(prompt).toContainText(roomName)
    await prompt.getByRole('button', { name: '關閉通知' }).click()
    await expect(prompt).toBeHidden()
    await expect(dot).toBeVisible()
  } finally {
    await visitor.close()
    await room.close()
  }
})

for (const kind of ['removed', 'dissolved'] as const) {
  test(`${kind} redirects the current room while keeping its five-second reason without a room action or persistence`, async ({ browser }) => {
    const roomName = `暫時通知 ${kind} ${Date.now()}`
    const game = await setupFastTwoPlayerGame(browser, {
      roomName,
      enabledRuleModules: [],
      activeMatch: false,
    })
    const { guest: page, guestContext, hostContext, gameId } = game

    try {
      await expect(page.locator('.connection')).toHaveText('已連線')
      await page.clock.install()
      const sessionResponse = await guestContext.request.get('/api/auth/get-session')
      expect(sessionResponse.ok()).toBe(true)
      const session = await sessionResponse.json() as { user: { id: string } }
      const endpoint = kind === 'removed' ? 'remove' : 'dissolve'
      const result = await hostContext.request.post(`/api/games/${gameId}/${endpoint}`, {
        data: kind === 'removed' ? { userId: session.user.id } : {},
      })
      expect(result.ok()).toBe(true)

      await expect(page).toHaveURL(/\/rooms$/)
      const prompt = page.getByRole('complementary', { name: '玩家通知' })
      await expect(prompt).toContainText(kind === 'removed' ? `你已被移出「${roomName}」。` : `「${roomName}」已解散。`)
      await expect(prompt.getByRole('button', { name: /進入房間/ })).toHaveCount(0)
      const stored = await page.evaluate((key) => {
        const value = JSON.parse(window.localStorage.getItem(key) ?? '{}')
        return value.notifications ?? []
      }, playerNotificationStorageKey(session.user.id))
      expect(stored).toEqual([])

      await page.clock.runFor(5000)
      await expect(prompt).toBeHidden()
      await page.reload({ waitUntil: 'domcontentloaded' })
      await expect(page.getByRole('heading', { name: '房間', exact: true })).toBeVisible()
      await expect(prompt).toBeHidden()
    } finally {
      await game.close()
    }
  })
}
