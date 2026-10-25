import { expect, test, type Page } from '@playwright/test'

const story = (title: string, name: string) => `/iframe.html?id=${encodeURIComponent(title)}--${name}&viewMode=story`
async function openRoom(page: Page, name: string) {
  await page.goto(story('頁面-正式房間', name))
  await expect(page.locator('.battlefield')).toBeVisible()
  await expect(page.locator('.setup-reveal')).toBeHidden()
}

test('discard presentation keeps readable text, separated cards, and a stable trigger', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 720 })
  await openRoom(page, 'actions')
  const empty = (await page.getByRole('button', { name: /查看共用棄牌詳情/ }).boundingBox())!
  for (const [selector, minimum] of [
    ['.battle-record-group-heading strong', 11], ['.battle-record-events li strong', 11],
    ['.battle-record-events li p', 10], ['.team-label', 8],
  ] as const) {
    expect(await page.locator(selector).first().evaluate(el => Number.parseFloat(getComputedStyle(el).fontSize))).toBeGreaterThanOrEqual(minimum)
  }
  const hand = await page.locator('.seat-bottom .playing-card').evaluateAll(cards => cards.map(card => {
    const { left, right } = card.getBoundingClientRect()
    return { left, right }
  }).sort((a, b) => a.left - b.left))
  expect(hand.length).toBeGreaterThan(1)
  for (let index = 1; index < hand.length; index++) expect(hand[index - 1]!.right).toBeLessThanOrEqual(hand[index]!.left)
  await openRoom(page, 'discard')
  const filled = (await page.getByRole('button', { name: /查看共用棄牌詳情/ }).boundingBox())!
  expect(Math.abs(empty.width - filled.width)).toBeLessThanOrEqual(1)
  expect(Math.abs(empty.height - filled.height)).toBeLessThanOrEqual(1)
})

for (const viewport of [{ width: 377, height: 734 }, { width: 1280, height: 720 }]) {
  test(`discard details fit and restore focus at ${viewport.width}`, async ({ page }) => {
    await page.setViewportSize(viewport)
    await openRoom(page, 'discard')
    const trigger = page.getByRole('button', { name: /查看共用棄牌詳情/ })
    const dialog = page.locator('#discard-composition')
    await trigger.click()
    await expect(dialog).toBeVisible()
    await expect(dialog.getByRole('columnheader')).toHaveText(['五行', '1 級', '2 級', '3 級', '4 級', '5 級'])
    await expect(dialog.getByRole('rowheader')).toHaveText(['金', '木', '水', '火', '土'])
    await expect(dialog.getByRole('cell')).toHaveCount(25)
    expect((await dialog.locator('tbody td strong').allTextContents()).reduce((sum, value) => sum + Number(value), 0)).toBe(2)
    await expect.poll(async () => {
      const box = (await dialog.boundingBox())!
      if (viewport.width < 600) return Math.abs(box.y + box.height - viewport.height) <= 1 && box.x === 0 && box.width === viewport.width
      const anchor = (await trigger.boundingBox())!
      return box.x >= 0 && box.y >= 0 && box.x + box.width <= viewport.width && box.y + box.height <= viewport.height
        && (box.y + box.height <= anchor.y || box.y >= anchor.y + anchor.height)
    }).toBe(true)
    expect(await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)).toBeLessThanOrEqual(0)
    expect(await page.locator('.game-page').evaluate(el => el.scrollHeight - el.clientHeight)).toBeLessThanOrEqual(0)
    for (const close of ['button', 'button', 'escape', 'backdrop']) {
      if (close === 'button') await dialog.getByRole('button', { name: '關閉詳情' }).click()
      else if (close === 'escape') await page.keyboard.press('Escape')
      else await page.locator('.battlefield-detail-backdrop').click({ position: { x: 4, y: 4 } })
      await expect(dialog).toBeHidden()
      await expect(trigger).toBeFocused()
      if (close !== 'backdrop') await trigger.click()
    }
  })
}

test('card choice overlay retains its light backdrop and immediate-selection presentation', async ({ page }) => {
  await openRoom(page, 'card-choice')
  const overlay = page.locator('.choice-overlay')
  await expect(overlay).toBeVisible()
  const style = await overlay.evaluate(el => ({ background: getComputedStyle(el).backgroundColor, filter: getComputedStyle(el).backdropFilter }))
  const alpha = Number(style.background.match(/[\d.]+\)$/)?.[0]?.slice(0, -1) ?? 1)
  expect(alpha).toBeLessThanOrEqual(0.35)
  expect(style.filter).toBe('none')
  await expect(overlay.getByRole('button', { name: '確認選擇' })).toHaveCount(0)
  await expect(overlay.getByRole('button', { name: '火 3', exact: true })).toBeEnabled()
})

test('initial pouch matrix provides full-cell selection targets', async ({ page }) => {
  await openRoom(page, 'initial-pouch')
  const dialog = page.getByRole('dialog', { name: '選擇初始錦囊' })
  await expect(dialog.locator('.pouch-composition')).toBeVisible()
  await expect(dialog.locator('tbody td')).toHaveCount(25)
  const cell = dialog.locator('td:has(button)').first()
  const cellBox = (await cell.boundingBox())!
  const buttonBox = (await cell.locator('button').boundingBox())!
  expect(buttonBox.width).toBeGreaterThanOrEqual(cellBox.width - 2)
  expect(buttonBox.height).toBeGreaterThanOrEqual(cellBox.height - 2)
})

test('chain selection updates stages, pressed states, confirmation, and strategy details', async ({ page }) => {
  await openRoom(page, 'chain')
  const dialog = page.getByRole('dialog', { name: '連環：選擇錦囊' })
  await expect(dialog.locator('.choice-selection-summary')).toContainText('錦囊給予：小明')
  await expect(dialog.getByLabel('選擇錦囊持有者')).toHaveCount(0)
  const headings = dialog.getByRole('heading', { level: 3 })
  await expect(headings).toHaveText(['錦囊給予對象', '選擇錦囊牌'])
  const matrix = dialog.getByLabel('連環錦囊牌組矩陣')
  await expect(matrix).toHaveCSS('margin-top', '8px')
  const pouch = matrix.getByRole('button', { name: /：金 2 級/ })
  await pouch.click()
  await expect(pouch).toHaveAttribute('aria-pressed', 'true')
  await expect(dialog.getByRole('button', { name: '確認', exact: true })).toBeEnabled()
  await expect(headings).toHaveText(['錦囊給予對象', '選擇錦囊牌', '選擇觸發牌（可選）'])
  const trigger = dialog.getByLabel('連環觸發牌組矩陣').getByRole('button', { name: /：木 3 級/ })
  await trigger.click()
  await expect(trigger).toHaveAttribute('aria-pressed', 'true')
  await expect(headings).toHaveText(['錦囊給予對象', '選擇錦囊牌', '選擇觸發牌（可選）', '觸發秘計'])
  await dialog.getByLabel('選擇秘計').getByRole('button', { name: '牽羊' }).click()
  await expect(dialog.locator('.action-detail')).toContainText('從牌組與棄牌堆各選兩張交換，之後洗牌')
})

test('pouch ability explains its effect and immediate activation on hover and focus', async ({ page }) => {
  await openRoom(page, 'pouch-ability')
  const button = page.getByRole('button', { name: /^秘計‧金蟬/ })
  await button.hover()
  await expect(page.locator('.action-detail')).toContainText('本回合保護自己')
  await expect(page.locator('.action-detail')).toContainText('點擊後立即發動')
  await page.mouse.move(0, 0)
  await button.focus()
  await expect(page.locator('.action-detail')).toBeVisible()
})

for (const width of [1280, 377]) {
  test(`production action panels keep their order and empty states at ${width}`, async ({ page }) => {
    await page.setViewportSize({ width, height: 734 })
    await openRoom(page, 'actions')
    const ability = page.getByRole('region', { name: '能力' })
    const action = page.getByRole('region', { name: '行動' })
    await expect(ability).toContainText('使用後仍可行動')
    await expect(ability).toContainText('目前沒有可用能力')
    await expect(action).toContainText('本回合擇一')
    await expect(action).toContainText('選擇手牌以尋找可用行動')
    const abilityBox = (await ability.boundingBox())!
    const actionBox = (await action.boundingBox())!
    expect(abilityBox.y + abilityBox.height).toBeLessThanOrEqual(actionBox.y)
  })
}

test('live record sheet closes and restores focus to its existing trigger', async ({ page }) => {
  await page.setViewportSize({ width: 600, height: 900 })
  await openRoom(page, 'actions')
  const trigger = page.locator('.mobile-event-summary')
  const dialog = page.getByRole('dialog', { name: '戰局紀錄' })
  for (const closeWithEscape of [false, true]) {
    await trigger.click()
    await expect(dialog.getByText(/^第 1 回合・/)).toBeVisible()
    if (closeWithEscape) await page.keyboard.press('Escape')
    else await dialog.getByRole('button', { name: '關閉戰局紀錄' }).click()
    await expect(dialog).toHaveCount(0)
    await expect(trigger).toBeFocused()
  }
})

test('player menu exposes the deck action and closes on outside click', async ({ page }) => {
  await page.goto(story('共用-玩家導覽與通知', 'light'))
  const trigger = page.getByRole('button', { name: '小明的玩家選單' })
  await trigger.click()
  await expect(page.getByRole('button', { name: '個人牌組', exact: true })).toBeVisible()
  await page.getByRole('heading', { name: '房間', exact: true }).click()
  await expect(trigger).toHaveAttribute('aria-expanded', 'false')
  await expect(page.getByRole('button', { name: '個人牌組', exact: true })).toBeHidden()
})

test('deck management buttons are equal width and reset an invalid draft', async ({ page }) => {
  await page.goto(story('頁面-完整功能頁', 'deck'))
  await expect(page.locator('.deck-validation')).toContainText('60 / 60 張')
  await expect(page.locator('.deck-validation')).toContainText('170 / 170 級')
  await expect(page.locator('.deck-validation')).toContainText('目前使用內建預組')
  const widths = await Promise.all(['匯入牌組', '匯出牌組', '重設為預組'].map(async name => (await page.getByRole('button', { name }).boundingBox())!.width))
  expect(Math.max(...widths) - Math.min(...widths)).toBeLessThanOrEqual(1)
  await page.getByRole('button', { name: '減少金1級' }).click()
  await expect(page.getByRole('button', { name: '儲存牌組' })).toBeDisabled()
  await page.getByRole('button', { name: '重設為預組' }).click()
  await expect(page.locator('.deck-validation')).toContainText('60 / 60 張')
})

test('deck import traps focus, preserves the name, exports counts, and rejects malformed input', async ({ page }) => {
  await page.context().grantPermissions(['clipboard-read', 'clipboard-write'])
  await page.goto(story('頁面-完整功能頁', 'deck'))
  const trigger = page.getByRole('button', { name: '匯入牌組' })
  await page.getByLabel('牌組名稱').fill('尚未儲存的匯入牌組')
  await trigger.click()
  const dialog = page.getByRole('dialog', { name: '匯入牌組' })
  const input = dialog.getByLabel('牌組張數')
  await expect(dialog.getByLabel('連續數字格式範例')).toHaveText('4113141122444334113341113')
  await expect(dialog.getByLabel('Tab 與換行格式範例')).toContainText('4\t1\t1\t3\t1')
  await expect(input).toBeFocused()
  await page.keyboard.press('Shift+Tab')
  await expect(dialog.getByRole('button', { name: '套用' })).toBeFocused()
  await page.keyboard.press('Tab')
  await expect(input).toBeFocused()
  const counts = [4, 1, 1, 3, 1, 4, 1, 1, 2, 2, 4, 4, 4, 3, 3, 4, 1, 1, 3, 3, 4, 1, 1, 1, 3]
  const text = Array.from({ length: 5 }, (_, row) => counts.slice(row * 5, row * 5 + 5).join('\t')).join('\n')
  await input.fill(text)
  await dialog.getByRole('button', { name: '套用' }).click()
  for (const [index, count] of counts.entries()) {
    await expect(page.getByLabel(`${['金', '木', '水', '火', '土'][Math.floor(index / 5)]}${index % 5 + 1}級張數`)).toHaveText(String(count))
  }
  await expect(page.getByLabel('牌組名稱')).toHaveValue('尚未儲存的匯入牌組')
  await page.getByRole('button', { name: '匯出牌組' }).click()
  await expect(page.getByText('牌組已複製到剪貼簿。')).toBeVisible()
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(text)
  await trigger.click()
  await input.fill('4 1 1 3 not-a-number')
  await dialog.getByRole('button', { name: '套用' }).click()
  await expect(dialog.getByRole('alert')).toContainText('每個值只能是一個 0 至 9 的十進位數字')
  await expect(page.getByLabel('金1級張數')).toHaveText('4')
  await expect(page.getByLabel('牌組名稱')).toHaveValue('尚未儲存的匯入牌組')
  await page.keyboard.press('Escape')
  await expect(trigger).toBeFocused()
})

test('public pages preserve their copy and outbound links', async ({ page }) => {
  await page.goto(story('頁面-首頁', 'light'))
  for (const name of ['以牌為陣， 決勝五行。', '三步開局，每手都是選擇', '一組牌，展開層層戰術']) await expect(page.getByRole('heading', { name })).toBeVisible()
  await expect(page.getByRole('list', { name: '五行卡牌展示' }).getByRole('listitem')).toHaveCount(5)
  const official = page.getByRole('link', { name: '五行戰鬥牌官方網站' }).first()
  await expect(official).toHaveAttribute('href', 'https://www.cfecards.org/')
  await expect(official).toHaveAttribute('target', '_blank')
  await page.goto(story('頁面-完整功能頁', 'privacy'))
  await expect(page.getByRole('heading', { level: 1, name: '隱私權政策' })).toBeVisible()
  await expect(page.getByText('我們不出售個人資料，也不將 Google 或 GitHub 提供的資料用於廣告投放。')).toBeVisible()
  for (const [name, href] of [
    ['rexkimta@gmail.com', 'mailto:rexkimta@gmail.com'], ['Google 隱私權政策', 'https://policies.google.com/privacy'],
    ['GitHub 一般隱私聲明', 'https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement'],
  ]) await expect(page.getByRole('link', { name })).toHaveAttribute('href', href!)
})
