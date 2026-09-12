# Web App Agent Instructions

## Development Environment

- Use pnpm for local dependency installation and changes; commit `pnpm-lock.yaml`.
- Keep Bun as the runtime for scripts and tests. CI installs pnpm with
  `pnpm/action-setup` and Bun with `oven-sh/setup-bun`.
- Development is the default environment. The `build`, `postinstall`, and
  `typecheck` package scripts intentionally rely on Nuxt's default `.env`
  loading instead of passing `--dotenv .env.development`. For staging or
  production, use the corresponding environment-specific script, which must
  explicitly select `.env.staging` or `.env.production`.
- For Worker/Durable Object local development, ensure `.env` exists and `BETTER_AUTH_SECRET` is at least 32 characters, otherwise Better Auth routes fail with `500 BETTER_AUTH_SECRET must contain at least 32 characters`.
- Local development servers may need sandbox escalation to bind localhost ports. If a server reports no available port while nothing is reachable, rerun with escalated permissions.

## 玩家介面文案

- 玩家可見的系統文案使用繁體中文，包含停用原因、錯誤提示、空狀態、
  tooltip、`title` 與無障礙標籤；使用者輸入與必要的專有名稱保留原文。
- 顯示規則、陣法、狀態或錯誤時，沿用既有中文名稱與呈現層映射。
  API 的機器 ID、enum 值與原始英文錯誤保留供程式處理及診斷，
  不直接作為玩家文案；未知值提供可理解的繁體中文備援訊息。
- 修改動態文案時，追查從 API／規則引擎到 UI 的顯示路徑，檢查正常、
  停用及錯誤分支。修正漏譯時，以對應單元或元件測試驗證實際中文文案，
  包含造成漏譯的分支，而不只檢查靜態模板。

## Browser and E2E Validation

- Keep E2E tests on Playwright's default timeouts whenever possible. If a test
  times out, use the `playwright-cli` skill first to inspect the live flow and
  identify where it is blocked before increasing a timeout or changing the
  test.
- Tests and development scenarios must be deterministic. Do not scan seed ranges,
  retry random outcomes, or rely on probability to reach the required state. When
  a scenario needs specific Cards, use a fixed Rules Engine-owned deck order and
  still reach the state through normal start, command, event, and replay paths.
- Tests can use bun api as possible.
- Do not use manual visual inspection or screenshot comparison as acceptance
  validation. Prefer repeatable Playwright assertions against routes, DOM state,
  accessible roles and names, focus, and element geometry.
- Run Worker/Durable Object browser flows with `bun test:e2e`. Tests start
  their own server by default and fail if its assigned port is occupied.
  `PLAYWRIGHT_REUSE_SERVER=1` explicitly reuses a compatible local E2E server,
  including its build and database; CI always starts its own server.
- For worktree setup, port allocation, or Codex Cloud initialization, follow
  `../../docs/worktree-setup.md`.
- Keep the browser choice local in the untracked `.env`: `PLAYWRIGHT_BROWSER`
  accepts `chromium`, `firefox`, or `webkit`; `PLAYWRIGHT_BROWSER_PATH` supplies
  a local executable when needed. The Playwright configuration explicitly loads
  `.env`; neither setting belongs in committed test configuration.
- A Playwright `click()` waits for the browser click action, not for an async Vue
  handler's command request to commit. Before reload or reconnect assertions,
  wait for the specific `/commands` response and identify it by request action
  type; otherwise navigation can race a valid command.
- When an E2E state assertion fails despite correct API JSON, inspect retained
  trace console errors before changing replay or persistence. A Vue render
  exception can hide state that is present in command responses, refresh
  responses, and WebSocket messages.
- Playwright locators are strict. Use the complete accessible name when controls
  share a label, such as `建立房間` and `建立房間 →`.
- Do not keep tombstone assertions whose only purpose is proving that a removed
  button, label, or component has not returned. Prefer positive assertions about
  the current workflow. Assertions about absence or invisibility are appropriate
  only for privacy, security, authorization, mutually exclusive state, or an
  explicit current product contract.
- Playwright treats `aria-disabled="true"` as disabled. Use a forced click only
  when a test deliberately verifies that the handler still refuses to open the
  control; normal workflow tests must use actionable controls.
- Do not manufacture end-of-match assertions by playing dozens of browser
  turns. Replaying the growing canonical record on every command can eventually
  restart the local Worker mid-POST with `503 Your worker restarted
  mid-request`. Use a narrowly scoped development-only endgame fixture, then
  verify the final action, outcome, and reset through normal UI controls. The
  current online-room fixture is `POST /api/games/:id/test-endgame`; production
  and staging must continue to return 404 for test-only routes.
- After clicking `開始遊戲`, wait for an active-game DOM marker such as the
  `啟用規則` region before invoking fixture or command APIs. The click handler is
  asynchronous; issuing the next request immediately can race the room update
  and receive `409 test fixture requires an active match`.
