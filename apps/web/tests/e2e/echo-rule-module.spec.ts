import type { Page } from '@playwright/test'
import {
  createPublicRoom,
  createRoom,
  expect,
  joinListedRoom,
  loginAsGuests,
  reloadAppRoute,
  seedDevelopmentScenario,
  startTwoPlayerMatch,
  test,
} from './fixtures'

async function waitForPlayableActions(page: Page) {
  return page.waitForResponse(response => (
    response.url().includes('/commands')
    && response.request().method() === 'POST'
    && (response.request().postData() ?? '').includes('"type":"playableActions"')
    && response.ok()
  ))
}

test('Echo defaults on and normalizes every Advanced Rule dependency', async ({ page }) => {
  await createRoom(page, `迴響測試 ${Date.now()}`)

  await expect(page.getByLabel('迴響')).toBeChecked()

  await page.getByLabel('英雄學派').uncheck()
  await expect(page.getByLabel('迴響')).not.toBeChecked()

  await page.getByLabel('迴響').check()
  await expect(page.getByLabel('星辰圖記')).toBeChecked()
  await expect(page.getByLabel('英雄學派')).toBeChecked()
  await expect(page.getByLabel('五方傳說')).toBeChecked()
})

test('Pure Fire target choice is private, accessible, and reconnectable', async ({ browser }) => {
  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()

  try {
    await loginAsGuests([host, guest])
    const roomName = `淨火選擇測試 ${Date.now()}`
    const roomId = await startTwoPlayerMatch(host, guest, roomName)
    await seedDevelopmentScenario(host, { name: 'echo-pure-fire' })

    await reloadAppRoute(host)
    await expect(host.getByRole('heading', { name: '變徵‧淨火：選擇受影響玩家' })).toBeVisible()
    const targets = host.getByLabel('選擇玩家').getByRole('button')
    await expect(targets).toHaveCount(2)

    await reloadAppRoute(guest)
    await expect(guest.getByText('變徵‧淨火：選擇受影響玩家', { exact: true })).toBeVisible()
    await expect(guest.getByLabel('選擇玩家')).toHaveCount(0)

    const [answer] = await Promise.all([
      host.waitForResponse(response => (
        response.url().endsWith(`/api/games/${roomId}/commands`)
        && response.request().postDataJSON()?.action?.type === 'answerChoice'
      )),
      targets.nth(1).click(),
    ])
    expect(answer.ok(), await answer.text()).toBe(true)
    await expect(host.getByText('淨火', { exact: true })).toBeVisible()
    await expect(host.getByText(/迴響 · .* · 第 \d+ 回合/)).toBeVisible()
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})

test('Split Earth selects a Formation through its providing rules', async ({ browser }) => {
  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()

  try {
    await loginAsGuests([host, guest])
    const roomName = `裂土選擇測試 ${Date.now()}`
    const roomId = await startTwoPlayerMatch(host, guest, roomName)
    await seedDevelopmentScenario(host, { name: 'echo-split-earth' })

    await reloadAppRoute(host)
    await expect(host.getByRole('heading', { name: '宮調‧裂土：選擇要壓制的陣法' })).toBeVisible()
    await expect(host.getByLabel('選擇提供陣法的規則')).toBeVisible()
    await expect(host.getByLabel('選擇陣法', { exact: true })).toHaveCount(0)

    await host.getByRole('button', { name: '選擇規則 五方傳說' }).click()
    await expect(host.getByLabel('選擇提供陣法的規則')).toHaveCount(0)
    await expect(host.getByLabel('選擇陣法', { exact: true })).toContainText('東‧青龍')
    await host.getByRole('button', { name: '重新選擇規則' }).click()
    await expect(host.getByLabel('選擇提供陣法的規則')).toBeVisible()

    await host.getByRole('button', { name: '選擇規則 五方傳說' }).click()
    await reloadAppRoute(host)
    await expect(host.getByLabel('選擇提供陣法的規則')).toBeVisible()
    await expect(host.getByLabel('選擇陣法', { exact: true })).toHaveCount(0)

    await reloadAppRoute(guest)
    await expect(guest.getByText('宮調‧裂土：選擇要壓制的陣法', { exact: true })).toBeVisible()
    await expect(guest.getByLabel('選擇提供陣法的規則')).toHaveCount(0)

    await host.getByRole('button', { name: '選擇規則 基礎規則' }).click()
    const [answer] = await Promise.all([
      host.waitForResponse(response => (
        response.url().endsWith(`/api/games/${roomId}/commands`)
        && response.request().postDataJSON()?.action?.type === 'answerChoice'
      )),
      host.getByRole('button', { name: '選擇陣法 武器' }).click(),
    ])
    expect(answer.ok(), await answer.text()).toBe(true)
    await expect(host.getByText(/裂土：壓制 武器/)).toBeVisible()
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})

test('Echo action detail renders scheduled typed consequences on the battlefield', async ({ browser }) => {
  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const page = await hostContext.newPage()
  const guest = await guestContext.newPage()

  try {
    await loginAsGuests([page, guest])
    const roomName = `迴響行動詳情 ${Date.now()}`
    const roomId = await startTwoPlayerMatch(page, guest, roomName)
    const seeded = await seedDevelopmentScenario<{ fixtureCards?: number[] }>(page, {
      name: 'echo-pure-fire',
      options: { mode: 'actionDetail' },
    })
    expect(seeded.fixtureCards?.length).toBe(2)

    await reloadAppRoute(page)
    await expect(page.getByRole('region', { name: '啟用規則' })).toBeVisible()

    for (const cardId of seeded.fixtureCards ?? []) {
      const response = waitForPlayableActions(page)
      await page.locator(`[data-card-id="${cardId}"]`).click()
      await response
    }

    const pureFire = page.getByRole('button', { name: '變徵‧淨火' })
    await expect(pureFire).toBeVisible()
    await pureFire.hover()
    const detail = page.locator('.action-detail')
    await expect(detail).toContainText('結算此曲調的主效果')
    await expect(detail).toContainText('自己下次回合開始')
    await expect(detail).toContainText('不視為新的陣法')
    await expect(detail).toContainText('不會再次排定迴響')
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})
