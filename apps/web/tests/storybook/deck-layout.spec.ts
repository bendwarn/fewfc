import { expect, test, type Locator, type Page } from '@playwright/test'

const story = (variant: string) => `/iframe.html?id=${encodeURIComponent('頁面-完整功能頁')}--${variant}&viewMode=story`
const viewportSizes = [
  { width: 1366, height: 768 },
  { width: 1024, height: 480 },
  { width: 820, height: 600 },
  { width: 390, height: 844 },
  { width: 320, height: 568 },
  { width: 320, height: 480 },
  { width: 568, height: 320 },
  { width: 667, height: 280 },
]

async function expectVisibleWithoutScrolling(page: Page, controls: Locator, top = 0, bottom = page.viewportSize()!.height) {
  expect(await controls.count()).toBeGreaterThan(0)
  for (const control of await controls.all()) {
    await expect(control).toBeInViewport({ ratio: 1 })
    const bounds = (await control.boundingBox())!
    expect(bounds.y).toBeGreaterThanOrEqual(top)
    expect(bounds.y + bounds.height).toBeLessThanOrEqual(bottom)
    expect(bounds.x).toBeGreaterThanOrEqual(0)
    expect(bounds.x + bounds.width).toBeLessThanOrEqual(page.viewportSize()!.width)
  }
  expect(await page.evaluate(() => ({
    scrollX, scrollY,
    horizontalOverflow: Math.max(0, document.documentElement.scrollWidth - innerWidth),
    verticalOverflow: Math.max(0, document.documentElement.scrollHeight - innerHeight),
  }))).toEqual({ scrollX: 0, scrollY: 0, horizontalOverflow: 0, verticalOverflow: 0 })
}

for (const viewport of viewportSizes) {
  for (const variant of ['deck', 'deck-dark']) {
    test(`${variant} keeps editing and management in view at ${viewport.width}×${viewport.height}`, async ({ page }) => {
      await page.setViewportSize(viewport)
      await page.goto(story(variant))
      const editor = page.getByRole('region', { name: '調整牌組' })
      await expect(editor.getByRole('textbox', { name: '牌組名稱' })).toHaveValue('五行均衡預組')
      await expectVisibleWithoutScrolling(page, editor.getByRole('textbox').or(editor.getByRole('button')))

      const countButtons = editor.getByRole('button', { name: /^(增加|減少)/ })
      if (viewport.width < 900) {
        const elements = editor.getByRole('group', { name: '選擇卡牌屬性' })
        await expectVisibleWithoutScrolling(page, elements.locator('label'))
        for (const element of ['金', '木', '水', '火', '土']) {
          await elements.getByText(element, { exact: true }).click()
          await expect(elements.getByRole('radio', { name: element })).toBeChecked()
          await expect(countButtons).toHaveCount(10)
          await expectVisibleWithoutScrolling(page, countButtons)
          await editor.getByRole('button', { name: `減少${element}5級` }).click()
          await expect(editor.getByLabel(`${element}5級張數`)).toHaveText('1')
          await editor.getByRole('button', { name: `增加${element}5級` }).click()
        }
      } else {
        await expect(countButtons).toHaveCount(50)
      }

      await editor.getByRole('textbox', { name: '牌組名稱' }).fill('林間試陣')
      await editor.getByRole('button', { name: '儲存牌組' }).click()
      const status = editor.getByRole('status')
      await expect(status).toHaveText('牌組已儲存。')
      await expectVisibleWithoutScrolling(page, status)
      await expect(editor.getByText('目前使用自訂牌組')).toBeVisible()
    })
  }
}

test('import, validation and reset stay usable with many problems on a small phone', async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 430 })
  await page.goto(story('deck-dark'))
  const trigger = page.getByRole('button', { name: '匯入牌組' })
  await expectVisibleWithoutScrolling(page, trigger)
  await trigger.click()
  const importDialog = page.getByRole('dialog', { name: '匯入牌組' })
  const input = importDialog.getByRole('textbox', { name: '牌組張數' })
  await expect(input).toBeFocused()
  await expectVisibleWithoutScrolling(page, input.or(importDialog.getByRole('button')))
  await input.fill('9'.repeat(25))
  await importDialog.getByRole('button', { name: '套用' }).click()
  await expect(trigger).toBeFocused()
  await expect(page.getByRole('button', { name: '儲存牌組' })).toBeDisabled()

  const issuesTrigger = page.getByRole('button', { name: '查看牌組問題' })
  await expect(issuesTrigger).toHaveText('27 項問題')
  await expectVisibleWithoutScrolling(page, page.getByRole('region', { name: '調整牌組' }).getByRole('button'))
  await issuesTrigger.click()
  const issues = page.getByRole('dialog', { name: '牌組問題' })
  await expect(issues.getByText('1 / 27')).toBeVisible()
  await expectVisibleWithoutScrolling(page, issues.getByRole('button'))
  await issues.getByRole('button', { name: '下一項' }).click()
  await expect(issues.getByText(/等級總和不得超過/)).toBeVisible()
  await issues.getByRole('button', { name: '下一項' }).click()
  await expect(issues.getByText('金1 級最多 4 張，目前為 9 張')).toBeVisible()
  await issues.getByRole('button', { name: '上一項' }).click()
  await expect(issues.getByText('2 / 27')).toBeVisible()
  await issues.getByRole('button', { name: '關閉' }).click()
  await expect(issuesTrigger).toBeFocused()
  await page.getByRole('button', { name: '重設為預組' }).click()
  await expect(page.getByRole('status')).toHaveText('已重設為內建預組。')
  await expect(page.getByRole('button', { name: '儲存牌組' })).toBeEnabled()
  await expectVisibleWithoutScrolling(page, page.getByRole('status'))
})

test('landscape import keeps confirm and cancel above the simulated keyboard', async ({ page }) => {
  await page.setViewportSize({ width: 568, height: 320 })
  await page.goto(story('deck'))
  const trigger = page.getByRole('button', { name: '匯入牌組' })
  await trigger.click()
  const dialog = page.getByRole('dialog', { name: '匯入牌組' })
  const input = dialog.getByRole('textbox', { name: '牌組張數' })
  await expect(input).toBeFocused()
  await input.fill('abc')
  await dialog.getByRole('button', { name: '套用' }).click()
  const alert = dialog.getByRole('alert')
  const controls = input.or(dialog.getByRole('button')).or(alert)
  await expect(alert).toBeVisible()
  await expectVisibleWithoutScrolling(page, controls)
  await input.focus()
  await page.evaluate(() => {
    const viewport = window.visualViewport!
    Object.defineProperties(viewport, { height: { value: 166 }, offsetTop: { value: 24 } })
    viewport.dispatchEvent(new Event('resize'))
  })
  await expect(dialog.locator('form')).toHaveClass(/deck-import-compact/)
  await expectVisibleWithoutScrolling(page, controls, 24, 190)
  await dialog.getByRole('button', { name: '取消' }).click()
  await expect(trigger).toBeFocused()
})

test('retry stays in view when deck loading fails on a short phone', async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 568 })
  await page.goto(story('deck-error'))
  await expect(page.getByRole('heading', { name: '無法載入牌組' })).toBeVisible()
  await expectVisibleWithoutScrolling(page, page.getByRole('button', { name: '重試' }))
})
