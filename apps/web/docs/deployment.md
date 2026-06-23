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

Configure Nitro for the Cloudflare Workers module preset:

```ts
export default defineNuxtConfig({
  nitro: {
    preset: 'cloudflare_module',
  },
})
```

Durable Objects should own authoritative online Game Records. Browser clients submit Commands and receive viewer-filtered Public Game State and Public Event Feed data.

## Local rules-engine bridge

The current Nuxt server route at `server/api/local-game.post.ts` is a local development adapter. It shells out to:

```bash
cargo run --quiet --bin fewfc_local_game_api
```

That route is useful for exercising the real Rust rules engine from the UI without rewriting rule behavior in TypeScript. It is not the Cloudflare deployment adapter because Cloudflare Workers cannot spawn local Cargo processes. Before deploying hosted multiplayer, replace this local bridge with a Worker/Wasm adapter or a dedicated service boundary that runs the same Rust rules-engine API.
