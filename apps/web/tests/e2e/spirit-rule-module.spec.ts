import { waitForGameCommand, gameCommandRequests } from './realtime-helpers'
import {
  battleRecordEntry,
  expect,
  seedDevelopmentScenario,
  setupFastTwoPlayerGame,
  test,
} from './fixtures'

test('a Spirit Skill is usable from the Ability panel and survives reconnect', async ({ browser }) => {
  test.setTimeout(90_000)
  const game = await setupFastTwoPlayerGame(browser, {
    roomName: `精靈技能測試 ${Date.now()}`,
  })
  const { host, guest, gameId: roomId } = game

  try {
    await seedDevelopmentScenario(host, { name: 'spirit-skill' })

    const flyingBlade = host
      .getByRole('region', { name: '能力' })
      .getByRole('button', { name: /^飛刃/ })
    await host.reload({ waitUntil: 'domcontentloaded' })
    await expect(flyingBlade).toBeEnabled({ timeout: 10_000 })
    const actingSeat = host.locator('.player-seat[aria-current="true"]')
    await actingSeat.getByRole('button', { name: /查看.*的詳細資訊/ }).click()
    const playerDetails = host.getByRole('dialog', { name: /的對局資訊/ })
    await expect(playerDetails).toBeVisible()
    await expect(playerDetails.locator('.player-detail-facts')).toContainText('金精靈・靈力 2 / 6')
    await playerDetails.getByRole('button', { name: '關閉詳情' }).click()
    await expect(flyingBlade).toBeVisible()
    await expect(host.getByText('你的回合', { exact: true })).toBeVisible()
    const [skillResponse] = await Promise.all([
      waitForGameCommand(host, action => action.type === 'useSpiritSkill'),
      flyingBlade.click(),
    ])
    expect(skillResponse.ok).toBe(true)

    const hostEntry = await battleRecordEntry(host, '使用「飛刃」')
    await expect(hostEntry).toContainText('靈力由 2 變為 0')
    await guest.reload({ waitUntil: 'domcontentloaded' })
    const guestEntry = await battleRecordEntry(guest, '使用「飛刃」')
    await expect(guestEntry).toContainText('靈力由 2 變為 0')
    await host.reload({ waitUntil: 'domcontentloaded' })
    const reloadedHostEntry = await battleRecordEntry(host, '使用「飛刃」')
    await expect(reloadedHostEntry).toContainText('的金精靈已破除')
  } finally {
    await game.close()
  }
})
