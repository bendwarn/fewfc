import { expect, reloadFastGameRoute, seedDevelopmentScenario, setupFastTwoPlayerGame, setupFastWaitingRoom, test } from './fixtures'

test('Tribulation defaults on and normalizes every Advanced Rule dependency', async ({ browser }) => {
  const room = await setupFastWaitingRoom(browser, {
    roomName: `天劫測試 ${Date.now()}`,
  })
  const { page } = room

  try {
    await expect(page.getByLabel('天劫')).toBeChecked()
    await page.getByLabel('英雄學派').uncheck()
    await expect(page.getByLabel('天劫')).not.toBeChecked()

    await page.getByLabel('天劫').check()
    await expect(page.getByLabel('星辰圖記')).toBeChecked()
    await expect(page.getByLabel('英雄學派')).toBeChecked()
    await expect(page.getByLabel('五方傳說')).toBeChecked()
  } finally {
    await room.close()
  }
})

test('Earth Rending Environment choice is private, accessible, and reconnectable', async ({ browser }) => {
  const game = await setupFastTwoPlayerGame(browser, {
    roomName: `裂地崩山選擇測試 ${Date.now()}`,
  })
  const { host, guest, gameId: roomId } = game

  try {
    await seedDevelopmentScenario(host, { name: 'tribulation-earth-rending' })

    await reloadFastGameRoute(host, roomId)
    await expect(host.getByRole('heading', {
      name: '裂地崩山：選擇要轉移的環境',
    })).toBeVisible()
    const environments = host.getByLabel('選擇環境').getByRole('button')
    await expect(environments).toHaveCount(5)

    await reloadFastGameRoute(guest, roomId)
    await expect(guest.getByText('裂地崩山：選擇要轉移的環境', {
      exact: true,
    })).toBeVisible()
    await expect(guest.getByLabel('選擇環境')).toHaveCount(0)

    const [answer] = await Promise.all([
      host.waitForResponse(response => (
        response.url().endsWith(`/api/games/${roomId}/commands`)
        && response.request().postDataJSON()?.action?.type === 'answerChoice'
      )),
      host.getByRole('button', { name: '選擇環境 火行' }).click(),
    ])
    expect(answer.ok()).toBe(true)

    await reloadFastGameRoute(host, roomId)
    await expect(host.getByText('裂地崩山', { exact: true }).first()).toBeVisible()
  } finally {
    await game.close()
  }
})

test('Rusted Forest drains trusted shuffles inside one Game Room command', async ({ browser }) => {
  const game = await setupFastTwoPlayerGame(browser, {
    roomName: `鏽鐵枯林隨機續接測試 ${Date.now()}`,
  })
  const { host, gameId: roomId } = game

  try {
    const fixture = await seedDevelopmentScenario<{
      fixtureAction: {
        type: 'performFormation'
        player: string
        formationId: string
        cards: number[]
      }
    }>(host, { name: 'tribulation-rusted-forest' })
    const commandId = `rusted-forest-${Date.now()}`

    const commandResponse = await host.context().request.post(`/api/games/${roomId}/commands`, {
      data: { commandId, action: fixture.fixtureAction },
    })
    const commandText = await commandResponse.text()
    expect(commandResponse.ok(), commandText).toBe(true)
    expect(commandResponse.status()).toBe(200)
    const commandBody = JSON.parse(commandText)
    expect(commandBody.state.phase).toBe('TurnDrawDiscardChoice')
    expect(commandBody.state.pendingRandomness).toBeNull()
    expect(commandBody).not.toHaveProperty('type')
    expect(commandBody).not.toHaveProperty('record')
    expect(commandBody).not.toHaveProperty('request')
    const serialized = JSON.stringify(commandBody)
    expect(serialized).not.toContain('needsRandomness')
    expect(serialized).not.toContain('currentOrder')
    expect(serialized).not.toContain('shuffledOrder')
    expect(commandBody.events.filter(
      (event: { eventType: string }) => event.eventType === 'RustedForestStarted',
    )).toHaveLength(1)
    expect(commandBody.events.filter(
      (event: { eventType: string }) => event.eventType === 'RustedForestCompleted',
    )).toHaveLength(1)

    const auditResponse = await host.context().request.get(
      `/api/games/${roomId}/test-record?commandId=${encodeURIComponent(commandId)}`,
    )
    const auditText = await auditResponse.text()
    expect(auditResponse.ok(), auditText).toBe(true)
    const audit = JSON.parse(auditText)
    expect(audit.commandCommitCount).toBe(1)
    expect(audit.canonicalEventTypes.filter(
      (eventType: string) => eventType === 'RandomnessResolved',
    ).length).toBeGreaterThan(0)
    expect(audit.canonicalEventTypes.filter(
      (eventType: string) => eventType === 'RustedForestCompleted',
    )).toHaveLength(1)
  } finally {
    await game.close()
  }
})
