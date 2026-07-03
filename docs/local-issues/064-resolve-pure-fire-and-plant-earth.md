# 64 Resolve Pure Fire and Plant Earth

## Triage

ready-for-agent

## What to build

Complete the Echo Rules Engine with 變徵‧淨火 and 變宮‧植土. Turn Turn Start
into a resumable resolution stage, reduce only published eligible timed effects
through typed module hooks, preserve Covered Passive secrecy, and reuse the
Melody continuation machinery without treating either delayed effect as a new
Formation Use.

## Acceptance criteria

- [x] 淨火 matches one Fire and one Water Card with effective level sum at least 7; 植土 matches one Earth and one Wood Card with effective level sum at least 7.
- [x] Star Element Substitution and Sacred Art Multiplicity cannot satisfy either pattern, while an in-scope Card Level Interpretation affects the level threshold.
- [x] 淨火 selects any Player, atomically reduces every eligible effect on that Player by one turn or layer, and remains a successfully executed no-change effect when none exist.
- [x] 淨火 automatically schedules Echo without a cost and chooses its target again from then-current Game State when Echo resolves.
- [x] Typed duration-reduction hooks keep each Rule Module's specialized state and return declarative deltas; Echo does not switch over other modules' internal types or convert all effects into generic Status Effects.
- [x] Hooks opt in timed Formation effects, Covered Passives, public Counter Effects, active Delayed Spells, and rulebook-named exceptions; unrelated Profession, Spirit Skill, and permanent effects are excluded even when they have expiry data.
- [x] Components of one logical effect are reduced together: 光芒's Cannot Act and Cannot Draw stay synchronized, while separate 光芒 uses remain independent without changing the serialized Status Effect contract.
- [x] Flow State loses one layer; 裂土 suppression, 天外飛扇, 殘霜手, 書仙 interactions, and every eligible effect contributed by enabled completed Rule Modules follow the 5.16 clarifications.
- [x] A Covered Passive reduced to zero remains face down and ineffective until its normal NextPlayerActionStart reveal and discard; public Counter Effects without covered Cards end immediately.
- [x] Scheduled Echo and 植土 are fixed next-Turn-Start schedules without a reducible duration or layer and are not affected by 淨火.
- [x] Turn Start records entry, expires effects due at that timing, then resolves Scheduled Echo or 植土 and pauses for typed Player or trusted randomness decisions before Main becomes available.
- [x] 植土 schedules its own distinct delayed effect; at the next Turn Start the Player selects 鳴金, 落木, 流水, 戰火, or 裂土 and executes only that fresh main effect.
- [x] 植土 never pays Echo Cost or schedules Echo, remains distinct from Echo in canonical events, and does not create a Formation Use or trigger a Covered Passive.
- [x] Seal and 裂土 do not suppress Turn Start Echo or 植土 execution because those effects are outside the Formation Use action pipeline.
- [x] A game-ending Turn Start main effect finishes the current atomic main effect, records the outcome, and never enters Main or processes later turn flow.
- [x] Canonical composite duration events and separate Echo/植土 lifecycle events reproduce direct execution, replay, verification, Public Views, and reconnect without transient state.
- [x] Focused Rust tests cover every eligible effect kind, excluded duration source, zero/one/multiple durations, neutralized Covered Passives, all five 植土 choices, nested choices and shuffle, no-change 淨火, different Echo targets, and all-enabled interactions.

## Validation

- `cargo test` and strict Clippy pass, including 24 focused Echo integration tests.
- Web unit tests (28), Nuxt type checking, and the production Nuxt build pass.
- Composite duration events replay and verify identically; neutralized Covered
  Passives remain hidden in Public Views until their ordinary trigger.
- A fresh Brave E2E run remains unavailable because the execution environment
  rejected the required sandbox escalation for quota reasons.

## Blocked by

- [#59 Release the Jianghu Rule Module](059-release-jianghu-rule-module.md)
- [#60 Release the Confluence Generation Rule Module](060-release-confluence-generation-rule-module.md)
- [#61 Release the Dark Glimmer Rule Module](061-release-dark-glimmer-rule-module.md)
- [#63 Resolve basic Melodies and Echo](063-resolve-basic-melodies-and-echo.md)
