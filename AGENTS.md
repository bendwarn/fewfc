# Agent Instructions

## Development Environment Tips

- This repository is a Rust crate. Prefer `cargo test` for the main validation path unless the task specifically touches package tooling.
- The Nuxt app lives under `apps/web`. Auth and online room APIs need Wrangler/Cloudflare bindings; plain `bun run dev` is only for Nuxt-only UI work.
- For `apps/web` Worker/Durable Object local dev, ensure `.dev.vars` exists and `BETTER_AUTH_SECRET` is at least 32 characters, otherwise Better Auth routes fail with `500 BETTER_AUTH_SECRET must contain at least 32 characters`.
- Local dev servers may need sandbox escalation to bind localhost ports. If a dev server reports no available port while nothing is reachable, rerun with escalated permissions.

## Browser and E2E Validation

- Do not use manual visual inspection or screenshot comparison as acceptance
  validation. Prefer repeatable Playwright assertions against routes, DOM state,
  accessible roles and names, focus, and element geometry.
- Run Worker/Durable Object browser flows with `cd apps/web && bun run test:e2e`.
  The Playwright configuration builds Nuxt and the rules WASM, applies migrations
  to isolated `.wrangler/e2e` storage, starts Wrangler, and launches Brave when it
  is installed.
- `PLAYWRIGHT_REUSE_SERVER=1` is only for local iteration after intentionally
  starting the matching E2E server. The default must remain a self-contained
  server lifecycle so a normal development server cannot make tests pass by
  accident.
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
- 允許在沙箱外跑 E2E

## Online Game Commands

- Before changing pending command drafts, pending-choice payloads, or canonical
  events, read the command and replay constraints in `docs/rule.md` and
  `docs/rules-engine-decisions.md`.
- Rust Web DTO fields consumed by TypeScript must serialize with the exact
  camelCase contract. Add a serialization contract test for new multiword
  action fields; Rust field names otherwise default to snake_case and can turn
  a valid UI action into a server error.

## Agent Skills

### Issue tracker

Issues are tracked as local markdown files under `docs/local-issues/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Use the default five-role triage vocabulary unless a local issue explicitly says otherwise. See `docs/agents/triage-labels.md`.

### Domain docs

This is a single-context rules-engine repo; read `CONTEXT.md` and relevant decision docs before domain-sensitive work. See `docs/agents/domain.md`.
