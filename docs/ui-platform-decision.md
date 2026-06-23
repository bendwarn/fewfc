# UI Platform Decision

## Decision

Build the website UI with Nuxt, backed by the existing Rust rules engine.

The Rust crate remains the authoritative rules engine for validation, event emission, replay, hidden information, and Public View derivation. Nuxt is an adapter layer for presentation, local interaction, and later online play.

## Frontend Stack

- Nuxt with Vue and TypeScript for the website application.
- Rust-to-Wasm package for deterministic local rules execution where useful.
- Server API routes or a Cloudflare Worker boundary for authoritative online games.
- Public Game State and Public Event Feed DTOs as the UI-facing data model.

Do not expose canonical Game State or canonical Game Events directly to browser clients as replay sources. The UI consumes viewer-filtered Public View data and submits Commands.

## Deployment Platform

Use Cloudflare as the primary deployment target.

- Nuxt can deploy to Cloudflare Pages with zero configuration for static or Pages-backed deployments.
- Nitro supports Cloudflare Workers with the `cloudflare_module` preset.
- Cloudflare Durable Objects are the preferred coordination point for online multiplayer game rooms.

## Phases

### Phase 1: Local Play UI

- Add a Nuxt app under `apps/web`.
- Add a Wasm adapter package around this crate.
- Let the UI start a game, submit Commands, advance automatic resolution, and render Public Game State.
- Persist local Game Records with import/export JSON until server persistence exists.

### Phase 2: Hosted Game API

- Add an authoritative server boundary that owns each Game Record.
- Accept player Commands from clients.
- Validate Commands through the Rust rules engine.
- Append accepted Game Events to the canonical event log.
- Return viewer-filtered Public Game State and Public Event Feed data.

### Phase 3: Online Multiplayer

- Map each game room to a Cloudflare Durable Object.
- Use Durable Object storage for canonical Game Records.
- Use WebSocket or server-sent event fanout for viewer-filtered updates.
- Keep all hidden information filtering server-side.

## Constraints

- UI code must depend on public DTOs or adapter DTOs, not internal domain mutation APIs.
- Replay remains event-log authoritative.
- Hidden information filtering remains a boundary concern.
- The Rust domain layer must not depend on Nuxt, Nitro, Cloudflare, or browser APIs.
