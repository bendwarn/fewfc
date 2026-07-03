# 65 Release the Echo Rule Module

## Triage

ready-for-agent

## What to build

Release the complete Echo Theme Rule Module through official configuration,
canonical and viewer-filtered Web contracts, Online Game Rooms, and the
battlefield. Make every Melody, Echo Cost, Turn Start continuation, schedule,
choice, and trusted shuffle usable and recoverable through direct play,
reconnect, event history, and self-contained Brave E2E flows.

## Acceptance criteria

- [x] Echo becomes an available Theme Rule Module only after all seven Melodies and their interactions are complete.
- [x] Echo requires Star, Five Directions Legend, and Hero Schools; enabling Echo normalizes those dependencies, disabling any dependency disables Echo, and Personal Deck remains optional.
- [x] New games and rooms enable Echo by default with every available Rule Module, while existing stored rooms retain their saved module list.
- [x] Rule Module changes retain readiness and Locked Deck List invalidation behavior and never infer Echo for an already active game.
- [x] Public State shows Scheduled Echo, 植土 schedules, Flow State layers, public 裂土 suppression, and whether resolution is waiting for a Player or trusted randomness.
- [x] Pending Choice exposes its Player and purpose publicly but sends private candidate Cards or other sensitive options only to the answering Player.
- [x] Pending Randomness exposes only that the server is resolving a shuffle; no viewer receives the submitted order through Public State or Public Event Feed.
- [x] Selected 淨火 targets, 裂土 Formations, 植土 Melodies, revealed 鳴金 Cards, Echo Cost Discards, lifecycle outcomes, and expiry are represented in viewer-safe event history.
- [x] The battlefield offers accessible controls for Card, Player, Formation, Melody, and Decline answers and blocks ordinary actions until every continuation is complete.
- [x] Reconnect during Echo Cost, 裂土 selection, 淨火 target selection, 植土 selection, Pending Randomness, and resumed Turn Start restores the exact actionable control without duplicating commands or events.
- [x] Worker restart or retry during 鳴金 shuffle cannot expose, lose, duplicate, or reorder Cards and cannot advance to Echo Cost or Main before the trusted shuffle commits.
- [x] Rust-to-TypeScript multiword action and event fields serialize with the exact camelCase contract and have focused contract tests.
- [x] Direct execution, replay, recorded-decision verification, Public Views, Web commands, event presentation, reconnect, and final Game Outcome agree for every Melody in shared- and Personal-Deck games.
- [x] Focused Rust, Web unit, Nuxt type checking, production build, and self-contained two-worker Brave Playwright tests cover default/dependency configuration, two- and four-Player play, all choices, both Deck modes, hidden information, reconnect, Worker retry, Seal, 裂土, 淨火, game over, and all-enabled compatibility.

## Validation

- `cargo test` and strict Clippy pass; the Echo suite contains 24 focused
  integration tests and the Web adapter contains focused contract tests.
- Web unit tests (30), Nuxt type checking, and the production Nuxt build pass.
- Self-contained Brave scenarios cover default/dependency normalization and a
  private, reconnectable 淨火 Player choice through a development-only fixture.
- A fresh Brave run could not be executed because the environment previously
  rejected the required sandbox escalation for quota reasons.

## Blocked by

- [#64 Resolve Pure Fire and Plant Earth](064-resolve-pure-fire-and-plant-earth.md)
