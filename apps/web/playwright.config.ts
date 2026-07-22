import { existsSync } from 'node:fs'
import { defineConfig } from '@playwright/test'

const defaultBravePath = '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser'
const browserPath = process.env.PLAYWRIGHT_BROWSER_PATH
  ?? (existsSync(defaultBravePath) ? defaultBravePath : undefined)
const e2eServerCommand =
  process.env.FEWFC_E2E_PREBUILT === "1"
    ? "bun run test:e2e:server:built"
    : "bun run test:e2e:server";

export default defineConfig({
  testDir: "./tests/e2e",
  workers: 1,
  use: {
    baseURL: "http://localhost:8727",
    launchOptions: browserPath ? { executablePath: browserPath } : {},
    screenshot: "off",
    trace: "retain-on-failure",
  },
  webServer: {
    command: e2eServerCommand,
    url: "http://localhost:8727/login",
    timeout: 360_000,
  },
});
