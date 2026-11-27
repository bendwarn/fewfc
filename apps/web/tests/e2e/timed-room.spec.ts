import { expect, test } from '@playwright/test'
import type { GameRoomResponse } from '../../shared/game-room'
import { gotoAppRoute, reloadAppRoute, signInAnonymously } from './fixtures'
import { waitForGameCommand } from './realtime-helpers'

test('限時房在重連後沿用原期限，伺服器會替斷線的逾時隊伍結算', async ({ browser }) => {
  // 此案例經過最短可選的 30 秒實際期限，還包含建立房間與兩個工作階段。
  test.setTimeout(60_000)
  const [hostContext, guestContext] = await Promise.all([browser.newContext(), browser.newContext()])
  try {
    await Promise.all([signInAnonymously(hostContext, '房主'), signInAnonymously(guestContext, '訪客')])
    const host = await hostContext.newPage()
    await gotoAppRoute(host, '/rooms')
    await host.getByRole('button', { name: '建立房間', exact: true }).click()
    const dialog = host.getByRole('dialog', { name: '建立房間' })
    await dialog.getByRole('button', { name: '限時模式' }).click()
    await dialog.getByLabel('操作時限').selectOption('30')
    await dialog.getByRole('button', { name: '建立房間 →' }).click()
    await expect(host).toHaveURL(/\/rooms\/[a-f0-9]+$/)
    const gameId = new URL(host.url()).pathname.split('/').at(-1)!
    const endpoint = `/api/games/${gameId}`

    const joined = await guestContext.request.post(`${endpoint}/join`, { data: {} })
    expect(joined.ok()).toBe(true)
    const waiting = await joined.json() as GameRoomResponse
    expect(waiting.metadata.operationTimeLimitSeconds).toBe(30)
    const guestPlayer = waiting.metadata.members.find(member => !member.owner)!.player
    const guest = await guestContext.newPage()
    await gotoAppRoute(guest, `/rooms/${gameId}`)
    await guest.getByRole('button', { name: '準備 →' }).click()
    const start = host.getByRole('button', { name: '開始遊戲 →' })
    await expect(start).toBeEnabled()
    await start.click()

    await expect(host.getByRole('timer', { name: '剩餘操作時間' })).toBeVisible()
    const pouch = waitForGameCommand(host, action => action.type === 'chooseInitialPouch')
    await host.getByRole('dialog', { name: '選擇初始錦囊' }).getByRole('button').first().click()
    const chosen = await pouch
    expect(chosen.ok).toBe(true)
    expect(chosen.data?.operationClock?.players).toEqual([guestPlayer])
    const deadline = chosen.data!.operationClock!.deadlineMs

    await guest.close()
    await reloadAppRoute(host)
    await expect(host.getByRole('timer', { name: '剩餘操作時間' })).toBeVisible()
    const reloaded = await (await hostContext.request.get(endpoint)).json() as GameRoomResponse
    expect(reloaded.operationClock?.deadlineMs).toBe(deadline)

    const conclusion = host.getByRole('region', { name: '對局結算' })
    await expect(conclusion.getByText(/操作逾時/)).toBeVisible({ timeout: 35_000 })
    const finished = await (await hostContext.request.get(endpoint)).json() as GameRoomResponse
    const losingTeam = finished.state.players.find(player => player.id === guestPlayer)!.team
    expect(finished.state.gameConclusion).toMatchObject({
      outcome: { type: 'winner' },
      causes: [{ type: 'roomTimeExpired', teams: [losingTeam] }],
    })
  } finally {
    await Promise.all([hostContext.close(), guestContext.close()])
  }
})
