import { expect, test } from '@playwright/test'

test('waiting room keeps controls visible on desktop and stacks its side panel on mobile', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 720 })
  await page.goto('/__test/waiting-room')

  const waitingOverlay = page.locator('.waiting-overlay')
  const waitingPanel = page.locator('.waiting-panel')
  const waitingMain = page.locator('.waiting-main')
  const waitingSide = page.locator('.waiting-side')
  const startButton = page.getByRole('button', { name: '開始遊戲 →' })
  await expect(waitingOverlay).toHaveCSS('overflow-y', 'auto')
  await startButton.scrollIntoViewIfNeeded()

  const [panelBox, mainBox, sideBox, buttonBox] = await Promise.all([
    waitingPanel.boundingBox(),
    waitingMain.boundingBox(),
    waitingSide.boundingBox(),
    startButton.boundingBox(),
  ])
  expect(panelBox).not.toBeNull()
  expect(mainBox).not.toBeNull()
  expect(sideBox).not.toBeNull()
  expect(buttonBox).not.toBeNull()
  expect(panelBox!.width).toBeGreaterThanOrEqual(800)
  expect(sideBox!.x).toBeGreaterThanOrEqual(mainBox!.x + mainBox!.width)
  expect(buttonBox!.x).toBeGreaterThanOrEqual(sideBox!.x)
  expect(buttonBox!.y).toBeGreaterThanOrEqual(0)
  expect(buttonBox!.y + buttonBox!.height).toBeLessThanOrEqual(720)

  await page.setViewportSize({ width: 390, height: 844 })
  const [mobilePanelBox, mobileMainBox, mobileSideBox] = await Promise.all([
    waitingPanel.boundingBox(),
    waitingMain.boundingBox(),
    waitingSide.boundingBox(),
  ])
  expect(mobilePanelBox).not.toBeNull()
  expect(mobileMainBox).not.toBeNull()
  expect(mobileSideBox).not.toBeNull()
  expect(mobilePanelBox!.width).toBeLessThanOrEqual(390 - 32)
  expect(mobileSideBox!.y).toBeGreaterThanOrEqual(mobileMainBox!.y + mobileMainBox!.height)
})
