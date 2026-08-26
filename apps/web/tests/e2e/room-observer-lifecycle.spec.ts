import { expect, test, type APIResponse, type BrowserContext } from '@playwright/test'
import type { GameRoomResponse } from '../../shared/game-room'
import {
  gotoAppRoute,
  seedDevelopmentScenario,
  setupFastTwoPlayerGame,
  signInAnonymously,
} from './fixtures'

async function responseJson<T>(responsePromise: Promise<APIResponse>): Promise<T> {
  const response = await responsePromise
  const body = await response.text()
  expect(response.ok(), `${response.status()}: ${body}`).toBe(true)
  return JSON.parse(body) as T
}

function roomState(context: BrowserContext, gameId: string) {
  return responseJson<GameRoomResponse>(context.request.get(`/api/games/${gameId}`))
}

async function joinObserver(context: BrowserContext, gameId: string): Promise<string> {
  const response = await responseJson<GameRoomResponse>(context.request.post(
    `/api/games/${gameId}/join`, { data: {} },
  ))
  const observer = response.metadata.observers.at(-1)
  expect(observer).toBeDefined()
  return observer!.userId
}

test('back navigation preserves the queue across tabs and reconnect fills an existing vacancy', async ({ browser }) => {
  const game = await setupFastTwoPlayerGame(browser, { activeMatch: false })
  const observers = await Promise.all(Array.from({ length: 3 }, () => browser.newContext()))
  const [first, second, third] = observers as [BrowserContext, BrowserContext, BrowserContext]
  const { gameId, hostContext, guestContext } = game

  try {
    await Promise.all(observers.map((context, index) => signInAnonymously(context, `queue ${index}`)))
    const waitingEventIds = (await roomState(hostContext, gameId)).battleRecord.preparation.entries.map(entry => entry.id)
    const firstId = await joinObserver(first, gameId)
    const secondId = await joinObserver(second, gameId)
    const firstPage = await first.newPage()
    const otherTab = await first.newPage()
    const secondPage = await second.newPage()
    for (const page of [firstPage, otherTab, secondPage]) {
      await gotoAppRoute(page, `/rooms/${gameId}`)
      await expect(page.getByRole('button', { name: '離開觀戰', exact: true })).toBeVisible()
    }
    await expect.poll(async () => (
      (await roomState(hostContext, gameId)).metadata.observers.map(observer => observer.connected)
    )).toEqual([true, true])

    // 同一帳號仍有另一分頁在線時，返回大廳不會使其離線或退出候補。
    await firstPage.getByRole('button', { name: '返回房間列表', exact: true }).click()
    await expect(firstPage).toHaveURL(/\/rooms$/)
    const rooms = await responseJson<{ myRooms: Array<{ gameId: string }> }>(first.request.get('/api/games'))
    expect(rooms.myRooms.map(room => room.gameId)).toContain(gameId)
    expect((await roomState(hostContext, gameId)).metadata.observers).toMatchObject([
      { userId: firstId, connected: true }, { userId: secondId, connected: true },
    ])

    await otherTab.getByRole('button', { name: '返回房間列表', exact: true }).click()
    await secondPage.getByRole('button', { name: '返回房間列表', exact: true }).click()
    await expect.poll(async () => (
      (await roomState(hostContext, gameId)).metadata.observers.map(observer => observer.connected)
    )).toEqual([false, false])
    expect((await roomState(hostContext, gameId)).battleRecord.preparation.entries.map(entry => entry.id))
      .toEqual(waitingEventIds)

    // 沒有在線候補時保留空席；最先重新連線的既有候補會自動補入，不需再次離房。
    const vacancy = await responseJson<GameRoomResponse>(guestContext.request.post(
      `/api/games/${gameId}/leave`, { data: {} },
    ))
    expect(vacancy.metadata.members).toHaveLength(1)
    expect(vacancy.metadata.observers.map(observer => observer.userId)).toEqual([firstId, secondId])
    await gotoAppRoute(firstPage, `/rooms/${gameId}`)
    await expect(firstPage.getByRole('button', { name: '準備 →', exact: true })).toBeVisible()
    await expect(firstPage.getByText('已補為玩家，請準備', { exact: true })).toBeVisible()
    const promoted = await roomState(hostContext, gameId)
    expect(promoted.metadata.members.find(member => member.userId === firstId)).toMatchObject({ ready: false })
    expect(promoted.metadata.observers.map(observer => observer.userId)).toEqual([secondId])
    expect(promoted.battleRecord.preparation.entries.map(entry => entry.id))
      .toEqual(vacancy.battleRecord.preparation.entries.map(entry => entry.id))

    // 明確離開才移除 membership；再次加入排在仍留房的候補之後。
    await gotoAppRoute(secondPage, `/rooms/${gameId}`)
    await expect(secondPage.getByRole('button', { name: '離開觀戰', exact: true })).toBeVisible()
    const thirdId = await joinObserver(third, gameId)
    await secondPage.getByRole('button', { name: '離開觀戰', exact: true }).click()
    await expect(secondPage).toHaveURL(/\/rooms$/)
    const departedRooms = await responseJson<{ myRooms: Array<{ gameId: string }> }>(second.request.get('/api/games'))
    expect(departedRooms.myRooms.map(room => room.gameId)).not.toContain(gameId)
    expect(await joinObserver(second, gameId)).toBe(secondId)
    expect((await roomState(hostContext, gameId)).metadata.observers.map(observer => observer.userId))
      .toEqual([thirdId, secondId])
    const removed = await responseJson<GameRoomResponse>(hostContext.request.post(
      `/api/games/${gameId}/remove`, { data: { userId: thirdId } },
    ))
    expect(removed.metadata.observers.map(observer => observer.userId)).toEqual([secondId])
    expect(removed.battleRecord.preparation.entries.map(entry => entry.id))
      .toEqual(promoted.battleRecord.preparation.entries.map(entry => entry.id))
  } finally {
    await Promise.allSettled([game.close(), ...observers.map(context => context.close())])
  }
})

test('finished rooms retain observers through reset without granting player replay rights', async ({ browser }) => {
  const game = await setupFastTwoPlayerGame(browser)
  const observers = await Promise.all(Array.from({ length: 3 }, () => browser.newContext()))
  const [observer, offlineObserver, newcomer] = observers as [BrowserContext, BrowserContext, BrowserContext]
  const { gameId, host, guest, hostContext, guestContext } = game

  try {
    await Promise.all(observers.map((context, index) => signInAnonymously(context, `finished ${index}`)))
    const beforeObservation = await roomState(hostContext, gameId)
    expect(beforeObservation.activeGameVersion).toBeDefined()
    const observerId = await joinObserver(observer, gameId)
    const offlineId = await joinObserver(offlineObserver, gameId)
    expect((await roomState(hostContext, gameId)).activeGameVersion).toEqual(beforeObservation.activeGameVersion)
    const observerPage = await observer.newPage()
    await gotoAppRoute(observerPage, `/rooms/${gameId}`)
    await expect(observerPage.getByRole('button', { name: '離開觀戰', exact: true })).toBeVisible()
    await expect.poll(async () => (
      (await roomState(hostContext, gameId)).metadata.observers.find(member => member.userId === observerId)?.connected
    )).toBe(true)
    await observerPage.getByRole('button', { name: '返回房間列表', exact: true }).click()
    await expect.poll(async () => (
      (await roomState(hostContext, gameId)).metadata.observers.find(member => member.userId === observerId)?.connected
    )).toBe(false)
    expect((await roomState(hostContext, gameId)).activeGameVersion).toEqual(beforeObservation.activeGameVersion)
    await gotoAppRoute(observerPage, `/rooms/${gameId}`)
    await expect.poll(async () => (
      (await roomState(hostContext, gameId)).metadata.observers.find(member => member.userId === observerId)?.connected
    )).toBe(true)
    expect((await roomState(hostContext, gameId)).activeGameVersion).toEqual(beforeObservation.activeGameVersion)
    const originalPlayers = (await roomState(hostContext, gameId)).metadata.members.map(member => member.userId)

    // 最後一次成員變更刻意是觀戰離開，證明加入與離開都不會阻斷之後的玩家命令。
    await joinObserver(newcomer, gameId)
    await responseJson(newcomer.request.post(`/api/games/${gameId}/leave`, { data: {} }))
    expect((await roomState(hostContext, gameId)).activeGameVersion).toEqual(beforeObservation.activeGameVersion)

    await seedDevelopmentScenario(host, { name: 'star-endgame' })
    await Promise.all([host, guest].map(page => (
      expect(page.locator('.player-identity')).toContainText(['1 HP', '1 HP'])
    )))
    const active = await host.locator('.playing-card:enabled:not(.hidden)').count() ? host : guest
    await active.locator('.playing-card:enabled:not(.hidden)').first().click()
    const attack = active.locator('.action-panel .action-candidates button[title*="進行五行攻擊"]')
    await expect(attack).toBeVisible()
    const finishedCommand = active.waitForResponse(response => (
      response.request().method() === 'POST'
      && new URL(response.url()).pathname === `/api/games/${gameId}/commands`
      && response.request().postDataJSON()?.action?.type === 'performFormation'
    ))
    await attack.click()
    const finishedResponse = await finishedCommand
    expect(finishedResponse.ok(), `${finishedResponse.status()}: ${await finishedResponse.text()}`).toBe(true)
    await expect(observerPage.locator('.battlefield .result-reason')).toContainText('生命值歸零')
    await expect(observerPage.locator('.result-panel').getByRole('button', { name: '離開觀戰', exact: true }))
      .toBeVisible()
    expect((await newcomer.request.post(`/api/games/${gameId}/join`, { data: {} })).status()).toBe(409)
    expect((await observer.request.post(`/api/games/${gameId}/reset`, { data: {} })).status()).toBe(403)

    const reset = await responseJson<GameRoomResponse>(guestContext.request.post(
      `/api/games/${gameId}/reset`, { data: {} },
    ))
    expect(reset.metadata.status).toBe('Waiting')
    expect(reset.metadata.members.map(member => member.userId)).toEqual(originalPlayers)
    expect(reset.metadata.members.every(member => !member.ready)).toBe(true)
    expect(reset.metadata.observers.map(member => member.userId)).toEqual([observerId, offlineId])
    await expect(observerPage.getByRole('button', { name: '離開觀戰', exact: true })).toBeVisible()
    await expect(observerPage.getByRole('heading', { name: '觀戰者（依候補順序）' })).toBeVisible()
    expect((await roomState(observer, gameId)).savableReplay).toBeUndefined()
    expect((await observer.request.post('/api/replays', { data: { sourceGameId: gameId } })).status()).toBe(403)
    await responseJson(guestContext.request.post('/api/replays', { data: { sourceGameId: gameId } }))
  } finally {
    await Promise.allSettled([game.close(), ...observers.map(context => context.close())])
  }
})
