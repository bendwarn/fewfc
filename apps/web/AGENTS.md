# Web App Agent Instructions

## Development Environment

- bun as default
- Development is the default environment. The `build`, `postinstall`, and
  `typecheck` package scripts intentionally rely on Nuxt's default `.env`
  loading instead of passing `--dotenv .env.development`. For staging or
  production, use the corresponding environment-specific script, which must
  explicitly select `.env.staging` or `.env.production`.
- For Worker/Durable Object local development, ensure `.env` exists and `BETTER_AUTH_SECRET` is at least 32 characters, otherwise Better Auth routes fail with `500 BETTER_AUTH_SECRET must contain at least 32 characters`.
- Local development servers may need sandbox escalation to bind localhost ports. If a server reports no available port while nothing is reachable, rerun with escalated permissions.

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
- Run Worker/Durable Object browser flows with `bun run test:e2e`. The Playwright
  configuration builds Nuxt and the rules WASM, applies migrations to isolated
  `.wrangler/e2e` storage, starts Wrangler, and launches Brave when it is installed.
- A Playwright `click()` waits for the browser click action, not for an async Vue
  handler's command request to commit. Before reload or reconnect assertions,
  wait for the specific `/commands` response and identify it by request action
  type; otherwise navigation can race a valid command.
- When an E2E state assertion fails despite correct API JSON, inspect retained
  trace console errors before changing replay or persistence. A Vue render
  exception can hide state that is present in command responses, refresh
  responses, and WebSocket messages.
- The suite is validated with two Playwright workers and uses that as the
  default. Do not increase parallelism further without isolating D1/Durable
  Object persistence per worker or proving the full suite stable, because tests
  share one local Worker and database.
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
