# 67 Align Echo action detail with rule consequences

## Triage

in-progress

## What to build

Make the Web `PlayableAction.summary` for Echo Melodies match the complete rule
consequences visible at action-commit time. Do not treat the engine
`FormationDef.rule_text` as sufficient UI action detail for Echo.

## Problem

Echo currently exposes playable action summaries by forwarding each candidate's
`rule_text`. For Echo Melodies, that text describes only the Formation pattern
and main effect. It omits the rule consequence that happens after that main
effect:

- optional Echo Cost for the five basic Melodies,
- automatic no-cost Echo for 變徵‧淨火,
- no Echo for 變宮‧植土,
- delayed Turn Start execution of only the main effect, not a new Formation Use.

This makes action detail inconsistent with the actual rule players are choosing
to enter.

## Acceptance criteria

- [x] Web action detail for each Echo Melody includes its matching requirement,
  main effect, and Echo policy in player-facing wording.
- [x] The five optional-cost Melodies show the correct allowed printed-element
  Echo Cost:
  - 商調‧鳴金: 金 or 土
  - 角調‧落木: 木 or 水
  - 羽調‧流水: 水 or 金
  - 徵調‧戰火: 火 or 木
  - 宮調‧裂土: 土 or 火
- [x] 變徵‧淨火 action detail states that Echo is automatically scheduled with
  no cost after the main effect resolves.
- [x] 變宮‧植土 action detail states that it schedules a next-Turn-Start choice
  of a basic Melody main effect and does not schedule Echo.
- [x] Action detail does not imply that Scheduled Echo or 植土 is a new
  Formation Use, consumes Formation Cards, triggers Covered Passive, updates
  previous Formation, or can recursively schedule Echo.
- [x] Existing Star substitution action detail remains intact and composes with
  the richer Echo action detail when applicable.
- [x] Rust Web serialization contract tests cover the Echo summaries for at
  least one optional-cost Melody, 變徵‧淨火, and 變宮‧植土.
- [x] A focused Web or E2E assertion verifies that the battlefield displays the
  Echo action detail without relying on screenshot comparison.

## Validation

- `cargo test web_api::tests::echo`
- `cargo test`
- `cargo fmt --check`
- `git diff --check`

## Notes

- Keep `FormationDef.rule_text` concise if it remains useful for catalog and
  engine-facing descriptions.
- Prefer a rule-aware summary composer in the Web adapter over hard-coding
  Echo-only tooltip text in Vue.
- See ADR 0023 for the boundary between catalog rule text and player-facing
  action detail.

## Blocked by

- none
