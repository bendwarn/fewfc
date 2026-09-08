import { fileURLToPath } from 'node:url'
import dotenv from 'dotenv'
import { port } from './scripts/local-ports'
dotenv.config({ path: fileURLToPath(new URL('.env', import.meta.url)), quiet: true })
const componentPort = port('FEWFC_COMPONENT_PORT', 8730)
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
    baseURL: `http://127.0.0.1:${componentPort}`,
    browserName: browserNameFromEnvironment(),
    launchOptions: browserPath ? { executablePath: browserPath } : {},
    serviceWorkers: 'block',
    screenshot: 'off',
    trace: 'retain-on-failure',
  },
  webServer: {
    gracefulShutdown: { signal: 'SIGTERM', timeout: 10_000 },
    command: 'bun scripts/local-server.ts --component',
    url: `http://127.0.0.1:${componentPort}/`,
    timeout: 120_000,
    reuseExistingServer: false,
  },
})
