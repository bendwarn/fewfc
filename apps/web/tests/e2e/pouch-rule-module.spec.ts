import { expect, test, type Page } from '@playwright/test'

async function loginAsGuest(page: Page) {
  await page.goto('/login')
  await page.getByRole('button', { name: '以訪客身份遊玩' }).click()
  await expect(page).toHaveURL(/\/rooms(?:\?.*)?$/)
}

async function createPublicRoom(host: Page, roomName: string) {
  await host.getByRole('button', { name: '建立房間', exact: true }).click()
  await host.getByLabel('房間名稱').fill(roomName)
  await host.getByRole('button', { name: '公開房間', exact: true }).click()
  await host.getByRole('button', { name: '建立房間 →' }).click()
  await expect(host).toHaveURL(/\/rooms\/[0-9a-f-]+$/)
}

async function joinListedRoom(page: Page, roomName: string) {
  const room = page.locator('.public-room-list button').filter({ hasText: roomName })
  await room.click()
  await expect(page).toHaveURL(/\/rooms\/[0-9a-f-]+$/)
}

async function waitForCommand(page: Page, actionType: string) {
  return page.waitForResponse(response => (
    response.url().includes('/commands')
    && response.request().method() === 'POST'
    && (response.request().postData() ?? '').includes(`"type":"${actionType}"`)
    && response.ok()
  ))
}

async function chooseVisibleInitialPouch(host: Page, guest: Page) {
  const hostChoice = host.getByRole('dialog', { name: '選擇初始錦囊' })
    .getByRole('button', { name: /作為初始錦囊/ })
    .first()
  const guestChoice = guest.getByRole('dialog', { name: '選擇初始錦囊' })
    .getByRole('button', { name: /作為初始錦囊/ })
    .first()

  await expect.poll(async () => (
    Number(await hostChoice.isVisible()) + Number(await guestChoice.isVisible())
  )).toBe(1)

  const page = await hostChoice.isVisible() ? host : guest
  const choice = page === host ? hostChoice : guestChoice
  const command = waitForCommand(page, 'chooseInitialPouch')
  await choice.click()
  await command
}

test('Pouch preparation is private, reconnectable, and triggers through the Ability panel', async ({ browser }) => {
  test.setTimeout(180_000)

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()
  const pages = [host, guest]

  try {
    await Promise.all(pages.map(loginAsGuest))
    const roomName = `錦囊規則測試 ${Date.now()}`
    await createPublicRoom(host, roomName)
    await host.getByLabel('主題規則‧錦囊').check()
    await expect(host.getByLabel('個人牌組')).toBeChecked()
    await expect(host.getByLabel('主題規則‧精靈')).toBeChecked()

    await joinListedRoom(guest, roomName)
    await guest.getByRole('button', { name: '準備 →' }).click()
    await host.getByRole('button', { name: '開始遊戲 →' }).click()

    for (let selection = 0; selection < 2; selection += 1) {
      await chooseVisibleInitialPouch(host, guest)
    }

    await Promise.all(pages.map(async (page) => {
      await expect(page.getByRole('region', { name: '啟用規則' }))
        .toContainText('主題規則‧錦囊')
      await expect(page.getByRole('dialog', { name: '選擇初始錦囊' })).toHaveCount(0)
    }))

    const active = (await host.getByRole('button', { name: /秘計‧金蟬/ }).count())
      ? host
      : guest
    await active.reload()
    await expect(active.getByRole('button', { name: /秘計‧金蟬/ })).toBeVisible()
    const command = waitForCommand(active, 'triggerSecretStrategy')
    await active.getByRole('button', { name: /秘計‧金蟬/ }).click()
    await command
    await expect(active.getByRole('button', { name: /秘計‧金蟬/ })).toHaveCount(0)
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})
