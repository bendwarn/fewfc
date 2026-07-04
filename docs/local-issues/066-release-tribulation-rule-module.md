# 66 Release the Tribulation Rule Module

## Triage

ready-for-agent

## What to build

Add Tribulation as one complete Theme Rule Module using the official 5.16
rules. Deliver all five Tribulations and Divine Calculation through Formation
matching, atomic multi-Player resolution, canonical replay, Public Views,
Online Game Rooms, and the battlefield, including shared- and Personal-Deck
behavior and interactions with existing Rule Modules.

## Acceptance criteria

- [x] Tribulation is independently selectable, requires Star, Five Directions Legend, and Hero Schools, and is enabled by default in new games and rooms without changing persisted room configurations.
- [x] The Formation Catalog exposes the five Tribulations and 神算 with their published names, categories, rule text, and exact matching rules.
- [x] Every Tribulation is a variable-card-count Special Attack containing exactly its two specified overcoming elements with an effective level sum of at least seven per element; it never satisfies a fixed Formation card-count condition.
- [x] 天雷劫火 resolves its 60-point Attack and one 15-point deduction per Team atomically, supplies the correct Affected Player Set, and triggers Shared Fate only for Teams that actually lose HP.
- [x] 烈風暴雨 applies independent two-turn Gale-Rain Status instances to every unprotected Player, counts each affected Player's Turn Ends under the main duration rules, and blocks only life recovery from Formations performed by that Player.
- [x] 泥石轟流 deducts up to 20 points from every unprotected Shield, becomes an 80-point Attack only when that additional effect deducts at least one Shield point, and routes the Attack according to the target's resulting Shield.
- [x] 裂地崩山 accepts any declared Environment, including the current one, and collects Environment-Element Card discards or hand reveals sequentially from the Next Player through the performer using printed Card elements.
- [x] 裂地崩山 stores every answer in a replayable pending Formation continuation and applies Environment Transfer, discards or reveals, Attack damage, Card movement, and Game Outcome atomically only after the final answer.
- [x] 鏽鐵枯林 processes a shared Deck once or every Personal Deck separately, reveals the applicable top eight Cards, discards printed level-three-or-higher Cards to their origin piles, and records every resulting shuffle through trusted midgame randomness.
- [x] 鏽鐵枯林 integrates with Tailwind shuffle recovery and Divine Calculation correctly: shared Deck processing remains global, while a protected Player's Personal Deck is skipped.
- [x] 神算 grants one exclusive Divine Calculation Status, replaces any previous owner, and the next Tribulation performed by any Player consumes it after applying its protection.
- [x] Divine Calculation reduces only Tribulation Attack damage personally received by its owner, leaves Shield damage unreduced, remains effective against Snow-Treading attacks, and applies its additional-effect immunity to the correct Player-, Team-, Deck-, Shield-, and Environment-owned resources.
- [x] Canonical events, direct execution, replay verification, persisted continuation recovery, Public State, viewer-filtered event history, and reconnect produce identical outcomes without leaking hands, Deck order, or private choice options.
- [x] The battlefield presents accessible Rule Module state, Formation declarations, Environment selection, pending discard choices, Divine Calculation Status, Gale-Rain Status, and resolved event history.
- [x] Any new Rust Web DTO multiword fields serialize with the exact camelCase contract and have serialization contract tests.
- [x] Focused Rust, Web unit, and self-contained two-worker Brave Playwright tests cover all six Formations, both Deck modes, two- and four-Player play, Shield and Countershock cases, Divine Calculation's full immunity matrix, reconnect, game-ending resolutions, and all-enabled compatibility.

## Validation

- `cargo test` and strict Clippy pass, including focused Tribulation, replay,
  persistence, Public View, and all-enabled regressions.
- Web unit tests, Nuxt type checking, production build, and Playwright spec
  discovery pass.
- The self-contained Brave spec covers default/dependency normalization and a
  private, reconnectable 裂地崩山 Environment choice through a development-only
  fixture.
- The Rust suite passes with 12 focused Tribulation integration tests, full
  `cargo test`, and strict Clippy.
- Web unit tests (31), Nuxt type checking, production build, and focused
  Playwright spec discovery pass.
- A fresh Brave run could not start because Wrangler requires sandbox-external
  access to its user preference directory and the escalation was rejected for
  quota reasons.

## Blocked by

None - can start immediately.
