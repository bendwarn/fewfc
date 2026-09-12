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

## Legacy game cutover (schema 7)

Schema 7 centralizes card supply and the serialized pending rule flow. It does
not read, migrate, or replay schema 6 Game Records. The one-time purge is
manual, dry-run-first, and is never run by deployment, migration, or tests. It
only removes GameRoom Durable Objects whose management-only authoritative probe
returns either `404 { status: "absent" }` or the explicit `500 { status: "broken" }`
classification, and removes only its matching stale public-room index and
membership rows. A healthy room is preserved exactly as-is. Ordinary
player-facing `401`/`403` responses, authentication failures, timeouts,
malformed responses, and arbitrary transport/server errors never qualify a
room for deletion.

The Worker management route requires maintenance mode and a short-lived Wrangler
token verified against the configured Cloudflare account. The account ID selects
the verification scope; it is not itself a credential.

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
`CLOUDFLARE_ACCOUNT_ID` to select the intended account explicitly. The same
account ID must be injected into the deployed Worker as a variable by the
protected deployment configuration; it is never sufficient as a credential by
itself. Do not set `FEWFC_*` target variables.

Run staging first. Use the **Set Worker maintenance mode** GitHub Action with
`staging` and `enable` (preferred). It promotes the maintenance version paired
with the version currently serving 100% traffic; it does not build or migrate.
Then take and review
a dry-run inventory:

```bash
bun run build:staging
bun run purge:legacy-games --env staging --epoch schema-7-2026-08-26
```

The dry-run sends read-only `room-probe` requests for every indexed room and
stored GameRoom object. Its `candidateRooms` output lists every candidate room
identity and classification; it includes only explicit `absent` or `broken`
responses. Malformed responses fail closed and no mutation is attempted. The
CLI forwards the short-lived Wrangler token only to the configured Worker URL,
using the management header; there is no `LEGACY_PURGE_SECRET`.

Only after confirming room identities and object counts, run the mutation and
its idempotency verification. A GameRoom is deleted only when its management-only
authoritative probe returns the explicit `404 { status: "absent" }` or
`500 { status: "broken" }` response. The same check is applied to each D1
public-room row before removing a stale index row. A successful probe returns
`200 { status: "preserved" }` and leaves the DO and index untouched. Replay
archives and D1 replay rows are also preserved unless the archive's own
`sourceGameId` matches a room explicitly deleted in this run; only then may the
archive and matching D1 references be removed.
A successful second confirmed run reports `mutationCount: 0`:

```bash
bun run purge:legacy-games --env staging --epoch schema-7-2026-08-26 --confirm
bun run purge:legacy-games --env staging --epoch schema-7-2026-08-26 --confirm
```

Keep maintenance enabled if either command fails. The script verifies that
preserved room identities still match the dry-run inventory, every removed room
object returns 404 after deletion, every preserved room remains readable,
readable Replay archives and unrelated D1 rows remain, and every explicitly
associated deleted Replay has no archive or selected D1 reference. Reopen
traffic only after that verification succeeds using the GitHub Action with
`staging` and `disable`.

Repeat the same sequence with `production` only after staging verification is
complete. The script follows the Cloudflare Durable Objects Objects API cursors
for both namespaces, so ReplayArchives without a surviving D1 reference are
included. It deletes only Replay data provably associated with explicitly
deleted rooms; account, profile, Deck List, readable Replay, and readable
public-room index rows are preserved.

### GitHub Actions maintenance switch

Use **Actions → Set Worker maintenance mode → Run workflow** from `main` to
choose `staging` or `production` and `enable`, `disable`, or the one-time
`bootstrap-normal` option. The workflow uses
the matching GitHub Environment, so its required reviewers and environment
secrets still apply. It resolves the active 100% deployment and all deployable
versions, then promotes exactly its same-SHA paired version with
`wrangler versions deploy`. It never builds, migrates, or runs the purge script;
untagged, split, unknown, or unpaired deployments fail closed.

Select `enable` before the dry-run and confirmed purge. Select `disable` only
after the script's verification succeeds. Ordinary CI refuses to run while
maintenance is active; it resolves the active deployment first and fails closed
instead of reopening traffic.

For the one-time adoption of a Worker that predates paired tags, choose
`bootstrap-normal` instead. Before entering the required acknowledgement
`BOOTSTRAP_NORMAL_VERSION`, verify in the dashboard or through the last known
deployment that maintenance is currently disabled. Bootstrap is deliberately a
separate, protected operation: it validates and builds `main`, applies pending
migrations, uploads the normal/maintenance pair, then promotes normal. It is
the only workflow path allowed to replace an untagged active Worker; ordinary
CI and `enable`/`disable` remain fail-closed.

Before deployment:

1. Verify the staging and production D1 IDs in `wrangler.toml`.
2. Replace `BETTER_AUTH_URL` with the actual production and staging origins.
3. Configure `BETTER_AUTH_SECRET` for each environment.

`bun run build` first compiles the Rust rules engine to `worker/wasm/fewfc.wasm`, then runs the Nuxt Cloudflare build. The generated Wasm binary is ignored by git and should be rebuilt in deploy environments.

Deployments run only through the protected CI/CD workflow. The former
`cf:deploy:staging` and `cf:deploy:production` commands now fail deliberately:
an ordinary `wrangler deploy` would create an unpaired version and make a later
maintenance promotion ambiguous. The deployment workflow validates its origin
and D1 ID, applies migrations, uploads the paired versions, and promotes normal
in one protected sequence. An existing untagged Worker requires the explicit
`bootstrap-normal` procedure above before automatic CI can take over.

## GitHub Actions CI/CD

Local dependency management remains pnpm (see `../README.md`), and dependency
changes must update `pnpm-lock.yaml`. GitHub Actions uses the stable
`pnpm/action-setup` action for pnpm and `oven-sh/setup-bun` for the repository's
Bun-based scripts. Each job installs from the checked-in lockfile with frozen
arguments passed through `run_install`. The `Web checks` job is the pnpm cache
writer; browser tests wait for it, and deployment jobs follow the browser tests
so parallel jobs do not race while creating the same cache.

Wrangler is pinned to `4.130.0` in `package.json`. The paired-version workflows
use its `versions upload` and `versions deploy` commands rather than the
traffic-changing `wrangler deploy` command.

`.github/workflows/ci-cd.yml` is the deployment source of truth:

- Pull requests and pushes to `main` run Rust tests, Web unit tests and type
  checking, and the Worker-backed Playwright suite.
- A successful push to `main` deploys `staging` automatically.
- A successful push to `main` deploys `production` automatically after the same
  run's `staging` deployment succeeds. The `production` Environment must not
  require reviewers or a wait timer; its branch policy may remain enabled.
- Each deployment builds the environment-specific Nuxt/Wasm output once,
  applies pending remote D1 migrations, uploads immutable normal and maintenance
  versions tagged `fewfc-<commit-sha>-normal` and
  `fewfc-<commit-sha>-maintenance`, and promotes normal to 100%.
- A preflight resolves the active deployment by exact version ID before any
  migration or upload. It rejects active maintenance, untagged, split, or
  otherwise unknown state, so an ordinary push cannot clear maintenance mode.

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
