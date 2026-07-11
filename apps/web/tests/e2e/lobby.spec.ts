import { expect, test } from './fixtures'

test('the lobby lists rooms before showing room settings', async ({ page }) => {
  await expect(page.getByRole('heading', { name: '公開房間' })).toBeVisible()
  await expect(page.getByRole('heading', { name: '我的房間' })).toBeVisible()

  const createRoom = page.getByRole('button', { name: '建立房間', exact: true })
  await createRoom.click()

  const dialog = page.getByRole('dialog', { name: '建立房間' })
  await expect(dialog).toBeVisible()
  await expect(page.getByLabel('房間名稱')).toBeFocused()

  await page.keyboard.press('Escape')
  await expect(dialog).toBeHidden()
  await expect(createRoom).toBeFocused()
})

test('all-enabled rooms omit a rule summary and list only disabled differences', async ({ page }) => {
  const roomName = `規則摘要 ${Date.now()}`
  const created = await page.context().request.post('/api/games', {
    data: { name: roomName, access: 'public', capacity: 2 },
  })
  expect(created.ok()).toBe(true)
  const body = await created.json() as {
    gameId: string
    metadata: { enabledRuleModules: string[] }
  }
  expect(body.metadata.enabledRuleModules).toContain('pouch')

  await page.reload()
  const room = page.locator('.public-rooms-card').first()
    .locator('.public-room-list button').filter({ hasText: roomName })
  await expect(room).toBeVisible()
  await expect(room).not.toContainText('停用：')

  const updated = await page.context().request.put(`/api/games/${body.gameId}/rules`, {
    data: {
      enabledRuleModules: body.metadata.enabledRuleModules.filter(module => module !== 'pouch'),
    },
  })
  expect(updated.ok()).toBe(true)
  await page.reload()
  await expect(room).toContainText('停用：錦囊')
})
