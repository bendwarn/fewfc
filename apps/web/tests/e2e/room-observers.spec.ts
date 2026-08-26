import { expect, test, type BrowserContext, type Page } from '@playwright/test'
import { gotoAppRoute, signInAnonymously } from './fixtures'

async function json(context: BrowserContext, path: string, data?: unknown) {
  const response = await context.request.fetch(path, {
    method: data === undefined ? 'GET' : 'POST',
    data,
  })
  return { response, body: await response.json() as Record<string, unknown> }
}

async function roomPage(context: BrowserContext, gameId: string): Promise<Page> {
  const page = await context.newPage()
  await gotoAppRoute(page, `/rooms/${gameId}`)
  return page
}

function initialPouchCommit(page: Page) {
  return page.waitForResponse(response => (
    response.request().method() === 'POST'
    && new URL(response.url()).pathname.endsWith('/commands')
    && response.request().postData()?.includes('chooseInitialPouch') === true
  ))
}

test('full and active rooms admit observers, skip offline FIFO entries, and promote a connected observer', async ({ browser }) => {
  test.setTimeout(120_000)
  const contexts = await Promise.all(Array.from({ length: 5 }, () => browser.newContext()))
  const [host, guest, offlineObserver, connectedObserver, activeObserver] = contexts
  try {
    await Promise.all(contexts.map((context, index) => signInAnonymously(context, `observer flow ${index}`)))
    const created = await json(host!, '/api/games', {
      name: `觀戰流程 ${Date.now()}`, access: 'public', capacity: 2,
    })
    expect(created.response.ok()).toBe(true)
    const gameId = String(created.body.gameId)
    expect((await json(guest!, `/api/games/${gameId}/join`, {})).response.ok()).toBe(true)
    const offlineJoined = await json(offlineObserver!, `/api/games/${gameId}/join`, {})
    expect(offlineJoined.response.ok()).toBe(true)
    const offlineObserverId = String((offlineJoined.body.metadata as { observers: Array<{ userId: string }> }).observers[0]?.userId)
    const connectedJoined = await json(connectedObserver!, `/api/games/${gameId}/join`, {})
    expect(connectedJoined.response.ok()).toBe(true)
    const connectedObserverId = String((connectedJoined.body.metadata as { observers: Array<{ userId: string }> }).observers[1]?.userId)

    const [hostPage, guestPage, observerPage] = await Promise.all([
      roomPage(host!, gameId), roomPage(guest!, gameId), roomPage(connectedObserver!, gameId),
    ])
    await expect(observerPage.getByRole('button', { name: '離開觀戰' })).toBeVisible()
    await expect(observerPage.getByText('觀戰者（依候補順序）')).toBeVisible()

    // 第一位候補離線時會被略過，下一位在線觀戰者取得空席。
    expect((await json(guest!, `/api/games/${gameId}/leave`, {})).response.ok()).toBe(true)
    await expect(observerPage.getByRole('button', { name: '準備 →' })).toBeVisible()
    await expect(observerPage.getByText('已補為玩家，請準備')).toBeVisible()
    const promoted = await json(connectedObserver!, `/api/games/${gameId}`)
    const metadata = promoted.body.metadata as { members: Array<{ userId: string, ready: boolean }>, observers: unknown[] }
    expect(metadata.members).toHaveLength(2)
    expect(metadata.members.find(member => member.userId === connectedObserverId)?.ready).toBe(false)
    expect(metadata.observers).toHaveLength(1)
    expect((metadata.observers as Array<{ userId: string }>)[0]?.userId).toBe(offlineObserverId)

    // 補位者以自己的牌組重新準備，並可正常開始此局。
    expect((await json(connectedObserver!, `/api/games/${gameId}/ready`, {})).response.ok()).toBe(true)
    expect((await json(host!, `/api/games/${gameId}/start`, {})).response.ok()).toBe(true)
    // 兩位玩家皆完成自己的初始錦囊，才會進入發牌後的私有手牌投影。
    const observerPouch = initialPouchCommit(observerPage)
    await observerPage.getByRole('dialog', { name: '選擇初始錦囊' }).getByRole('button').first().click()
    expect((await observerPouch).ok()).toBe(true)
    const hostPouch = initialPouchCommit(hostPage)
    await hostPage.getByRole('dialog', { name: '選擇初始錦囊' }).getByRole('button').first().click()
    expect((await hostPouch).ok()).toBe(true)
    await expect(observerPage.getByRole('region', { name: '五行戰鬥牌對戰桌' })).toBeVisible()
    const promotedState = await json(connectedObserver!, `/api/games/${gameId}`)
    const promotedPlayer = (promotedState.body.metadata as { members: Array<{ userId: string, player: string }> })
      .members.find(member => member.userId === connectedObserverId)?.player
    const ownHand = (promotedState.body.state as { hands: Array<{ player: string, cards: { kind: string } }> })
      .hands.find(hand => hand.player === promotedPlayer)
    expect(ownHand?.cards.kind).toBe('known')

    // 已開局仍可加入，但只能取得 Observer 投影與離開觀戰操作。
    const activeJoined = await json(activeObserver!, `/api/games/${gameId}/join`, {})
    expect(activeJoined.response.ok()).toBe(true)
    const activeObserverId = String((activeJoined.body.metadata as { observers: Array<{ userId: string }> }).observers.at(-1)?.userId)
    const observerState = activeJoined.body.state as {
      hands: Array<{ cards: { kind: string } }>
      playerDecks: Array<{ cards: { kind: string } }>
    }
    expect(observerState.hands.every(hand => hand.cards.kind === 'hidden')).toBe(true)
    expect(observerState.playerDecks.every(deck => deck.cards.kind === 'hidden')).toBe(true)
    const activeObserverPage = await roomPage(activeObserver!, gameId)
    await expect(activeObserverPage.getByRole('button', { name: '離開觀戰' })).toBeVisible()
    expect((await json(activeObserver!, `/api/games/${gameId}/ready`, {})).response.status()).toBe(403)
    expect((await json(activeObserver!, `/api/games/${gameId}/reset`, {})).response.status()).toBe(403)
    const activeVersion = activeJoined.body.activeGameVersion as { gameInstanceId: string }
    expect((await json(activeObserver!, `/api/games/${gameId}/commands`, {
      commandId: crypto.randomUUID(), gameInstanceId: activeVersion.gameInstanceId,
      transactionId: crypto.randomUUID(), action: { type: 'passAction', reason: 'NoCardsInHand' },
    })).response.status()).toBe(403)
    expect((await json(host!, `/api/games/${gameId}/remove`, { userId: activeObserverId })).response.status()).toBe(409)

    await activeObserverPage.getByRole('button', { name: '離開觀戰' }).click()
    await expect(activeObserverPage).toHaveURL(/\/rooms$/)
    await hostPage.close()
    await guestPage.close()
    await observerPage.close()
    await activeObserverPage.close()
  } finally {
    await Promise.all(contexts.map(context => context.close()))
  }
})
