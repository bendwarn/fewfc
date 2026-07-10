import { expect, test as base, type Page } from '@playwright/test'
import type { DevelopmentScenario } from '../../shared/development-scenarios'

const roomUrl = /\/rooms\/[0-9a-f-]+$/

export async function loginAsGuest(page: Page) {
  await page.goto('/login')
  await page.getByRole('button', { name: '以訪客身份遊玩' }).click()
  await expect(page).toHaveURL(/\/rooms(?:\?.*)?$/)
}

export async function loginAsGuests(pages: Page[]) {
  await Promise.all(pages.map(loginAsGuest))
}

type CreateRoomOptions = {
  access?: 'private' | 'public'
  teamMode?: boolean
}

export async function createRoom(
  page: Page,
  roomName: string,
  { access = 'private', teamMode = false }: CreateRoomOptions = {},
) {
  await page.getByRole('button', { name: '建立房間', exact: true }).click()
  await expect(page.getByRole('dialog', { name: '建立房間' })).toBeVisible()
  await page.getByLabel('房間名稱').fill(roomName)

  if (teamMode) {
    await page.getByRole('button', { name: /團隊對戰/ }).click()
  }

  if (access === 'public') {
    await page.getByRole('button', { name: '公開房間', exact: true }).click()
  }

  await page.getByRole('button', { name: '建立房間 →' }).click()
  await expect(page).toHaveURL(roomUrl)
}

export async function createPublicRoom(page: Page, roomName: string, teamMode = false) {
  await createRoom(page, roomName, { access: 'public', teamMode })
}

export async function createPublicRoomViaApi(
  page: Page,
  roomName: string,
  teamMode = false,
): Promise<string> {
  let lastFailure = 'request did not run'
  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      const response = await page.context().request.post('/api/games', {
        data: {
          name: roomName,
          access: 'public',
          capacity: teamMode ? 4 : 2,
        },
        timeout: 15_000,
      })
      if (response.ok()) {
        const body = await response.json() as { gameId: string }
        await page.goto(`/rooms/${body.gameId}`)
        await expect(page).toHaveURL(roomUrl)
        return body.gameId
      }
      lastFailure = `${response.status()} ${await response.text()}`
      if (response.status() < 500) break
    } catch (error) {
      lastFailure = error instanceof Error ? error.message : String(error)
    }
    await page.waitForTimeout(250 * (attempt + 1))
  }
  throw new Error(`Could not create E2E room through API: ${lastFailure}`)
}

export async function joinListedRoom(
  page: Page,
  roomName: string,
  expectedRuleText?: string,
) {
  const room = page.locator('.public-room-list button').filter({ hasText: roomName })
  if (expectedRuleText) {
    await expect(room).toContainText(expectedRuleText)
  }
  await room.click()
  await expect(page).toHaveURL(roomUrl)
}

export async function seedDevelopmentScenario<T = unknown>(
  page: Page,
  scenario: DevelopmentScenario,
): Promise<T> {
  return await page.evaluate(async (requestedScenario) => {
    const id = location.pathname.split('/').at(-1)
    const response = await fetch(`/api/games/${id}/test-scenarios`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(requestedScenario),
    })
    const body = await response.json() as T & {
      error?: string | boolean
      message?: string
      statusMessage?: string
    }
    if (!response.ok) {
      throw new Error(
        body.statusMessage
        ?? body.message
        ?? (typeof body.error === 'string' ? body.error : `scenario failed with ${response.status}`),
      )
    }
    return body
  }, scenario)
}

export async function startTwoPlayerMatch(
  host: Page,
  guest: Page,
  roomName: string,
  configure?: (host: Page) => Promise<void>,
): Promise<string> {
  await createPublicRoom(host, roomName)
  await configure?.(host)
  await joinListedRoom(guest, roomName)
  await guest.getByRole('button', { name: '準備 →' }).click()
  await host.getByRole('button', { name: '開始遊戲 →' }).click()
  await expect(host.getByRole('region', { name: '啟用規則' })).toBeVisible()
  await expect(guest.getByRole('region', { name: '啟用規則' })).toBeVisible()
  return new URL(host.url()).pathname.split('/').at(-1) ?? ''
}

export { expect }

export const test = base.extend({
  page: async ({ page }, use) => {
    await loginAsGuest(page)
    await use(page)
  },
})
