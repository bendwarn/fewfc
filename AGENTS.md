# Agent Instructions

## Development Environment Tips

- This repository is a Rust crate. Prefer `cargo test` for the main validation path unless the task specifically touches package tooling.
- The Nuxt app lives under `apps/web`. Auth and online room APIs need Wrangler/Cloudflare bindings; plain `bun run dev` is only for Nuxt-only UI work.
- For `apps/web` Worker/Durable Object local dev, ensure `.dev.vars` exists and `BETTER_AUTH_SECRET` is at least 32 characters, otherwise Better Auth routes fail with `500 BETTER_AUTH_SECRET must contain at least 32 characters`.
- Wrangler must run under Node, not Bun. If `bun run cf:dev` fails with `Wrangler does not support the Bun runtime`, run `bun run build`, then start Wrangler with Node: `node node_modules/.bin/wrangler dev --env=""`. In Codex Desktop, use `load_workspace_dependencies` if `node` is not on `PATH`.
- Local dev servers may need sandbox escalation to bind localhost ports. If a dev server reports no available port while nothing is reachable, rerun with escalated permissions.
- When supplying a custom `PATH` for Node in Codex Desktop, preserve the Rust toolchain path as well. Playwright's web-server process runs the WASM build and otherwise fails with `cargo: command not found`.

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
- Playwright locators are strict. Use the complete accessible name when controls
  share a label, such as `建立房間` and `建立房間 →`.
- Playwright treats `aria-disabled="true"` as disabled. Use a forced click only
  when a test deliberately verifies that the handler still refuses to open the
  control; normal workflow tests must use actionable controls.
- An in-app browser tab left on a room from different Wrangler storage can produce
  repeated WebSocket 500 logs. Those requests are unrelated to isolated E2E state;
  do not replace automated assertions with manual browser checks to investigate
  them.

## Online Game Commands

- Before changing pending command drafts, pending-choice payloads, or canonical
  events, read the command and replay constraints in `docs/rule.md` and
  `docs/rules-engine-decisions.md`.

## Agent Skills

### Issue tracker

Issues are tracked as local markdown files under `docs/local-issues/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Use the default five-role triage vocabulary unless a local issue explicitly says otherwise. See `docs/agents/triage-labels.md`.

### Domain docs

This is a single-context rules-engine repo; read `CONTEXT.md` and relevant decision docs before domain-sensitive work. See `docs/agents/domain.md`.
