# 61 Release the Dark Glimmer Rule Module

## Triage

ready-for-agent

## What to build

Add Dark Glimmer as one complete Theme Rule Module using the official 5.16
rules. Deliver all five Professions, Dark Formations, Evil and Death Spirits,
Persistent Spirit Skills, and their cross-module interactions with Environment,
Hero Schools, Spirit Power, Void Spirit-Shattering, replay, Online Game Rooms,
and the battlefield.

## Acceptance criteria

- [x] Dark Glimmer requires Spirit transitively in addition to all three Advanced Rule Modules, is independently selectable, and is enabled by default in new games and rooms.
- [x] Every Dark Glimmer Profession transition, inherited ability, Dark Formation, and Environment or 影遁 interaction conforms to the 5.16 PDF.
- [x] Evil and Death Spirits occupy the existing single Spirit slot, use the zero-through-six Spirit Power model, and distinguish transformation from a new two-power summon.
- [x] Mischief calculates from only the Cards actually inspected, and trusted random selection records complete canonical outcomes without leaking other hidden Cards.
- [x] Shared Fate follows an actual Formation-effect deduction from its Death Spirit owner's Team HP, excludes ordinary Attack damage, and cannot recursively trigger itself.
- [x] Void Spirit-Shattering triggers Shared Fate for a surviving Death Spirit, suppresses it for a Death Spirit broken at zero power, and does not retroactively trigger a Spirit summoned by 魔靈復甦.
- [x] Void Spirit-Shattering snapshots initial owners, resolves Spirit changes, revival, Team HP, Shared Fate, and Game Outcome atomically and replayably.
- [x] Focused Rust, Web unit, and self-contained Brave Playwright tests cover all Professions, Dark Formations, both Spirits, two- and four-Player Team-HP interactions, dependencies, and all-enabled compatibility.

## Validation

- `cargo test` and strict Clippy pass, including `tests/dark_glimmer.rs`, trusted random hand selection, two- and four-Player affected sets, atomic shattering, replay, and all-enabled regressions.
- Web unit tests, Nuxt type checking, production build, and Playwright spec discovery pass.
- Catalog contract tests cover all 5 Professions and 10 Dark Formations.
- The complete self-contained Brave suite passes: 19 of 19, including `apps/web/tests/e2e/dark-glimmer.spec.ts`.

## Blocked by

- [#57 Resolve Bloom and Void Spirit-Shattering atomically](057-resolve-bloom-and-void-spirit-shattering.md)
- [#58 Compose Profession and Rule Module catalogs](058-compose-profession-and-rule-module-catalogs.md)
- [#60 Release the Confluence Generation Rule Module](060-release-confluence-generation-rule-module.md)
