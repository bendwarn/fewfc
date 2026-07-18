import type { Page } from '@playwright/test'
import {
  createPublicRoom,
  createPublicRoomViaApi,
  expect,
  joinListedRoom,
  loginAsGuests,
  seedDevelopmentScenario,
  startTwoPlayerMatch,
  test,
} from './fixtures'

async function indexedModules(page: Page, roomName: string): Promise<string[]> {
  return await page.evaluate(async (name) => {
    const response = await fetch('/api/games')
    const result = await response.json() as {
      myRooms: Array<{ name: string; enabledRuleModules: string[] }>
    }
    return result.myRooms.find(room => room.name === name)?.enabledRuleModules ?? []
  }, roomName)
}

test('Star defaults on, survives reconnect, and is immutable after a two-player start', async ({ browser }) => {

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()

  try {
    await loginAsGuests([host, guest])
    const roomName = `星辰預設測試 ${Date.now()}`
    await createPublicRoom(host, roomName)

    await expect(host.getByRole('heading', { name: '選用規則' })).toBeVisible()
    await expect(host.getByRole('heading', { name: '進階規則' })).toBeVisible()
    await expect(host.getByRole('heading', { name: '主題規則' })).toBeVisible()
    await expect(host.getByLabel('星辰圖記')).toBeChecked()
    await expect(host.getByLabel('星辰圖記')).toBeEnabled()
    expect(await indexedModules(host, roomName)).toContain('star')

    await joinListedRoom(guest, roomName, '停用：錦囊')
    await expect(guest.getByLabel('星辰圖記')).toBeChecked()
    await expect(guest.getByLabel('星辰圖記')).toBeDisabled()
    await guest.getByRole('button', { name: '準備 →' }).click()
    await host.getByRole('button', { name: '開始遊戲 →' }).click()

    await Promise.all([host, guest].map(async (page) => {
      const rules = page.getByRole('region', { name: '啟用規則' })
      await expect(rules).toContainText('基礎規則')
      await expect(rules).toContainText('星辰圖記')
      await expect(page.locator('.player-identity').filter({ hasText: '召星' })).toHaveCount(0)
    }))

    const roomId = new URL(host.url()).pathname.split('/').pop()
    const updateStatus = await host.evaluate(async (id) => {
      const response = await fetch(`/api/games/${id}/rules`, {
        method: 'PUT',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ enabledRuleModules: [] }),
      })
      return response.status
    }, roomId)
    expect(updateStatus).toBe(409)

    await guest.reload()
    await expect(guest.getByRole('region', { name: '啟用規則' }))
      .toContainText('星辰圖記')
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})

test('disabling Star invalidates readiness and locked decks while preserving Base play', async ({ browser }) => {

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()

  try {
    await loginAsGuests([host, guest])
    const roomName = `星辰關閉測試 ${Date.now()}`
    await createPublicRoom(host, roomName)
    await joinListedRoom(guest, roomName, '停用：錦囊')

    await guest.getByRole('button', { name: '準備 →' }).click()
    await expect(guest.getByText('本局使用：五行均衡預組')).toBeVisible()
    await host.getByLabel('星辰圖記').uncheck()

    await expect(guest.getByLabel('星辰圖記')).not.toBeChecked()
    await expect(guest.getByRole('button', { name: '準備 →' })).toBeVisible()
    await expect(guest.getByText('本局使用：五行均衡預組')).toHaveCount(0)
    expect(await indexedModules(host, roomName)).not.toContain('star')

    await guest.reload()
    await expect(guest.getByLabel('星辰圖記')).not.toBeChecked()
    await guest.getByRole('button', { name: '準備 →' }).click()
    await host.getByRole('button', { name: '開始遊戲 →' }).click()

    await Promise.all([host, guest].map(async (page) => {
      await expect(page.getByRole('region', { name: '啟用規則' }))
        .not.toContainText('星辰圖記')
      await expect(page.locator('.player-identity').filter({ hasText: '召星' })).toHaveCount(0)
    }))

    const active = (await host.locator('.playing-card:enabled:not(.hidden)').count()) ? host : guest
    await active.locator('.playing-card:enabled:not(.hidden)').first().click()
    await expect(active.locator('.action-panel .action-candidates button:not(.skip-action)').first())
      .toBeVisible()

    const roomId = new URL(host.url()).pathname.split('/').pop()
    const publicState = await host.evaluate(async (id) => {
      const response = await fetch(`/api/games/${id}`)
      return (await response.json()).state as {
        enabledRuleModules: string[]
        teamStars: unknown[]
        starHistories: unknown[]
      }
    }, roomId)
    expect(publicState.enabledRuleModules).not.toContain('star')
    expect(publicState.teamStars).toEqual([])
    expect(publicState.starHistories).toEqual([])
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})

test('a four-player team room starts with one shared immutable Star configuration', async ({ browser }) => {

  const contexts = await Promise.all(Array.from({ length: 4 }, () => browser.newContext()))
  const pages = await Promise.all(contexts.map(context => context.newPage()))
  const [host, ...guests] = pages

  try {
    await loginAsGuests(pages)
    const roomName = `星辰團隊測試 ${Date.now()}`
    await createPublicRoomViaApi(host!, roomName, true)

    for (const guest of guests) {
      await joinListedRoom(guest, roomName, '停用：錦囊')
      await expect(guest.getByLabel('星辰圖記')).toBeChecked()
      await guest.getByRole('button', { name: '準備 →' }).click()
    }

    const start = host!.getByRole('button', { name: '開始遊戲 →' })
    await expect(start).toBeEnabled()
    await start.click()

    await Promise.all(pages.map(async (page) => {
      await expect(page.getByRole('region', { name: '啟用規則' }))
        .toContainText('星辰圖記')
      await expect(page.locator('.player-seat')).toHaveCount(4)
      await expect(page.getByLabel('棄牌堆').locator('.discard-pile')).toHaveCount(4)
      await expect(page.locator('.discard-position-top')).toHaveCount(1)
      await expect(page.locator('.discard-position-left')).toHaveCount(1)
      await expect(page.locator('.discard-position-right')).toHaveCount(1)
      await expect(page.locator('.discard-position-bottom')).toHaveCount(1)
      await expect(page.locator('.player-identity').filter({ hasText: '召星' })).toHaveCount(0)
    }))
  } finally {
    await Promise.all(contexts.map(context => context.close()))
  }
})

test('a Star endgame fixture finishes through normal UI play and resets with its rules', async ({ browser }) => {

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()
  const pages = [host, guest]

  try {
    await loginAsGuests(pages)
    const roomName = `星辰殘局測試 ${Date.now()}`
    const roomId = await startTwoPlayerMatch(host, guest, roomName)
    await seedDevelopmentScenario(host, { name: 'star-endgame' })

    await Promise.all(pages.map(page => (
      expect(page.locator('.player-identity')).toContainText(['1 HP', '1 HP'])
    )))
    const active = (await host.locator('.playing-card:enabled:not(.hidden)').count()) ? host : guest
    await active.locator('.playing-card:enabled:not(.hidden)').first().click()
    const attack = active.locator('.action-panel .action-candidates button:not(.skip-action)').first()
    await expect(attack).toBeVisible()
    await attack.click()

    await Promise.all(pages.map(async (page) => {
      await expect(page.locator('.result-panel')).toBeVisible()
      await expect(page.getByRole('region', { name: '啟用規則' }))
        .toContainText('星辰圖記')
    }))

    await host.getByRole('button', { name: '返回房間 →' }).click()
    await Promise.all(pages.map(async (page) => {
      await expect(page.getByLabel('星辰圖記')).toBeChecked()
      await expect(page.getByRole('heading', { name: '進階規則' })).toBeVisible()
    }))
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})
