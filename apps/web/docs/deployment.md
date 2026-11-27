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

Wrangler uses `worker/index.ts` as an authenticated WebSocket gateway around the Nuxt build output. It exports the `GameRoom`, `PlayerNotifications`, `ReplayArchive`, and `CompetitionRoom` Durable Object classes declared in `wrangler.toml`.

## Durable Object lifecycle

`wrangler.toml` declares the current Durable Object classes under `exports` for
the default, staging, and production Workers. All four namespaces use SQLite,
matching the original `new_sqlite_classes` entries. Switching from the
Durable Object `migrations` history to `exports` preserves those provisioned
namespaces and their data; it does not change their storage backend.

The `exports` declaration is the current lifecycle source of truth. The first
deployment using it is a one-way switch from Durable Object `migrations`;
review Wrangler's reconciliation output before confirming that deployment.
Afterward, apply lifecycle changes with `wrangler deploy`. Do not use
`wrangler versions upload` or gradual deployment for Durable Object lifecycle
changes. D1 migrations remain configured separately through `migrations_dir`.

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

`MAINTENANCE_MODE` is a Worker variable managed separately for each deployed
environment. CI uses `wrangler deploy --keep-vars` to preserve the
operator-managed value across code deployments, keeping the maintenance state
under operator control.

Before deployment:

1. Verify the staging and production D1 IDs in `wrangler.toml`.
2. Replace `BETTER_AUTH_URL` with the actual production and staging origins.
3. Configure `BETTER_AUTH_SECRET` for each environment.

`bun run build` first compiles the Rust rules engine to `worker/wasm/fewfc.wasm`, then runs the Nuxt Cloudflare build. The generated Wasm binary is ignored by git and should be rebuilt in deploy environments.

Deployments run through the protected CI/CD workflow. A push to `main` runs the
normal CI checks and, after they pass, automatically builds and deploys the
staging Worker. Production is not part of the push path: start the same
workflow manually with its `deploy_production` input enabled. Both code paths
apply pending migrations and deploy one normal Worker version with
`wrangler deploy --keep-vars`, preserving the separately managed maintenance
secret.

## GitHub Actions CI/CD

Local dependency management remains pnpm (see `../README.md`), and dependency
changes must update `pnpm-lock.yaml`. GitHub Actions uses the stable
`pnpm/action-setup` action for pnpm and `oven-sh/setup-bun` for the repository's
Bun-based scripts. Each job installs from the checked-in lockfile with frozen
arguments passed through `run_install`. The `Web checks` job runs Storybook and
browser checks and writes the pnpm cache; deployment jobs wait for both Rust and
Web checks.

Wrangler is pinned to `4.151.3` in `package.json`. Code deployment uses its
standard `deploy` command.

`.github/workflows/ci-cd.yml` is the deployment source of truth:

- Pull requests and pushes to `main` run Rust tests, Storybook render and
  interaction tests, the Storybook build, Web unit tests and type checking, and
  the Worker-backed Playwright E2E suite. Storybook visual acceptance and the
  responsibilities of each test layer are documented in
  [Storybook](storybook.md#validation-scope); the current CI commands do not run
  automated screenshot comparisons.
- A successful push to `main` deploys `staging` automatically after the normal
  Rust, Web, browser, and type checks.
- Production deploys only from a manually dispatched run with
  `deploy_production` enabled; a push to `main` never deploys production.
- Each deployment builds the environment-specific Nuxt/Wasm output once,
  applies pending remote D1 migrations, and deploys one Worker version with
  `wrangler deploy --keep-vars`.
- `MAINTENANCE_MODE` is an operator-controlled Worker variable. CI uses
  `wrangler deploy --keep-vars` to preserve its value throughout code deployments.

Create GitHub Environments named `staging` and `production`. Add these encrypted
secrets to both environments:

- `CLOUDFLARE_ACCOUNT_ID`
- `CLOUDFLARE_API_TOKEN`
- `BETTER_AUTH_SECRET`

Use different `BETTER_AUTH_SECRET` values for staging and production. The
Cloudflare token needs Workers Scripts read and write access, D1 edit access,
and Account Settings read access, scoped to the deployment account. Add Workers Routes write
access for the relevant zone if a custom domain is managed by Wrangler.

Before enabling automatic deployment, replace both placeholder
`BETTER_AUTH_URL` values in `wrangler.toml` with the actual HTTPS origins.

## Social sign-in

Set each provider as a Worker secret only when it should be available. A provider
is hidden from the login and account-settings screens unless both variables are
present:

- `GOOGLE_CLIENT_ID` and `GOOGLE_CLIENT_SECRET`
- `GITHUB_CLIENT_ID` and `GITHUB_CLIENT_SECRET`

Register these callback URLs with Google and GitHub, replacing the origin for
each environment:

- `https://<origin>/api/auth/callback/google`
- `https://<origin>/api/auth/callback/github`

For example, use `bunx wrangler secret put GOOGLE_CLIENT_ID --env staging` and
the corresponding secret command for every provider value. Do not place client
secrets in `wrangler.toml` or public runtime configuration.

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
