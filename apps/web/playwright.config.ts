import { dirname, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { defineConfig } from '@playwright/test'
import dotenv from "dotenv"

dotenv.config({
  path: resolve(dirname(fileURLToPath(import.meta.url)), ".env"),
  quiet: true,
})

type BrowserName = "chromium" | "firefox" | "webkit"

function browserNameFromEnvironment(): BrowserName {
  const browserName = process.env.PLAYWRIGHT_BROWSER

  if (!browserName) return "chromium"
  if (browserName === "chromium" || browserName === "firefox" || browserName === "webkit") {
    return browserName
  }

  throw new Error(
    `PLAYWRIGHT_BROWSER must be chromium, firefox, or webkit; received ${browserName}`,
  )
}

const browserName = browserNameFromEnvironment()
const browserPath = process.env.PLAYWRIGHT_BROWSER_PATH
const reuseExistingServer = !process.env.CI && process.env.PLAYWRIGHT_REUSE_SERVER !== "0"
const e2eServerCommand =
  process.env.FEWFC_E2E_PREBUILT === "1"
    ? "bun run test:e2e:server:built"
    : "bun run test:e2e:server";

export default defineConfig({
  testDir: "./tests/e2e",
  workers: process.env.CI ? undefined : 1,
  use: {
    baseURL: "http://localhost:8727",
    browserName,
    launchOptions: browserPath ? { executablePath: browserPath } : {},
    screenshot: "off",
    trace: "retain-on-failure",
  },
  webServer: {
    command: e2eServerCommand,
    url: "http://localhost:8727/login",
    timeout: 360_000,
    reuseExistingServer,
  },
});
