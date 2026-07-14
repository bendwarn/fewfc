# CFECards Web UI

Nuxt adapter for the CFECards Rust rules engine.

The UI consumes viewer-filtered Public Game State and Public Event Feed data. It should not use canonical Game State or canonical Game Events directly as browser replay sources.

## Setup

Make sure to install dependencies:

```bash
bun install
```

Create a Cloudflare D1 database, copy its ID into `wrangler.toml`, then apply the authentication schema:

```bash
bunx wrangler d1 create fewfc-auth
bun run db:migrate:local
```

Copy `.dev.vars.example` to `.dev.vars` and replace the value with a random secret containing at least 32 characters.

## Development Server

Authentication and online room APIs require Cloudflare bindings, so run the Worker-backed development server:

```bash
bun run cf:dev
```

Plain `bun run dev` remains useful for Nuxt-only work, but D1 authentication routes return an unavailable-binding error outside Wrangler.

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
