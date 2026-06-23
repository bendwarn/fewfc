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
