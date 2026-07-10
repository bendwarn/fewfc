import { expect, test as base, type Page } from '@playwright/test'

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

export { expect }

export const test = base.extend({
  page: async ({ page }, use) => {
    await loginAsGuest(page)
    await use(page)
  },
})
