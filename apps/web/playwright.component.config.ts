import { defineConfig } from '@playwright/test'

type BrowserName = 'chromium' | 'firefox' | 'webkit'

function browserNameFromEnvironment(): BrowserName {
  const browserName = process.env.PLAYWRIGHT_BROWSER

  if (!browserName) return 'chromium'
  if (browserName === 'chromium' || browserName === 'firefox' || browserName === 'webkit') {
    return browserName
  }

  throw new Error(`PLAYWRIGHT_BROWSER must be chromium, firefox, or webkit; received ${browserName}`)
}

const browserPath = process.env.PLAYWRIGHT_BROWSER_PATH

export default defineConfig({
  testDir: './tests/browser-component',
  workers: 1,
  use: {
    baseURL: 'http://127.0.0.1:8730',
    browserName: browserNameFromEnvironment(),
    launchOptions: browserPath ? { executablePath: browserPath } : {},
    serviceWorkers: 'block',
    screenshot: 'off',
    trace: 'retain-on-failure',
  },
  webServer: {
    command: 'APP_ENV=development bunx nuxi dev --host 127.0.0.1 --port 8730',
    url: 'http://127.0.0.1:8730/',
    timeout: 120_000,
    reuseExistingServer: !process.env.CI,
  },
})
