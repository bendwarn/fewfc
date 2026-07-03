# 59 Release the Jianghu Rule Module

## Triage

ready-for-agent

## What to build

Add Jianghu as one complete Theme Rule Module using the official 5.16 rules.
Deliver all nine Professions, their Profession Changes, abilities, and
Formations; model 千鋒, 踏雪, and 中毒 as typed Jianghu States; and expose the
complete behavior through canonical replay, Public Views, playable Actions,
Online Game Rooms, and the battlefield.

## Acceptance criteria

- [x] Jianghu is independently selectable, requires all three Advanced Rule Modules, and is enabled by default in new games and rooms.
- [x] Every Jianghu Profession transition, inherited ability, Activated Profession Ability, Automatic Profession Ability, and Profession Formation conforms to the 5.16 PDF.
- [x] 千鋒 and 踏雪 apply immediately, retain their published duration and stacking behavior, and integrate at the correct Attack-resolution stages.
- [x] 中毒 aggregates remaining turns, deducts HP at the affected Player's Turn End, blocks only Formation-provided recovery, and applies 毒絕 from the unique Previous Player source.
- [x] 星行牌 reads the printed element matching the performing Player's Team Star, and 流星步 uses that definition without Star Element Substitution.
- [x] Canonical events, replay, verification, viewer filtering, reconnect, Profession badges, Jianghu State presentation, and legal-action controls agree.
- [x] Focused Rust, Web unit, and self-contained Brave Playwright tests cover all Profession systems, state timing, two- and four-Player play, dependencies, and all-enabled compatibility.

## Validation

- `cargo test` and strict Clippy pass, including `tests/jianghu.rs` and all-enabled regressions.
- Web unit tests, Nuxt type checking, production build, and Playwright spec discovery pass.
- Catalog contract tests cover all 9 Professions and 14 Profession Formations.
- The complete self-contained Brave suite passes: 19 of 19, including `apps/web/tests/e2e/jianghu-rule-module.spec.ts`.

## Blocked by

- [#58 Compose Profession and Rule Module catalogs](058-compose-profession-and-rule-module-catalogs.md)
