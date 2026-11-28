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
  await expect(controls.first()).toBeVisible()
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
      const table = editor.getByRole('table', { name: '個人牌組卡牌張數' })
      await expect(table.getByRole('button')).toHaveCount(viewport.width >= 900 ? 75 : 25)
      await expect(table.getByRole('rowheader')).toHaveText(['金', '木', '水', '火', '土', '等級張數'])
      await expectVisibleWithoutScrolling(page, editor.getByRole('textbox').or(editor.getByRole('button')).or(table))
      expect(await table.locator('.deck-grand-total').evaluateAll(totals => (
        totals.every(total => total.scrollWidth <= total.clientWidth)
      ))).toBe(true)

      const count = table.getByRole('button', { name: /^土5級張數/ })
      const menu = page.getByRole('menu', { name: '土5 級張數' })
      await count.click()
      await expect(menu.getByRole('menuitemradio')).toHaveCount(4)
      await expectVisibleWithoutScrolling(page, menu.getByRole('menuitemradio'))
      await menu.getByRole('menuitemradio', { name: '1 張' }).click()
      await expect(count).toHaveText('1')
      await expect(count).toBeFocused()
      await count.click()
      await menu.getByRole('menuitemradio', { name: '2 張' }).click()

      await editor.getByRole('textbox', { name: '牌組名稱' }).fill('林間試陣')
      await editor.getByRole('button', { name: '儲存牌組' }).click()
      const status = editor.getByRole('status')
      await expect(status).toHaveText('牌組已儲存。')
      await expectVisibleWithoutScrolling(page, status)
      await expect(editor.getByText('目前使用自訂牌組')).toBeVisible()
    })
  }
}

test('the deck table updates row, level and deck totals when choosing a count', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 })
  await page.goto(story('deck'))
  const table = page.getByRole('table', { name: '個人牌組卡牌張數' })
  const earth = table.getByRole('row').filter({ has: page.getByRole('rowheader', { name: '土', exact: true }) })
  const footer = table.getByRole('row').last()
  await expect(table.getByRole('columnheader')).toHaveText(['屬性', '1 級', '2 級', '3 級', '4 級', '5 級', '張數', '等級總和'])
  await expect(earth.getByRole('cell')).toHaveText(['3', '2', '3', '2', '2', '12', '34'])
  await expect(footer.getByRole('cell')).toHaveText(['15', '10', '15', '10', '10', '60 / 60 張', '170 / 170 級'])
  const count = earth.getByRole('button', { name: /^土5級張數/ })
  await count.click()
  const menu = page.getByRole('menu', { name: '土5 級張數' })
  await expect(menu.getByRole('menuitemradio')).toHaveText(['0 張', '1 張', '2 張', '3 張'])
  await menu.getByRole('menuitemradio', { name: '0 張', exact: true }).click()
  await expect(earth.getByRole('cell')).toHaveText(['3', '2', '3', '2', '0', '10', '24'])
  await expect(footer.getByRole('cell')).toHaveText(['15', '10', '15', '10', '8', '58 / 60 張', '160 / 170 級'])
  await expect(page.getByRole('button', { name: '儲存牌組' })).toBeDisabled()
})

test('the count menu supports keyboard selection, dismissal and focus restoration', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 })
  await page.goto(story('deck'))
  const trigger = page.getByRole('button', { name: /^金1級張數/ })
  await trigger.press('Enter')
  const menu = page.getByRole('menu', { name: '金1 級張數' })
  await expect(menu.getByRole('menuitemradio')).toHaveCount(5)
  await expect(menu.getByRole('menuitemradio', { name: '3 張', checked: true })).toBeFocused()
  await page.keyboard.press('ArrowRight')
  await expect(menu.getByRole('menuitemradio', { name: '4 張' })).toBeFocused()
  await page.keyboard.press('Enter')
  await expect(trigger).toHaveText('4')
  await expect(trigger).toBeFocused()

  await trigger.press('Space')
  await expect(menu.getByRole('menuitemradio', { name: '4 張', checked: true })).toBeFocused()
  await page.keyboard.press('Escape')
  await expect(trigger).toBeFocused()
  await expect(trigger).toHaveAttribute('aria-expanded', 'false')

  await trigger.press('Enter')
  await expect(menu.getByRole('menuitemradio', { name: '4 張', checked: true })).toBeFocused()
  await page.keyboard.press('Tab')
  await expect(page.getByRole('button', { name: /^金2級張數/ })).toBeFocused()

  await trigger.click()
  await expect(menu).toBeVisible()
  const name = page.getByRole('textbox', { name: '牌組名稱' })
  await name.click()
  await expect(menu).toBeHidden()
  await expect(name).toBeFocused()
})

test('desktop count steps update the table and respect both copy limits', async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 480 })
  await page.goto(story('deck'))
  const table = page.getByRole('table', { name: '個人牌組卡牌張數' })
  const plus = table.getByRole('button', { name: '增加金1級張數' })
  const minus = table.getByRole('button', { name: '減少金1級張數' })
  const count = table.getByRole('button', { name: /^金1級張數：/ })
  await plus.click()
  await expect(count).toHaveText('4')
  await expect(plus).toBeDisabled()
  const footer = table.getByRole('row').last()
  await expect(footer.getByRole('cell', { name: '61 / 60 張' })).toBeVisible()
  await expect(footer.getByRole('cell', { name: '171 / 170 級' })).toBeVisible()
  await minus.click()
  await expect(count).toHaveText('3')
  await expect(page.getByRole('button', { name: '儲存牌組' })).toBeEnabled()

  await count.click()
  await page.getByRole('menuitemradio', { name: '1 張', exact: true }).click()
  await minus.click()
  await expect(count).toHaveText('0')
  await expect(minus).toBeDisabled()
  await expect(plus).toBeEnabled()
})

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
  const count = page.getByRole('button', { name: '金1級張數：9 張' })
  await count.click()
  const menu = page.getByRole('menu', { name: '金1 級張數' })
  await expect(menu.getByRole('menuitemradio')).toHaveText(['0 張', '1 張', '2 張', '3 張', '4 張'])
  await menu.getByRole('menuitemradio', { name: '4 張' }).click()
  await expect(issuesTrigger).toHaveText('26 項問題')
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
