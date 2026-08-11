import type { APIResponse, Page } from '@playwright/test'
import {
  expect,
  reloadFastGameRoute,
  seedDevelopmentScenario,
  setupFastTwoPlayerGame,
  test,
} from './fixtures'

const pouchRuleModules = [
  'personal-deck',
  'five-directions-legend',
  'star',
  'hero-schools',
  'spirit',
  'pouch',
] as const

async function requestJson<T>(operation: string, responsePromise: Promise<APIResponse>): Promise<T> {
  const response = await responsePromise
  const body = await response.text()
  if (!response.ok()) {
    throw new Error(`${operation} failed with HTTP ${response.status()}: ${body}`)
  }
  try {
    return JSON.parse(body) as T
  } catch {
    throw new Error(`${operation} returned invalid JSON: ${body}`)
  }
}

async function waitForCommand(page: Page, actionType: string) {
  const response = await page.waitForResponse(response => (
    response.url().includes('/commands')
    && response.request().method() === 'POST'
    && (response.request().postData() ?? '').includes(`"type":"${actionType}"`)
  ))
  expect(response.ok(), await response.text()).toBe(true)
  return response
}

async function completeIndependentInitialPouchSelection(host: Page, guest: Page, roomId: string) {
  const hostChoice = host.getByRole('dialog', { name: '選擇初始錦囊' })
    .getByRole('button', { name: /作為初始錦囊/ })
    .first()
  const guestChoice = guest.getByRole('dialog', { name: '選擇初始錦囊' })
    .getByRole('button', { name: /作為初始錦囊/ })
    .first()

  await expect(hostChoice).toBeVisible()
  await expect(guestChoice).toBeVisible()
  const dialog = host.getByRole('dialog', { name: '選擇初始錦囊' })
  await expect(dialog.locator('.pouch-composition')).toBeVisible()
  await expect(dialog.locator('tbody td')).toHaveCount(25)
  const command = waitForCommand(host, 'chooseInitialPouch')
  await hostChoice.click()
  await command
  await expect(host.getByRole('dialog', { name: '選擇初始錦囊' })).toHaveCount(0)
  await expect(
    host.getByRole('region', { name: '五行戰鬥牌對戰桌' }).getByRole('status'),
  ).toContainText('等待')
  await expect(guestChoice).toBeVisible()

  // A fresh owner response retains the selected Card, while the other Player
  // receives only public completion progress and a Card Back.
  await reloadFastGameRoute(host, roomId)
  const [ownerState, otherViewerState] = await Promise.all([
    requestJson<{
      state: {
        initialPouchSelection: { remainingPlayers: string[] } | null
        pouches: Array<{ card: unknown | null }>
      }
    }>(
      `GET /api/games/${roomId} as owner`,
      host.context().request.get(`/api/games/${roomId}`),
    ),
    requestJson<{
      state: {
        initialPouchSelection: { remainingPlayers: string[] } | null
        pouches: Array<{ card: unknown | null }>
      }
    }>(
      `GET /api/games/${roomId} as other viewer`,
      guest.context().request.get(`/api/games/${roomId}`),
    ),
  ])
  expect(ownerState.state.initialPouchSelection?.remainingPlayers).toHaveLength(1)
  expect(otherViewerState.state.initialPouchSelection?.remainingPlayers).toHaveLength(1)
  expect(ownerState.state.pouches[0]?.card).not.toBeNull()
  expect(otherViewerState.state.pouches[0]?.card).toBeNull()

  const guestCommand = waitForCommand(guest, 'chooseInitialPouch')
  await guestChoice.click()
  await guestCommand
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
  const game = await setupFastTwoPlayerGame(browser, {
    roomName: `錦囊規則測試 ${Date.now()}`,
    enabledRuleModules: pouchRuleModules,
  })
  const { host, guest, pages, gameId: roomId } = game

  try {
    await completeIndependentInitialPouchSelection(host, guest, roomId)

    await Promise.all(pages.map(async (page) => {
      await expect(page.getByRole('region', { name: '啟用規則' }))
        .toContainText('錦囊')
      await expect(page.getByRole('dialog', { name: '選擇初始錦囊' })).toHaveCount(0)
    }))

    const active = (await host.getByRole('button', { name: /秘計‧金蟬/ }).count())
      ? host
      : guest
    await reloadFastGameRoute(active, roomId)
    const goldenCicada = active.getByRole('button', { name: /秘計‧金蟬/ })
    await expect(goldenCicada).toBeVisible()
    await goldenCicada.hover()
    await expect(active.locator('.action-detail')).toContainText('本回合保護自己')
    await expect(active.locator('.action-detail')).toContainText('點擊後立即發動')
    const command = waitForCommand(active, 'triggerSecretStrategy')
    await goldenCicada.click()
    await command
    await expect(active.getByRole('button', { name: /秘計‧金蟬/ })).toHaveCount(0)
    await expect(active.locator('.persistent-effect')).toContainText('本回合結束 · 金蟬')
  } finally {
    await game.close()
  }
})

test('Chain stages Sheep Stealing as a typed exchange choice', async ({ browser }) => {
  const game = await setupFastTwoPlayerGame(browser, {
    roomName: `連環牽羊測試 ${Date.now()}`,
    enabledRuleModules: pouchRuleModules,
  })
  const { host, gameId: roomId } = game

  try {
    const fixture = await seedDevelopmentScenario<{
      metadata: { gameInstanceId?: string }
      fixtureAction: { player: string; formationId: string; cards: number[] }
    }>(host, { name: 'pouch-chain-sheep' })
    const actor = host
    await reloadFastGameRoute(actor, roomId)
    const commandId = `chain-sheep-${Date.now()}`
    const gameInstanceId = fixture.metadata.gameInstanceId
    expect(gameInstanceId).toBeTruthy()
    const chain = await requestJson<{ state: { pendingChoice?: { choice: { type: string } } } }>(
      `POST /api/games/${roomId}/commands`,
      actor.context().request.post(`/api/games/${roomId}/commands`, {
        data: {
          commandId,
          gameInstanceId,
          transactionId: `transaction:${commandId}`,
          action: {
            type: 'performFormation',
            player: fixture.fixtureAction.player,
            formationId: fixture.fixtureAction.formationId,
            cards: fixture.fixtureAction.cards,
          },
        },
      }),
    )
    expect(chain.state.pendingChoice?.choice.type).toBe('chain')

    await reloadFastGameRoute(actor, roomId)
    const chainDialog = actor.getByRole('dialog', { name: '連環：選擇錦囊' })
    await expect(chainDialog).toBeVisible()
    const chainHeadings = chainDialog.getByRole("heading", { level: 3 });
    await expect(
      chainDialog.locator(".choice-selection-summary"),
    ).toContainText("錦囊給予：");
    await expect(chainDialog.getByLabel("選擇錦囊持有者")).toHaveCount(0);
    await expect(chainHeadings).toHaveText(["錦囊給予對象", "選擇錦囊牌"]);
    const state = await requestJson<{
      state: { pendingChoice: { visibility: 'visible'; choice: { type: 'chain'; deckCards: Array<{
        element: string | null
        level: number | null
        secretStrategies: Array<{ strategy: string }>
      }> } } | null }
    }>(
      `GET /api/games/${roomId}`,
      actor.context().request.get(`/api/games/${roomId}`),
    )
    const cards = state.state.pendingChoice?.visibility === 'visible'
      && state.state.pendingChoice.choice.type === 'chain'
      ? state.state.pendingChoice.choice.deckCards
      : []
    let sheepTrigger: { pouch: typeof cards[number]; trigger: typeof cards[number] } | null = null
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
      if (pouch) {
        sheepTrigger = { pouch, trigger }
        break
      }
    }
    expect(sheepTrigger).not.toBeNull()
    const pouchMatrix = chainDialog.getByLabel('連環錦囊牌組矩陣')
    const pouchButton = pouchMatrix.getByRole('button', {
      name: new RegExp(`：${elementLabel(sheepTrigger!.pouch.element!)} ${sheepTrigger!.pouch.level} 級`),
    }).first()
    await pouchButton.click()
    await expect(pouchButton).toHaveAttribute('aria-pressed', 'true')
    await expect(chainHeadings).toHaveText([
      "錦囊給予對象",
      "選擇錦囊牌",
      "選擇觸發牌（可選）",
    ]);
    const triggerMatrix = chainDialog.getByLabel('連環觸發牌組矩陣')
    const triggerButton = triggerMatrix.getByRole('button', {
      name: new RegExp(`：${elementLabel(sheepTrigger!.trigger.element!)} ${sheepTrigger!.trigger.level} 級`),
    }).first()
    await triggerButton.click()
    await expect(triggerButton).toHaveAttribute('aria-pressed', 'true')
    await expect(chainHeadings).toHaveText([
      "錦囊給予對象",
      "選擇錦囊牌",
      "選擇觸發牌（可選）",
      "觸發秘計",
    ]);
    await chainDialog.getByLabel('選擇秘計').getByRole('button', { name: '牽羊' }).click()
    await expect(chainDialog.locator('.action-detail')).toContainText('從牌組與棄牌堆各選兩張交換，之後洗牌')
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
    await game.close()
  }
})
