import { activePlayerPage, expect, reloadFastGameRoute, setupFastTwoPlayerGame, test } from './fixtures'

test('selected cards query rule-backed actions and surface transport failures', async ({ browser }) => {
  const game = await setupFastTwoPlayerGame(browser, {
    roomName: `可用行動測試 ${Date.now()}`,
    enabledRuleModules: ['discard-retrieval', 'star', 'hero-schools'],
  })
  const { pages } = game

  try {
    const active = await activePlayerPage(pages)
    const action = active.getByRole('region', { name: '行動' })

    let queryMode: 'delay' | 'fail' = 'delay'
    await active.routeWebSocket('**/api/games/*/socket', socket => {
      const server = socket.connectToServer()
      socket.onMessage(async wire => {
        const request = JSON.parse(String(wire))
        if (request.operation === 'command' && request.input.action.type === 'playableActions') {
          expect(request.input.commandId).toMatch(/\S/)
          expect(request.input.gameInstanceId).toMatch(/\S/)
          expect(request.input.transactionId).toMatch(/\S/)
          if (queryMode === 'fail') {
            socket.send(JSON.stringify({ type: 'result', requestId: request.requestId, ok: false,
              error: { statusCode: 500, code: 'queryFailed', message: 'query failed' } }))
            return
          }
          await new Promise(resolve => setTimeout(resolve, 300))
        }
        server.send(wire)
      })
    })
    await reloadFastGameRoute(active, game.gameId)

    const card = active.locator('.playing-card:enabled:not(.hidden)').first()
    await card.click()
    await expect(active.locator('.action-processing')).toHaveText('處理中')

    const formation = action.locator('.action-candidates button:not(.skip-action)').first()
    await expect(formation).toBeVisible()
    await card.click()
    queryMode = 'fail'
    await card.click()
    await expect(active.locator('.action-error'))
      .toHaveText('無法取得可用行動，請稍後再試。')
    await expect(active.locator('.action-error')).not.toContainText('500')
    await expect(active.locator('.action-error')).not.toContainText('Internal Server Error')
  } finally {
    await game.close()
  }
})
