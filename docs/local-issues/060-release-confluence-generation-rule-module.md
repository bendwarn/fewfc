# 60 Release the Confluence Generation Rule Module

## Triage

ready-for-agent

## What to build

Add Confluence Generation as one complete Theme Rule Module using the official
5.16 rules. Deliver all nine Professions, their cross-module inheritance,
abilities, and Formations; persist public Limited Uses; and resolve every
retrieval, Deck, Card-retention, and typed choice flow through deterministic
events and replay.

## Acceptance criteria

- [x] Confluence Generation is independently selectable, requires all three Advanced Rule Modules, and is enabled by default in new games and rooms.
- [x] Every Profession transition, inherited ability, Formation, and ordering rule conforms to the 5.16 PDF, including 道法師 and 虛空破滅者 interactions.
- [x] Residual Element and Residual Level read the printed definition of the existing Retrievable Discard, and missing or moved discards make dependent actions unavailable.
- [x] 調律 enforces its turn-scoped Card-use obligation, 易弦 widens only the published scope, and 天響 retrieves without that restriction.
- [x] Limited Use consumption, maximum changes, recovery, shuffle recovery, and Profession-reacquisition resets are canonical, replayable, and publicly visible.
- [x] Shared-Deck shuffles recover every eligible 順風; Personal Deck shuffles recover only the affected Pile Owner's use.
- [x] Typed Pending Choices cover every Deck inspection, retained Card set, 五鳴術 selection, and 晴風 top-Card decision without leaking hidden information.
- [x] Focused Rust, Web unit, and self-contained Brave Playwright tests cover all Profession systems, both Deck modes, replay, dependencies, and all-enabled compatibility.

## Validation

- `cargo test` and strict Clippy pass, including `tests/confluence_generation.rs`, both Deck modes, replay, and all-enabled regressions.
- Web unit tests, Nuxt type checking, production build, and Playwright spec discovery pass.
- Catalog contract tests cover all 9 Professions and 13 Profession Formations.
- The complete self-contained Brave suite passes: 19 of 19, including `apps/web/tests/e2e/confluence-generation.spec.ts`.

## Blocked by

- [#58 Compose Profession and Rule Module catalogs](058-compose-profession-and-rule-module-catalogs.md)
- [#59 Release the Jianghu Rule Module](059-release-jianghu-rule-module.md)
