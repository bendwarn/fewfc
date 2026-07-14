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
cp .dev.vars.example .dev.vars
bun run cf:dev
```

`APP_ENV` is the shared environment policy input for the Nuxt build and Worker
runtime. Nuxt build scripts load it through `.env.development`, `.env.staging`,
or `.env.production`; Wrangler injects the matching value through `vars` in the
default, `staging`, or `production` configuration. A missing or unsupported
value fails the build or request.

`BETTER_AUTH_SECRET` must contain at least 32 random characters. Keep it in `.dev.vars` locally and store it as a Worker secret in deployed environments:

```bash
bunx wrangler secret put BETTER_AUTH_SECRET
bunx wrangler secret put BETTER_AUTH_SECRET --env staging
bunx wrangler secret put BETTER_AUTH_SECRET --env production
```

Before deployment:

1. Replace the D1 placeholder IDs in `wrangler.toml`.
2. Replace `BETTER_AUTH_URL` with the actual production and staging origins.
3. Apply migrations to each remote database.
4. Configure `BETTER_AUTH_SECRET` for each environment.

```bash
bun run db:migrate:remote
bunx wrangler d1 migrations apply fewfc-auth-staging --remote --env staging
bunx wrangler d1 migrations apply fewfc-auth-production --remote --env production
```

`bun run build` first compiles the Rust rules engine to `worker/wasm/fewfc.wasm`, then runs the Nuxt Cloudflare build. The generated Wasm binary is ignored by git and should be rebuilt in deploy environments.

Deploy staging or production:

```bash
bun run cf:deploy:staging
bun run cf:deploy:production
```

There is intentionally no unqualified deployment command. Each deployment command
validates that its origin and D1 ID no longer contain repository placeholders.

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
