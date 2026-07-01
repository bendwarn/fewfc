import { existsSync } from 'node:fs'
import { defineConfig } from '@playwright/test'

const defaultBravePath = '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser'
const browserPath = process.env.PLAYWRIGHT_BROWSER_PATH
  ?? (existsSync(defaultBravePath) ? defaultBravePath : undefined)

export default defineConfig({
  testDir: './tests/e2e',
  fullyParallel: false,
  workers: 2,
  timeout: 60_000,
  expect: {
    timeout: 10_000,
  },
  use: {
    baseURL: 'http://localhost:8787',
    browserName: 'chromium',
    headless: true,
    launchOptions: browserPath ? { executablePath: browserPath } : {},
    screenshot: 'off',
    trace: 'retain-on-failure',
  },
  webServer: {
    command: 'bun run test:e2e:server',
    url: 'http://localhost:8787/login',
    reuseExistingServer: process.env.PLAYWRIGHT_REUSE_SERVER === '1',
    timeout: 180_000,
  },
})
