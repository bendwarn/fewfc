# 34 Scaffold Nuxt UI adapter

## Type

AFK

## Parent

Derived from `docs/ui-platform-decision.md`.

## What to build

Create the first Nuxt website adapter around the rules engine without changing the Rust domain boundary. The UI should make the current Public Game State inspectable and provide enough controls to start a local game, submit legal Commands, advance automatic resolution, and inspect recent Public Event Feed entries.

## Acceptance criteria

- [x] Add a Nuxt app under `apps/web`.
- [x] Use TypeScript for UI-facing DTOs.
- [x] Add a rules-engine adapter boundary that can later be backed by Rust Wasm or a server API without rewriting Vue components.
- [x] Render current phase, current player, turn number, teams/HP, hands as own cards or hidden counts, discard, covered passives, pending choice, shields, statuses, and game status.
- [x] Provide local-play controls for starting a sample game, submitting an action pass, choosing turn discard when required, and advancing automatic resolution.
- [x] Let a player select hand cards, show playable formations, and submit a formation by clicking the formation option without a separate play button.
- [x] Show a recent viewer-filtered Public Event Feed.
- [x] Do not expose canonical Game State or canonical Game Events directly as browser replay sources.
- [x] Add a documented Cloudflare deployment path using Nuxt/Nitro presets.
- [ ] Add focused UI tests or component tests for hidden-card rendering and pending-choice rendering.

## Blocked by

- None - can start immediately
