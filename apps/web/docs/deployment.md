# Deployment

The Nuxt UI is intended to deploy on Cloudflare.

## Static or Pages-backed preview

Use this path for local-play UI and early public demos.

```bash
bun run generate
```

Deploy the generated output to Cloudflare Pages.

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

Wrangler uses `worker/index.ts` as a thin wrapper around the Nuxt build output. This wrapper exports the default Nuxt Worker handler and the `GameRoom` Durable Object class required by the binding in `wrangler.toml`.

Build and run the Cloudflare Worker locally:

```bash
bun run cf:dev
```

`bun run build` first compiles the Rust rules engine to `worker/wasm/fewfc.wasm`, then runs the Nuxt Cloudflare build. The generated Wasm binary is ignored by git and should be rebuilt in deploy environments.

Deploy staging or production:

```bash
bun run cf:deploy:staging
bun run cf:deploy
```

Durable Objects should own authoritative online Game Records. Browser clients submit Commands and receive viewer-filtered Public Game State and Public Event Feed data.

Current Worker game-room endpoints:

- `POST /api/games` creates or returns a Durable Object game room.
- `GET /api/games/:id` returns the room snapshot and event feed.
- `POST /api/games/:id/commands` records an idempotent command by `commandId`.

The `GameRoom` Durable Object stores metadata, an audit event log, and a snapshot backed by the Rust rules-engine record/state/events returned from the Worker Wasm adapter.

## Local rules-engine bridge

The current Nuxt server route at `server/api/local-game.post.ts` is a local development adapter. It shells out to:

```bash
cargo run --quiet --bin fewfc_local_game_api
```

That route is useful for exercising the real Rust rules engine from the UI without rewriting rule behavior in TypeScript. It is not the Cloudflare deployment adapter because Cloudflare Workers cannot spawn local Cargo processes. Before deploying hosted multiplayer, replace this local bridge with a Worker/Wasm adapter or a dedicated service boundary that runs the same Rust rules-engine API.
