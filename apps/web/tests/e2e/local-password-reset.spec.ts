import { expect, test } from '@playwright/test'

test('an enabled local password reset repairs an email credential and signs the player in', async ({ page }) => {
  const email = `reset-${Date.now()}@example.com`
  const originalPassword = 'OriginalPassword123!'
  const newPassword = 'ReplacementPassword123!'

  await page.goto('/login')
  await expect(page.getByRole('button', { name: '重設本機密碼' })).toBeVisible()
  await page.getByRole('button', { name: '還沒有帳號？建立帳號' }).click()
  await page.getByLabel('玩家名稱').fill('重設測試玩家')
  await page.getByLabel('Email').fill(email)
  await page.getByLabel('密碼').fill(originalPassword)
  await page.getByRole('button', { name: '註冊並登入' }).click()
  await expect(page).toHaveURL(/\/rooms$/)

  await page.context().clearCookies()
  await page.goto('/login')
  await page.getByRole('button', { name: '重設本機密碼' }).click()
  await expect(page).toHaveURL(/\/reset-password$/)
  await page.getByLabel('Email').fill(email)
  await page.getByLabel('新密碼', { exact: true }).fill(newPassword)
  await page.getByLabel('確認新密碼').fill(newPassword)

  const resetResponse = page.waitForResponse((response) => (
    new URL(response.url()).pathname === '/api/local-password-reset'
      && response.request().method() === 'POST'
  ))
  await page.getByRole('button', { name: '重設密碼並登入' }).click()
  await expect((await resetResponse).json()).resolves.toEqual({ status: 'reset' })
  await expect(page).toHaveURL(/\/rooms$/)

  await page.context().clearCookies()
  await page.goto('/login')
  await page.getByLabel('Email').fill(email)
  await page.getByLabel('密碼').fill(newPassword)
  await page.getByRole('button', { name: '登入' }).click()
  await expect(page).toHaveURL(/\/rooms$/)
})

test('the local reset form reports an unknown account in its development-only diagnostics', async ({ page }) => {
  await page.goto('/reset-password')
  await expect(page.getByRole('heading', { name: '重設本機密碼' })).toBeVisible()
  await page.getByLabel('Email').fill(`missing-${Date.now()}@example.com`)
  await page.getByLabel('新密碼', { exact: true }).fill('ReplacementPassword123!')
  await page.getByLabel('確認新密碼').fill('ReplacementPassword123!')
  await page.getByRole('button', { name: '重設密碼並登入' }).click()
  await expect(page.getByRole('alert')).toHaveText('找不到這個 Email 的帳號')
})
