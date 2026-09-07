# CFECards Web UI

Nuxt adapter for the CFECards Rust rules engine.

The UI consumes viewer-filtered Public Game State and Public Event Feed data. It should not use canonical Game State or canonical Game Events directly as browser replay sources.

## Setup

From `apps/web`, install dependencies with pnpm. Keep Bun installed for scripts
and tests:

```bash
pnpm install --frozen-lockfile
```

Use `pnpm add`, `pnpm remove`, and `pnpm update` to change dependencies, and
commit the resulting `pnpm-lock.yaml`. Existing `bun run` commands use these
installed dependencies.

Better Auth and its Drizzle adapter are pinned to 1.7.3. Existing databases that
were migrated through Better Auth 1.7.2 must apply `0009_account_provider.sql`
before serving the upgraded Worker: it preserves historical `account.issuer`
values as nullable data, restores the unique `(provider_id, account_id)` key, and
keeps existing account, password, token, and session data.
Keep Vitest within the peer dependency range supported by `@nuxt/test-utils`.

Each Git worktree should install its own `node_modules`; pnpm shares package
contents through its store. Prepare a separate `.env` and local Wrangler storage
in each worktree. The E2E server currently uses port 8727, so run E2E suites
sequentially with `PLAYWRIGHT_REUSE_SERVER=0` to avoid reusing another worktree’s
server. Stop any existing server on that port first.

Create a Cloudflare D1 database, copy its ID into `wrangler.toml`, then apply the authentication schema:

```bash
bunx wrangler d1 create fewfc-auth
bun run db:migrate:local
```

Copy `.env.example` to `.env` and replace the value with a random secret containing at least 32 characters.

## Development Server

Authentication and online room APIs require Cloudflare bindings, so run the Worker-backed development server:

```bash
bun run cf:dev
```

`bun run dev` starts Wrangler directly when the application has already been built. `bun run cf:dev` additionally builds the app and applies local D1 migrations first.

Delete a local room by UUID when the Wrangler development server is stopped:

```bash
bun apps/web/scripts/delete-room.ts ROOM_UUID
# or, from apps/web:
bun run room:delete -- ROOM_UUID
```

The command removes the matching local GameRoom Durable Object storage and D1
room/member indexes. It scans the local Wrangler stores under `apps/web/.wrangler`
and refuses to run while this workspace's Wrangler server is active.

The login UI supports:

- Email and password registration/sign-in.
- Anonymous guest accounts that can later be linked to a permanent account.
- Cookie-backed sessions.

Online rooms support:

- Stable `/rooms/:id` routes that recover the current room after reload.
- Public discovery plus separate private invitation links and short room codes.
- Two-player and four-player team rooms.
- WebSocket-synchronized membership, readiness, game state, and notifications.
- Durable Object WebSocket Hibernation without client polling.

## Production

Build the application for production:

```bash
bun run build:production
```

Locally preview production build:

```bash
bun run preview
```

See `docs/deployment.md` for the Cloudflare deployment path.
