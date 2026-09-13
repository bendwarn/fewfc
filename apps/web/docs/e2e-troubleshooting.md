# E2E Troubleshooting

Read the matching symptom when debugging browser flows or preparing endgame tests.

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
