# Deployment

The Nuxt UI is intended to deploy on Cloudflare.

## Worker-backed deployment

Use this path once the UI needs server routes or a game-room API.

The app is configured for the Cloudflare Workers module preset:

```ts
export default defineNuxtConfig({
  nitro: {
    preset: 'cloudflare-module',
  },
})
```

Wrangler uses `worker/index.ts` as an authenticated WebSocket gateway around the Nuxt build output. It exports the `GameRoom` and `PlayerNotifications` Durable Object classes required by `wrangler.toml`.

Build and run the Cloudflare Worker locally:

```bash
bunx wrangler d1 create fewfc-auth
# Copy the returned database_id into wrangler.toml.
bun run db:migrate:local
cp .env.example .env
bun run cf:dev
```

`APP_ENV` is the shared environment policy input for the Nuxt build and Worker
runtime. Nuxt build scripts load it through `.env.development`, `.env.staging`,
or `.env.production`; Wrangler injects the matching value through `vars` in the
default, `staging`, or `production` configuration. A missing or unsupported
value fails the build or request.

`BETTER_AUTH_SECRET` must contain at least 32 random characters. Keep it in `.env` locally and store it as a Worker secret in deployed environments:

```bash
bunx wrangler secret put BETTER_AUTH_SECRET
bunx wrangler secret put BETTER_AUTH_SECRET --env staging
bunx wrangler secret put BETTER_AUTH_SECRET --env production
```

## Legacy game cutover (issue #75)

The independent Initial Pouch Selection release deliberately does not read old
Game Records or Replays. The one-time purge is manual, dry-run-first, and is
never run by deployment, migration, or tests. It preserves waiting-room identity
and configuration; it resets only active and finished matches.

Before the cutover, configure a dedicated `LEGACY_PURGE_SECRET` for both
environments. Do not put its value in a file or command argument:

```bash
bunx wrangler secret put LEGACY_PURGE_SECRET --env staging
bunx wrangler secret put LEGACY_PURGE_SECRET --env production
```

The management script reuses the local Wrangler login instead of requiring a
`CLOUDFLARE_API_TOKEN`. Authenticate and confirm the intended account before
the dry run:

```bash
bunx wrangler login
bunx wrangler whoami
```

The script reads the selected `[env.staging]` or `[env.production]` block of
the checked-in `wrangler.toml` for the Worker name, `BETTER_AUTH_URL`, and the
`DB` D1 binding. It then resolves the two Durable Object namespace IDs through
Cloudflare's read-only namespace listing by the configured Worker and
`GAME_ROOM`/`REPLAY` class names. The script obtains its account from `wrangler
whoami --json` and temporary Cloudflare authorization from `wrangler auth token
--json`; both remain only in memory and are never printed. If the logged-in
profile belongs to multiple accounts, set the non-secret
`CLOUDFLARE_ACCOUNT_ID` to select the intended account explicitly. Set only
`LEGACY_PURGE_SECRET` in the operator's environment; do not set
`FEWFC_*` target variables.

Run staging first. Deploy the release with `MAINTENANCE_MODE=true` as explicit
Worker configuration, then take and review a dry-run inventory:

```bash
bun run build:staging
wrangler deploy --env staging --var MAINTENANCE_MODE:true
bun run purge:legacy-games --env staging --epoch issue-75-2026-08-11
```

Only after confirming room identities and object counts, run the mutation and
its idempotency verification. A successful second confirmed run reports
`mutationCount: 0`:

```bash
bun run purge:legacy-games --env staging --epoch issue-75-2026-08-11 --confirm
bun run purge:legacy-games --env staging --epoch issue-75-2026-08-11 --confirm
```

Keep maintenance enabled if either command fails. The script verifies that no
Game Record remains in enumerated rooms, active/finished rooms are waiting, both
replay D1 tables are empty, and every enumerated ReplayArchive is empty. Reopen
traffic only after that verification succeeds:

```bash
wrangler deploy --env staging --var MAINTENANCE_MODE:false
```

Repeat the same sequence with `production` only after staging verification is
complete. The script follows the Cloudflare Durable Objects Objects API cursors
for both namespaces, so ReplayArchives without a surviving D1 reference are
included. It deletes only `player_saved_replay` and `replay_archive_lifecycle`;
account, profile, Deck List, and public-room index rows are preserved.

CI/CD supplies this secret from the matching GitHub Environment instead. Do not
commit the value to this repository.

Before deployment:

1. Verify the staging and production D1 IDs in `wrangler.toml`.
2. Replace `BETTER_AUTH_URL` with the actual production and staging origins.
3. Configure `BETTER_AUTH_SECRET` for each environment.
4. Apply migrations to each remote database.

```bash
bun run db:migrate:staging
bun run db:migrate:production
```

`bun run build` first compiles the Rust rules engine to `worker/wasm/fewfc.wasm`, then runs the Nuxt Cloudflare build. The generated Wasm binary is ignored by git and should be rebuilt in deploy environments.

Deploy staging or production:

```bash
bun run cf:deploy:staging
bun run cf:deploy:production
```

There is intentionally no unqualified deployment command. Each deployment command
validates that its origin and D1 ID no longer contain repository placeholders.
The commands build first, then apply the matching remote D1 migrations immediately
before deploying the Worker.

## GitHub Actions CI/CD

`.github/workflows/ci-cd.yml` is the deployment source of truth:

- Pull requests and pushes to `main` run Rust tests, Web unit tests and type
  checking, and the Worker-backed Playwright suite.
- A successful push to `main` deploys `staging` automatically.
- `production` is deployed from `main` through the workflow's manual
  `workflow_dispatch` action. Protect the GitHub `production` Environment with
  required reviewers.
- Each deployment builds the environment-specific Nuxt/Wasm output, uploads
  `BETTER_AUTH_SECRET`, applies pending remote D1 migrations, and then deploys
  the Worker.

Create GitHub Environments named `staging` and `production`. Add these encrypted
secrets to both environments:

- `CLOUDFLARE_ACCOUNT_ID`
- `CLOUDFLARE_API_TOKEN`
- `BETTER_AUTH_SECRET`

Use different `BETTER_AUTH_SECRET` values for staging and production. The
Cloudflare token needs Workers Scripts write access, D1 edit access, and Account
Settings read access, scoped to the deployment account. Add Workers Routes write
access for the relevant zone if a custom domain is managed by Wrangler.

Before enabling automatic deployment, replace both placeholder
`BETTER_AUTH_URL` values in `wrangler.toml` with the actual HTTPS origins.

Durable Objects should own authoritative online Game Records. Browser clients submit Commands and receive viewer-filtered Public Game State and Public Event Feed data.

Current Worker game-room endpoints:

- `GET /api/games` returns joinable public rooms and the authenticated player's rooms.
- `POST /api/games` creates a two-player or four-player public/private room.
- `GET /api/games/:id` returns a viewer-filtered room snapshot derived from the authenticated user's seat.
- `POST /api/games/:id/join` joins a public room or redeems a private invitation token.
- `POST /api/games/join` resolves and redeems a human-entered room code.
- `POST /api/games/:id/ready` toggles a non-owner player's readiness.
- `POST /api/games/:id/start` atomically validates connected/ready players and starts the match.
- `POST /api/games/:id/leave`, `/remove`, and `/dissolve` manage waiting-room membership.
- `POST /api/games/:id/commands` records an idempotent command by `commandId` and derives its Player from the authenticated room membership.
- `GET /api/games/:id/socket` pushes viewer-filtered room and game state.
- `GET /api/notifications/socket` pushes transient player notifications and public-room invalidations.

The browser cannot choose its own `player` or `viewer` identity. Nitro resolves the Better Auth session, and the `GameRoom` Durable Object maps the immutable account ID to a game Player seat.

The D1 database stores Better Auth data, player profiles, the room discovery index, and player-to-room membership. `GameRoom` stores authoritative room metadata, an audit event log, WebSocket sessions, and the Rust rules-engine record. `PlayerNotifications` owns the single global WebSocket channel used for targeted notifications and room-list invalidation.

## Player routes

- `/login` handles account and guest authentication.
- `/rooms?tab=create|join` displays the room lobby.
- `/rooms/:id` restores a member's current waiting, active, or completed room state.
- `/rooms/:id?invite=:token` redeems a private invitation and then removes the token
  from the URL.

## Development rules-engine bridge

The current Nuxt server route at `server/api/local-game.post.ts` is a local development adapter. It shells out to:

```bash
cargo run --quiet --bin fewfc_local_game_api
```

The route returns `404` unless `APP_ENV=development`. It is not used by the general
player UI. Hosted multiplayer uses the same Rust API compiled to
`worker/wasm/fewfc.wasm`; Cloudflare does not spawn Cargo processes at runtime.
