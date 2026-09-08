import { dirname, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { defineConfig } from '@playwright/test'
import dotenv from "dotenv"
import { port } from "./scripts/local-ports"

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

const baseURL = `http://localhost:${port("FEWFC_E2E_PORT", 8727)}`
const browserName = browserNameFromEnvironment()
const browserPath = process.env.PLAYWRIGHT_BROWSER_PATH
const reuseExistingServer = !process.env.CI && process.env.PLAYWRIGHT_REUSE_SERVER === "1"
const e2eServerCommand =
  process.env.FEWFC_E2E_PREBUILT === "1"
    ? "bun run test:e2e:server:built"
    : "bun run test:e2e:server";

export default defineConfig({
  testDir: "./tests/e2e",
  workers: process.env.CI ? undefined : 1,
  use: {
    baseURL,
    browserName,
    launchOptions: browserPath ? { executablePath: browserPath } : {},
    screenshot: "off",
    trace: "retain-on-failure",
  },
  webServer: {
    gracefulShutdown: { signal: 'SIGTERM', timeout: 10_000 },
    command: e2eServerCommand,
    url: `${baseURL}/login`,
    timeout: 360_000,
    reuseExistingServer,
  },
});
