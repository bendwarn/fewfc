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
