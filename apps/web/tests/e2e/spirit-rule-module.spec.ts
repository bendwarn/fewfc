import {
  createPublicRoom,
  expect,
  joinListedRoom,
  loginAsGuests,
  reloadAppRoute,
  seedDevelopmentScenario,
  test,
} from './fixtures'

test('Spirit defaults on and keeps its Advanced Rule dependencies coherent', async ({ browser }) => {

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()

  try {
    await loginAsGuests([host, guest])
    const roomName = `精靈規則測試 ${Date.now()}`
    await createPublicRoom(host, roomName)

    await expect(host.getByLabel('精靈')).toBeChecked()
    await expect(host.getByLabel('星辰圖記')).toBeChecked()
    await expect(host.getByLabel('英雄學派')).toBeChecked()
    await expect(host.getByLabel('五方傳說')).toBeChecked()

    await joinListedRoom(guest, roomName, '停用：錦囊')
    await guest.getByRole('button', { name: '準備 →' }).click()

    await host.getByLabel('星辰圖記').uncheck()
    await expect(host.getByLabel('精靈')).not.toBeChecked()
    await expect(guest.getByLabel('精靈')).not.toBeChecked()
    await expect(guest.getByRole('button', { name: '準備 →' })).toBeVisible()

    await host.getByLabel('精靈').check()
    await expect(host.getByLabel('星辰圖記')).toBeChecked()
    await expect(host.getByLabel('英雄學派')).toBeChecked()
    await expect(host.getByLabel('五方傳說')).toBeChecked()

    const roomId = new URL(host.url()).pathname.split('/').pop()
    const invalidStatus = await host.evaluate(async (id) => {
      const response = await fetch(`/api/games/${id}/rules`, {
        method: 'PUT',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ enabledRuleModules: ['spirit'] }),
      })
      return response.status
    }, roomId)
    expect(invalidStatus).toBe(400)

    await guest.getByRole('button', { name: '準備 →' }).click()
    await host.getByRole('button', { name: '開始遊戲 →' }).click()

    await Promise.all([host, guest].map(async (page) => {
      await expect(page.getByRole('region', { name: '啟用規則' }))
        .toContainText('精靈')
    }))

    await reloadAppRoute(guest)
    await expect(guest.getByRole('region', { name: '啟用規則' }))
      .toContainText('精靈')
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})

test('a Spirit Skill is usable from the Ability panel and survives reconnect', async ({ browser }) => {

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()

  try {
    await loginAsGuests([host, guest])
    const roomName = `精靈技能測試 ${Date.now()}`
    await createPublicRoom(host, roomName)
    await joinListedRoom(guest, roomName, '停用：錦囊')
    await guest.getByRole('button', { name: '準備 →' }).click()
    await host.getByRole('button', { name: '開始遊戲 →' }).click()
    await expect(host.getByRole('region', { name: '啟用規則' })).toBeVisible()
    await expect(host.getByLabel('棄牌堆').locator('.discard-pile')).toHaveCount(2)
    await expect(host.locator('.discard-position-top')).toHaveCount(1)
    await expect(host.locator('.discard-position-bottom')).toHaveCount(1)
    const discardOverlapsFormation = await host.locator('.board-center').evaluate((center) => {
      const formation = center.querySelector('.formation-field')!.getBoundingClientRect()
      return [...center.querySelectorAll('.discard-pile')].some((pile) => {
        const box = pile.getBoundingClientRect()
        return box.left < formation.right
          && box.right > formation.left
          && box.top < formation.bottom
          && box.bottom > formation.top
      })
    })
    expect(discardOverlapsFormation).toBe(false)

    const roomId = new URL(host.url()).pathname.split('/').pop()
    await seedDevelopmentScenario(host, { name: 'spirit-skill' })

    await reloadAppRoute(host)
    await expect(host.locator('.spirit-status')).toContainText('精靈 · 金精靈 · 靈力 2 / 6')
    const flyingBlade = host
      .getByRole('region', { name: '能力' })
      .getByRole('button', { name: /^飛刃：/ })
    await expect(flyingBlade).toBeVisible()
    const selectedCard = host.locator('.playing-card:enabled:not(.hidden)').first()
    await Promise.all([
      host.waitForResponse(response => (
        response.url().endsWith(`/api/games/${roomId}/commands`)
        && response.request().postDataJSON()?.action?.type === 'playableActions'
      )),
      selectedCard.click(),
    ])
    await Promise.all([
      host.waitForResponse(response => (
        response.url().endsWith(`/api/games/${roomId}/commands`)
        && response.request().postDataJSON()?.action?.type === 'playableActions'
      )),
      selectedCard.click(),
    ])
    await expect(flyingBlade).toBeVisible()
    const [skillResponse] = await Promise.all([
      host.waitForResponse(response => (
        response.url().endsWith(`/api/games/${roomId}/commands`)
        && response.request().postDataJSON()?.action?.type === 'useSpiritSkill'
      )),
      flyingBlade.click(),
    ])
    expect(skillResponse.ok()).toBe(true)

    await expect(host.getByText('使用精靈技能', { exact: true })).toBeVisible()
    await expect(host.getByText(/使用「飛刃」，靈力由 2 變為 0/)).toBeVisible()
    await reloadAppRoute(guest)
    await expect(guest.getByText(/使用「飛刃」，靈力由 2 變為 0/)).toBeVisible()
    await reloadAppRoute(host)
    await expect(host.getByText(/的金精靈已破除/)).toBeVisible()
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})

test('Splendor exposes its declared levels on click and uses the chosen level', async ({ browser }) => {

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()

  try {
    await loginAsGuests([host, guest])
    const roomName = `絢爛選級測試 ${Date.now()}`
    await createPublicRoom(host, roomName)
    await joinListedRoom(guest, roomName, '停用：錦囊')
    await guest.getByRole('button', { name: '準備 →' }).click()
    await host.getByRole('button', { name: '開始遊戲 →' }).click()
    await expect(host.getByRole('region', { name: '啟用規則' })).toBeVisible()

    const roomId = new URL(host.url()).pathname.split('/').pop()
    await seedDevelopmentScenario(host, {
      name: 'spirit-skill',
      options: { spirit: 'Fire' },
    })

    await reloadAppRoute(host)
    await expect(host.locator('.spirit-status')).toContainText('精靈 · 火精靈 · 靈力 3 / 6')
    const selectedCard = host.locator('.playing-card:enabled:not(.hidden)').first()
    await Promise.all([
      host.waitForResponse(response => (
        response.url().endsWith(`/api/games/${roomId}/commands`)
        && response.request().postDataJSON()?.action?.type === 'playableActions'
      )),
      selectedCard.click(),
    ])

    const picker = host.getByRole('group', { name: '絢爛：選擇指定等級' })
    await expect(picker).toBeVisible()
    const levelOptions = picker.getByRole('menuitem')
    await expect(levelOptions).toHaveCount(0)
    const trigger = picker.getByRole('button', { name: '絢爛', exact: true })
    await expect(trigger).toHaveAttribute('aria-expanded', 'false')
    await trigger.click()
    await expect(trigger).toHaveAttribute('aria-expanded', 'true')
    await expect(levelOptions).toHaveCount(5)
    await expect(picker.getByRole('menuitem', { name: '絢爛：指定為 4 級' })).toBeVisible()

    const [skillResponse] = await Promise.all([
      host.waitForResponse(response => (
        response.url().endsWith(`/api/games/${roomId}/commands`)
        && response.request().postDataJSON()?.action?.type === 'useSpiritSkill'
      )),
      picker.getByRole('menuitem', { name: '絢爛：指定為 4 級' }).click(),
    ])
    expect(skillResponse.ok()).toBe(true)
    await expect(host.getByText(/使用「絢爛」.*宣告 4 級/)).toBeVisible()
    await reloadAppRoute(host)
    await expect(host.locator('.card-interpretation')).toContainText('已生效 · 絢爛')
    await expect(host.locator('.card-interpretation')).toContainText('視為 4 級')
    await reloadAppRoute(guest)
    await expect(guest.locator('.card-interpretation')).toContainText('一張手牌視為 4 級')
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})
