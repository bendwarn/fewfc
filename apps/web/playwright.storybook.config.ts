import { fileURLToPath } from 'node:url'
import dotenv from 'dotenv'
import { defineConfig } from '@playwright/test'

dotenv.config({ path: fileURLToPath(new URL('.env', import.meta.url)), quiet: true })

type BrowserName = 'chromium' | 'firefox' | 'webkit'

function browserNameFromEnvironment(): BrowserName {
  const browserName = process.env.PLAYWRIGHT_BROWSER

  if (!browserName) return 'chromium'
  if (browserName === 'chromium' || browserName === 'firefox' || browserName === 'webkit') {
    return browserName
  }

  throw new Error(`PLAYWRIGHT_BROWSER must be chromium, firefox, or webkit; received ${browserName}`)
}

const port = 6006
const origin = `http://127.0.0.1:${port}`
const browserPath = process.env.PLAYWRIGHT_BROWSER_PATH

export default defineConfig({
  testDir: './tests/storybook',
  outputDir: './test-results/storybook-layout',
  workers: 1,
  use: {
    baseURL: origin,
    browserName: browserNameFromEnvironment(),
    launchOptions: browserPath ? { executablePath: browserPath } : {},
    screenshot: 'off',
    trace: 'retain-on-failure',
  },
  webServer: {
    gracefulShutdown: { signal: 'SIGTERM', timeout: 10_000 },
    command: `pnpm exec storybook dev --ci --no-open --exact-port --port ${port}`,
    url: `${origin}/index.json`,
    timeout: 120_000,
    reuseExistingServer: !process.env.CI,
  },
})
