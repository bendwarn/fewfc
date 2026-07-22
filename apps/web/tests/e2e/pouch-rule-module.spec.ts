import type { Page } from '@playwright/test'
import {
  createPublicRoom,
  expect,
  joinListedRoom,
  loginAsGuests,
  reloadAppRoute,
  seedDevelopmentScenario,
  startTwoPlayerMatch,
  test,
} from './fixtures'

async function waitForCommand(page: Page, actionType: string) {
  const response = await page.waitForResponse(response => (
    response.url().includes('/commands')
    && response.request().method() === 'POST'
    && (response.request().postData() ?? '').includes(`"type":"${actionType}"`)
  ))
  expect(response.ok(), await response.text()).toBe(true)
  return response
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
  const dialog = page.getByRole('dialog', { name: '選擇初始錦囊' })
  await expect(dialog.locator('.pouch-composition')).toBeVisible()
  await expect(dialog.locator('tbody td')).toHaveCount(25)
  const command = waitForCommand(page, 'chooseInitialPouch')
  await choice.click()
  await command
}

async function chooseTwoMatrixCards(page: Page, label: string) {
  const matrix = page.getByLabel(label)
  const buttons = matrix.getByRole('button')
  expect(await buttons.count()).toBeGreaterThanOrEqual(2)
  await buttons.nth(0).click()
  await buttons.nth(1).click()
}

function elementLabel(element: string) {
  return ({ Metal: '金', Wood: '木', Water: '水', Fire: '火', Earth: '土' } as Record<string, string>)[element]
}

test('Pouch preparation is private, reconnectable, and triggers through the Ability panel', async ({ browser }) => {

  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()
  const pages = [host, guest]

  try {
    await loginAsGuests(pages)
    const roomName = `錦囊規則測試 ${Date.now()}`
    await createPublicRoom(host, roomName)
    await host.getByLabel('錦囊').check()
    await expect(host.getByLabel('個人牌組')).toBeChecked()
    await expect(host.getByLabel('精靈')).toBeChecked()

    await joinListedRoom(guest, roomName)
    await guest.getByRole('button', { name: '準備 →' }).click()
    await host.getByRole('button', { name: '開始遊戲 →' }).click()

    for (let selection = 0; selection < 2; selection += 1) {
      await chooseVisibleInitialPouch(host, guest)
    }

    await Promise.all(pages.map(async (page) => {
      await expect(page.getByRole('region', { name: '啟用規則' }))
        .toContainText('錦囊')
      await expect(page.getByRole('dialog', { name: '選擇初始錦囊' })).toHaveCount(0)
    }))

    const active = (await host.getByRole('button', { name: /秘計‧金蟬/ }).count())
      ? host
      : guest
    await reloadAppRoute(active)
    const goldenCicada = active.getByRole('button', { name: /秘計‧金蟬/ })
    await expect(goldenCicada).toBeVisible()
    await goldenCicada.hover()
    await expect(active.locator('.action-detail')).toContainText('本回合保護自己')
    const command = waitForCommand(active, 'triggerSecretStrategy')
    await goldenCicada.click()
    await command
    await expect(active.getByRole('button', { name: /秘計‧金蟬/ })).toHaveCount(0)
    await expect(active.locator('.persistent-effect')).toContainText('本回合結束 · 金蟬')
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})

test('Chain stages Sheep Stealing as a typed exchange choice', async ({ browser }) => {
  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const host = await hostContext.newPage()
  const guest = await guestContext.newPage()
  const pages = [host, guest]

  try {
    await loginAsGuests(pages)
    const roomName = `連環牽羊測試 ${Date.now()}`
    const roomId = await startTwoPlayerMatch(host, guest, roomName, async (page) => {
      await page.getByLabel('錦囊').check()
    })
    const fixture = await seedDevelopmentScenario<{
      fixtureAction: { player: string; formationId: string; cards: number[] }
    }>(host, { name: 'pouch-chain-sheep' })
    const actor = host
    await reloadAppRoute(actor)
    const commandId = `chain-sheep-${Date.now()}`
    const chain = await actor.evaluate(async ({ gameId, command, action }) => {
      const response = await fetch(`/api/games/${gameId}/commands`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          commandId: command,
          action: {
            type: 'performFormation',
            player: action.player,
            formationId: action.id,
            cards: action.cards,
          },
        }),
      })
      return { ok: response.ok, body: await response.json() }
    }, {
      gameId: roomId,
      command: commandId,
      action: {
        player: fixture.fixtureAction.player,
        id: fixture.fixtureAction.formationId,
        cards: fixture.fixtureAction.cards,
      },
    })
    expect(chain.ok, JSON.stringify(chain.body)).toBe(true)
    expect(chain.body.state.pendingChoice?.choice.type).toBe('chain')

    await reloadAppRoute(actor)
    const chainDialog = actor.getByRole('dialog', { name: '連環：選擇錦囊' })
    await expect(chainDialog).toBeVisible()
    const sheepTrigger = await actor.evaluate(async ({ gameId }) => {
      const response = await fetch(`/api/games/${gameId}`)
      const body = await response.json() as {
        state: { pendingChoice: { visibility: 'visible'; choice: { type: 'chain'; deckCards: Array<{
          element: string | null
          level: number | null
          secretStrategies: Array<{ strategy: string }>
        }> } } | null }
      }
      const cards = body.state.pendingChoice?.visibility === 'visible'
        && body.state.pendingChoice.choice.type === 'chain'
        ? body.state.pendingChoice.choice.deckCards
        : []
      for (const trigger of cards.filter(card => (
        card.secretStrategies.some(option => option.strategy === 'SheepStealing')
      ))) {
        const pouch = cards.find(candidate => (
          candidate.element !== null
          && candidate.level !== null
          && candidate !== trigger
          && candidate.element !== trigger.element
          && candidate.level !== trigger.level
        ))
        if (pouch) return { pouch, trigger }
      }
      return null
    }, { gameId: roomId })
    expect(sheepTrigger).not.toBeNull()
    const pouchMatrix = chainDialog.getByLabel('連環錦囊牌組矩陣')
    const pouchButton = pouchMatrix.getByRole('button', {
      name: new RegExp(`：${elementLabel(sheepTrigger!.pouch.element!)} ${sheepTrigger!.pouch.level} 級`),
    }).first()
    await pouchButton.click()
    await expect(pouchButton).toHaveAttribute('aria-pressed', 'true')
    const triggerMatrix = chainDialog.getByLabel('連環觸發牌組矩陣')
    const triggerButton = triggerMatrix.getByRole('button', {
      name: new RegExp(`：${elementLabel(sheepTrigger!.trigger.element!)} ${sheepTrigger!.trigger.level} 級`),
    }).first()
    await triggerButton.click()
    await expect(triggerButton).toHaveAttribute('aria-pressed', 'true')
    await chainDialog.getByLabel('選擇錦囊持有者').getByRole('button').first().click()
    await chainDialog.getByLabel('選擇秘計').getByRole('button', { name: '牽羊' }).click()
    await expect(chainDialog.locator('.action-detail')).toContainText('各選兩張牌交換牌組與棄牌堆')
    const chainAnswer = waitForCommand(actor, 'answerChoice')
    await chainDialog.getByRole('button', { name: '確認' }).click()
    await chainAnswer

    const sheepDialog = actor.getByRole('dialog', { name: '牽羊：交換牌' })
    await expect(sheepDialog).toBeVisible()
    await chooseTwoMatrixCards(actor, '牽羊牌組矩陣')
    await chooseTwoMatrixCards(actor, '牽羊回收矩陣')
    const sheepAnswer = waitForCommand(actor, 'answerChoice')
    await sheepDialog.getByRole('button', { name: '確認' }).click()
    await sheepAnswer
    await expect(sheepDialog).toHaveCount(0)
  } finally {
    await hostContext.close()
    await guestContext.close()
  }
})
