import { existsSync } from 'node:fs'
import { defineConfig } from '@playwright/test'

const defaultBravePath = '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser'
const browserPath = process.env.PLAYWRIGHT_BROWSER_PATH
  ?? (existsSync(defaultBravePath) ? defaultBravePath : undefined)

export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: false,
  workers: 2,
  use: {
    baseURL: "http://localhost:8727",
    browserName: "chromium",
    headless: true,
    launchOptions: browserPath ? { executablePath: browserPath } : {},
    screenshot: "off",
    trace: "retain-on-failure",
  },
  webServer: {
    command: "bun run test:e2e:server",
    url: "http://localhost:8727/login",
    timeout: 360_000,
  },
});
